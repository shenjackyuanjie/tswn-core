use crate::{
    BattleRow, DatasetConfig, GenerateArgs, SampleRow, input, random, sampling,
    storage::{self, ShardReceipt, TableWriter},
    validate::Summary,
};
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tswn_core::{
    cli_api::battle::{BattleModelSession, BattleStopReason},
    runtime::{PreparedRuntimeRunner, RuntimeRunner},
};

pub fn generate(args: &GenerateArgs) -> Result<()> {
    ensure!(
        args.games_per_matchup > 0 && args.samples_per_game > 0 && args.battles_per_shard > 0 && args.max_rounds > 0,
        "次数、样本数、分片大小和轮数上限必须为正整数"
    );
    ensure!(args.eval_rq.is_finite(), "eval_rq 必须有限");
    let (cases, input_sha256) = input::load(args)?;
    let config = DatasetConfig {
        format_version: 1,
        state_schema_version: tswn_core::runtime::model_state::MODEL_STATE_SCHEMA_VERSION,
        seed: args.seed.clone(),
        games_per_matchup: args.games_per_matchup,
        samples_per_game: args.samples_per_game,
        battles_per_shard: args.battles_per_shard,
        max_rounds: args.max_rounds,
        eval_rq: args.eval_rq,
        input_sha256,
        input_mode: if args.names.is_some() { "names" } else { "files" }.into(),
        team_sizes: args.team_sizes.clone(),
        matchups: args.matchups,
        executable_sha256: storage::file_hash(&std::env::current_exe()?)?,
        cases,
    };
    let total = config.cases.len().checked_mul(config.games_per_matchup).context("对局数量溢出")?;
    fs::create_dir_all(&args.out)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(args.out.join(".generator.lock"))?;
    lock.try_lock_exclusive().context("输出目录正在被另一个生成任务使用")?;
    let manifest = args.out.join("manifest.json");
    if manifest.try_exists()? {
        ensure!(args.resume, "输出已有 manifest；续跑请使用 --resume");
        let previous: DatasetConfig = storage::read_json(&manifest)?;
        ensure!(previous == config, "输入、配置或可执行文件版本不同，不能混合续跑");
    } else {
        ensure!(!args.resume, "找不到 manifest，不能续跑");
        for entry in fs::read_dir(&args.out)? {
            ensure!(entry?.file_name() == ".generator.lock", "首次生成要求输出目录为空");
        }
        storage::write_json(&args.out.join("manifest.tmp"), &config)?;
        fs::rename(args.out.join("manifest.tmp"), &manifest)?;
    }
    let shard_count = total.div_ceil(config.battles_per_shard);
    let mut pending = Vec::new();
    for index in 0..shard_count {
        let (first, end) = range(&config, index, total);
        let dir = args.out.join(format!("shard-{index:06}"));
        if dir.try_exists()? {
            storage::check_receipt(&dir, first, end).with_context(|| format!("已完成分片 {index} 损坏；保留现场并停止"))?;
        } else {
            pending.push(index);
        }
    }
    let workers = if args.threads == 0 {
        std::thread::available_parallelism().map_or(1, usize::from)
    } else {
        args.threads
    };
    let workers = workers.min(pending.len());
    eprintln!("共 {total} 局，{} 个分片待生成，{workers} 个 worker", pending.len());
    if let Some(available) = std::thread::available_parallelism().map(usize::from).ok()
        && workers < available
    {
        eprintln!(
            "提示：分片数限制了并行度（可用 {available} 个逻辑 CPU，当前 {workers} 个 worker）；\
             需要更高并行度可减小 --battles-per-shard"
        );
    }
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let done = AtomicUsize::new(shard_count - pending.len());
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<Vec<(usize, Summary)>> {
                let mut summaries = Vec::new();
                while !stop.load(Ordering::Acquire) {
                    let position = next.fetch_add(1, Ordering::Relaxed);
                    let Some(index) = pending.get(position).copied() else {
                        break;
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        write_shard(&args.out, &config, index, total, &stop)
                    }));
                    let result = result.unwrap_or_else(|panic| {
                        let message = panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                            .unwrap_or_else(|| "未知 panic".into());
                        Err(anyhow::anyhow!("分片 {index} Runtime panic：{message}"))
                    });
                    match result {
                        Ok(summary) => summaries.push((index, summary)),
                        Err(error) => {
                            stop.store(true, Ordering::Release);
                            return Err(error);
                        }
                    }
                    eprintln!("分片完成 {}/{}", done.fetch_add(1, Ordering::Relaxed) + 1, shard_count);
                }
                Ok(summaries)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| anyhow::anyhow!("生成 worker 异常退出")).and_then(|value| value))
            .collect::<Vec<_>>()
    });
    let errors: Vec<_> = results.iter().filter_map(|result| result.as_ref().err()).collect();
    if !errors.is_empty() {
        let message = errors.iter().map(|error| format!("{error:#}")).collect::<Vec<_>>().join("\n");
        storage::write_json(&args.out.join("failure.json"), &serde_json::json!({"errors": message}))?;
        bail!("生成停止；已完成分片保留。复现信息见临时分片 active-battle.json。\n{message}");
    }
    // 分片在 worker 内已经回读校验过一次；本次运行覆盖全部分片时直接汇总，
    // 不再让 `validate_dataset` 把全部 Parquet 重解一遍（8 分片规模约省 1.1 s）。
    // 续跑时已有分片没有本轮摘要，仍走完整校验。
    let summary = if pending.len() == shard_count {
        let fresh = results.into_iter().filter_map(Result::ok).flatten().collect();
        crate::validate::summarize_fresh(&config, &args.out, total, fresh)?
    } else {
        crate::validate::validate_dataset(&args.out)?
    };
    storage::write_json(&args.out.join("summary.json"), &summary)?;
    let failure = args.out.join("failure.json");
    if failure.try_exists()? {
        fs::remove_file(failure)?;
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

pub(crate) fn range(config: &DatasetConfig, index: usize, total: usize) -> (usize, usize) {
    let first = index * config.battles_per_shard;
    (first, first.saturating_add(config.battles_per_shard).min(total))
}

/// 样本行转 Parquet 的批量大小。
///
/// 每个 `SampleRow.state` 约有 450 个叶子列，逐局（8 行）调用一次 `serde_arrow`
/// 时，schema 遍历和列缓冲的固定开销要摊在 8 行上。攒到 256 行再转一次
/// `RecordBatch`，同样的数据量下写入阶段的开销明显下降。
const APPEND_BATCH_ROWS: usize = 256;

fn write_shard(out: &Path, config: &DatasetConfig, index: usize, total: usize, stop: &AtomicBool) -> Result<Summary> {
    let name = format!(".shard-{index:06}.tmp");
    storage::remove_incomplete(out, &name)?;
    let temp = out.join(&name);
    fs::create_dir(&temp)?;
    let mut battles = TableWriter::<BattleRow>::create(&temp.join("battles.parquet"))?;
    let mut samples = TableWriter::<SampleRow>::create(&temp.join("samples.parquet"))?;
    let (first, end) = range(config, index, total);
    let mut active = ActiveBattle::new(temp.join("active-battle.json"), config);
    let mut prepared: Option<(usize, PreparedRuntimeRunner)> = None;
    let mut pending: Vec<SampleRow> = Vec::new();
    for battle_id in first..end {
        ensure!(!stop.load(Ordering::Acquire), "其他分片失败，当前分片停止");
        let case_index = battle_id / config.games_per_matchup;
        let case = &config.cases[case_index];
        let seed = random::battle_seed(&config.seed, &case.matchup_id, battle_id as u64);
        active.set(battle_id, case_index, &seed);
        if prepared.as_ref().is_none_or(|(previous, _)| *previous != case_index) {
            prepared = Some((
                case_index,
                RuntimeRunner::prepare_groups_with_eval_rq(&case.groups, config.eval_rq)?,
            ));
        }
        let (battle, rows) = generate_battle(config, battle_id, &prepared.as_ref().unwrap().1, &seed)
            .with_context(|| format!("battle_id={battle_id}, seed={seed}, source={}", case.source))?;
        pending.extend(rows);
        if pending.len() >= APPEND_BATCH_ROWS {
            samples.append(&pending)?;
            pending.clear();
        }
        battles.append(&[battle])?;
    }
    active.defuse();
    ensure!(!stop.load(Ordering::Acquire), "其他分片失败，当前分片停止");
    if !pending.is_empty() {
        samples.append(&pending)?;
    }
    let battles = battles.finish()?;
    let samples = samples.finish()?;
    let receipt = ShardReceipt {
        first_battle: first,
        end_battle: end,
        battles,
        samples,
        battles_sha256: storage::file_hash(&temp.join("battles.parquet"))?,
        samples_sha256: storage::file_hash(&temp.join("samples.parquet"))?,
    };
    storage::write_json(&temp.join("complete.json"), &receipt)?;
    // 在提交目录前回读并验证所有引用、标签和分片行序。
    let summary = crate::validate::validate_shard(&temp, config, &receipt)?;
    let active_battle = temp.join("active-battle.json");
    if active_battle.try_exists()? {
        fs::remove_file(active_battle)?;
    }
    fs::rename(temp, out.join(format!("shard-{index:06}")))?;
    Ok(summary)
}

/// 分片在飞对局记录：只在失败或 panic 时落盘，避免每局一次卷级刷盘。
///
/// 旧实现每局都写一次 `active-battle.json` 并 `sync_all`，1000 局就是 1000 次
/// `FlushFileBuffers`；多 worker 并行时这些刷盘互相排队，实测同一批对局在 4 分片
/// 并行下的墙钟比 4 个独立进程慢 70%，CPU 时间也多 24%。失败复现信息只在真正
/// 出错时才有用，因此改为在内存里记住当前对局，`Drop` 时再写。
struct ActiveBattle<'a> {
    path: PathBuf,
    config: &'a DatasetConfig,
    current: Option<(usize, usize, String)>,
}

