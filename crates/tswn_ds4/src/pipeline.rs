//! 全流程执行管道。
//!
//! [`run_full`] / [`run_with_progress`] 按配置处理输入、单人评分、增量配对、
//! 三人预测、ABCP5、实战筛选与历史归档，返回本轮各阶段统计。
//! 工作目录中的 tmp 在输入阶段重建；历史与结果按阶段分别发布或追加，没有全流程回滚。
//! 同一目录必须由宿主串行使用；改变模型、队伍或门槛时，既有缓存不会自动重新评分。

#![allow(dead_code)]

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use crate::abcp;
use crate::config::{Config, PairMode, SingleMode};
use crate::error::Ds4Result;
use crate::model::{bc, fs as model_fs, fz, pj, wc as model_wc};
use crate::ops::dedup::{DedupStats, remove_duplicates};
use crate::ops::merge::merge_input_files;
use crate::ops::sort::{SortOptions, sort_scored_file};
use crate::output::{AtomicFileWriter, append_file, write_bytes_atomic};
use crate::pairing::{fc, rh, wc as pair_wc};
use crate::sp1::{self, ScoreNow, Sp1Mode};
use crate::store::Store;
use crate::three::{self, ThreeMode};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// 输入合并与去重结果，供判断本轮新增工作量。
///
/// merged_files 统计实际拼接的输入文件，不代表有效名称数；dedup 的计数口径随去重开关而变。
pub struct Stage1Report {
    pub merged_files: usize,
    pub dedup: DedupStats,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// 五类基础单人模型本轮写入的记录数，名称通过基础分或潜力分任一门槛即计入。
///
/// 同一名称可同时通过多个模型，因此字段之和不等于去重人数，也不包含 SP1 结果。
pub struct SingleReport {
    pub bc: usize,
    pub fz: usize,
    pub wc: usize,
    pub fs: usize,
    pub pj: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// FC、WC、RH 各自增量遍历中新追加的通过记录数，未启用模式为 0。
///
/// 计数包含新×新、新×旧等批次之和，不代表历史文件总行数或跨模式去重后的组合数。
pub struct PairReport {
    pub fc: usize,
    pub wc: usize,
    pub rh: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// 一次成功文件工作流的统计；历史与明细保存在工作目录，不在报告中复制。
///
/// sp1 顺序为 QP/QD/PP/PD/CQD，three 顺序为 FFC/WFC/FWC/WWC/RWC/RRH/PRH/WRH；
/// 禁用或旧格式未覆盖的阶段计为 0。失败返回错误而非部分报告，但已写入文件可能仍存在。
pub struct FullRunReport {
    pub stage1: Stage1Report,
    pub single: SingleReport,
    pub pair: PairReport,
    pub sp1: [usize; 5],
    pub three: [usize; 8],
    pub abcp: usize,
}

/// 进度回调的阶段标记；在进入阶段前通知，Complete 只在成功收尾后通知。
///
/// 旧配置或空输入可跳过部分阶段；completed/TOTAL 只用于展示阶段序号，不表示等比例耗时。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RunStage {
    Input,
    Single,
    Pair,
    Three,
    Abcp,
    Openbox,
    Archive,
    Complete,
}

impl RunStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Input => "合并输入、队伍筛选与去重",
            Self::Single => "单人评分与分类",
            Self::Pair => "二人增量配对",
            Self::Three => "三人预测与增量配对",
            Self::Abcp => "ABCP5 二人预测",
            Self::Openbox => "Openbox 三轮实战筛选",
            Self::Archive => "归档历史结果",
            Self::Complete => "处理完成",
        }
    }

    pub fn completed(self) -> usize { self as usize }

    pub const TOTAL: usize = Self::Complete as usize;
}

/// 同步执行配置对应的文件工作流，等价于忽略进度的 run_with_progress。
///
/// 不重新读取配置；root 为相对路径时按调用瞬间的当前目录解析。文件、模型或后端失败
/// 返回错误，不撤销此前的阶段输出；同一 root 的并发互斥由宿主负责。
pub fn run_full(root: &Path, config: &Config) -> Ds4Result<FullRunReport> { run_with_progress(root, config, |_| {}) }

