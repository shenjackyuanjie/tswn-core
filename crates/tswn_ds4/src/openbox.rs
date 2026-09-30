//! 使用 Openbox 带权靶子预设对二人组进行三轮实战筛选。

use std::cell::RefCell;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};

use tswn_openbox_backend::backend::{self, BatchRateInput, CommonBenchOptions, OutputMode, ProgressEvent};
use tswn_openbox_backend::presets::{load_target_preset_text, load_target_presets_from_root};

use crate::error::{Ds4Error, Ds4Result};
use crate::output::{append_file, write_bytes_atomic};

/// 使用工作目录中 Openbox 靶子预设 2 进行三轮筛选。
pub fn screen_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<()> {
    let presets = load_target_presets_from_root(root).map_err(Ds4Error::parse)?;
    let preset = presets
        .iter()
        .find(|preset| preset.id == 2)
        .ok_or_else(|| Ds4Error::parse("Openbox 设置中缺少靶子预设 2"))?;
    let target_text = load_target_preset_text(preset).map_err(Ds4Error::parse)?;
    screen_pairs_with(root, sieve, |input, output, count, threshold, pure| {
        eprintln!("[Openbox] 每组 {count} 局筛选，胜率阈值 {threshold}%");
        let job = BatchRateInput {
            target_text: target_text.clone(),
            player_text: fs::read_to_string(input)?,
            target_factor_enabled: preset.factor_enabled,
            target_double_plus: preset.diy,
            player_double_plus: false,
            show_matchups: false,
            highlight_delta: None,
            output_mode: if pure { OutputMode::Pure } else { OutputMode::Log },
            output_file: Some(output.to_path_buf()),
            options: CommonBenchOptions {
                count,
                threads: Some(threads.max(1)),
                keep_rq: true,
                verbose: false,
                min_screen: Some(101.0),
                min_file: Some(threshold as f64),
                wr_precision: 3,
            },
            cancel: Arc::new(AtomicBool::new(false)),
        };
        run_batch(job)
    })
}

fn run_batch(job: BatchRateInput) -> Ds4Result<()> {
    let result = RefCell::new(None);
    backend::run_batch_rate(job, |event| match event {
        ProgressEvent::Done(done) => {
            *result.borrow_mut() = Some(done);
        }
        ProgressEvent::Progress { done, total } if done == total => {
            eprintln!("[Openbox] 进度 {done}/{total}");
        }
        _ => {}
    });
    result
        .into_inner()
        .ok_or_else(|| Ds4Error::parse("Openbox 未返回完成状态"))?
        .map(|_| ())
        .map_err(Ds4Error::parse)
}

fn screen_pairs_with(
    root: &Path,
    sieve: i32,
    mut run: impl FnMut(&Path, &Path, usize, i32, bool) -> Ds4Result<()>,
) -> Ds4Result<()> {
    if sieve <= 0 {
        return Err(Ds4Error::parse("Openbox 筛选要求 three.pair_abcp_sieve 大于 0"));
    }
    let mut input = root.join("abcp5/result_without_score.txt");
    if !input.is_file() {
        return Err(Ds4Error::parse(format!("缺少 Openbox 筛选输入: {}", input.display())));
    }
    fs::create_dir_all(root.join("tmp"))?;
    for (stage, count, increment, pure) in [("1pct", 100, 1, true), ("10pct", 1000, 2, true), ("100pct", 10000, 2, false)] {
        let output = root.join(format!("tmp/openbox_two_cqp_{stage}.txt"));
        // 清除旧阶段结果；空候选直接传递，避免后端将空文件作为错误输入。
        write_bytes_atomic(&output, b"")?;
        let has_input = BufReader::new(fs::File::open(&input)?)
            .lines()
            .try_fold(false, |found, line| line.map(|line| found || !line.trim().is_empty()))?;
        if has_input {
            run(&input, &output, count, sieve / 100 + increment, pure)?;
        }
        input = output;
    }
    append_file(&input, &root.join("file/real_two.txt"))?;
    // 只有最终结果归档成功才清空 ABCP5 输入和结果，失败时保留可重试的数据。
    for name in ["input.txt", "result.txt", "result_without_score.txt"] {
        write_bytes_atomic(&root.join("abcp5").join(name), b"")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_screening_loads_workspace_preset_and_runs_without_executable() {
        let root = std::env::temp_dir().join(format!("ds4-openbox-api-{}", std::process::id()));
        fs::create_dir_all(root.join("abcp5")).unwrap();
        fs::create_dir_all(root.join("setting")).unwrap();
        fs::write(
            root.join("setting/settings.toml"),
            "[[targets]]\nid=2\nname='测试双人组'\nfile='custom.toml'\nfactor_enabled=true\n",
        )
        .unwrap();
        // 镜像组由 Openbox 后端按 50% 处理，三轮都应保留，不需要外部可执行文件。
        fs::write(
            root.join("setting/custom.toml"),
            "[[targets]]\nfactor=2.5\nplayers=['alpha@teamA','beta@teamA']\n",
        )
        .unwrap();
        fs::write(root.join("abcp5/result_without_score.txt"), "alpha@teamA+beta@teamA\n").unwrap();
        screen_pairs(&root, 4400, 2).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("file/real_two.txt")).unwrap().trim(),
            "50.000 alpha@teamA+beta@teamA"
        );
        fs::write(root.join("abcp5/result_without_score.txt"), "alpha@teamA+beta@teamA\n").unwrap();
        fs::write(root.join("setting/custom.toml"), "非法靶子配置").unwrap();
        assert!(screen_pairs(&root, 4400, 2).is_err());
        assert!(!fs::read_to_string(root.join("abcp5/result_without_score.txt")).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn screening_preserves_failure_then_archives_three_stages() {
        let root = std::env::temp_dir().join(format!("ds4-openbox-{}", std::process::id()));
        fs::create_dir_all(root.join("abcp5")).unwrap();
        fs::write(root.join("abcp5/result_without_score.txt"), "甲@队+乙@队\n").unwrap();
        fs::write(root.join("abcp5/result.txt"), "4500 甲@队+乙@队\n").unwrap();
        let failed = screen_pairs_with(&root, 4400, |_, _, _, _, _| Err(Ds4Error::parse("模拟筛选失败")));
        assert!(failed.is_err());
        assert!(!root.join("file/real_two.txt").exists());
        assert!(!fs::read_to_string(root.join("abcp5/result.txt")).unwrap().is_empty());

        let mut stages = Vec::new();
        screen_pairs_with(&root, 4400, |input, output, count, threshold, pure| {
            stages.push((count, threshold, pure));
            assert_eq!(fs::read_to_string(input)?, "甲@队+乙@队\n");
            fs::write(output, if pure { "甲@队+乙@队\n" } else { "48.5 甲@队+乙@队\n" })?;
            Ok(())
        })
        .unwrap();
        assert_eq!(stages, [(100, 45, true), (1000, 46, true), (10000, 46, false)]);
        assert_eq!(
            fs::read_to_string(root.join("file/real_two.txt")).unwrap(),
            "48.5 甲@队+乙@队\n"
        );
        for name in ["input.txt", "result.txt", "result_without_score.txt"] {
            assert_eq!(fs::metadata(root.join("abcp5").join(name)).unwrap().len(), 0);
        }
        screen_pairs_with(&root, 4400, |_, _, _, _, _| panic!("空结果不应调用后端")).unwrap();
        assert_eq!(fs::read_to_string(root.join("file/real_two.txt")).unwrap().lines().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