impl<'a> ActiveBattle<'a> {
    fn new(path: PathBuf, config: &'a DatasetConfig) -> Self {
        Self {
            path,
            config,
            current: None,
        }
    }

    fn set(&mut self, battle_id: usize, case_index: usize, seed: &str) {
        self.current = Some((battle_id, case_index, seed.to_owned()));
    }

    /// 分片正常收尾后调用：不再需要写失败现场。
    fn defuse(&mut self) { self.current = None; }
}

impl Drop for ActiveBattle<'_> {
    fn drop(&mut self) {
        let Some((battle_id, case_index, seed)) = self.current.take() else {
            return;
        };
        let case = &self.config.cases[case_index];
        let payload = serde_json::json!({
            "battle_id": battle_id,
            "groups": case.groups,
            "seed": seed,
            "max_rounds": self.config.max_rounds,
            "eval_rq": self.config.eval_rq,
        });
        if let Err(error) = storage::write_json(&self.path, &payload) {
            eprintln!("写入 {} 失败：{error:#}", self.path.display());
        }
    }
}

pub(crate) fn generate_battle(
    config: &DatasetConfig,
    battle_id: usize,
    prepared: &PreparedRuntimeRunner,
    seed: &str,
) -> Result<(BattleRow, Vec<SampleRow>)> {
    let seeds = [seed.to_owned()];
    let case_index = battle_id / config.games_per_matchup;
    let case = &config.cases[case_index];
    let mut first = BattleModelSession::from_prepared(prepared, &seeds, config.max_rounds)?;
    let initial_terminal = first.is_done();
    let mut frames = Vec::new();
    while let Some(frame) = first.next_frame()? {
        frames.push(frame);
    }
    let outcome = first.result().context("首遍未得到终止结果")?;
    let winner = match (outcome.stop_reason, outcome.winner_team_indices.as_slice()) {
        (BattleStopReason::Winner, [winner]) if *winner < case.groups.len() => Some(*winner),
        (BattleStopReason::MaxRounds | BattleStopReason::NoProgress, []) => None,
        _ => bail!("胜者必须唯一且映射到输入队伍：{:?}", outcome),
    };
    let selected = sampling::select(&frames, &outcome, config.samples_per_game, seed);
    let mut second = BattleModelSession::from_prepared(prepared, &seeds, config.max_rounds)?;
    ensure!(second.is_done() == initial_terminal, "两遍初始终止状态不一致");
    let mut rows = Vec::new();
    let mut add = |session: &BattleModelSession, frame: Option<tswn_core::cli_api::battle::BattleModelFrame>| -> Result<()> {
        let rounds_advanced = frame.map_or(0, |frame| frame.rounds_advanced);
        let state = session.model_state()?;
        ensure!(state.world.winner_team.is_none(), "不得采样已决终局");
        rows.push(SampleRow {
            battle_id: battle_id as u64,
            matchup_id: case.matchup_id.clone(),
            split: random::split(&case.matchup_id).into(),
            frame,
            rounds_advanced,
            progress: rounds_advanced as f64 / outcome.rounds_advanced.max(1) as f64,
            winner_team_index: winner,
            state,
        });
        Ok(())
    };
    if !initial_terminal {
        add(&second, None)?;
    }
    let mut seen = 0;
    while let Some(frame) = second.next_frame()? {
        ensure!(frames.get(seen) == Some(&frame), "两遍可见帧边界不一致");
        if selected.binary_search(&seen).is_ok() {
            add(&second, Some(frame))?;
        }
        seen += 1;
    }
    ensure!(
        seen == frames.len() && second.result().as_ref() == Some(&outcome),
        "两遍最终结果或帧数不一致"
    );
    Ok((
        BattleRow {
            battle_id: battle_id as u64,
            case_index,
            matchup_id: case.matchup_id.clone(),
            split: random::split(&case.matchup_id).into(),
            seed: seed.into(),
            outcome,
            samples: rows.len(),
        },
        rows,
    ))
}