/// 同步执行流程，在当前线程进入各阶段前调用 progress，成功返回完整统计。
///
/// 回调应快速返回；这里没有取消协议，也不捕获回调 panic。输入阶段会重建 tmp，
/// 后续失败保留已经发布/追加的结果；函数不加目录锁，宿主应串行使用同一工作目录。
pub fn run_with_progress(root: &Path, config: &Config, mut progress: impl FnMut(RunStage)) -> Ds4Result<FullRunReport> {
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    let store = Store::new(absolute);
    run_with_store_observed(&store, config, &mut progress)
}

pub fn run_full_with_store(store: &Store, config: &Config) -> Ds4Result<FullRunReport> {
    run_with_store_observed(store, config, &mut |_| {})
}

fn run_with_store_observed(store: &Store, config: &Config, progress: &mut dyn FnMut(RunStage)) -> Ds4Result<FullRunReport> {
    progress(RunStage::Input);
    if let Some(team_name) = &config.team_name {
        return run_ds4_with_store(store, config, team_name, progress);
    }
    let stage1 = run_stage1_with_store(store, config)?;
    progress(RunStage::Single);
    let single = run_single_scoring(store, config)?;
    sort_tmp_single(store)?;
    progress(RunStage::Pair);
    let pair = run_pairing(store, config)?;
    progress(RunStage::Archive);
    copy_to_file_store(store)?;
    sort_file_store(store)?;
    if config.copy_to_new {
        copy_to_new_store(store)?;
        sort_new_store(store)?;
    }
    progress(RunStage::Complete);
    Ok(FullRunReport {
        stage1,
        single,
        pair,
        sp1: [0; 5],
        three: [0; 8],
        abcp: 0,
    })
}

fn run_ds4_with_store(
    store: &Store,
    config: &Config,
    team_name: &str,
    progress: &mut dyn FnMut(RunStage),
) -> Ds4Result<FullRunReport> {
    prepare_dirs(store)?;
    fs::create_dir_all(store.root().join("3ren"))?;
    fs::create_dir_all(store.root().join("3-out"))?;
    reset_tmp(store)?;
    write_bytes_atomic(&store.tmp_blank_file(), b"")?;
    let merged_files = merge_input_files(&store.input_dir(), &store.tmp_new_file())?;
    let mut accepted = AtomicFileWriter::new(&store.tmp_dir().join("new_team.txt"))?;
    let mut ignored = AtomicFileWriter::new(&store.out_dir().join("ignore_input.txt"))?;
    let mut accepted_count = 0;
    let mut ignored_count = 0;
    for raw in BufReader::new(fs::File::open(store.tmp_new_file())?).lines() {
        let raw = raw?;
        let line = raw.trim_end_matches(char::is_whitespace);
        match line.rsplit_once('@') {
            Some((name, team)) if !name.is_empty() && team.starts_with(team_name) => {
                write!(accepted.writer(), "{name}@{team_name}\r\n")?;
                accepted_count += 1;
            }
            _ => {
                write!(ignored.writer(), "{line}\r\n")?;
                ignored_count += 1;
            }
        }
    }
    accepted.commit()?;
    ignored.commit()?;
    if accepted_count == 0 && ignored_count != 0 {
        return Err(crate::error::Ds4Error::parse(format!("没有输入属于 team_name={team_name}")));
    }
    let dedup = if config.run_dedup {
        remove_duplicates(
            &store.tmp_dir().join("new_team.txt"),
            &store.file_old(),
            &store.tmp_new_dup_file(),
        )?
    } else {
        let mut output = AtomicFileWriter::new(&store.tmp_new_dup_file())?;
        io::copy(&mut fs::File::open(store.tmp_dir().join("new_team.txt"))?, output.writer())?;
        output.commit()?;
        DedupStats {
            new_unique: accepted_count,
            old_hits: 0,
            remaining: accepted_count,
        }
    };
    let stage1 = Stage1Report { merged_files, dedup };
    if accepted_count == 0 {
        progress(RunStage::Complete);
        return Ok(FullRunReport {
            stage1,
            single: SingleReport {
                bc: 0,
                fz: 0,
                wc: 0,
                fs: 0,
                pj: 0,
            },
            pair: PairReport { fc: 0, wc: 0, rh: 0 },
            sp1: [0; 5],
            three: [0; 8],
            abcp: 0,
        });
    }

    progress(RunStage::Single);
    let single = run_single_scoring(store, config)?;
    sort_tmp_single(store)?;
    let sp1 = run_sp1_scoring(store, config)?;
    for mode in PairMode::ALL {
        write_bytes_atomic(&store.out_pair_file(mode), b"")?;
    }
    progress(RunStage::Pair);
    let pair = run_pairing(store, config)?;

    let mut three_counts = [0; 8];
    progress(RunStage::Three);
    if config.get_3 {
        abcp::prepare_three_pairs(store.root(), config.three_pair_abcp_sieve, config.threads)?;
        for kind in ["FC", "WC", "RH"] {
            let new_path = store.tmp_dir().join(format!("new_three_{kind}.txt"));
            let mut output = AtomicFileWriter::new(&store.new_dir().join(format!("new_three_{kind}.txt")))?;
            io::copy(&mut fs::File::open(&new_path)?, output.writer())?;
            output.commit()?;
            let old_path = store.file_dir().join(format!("old_three_{kind}.txt"));
            if !old_path.exists() {
                write_bytes_atomic(&old_path, b"")?;
            }
        }
        for (index, mode) in ThreeMode::ALL.into_iter().enumerate() {
            if config.three[index].enabled {
                three_counts[index] = three::run_incremental(store.root(), mode, config.three[index].sieve, config.threads)?;
            }
        }
        for kind in ["FC", "WC", "RH"] {
            let new_path = store.tmp_dir().join(format!("new_three_{kind}.txt"));
            let old_path = store.file_dir().join(format!("old_three_{kind}.txt"));
            append_file(&new_path, &old_path)?;
            sort_scored_file(&old_path, &old_path, &sort_by_score(1))?;
        }
    }
    progress(RunStage::Abcp);
    let abcp = if config.abcp.enabled {
        abcp::predict_final_pairs(store.root(), config.abcp.sieve, config.threads)?
    } else {
        0
    };
    progress(RunStage::Openbox);
    if config.openbox_cqp {
        crate::openbox::screen_pairs(store.root(), config.three_pair_abcp_sieve, config.threads)?;
    }

    progress(RunStage::Archive);
    copy_to_file_store(store)?;
    sort_file_store(store)?;
    archive_sp1(store, config)?;
    if config.copy_to_new {
        copy_to_new_store(store)?;
        sort_new_store(store)?;
        copy_sp1_to_new(store, config)?;
    }
    progress(RunStage::Complete);
    Ok(FullRunReport {
        stage1,
        single,
        pair,
        sp1,
        three: three_counts,
        abcp,
    })
}

fn sort_by_score(score_number: usize) -> SortOptions {
    SortOptions {
        score_number,
        sort_key_zero_based: 0,
        output_score: true,
    }
}

fn run_sp1_scoring(store: &Store, config: &Config) -> Ds4Result<[usize; 5]> {
    let score_now = ScoreNow::load(store.root())?;
    let mut counts = [0; 5];
    for (index, mode) in Sp1Mode::ALL.into_iter().enumerate() {
        let settings = config.sp1[index];
        if !settings.enabled {
            continue;
        }
        let name = mode.as_str();
        let output = store.tmp_dir().join(format!("new_{name}.txt"));
        let skill_path = store.tmp_dir().join(format!("new_{name}_skill.txt"));
        let skill = settings.skill_sieve.map(|sieve| (skill_path.as_path(), sieve));
        counts[index] = sp1::score_file(
            &store.tmp_new_dup_file(),
            &output,
            skill,
            mode,
            settings.sieve,
            &score_now,
            config.threads,
        )?;
        if settings.skill_sieve.is_some() {
            append_file(&skill_path, &store.out_dir().join(format!("{name}_skill.txt")))?;
        }
        if config.copy_pf_to_out {
            let out = store.out_dir().join(format!("new_{name}.txt"));
            append_file(&output, &out)?;
            sort_scored_file(&out, &out, &sort_by_score(1))?;
        }
    }
    Ok(counts)
}

fn archive_sp1(store: &Store, config: &Config) -> Ds4Result<()> {
    for (index, mode) in Sp1Mode::ALL.into_iter().enumerate() {
        if config.sp1[index].enabled {
            let name = mode.as_str();
            let old = store.file_dir().join(format!("old_{name}.txt"));
            append_file(&store.tmp_dir().join(format!("new_{name}.txt")), &old)?;
            sort_scored_file(&old, &old, &sort_by_score(1))?;
        }
    }
    Ok(())
}

fn copy_sp1_to_new(store: &Store, config: &Config) -> Ds4Result<()> {
    for (index, mode) in Sp1Mode::ALL.into_iter().enumerate() {
        if config.sp1[index].enabled {
            let name = mode.as_str();
            let new = store.new_dir().join(format!("new_{name}.txt"));
            append_file(&store.tmp_dir().join(format!("new_{name}.txt")), &new)?;
            sort_scored_file(&new, &new, &sort_by_score(1))?;
        }
    }
    Ok(())
}

pub fn run_stage1(root: &Path, config: &Config) -> Ds4Result<Stage1Report> {
    let store = Store::new(root.to_path_buf());
    run_stage1_with_store(&store, config)
}

pub fn run_stage1_with_store(store: &Store, config: &Config) -> Ds4Result<Stage1Report> {
    prepare_dirs(store)?;
    reset_tmp(store)?;
    let merged_files = merge_input_files(&store.input_dir(), &store.tmp_new_file())?;

    let dedup = if config.run_dedup {
        remove_duplicates(&store.tmp_new_file(), &store.file_old(), &store.tmp_new_dup_file())?
    } else {
        let mut output = AtomicFileWriter::new(&store.tmp_new_dup_file())?;
        io::copy(&mut fs::File::open(store.tmp_new_file())?, output.writer())?;
        output.commit()?;
        let remaining = BufReader::new(fs::File::open(store.tmp_new_dup_file())?).lines().count();
        DedupStats {
            new_unique: remaining,
            old_hits: 0,
            remaining,
        }
    };

    Ok(Stage1Report { merged_files, dedup })
}

fn run_single_scoring(store: &Store, config: &Config) -> Ds4Result<SingleReport> {
    let input = store.tmp_new_dup_file();
    let bc_count = bc::score_file_bc_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Bc),
        config.single_bc.score,
        config.single_bc.potential,
        config.threads,
    )?;
    let fz_count = fz::score_file_fz_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Fz),
        config.single_fz.score,
        config.single_fz.potential,
        config.threads,
    )?;
    let wc_count = model_wc::score_file_wc_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Wc),
        config.single_wc.score,
        config.single_wc.potential,
        config.threads,
    )?;
    let fs_count = model_fs::score_file_fs_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Fs),
        config.single_fs.score,
        config.single_fs.potential,
        config.threads,
    )?;
    let pj_count = pj::score_file_pj_with_threads(
        &input,
        &store.tmp_new_mode_file(SingleMode::Pj),
        config.single_pj.score,
        config.single_pj.potential,
        config.threads,
    )?;

    Ok(SingleReport {
        bc: bc_count,
        fz: fz_count,
        wc: wc_count,
        fs: fs_count,
        pj: pj_count,
    })
}

fn sort_tmp_single(store: &Store) -> Ds4Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let path = store.tmp_new_mode_file(mode);
        sort_scored_file(&path, &path, &by_score)?;
    }
    Ok(())
}

fn run_pairing(store: &Store, config: &Config) -> Ds4Result<PairReport> {
    let mut fc_count = 0usize;
    if config.pair_fc.enabled {
        let out = store.out_pair_file(PairMode::Fc);
        fc_count += fc::run_fc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fz),
            &store.tmp_new_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
        fc_count += fc::run_fc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fz),
            &store.file_old_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
        fc_count += fc::run_fc_with_threads(
            false,
            &store.file_old_mode_file(SingleMode::Fz),
            &store.tmp_new_mode_file(SingleMode::Bc),
            &out,
            config.pair_fc.sieve,
            config.threads,
        )?;
    }

    let mut wc_count = 0usize;
    if config.pair_wc.enabled {
        let out = store.out_pair_file(PairMode::Wc);
        wc_count += pair_wc::run_wc_with_threads(
            true,
            &store.tmp_new_mode_file(SingleMode::Wc),
            &store.tmp_blank_file(),
            &out,
            config.pair_wc.sieve,
            config.threads,
        )?;
        wc_count += pair_wc::run_wc_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Wc),
            &store.file_old_mode_file(SingleMode::Wc),
            &out,
            config.pair_wc.sieve,
            config.threads,
        )?;
    }

    let mut rh_count = 0usize;
    if config.pair_rh.enabled {
        let out = store.out_pair_file(PairMode::Rh);
        rh_count += rh::run_rh_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fs),
            &store.tmp_new_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
        rh_count += rh::run_rh_with_threads(
            false,
            &store.tmp_new_mode_file(SingleMode::Fs),
            &store.file_old_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
        rh_count += rh::run_rh_with_threads(
            false,
            &store.file_old_mode_file(SingleMode::Fs),
            &store.tmp_new_mode_file(SingleMode::Pj),
            &out,
            config.pair_rh.sieve,
            config.threads,
        )?;
    }

    Ok(PairReport {
        fc: fc_count,
        wc: wc_count,
        rh: rh_count,
    })
}

fn copy_to_file_store(store: &Store) -> Ds4Result<()> {
    append_file(&store.tmp_new_dup_file(), &store.file_old())?;
    for mode in SingleMode::ALL {
        append_file(&store.tmp_new_mode_file(mode), &store.file_old_mode_file(mode))?;
    }
    Ok(())
}

fn sort_file_store(store: &Store) -> Ds4Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    let by_potential = SortOptions {
        score_number: 2,
        sort_key_zero_based: 1,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let old_path = store.file_old_mode_file(mode);
        sort_scored_file(&old_path, &old_path, &by_score)?;
        sort_scored_file(&old_path, &store.file_old_mode_ptt_file(mode), &by_potential)?;
    }
    Ok(())
}

fn copy_to_new_store(store: &Store) -> Ds4Result<()> {
    let new_all = store.new_dir().join("new.txt");
    append_file(&store.tmp_new_dup_file(), &new_all)?;
    append_file(&store.tmp_new_dup_file(), &new_all)?;
    for mode in SingleMode::ALL {
        append_file(&store.tmp_new_mode_file(mode), &store.new_mode_file(mode))?;
    }
    Ok(())
}

fn sort_new_store(store: &Store) -> Ds4Result<()> {
    let by_score = SortOptions {
        score_number: 2,
        sort_key_zero_based: 0,
        output_score: true,
    };
    let by_potential = SortOptions {
        score_number: 2,
        sort_key_zero_based: 1,
        output_score: true,
    };
    for mode in SingleMode::ALL {
        let new_path = store.new_mode_file(mode);
        sort_scored_file(&new_path, &new_path, &by_score)?;
        sort_scored_file(&new_path, &store.new_mode_ptt_file(mode), &by_potential)?;
    }
    Ok(())
}

fn prepare_dirs(store: &Store) -> Ds4Result<()> {
    fs::create_dir_all(store.root())?;
    fs::create_dir_all(store.input_dir())?;
    fs::create_dir_all(store.file_dir())?;
    fs::create_dir_all(store.new_dir())?;
    fs::create_dir_all(store.out_dir())?;
    fs::create_dir_all(store.tmp_dir())?;
    Ok(())
}

fn reset_tmp(store: &Store) -> Ds4Result<()> {
    if store.tmp_dir().exists() {
        fs::remove_dir_all(store.tmp_dir())?;
    }
    fs::create_dir_all(store.tmp_dir())?;
    write_bytes_atomic(&store.tmp_blank_file(), b"1@1\r\n")?;
    Ok(())
}
