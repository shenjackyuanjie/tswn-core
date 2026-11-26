//! selfplay-dump.rs — 自对弈状态采集：
//! - 默认模式：评分四模式（pp/pd/qp/qd）战斗，每轮调度前导出双方视角特征，
//!   战斗结束后按终局胜负标注 ±1。
//! - --rollout-ends 模式：搜索估值调用点采集（TSWN_DUMP_VALUE=1 生效），
//!   导出滚出终点状态（即 search_position_value 实际被调用的分布）供训练对齐。
//!
//! 文件格式（VERSION=3）：小端 [magic u32=0x4E4E4453][version u32=3][dims u32][samples u64]
//! 后接 samples 行 (dims+3) 个 f32：[特征..., 标签(±1), stage(0/1/2), name_group]。
//! 标签以行内特征视角给出；stage 为 0（回合起点或筛选）、1（主续局末端）、2（短战术比较）。
use clap::Parser;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tswn_core::runtime::search::SearchConfig;
use tswn_core::runtime::{CombatRuntime, EntityIdx, ScoreTestSession, nn_eval};

const MAGIC: u32 = 0x4E4E4453;
const VERSION: u32 = 3;
const MODES: [(bool, bool, &str); 4] = [
    (false, false, "pp"),
    (false, true, "pd"),
    (true, false, "qp"),
    (true, true, "qd"),
];
/// 单场最多保留的回合样本对数（防拖延局撑爆输出）。
const MAX_ROUND_PAIRS_PER_GAME: usize = 2000;
/// 每个 (名字, 模式) 任务连跑的战斗数，摊销会话构造开销。
const GAMES_PER_JOB: u32 = 4;

#[derive(Parser, Clone)]
struct Args {
    /// 名字池文件（每行 名字@队伍）
    #[arg(long, required = true)]
    names: PathBuf,
    /// 目标样本数（达到即停）
    #[arg(long, default_value_t = 2_000_000)]
    max_samples: u64,
    /// 并行工作线程数
    #[arg(long, default_value_t = 40)]
    workers: usize,
    /// 输出文件
    #[arg(long, required = true)]
    out: PathBuf,
    /// 池抽样步长
    #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(1..))]
    stride: u64,
    /// 每个名字和模式采集的对局数；增加此值可扩大独立 profile 的覆盖
    #[arg(long, default_value_t = GAMES_PER_JOB, value_parser = clap::value_parser!(u32).range(1..=10000))]
    games_per_job: u32,
    /// 每 K 轮采样一次
    #[arg(long, default_value_t = 1)]
    every: u64,
    /// 采集搜索滚出终点的估值状态（TSWN_DUMP_VALUE=1 生效），而非回合起点状态
    #[arg(long)]
    rollout_ends: bool,
}

struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

/// 蒙特卡洛自标签：从给定状态继续原版策略（与原搜索滚出同一策略）推演至终局，
/// 返回该状态在 team 视角下的终局标签；超过 cap 轮未分胜负则丢弃（None）。
fn mc_label(state: &CombatRuntime, team: usize) -> Option<f32> {
    let mut sim = state.clone();
    for _ in 0..2000 {
        if sim.world.winner_team().is_some() {
            break;
        }
        let _ = sim.run_minimal_round();
    }
    let winner = sim.world.winner_team()?;
    Some(if winner == team { 1.0 } else { -1.0 })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let mut names: Vec<String> = Vec::new();
    let file = File::open(&args.names)?;
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if !line.is_empty() && i as u64 % args.stride == 0 {
            names.push(line);
        }
    }
    if names.is_empty() {
        return Err("名字池为空".into());
    }
    names.sort_unstable();
    names.dedup();
    if names.len() > 1 << 24 {
        return Err("名字数量超过 f32 分组编号的精确范围".into());
    }
    if args.rollout_ends && std::env::var("TSWN_DUMP_VALUE").as_deref() != Ok("1") {
        return Err("--rollout-ends 需要 TSWN_DUMP_VALUE=1".into());
    }
    let mut rng = SplitMix64(0x2026_1122 ^ args.stride);
    let mut jobs: Vec<(usize, String, bool, bool)> = Vec::new();
    for (group, name) in names.iter().enumerate() {
        for (enhanced, duel, _) in MODES {
            jobs.push((group, name.clone(), enhanced, duel));
        }
    }
    for i in (1..jobs.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        jobs.swap(i, j);
    }
    let jobs = Arc::new(jobs);
    let next_job = Arc::new(AtomicUsize::new(0));
    let total_samples = Arc::new(AtomicU64::new(0));
    let done_games = Arc::new(AtomicU64::new(0));
    let writer = Arc::new(Mutex::new(BufWriter::new(File::create(&args.out)?)));
    writer.lock().unwrap().write_all(&MAGIC.to_le_bytes())?;
    writer.lock().unwrap().write_all(&VERSION.to_le_bytes())?;
    writer.lock().unwrap().write_all(&(nn_eval::NN_FEATURE_DIMS as u32).to_le_bytes())?;
    writer.lock().unwrap().write_all(&0_u64.to_le_bytes())?;
    let dims = nn_eval::NN_FEATURE_DIMS;
    let every = args.every.max(1);
    let max_samples = args.max_samples;
    let rollout_ends = args.rollout_ends;
    let out_path = args.out.clone();
    let mut handles = Vec::new();
    for _ in 0..args.workers.max(1) {
        let (jobs, next_job, total_samples, done_games, writer) = (
            jobs.clone(),
            next_job.clone(),
            total_samples.clone(),
            done_games.clone(),
            writer.clone(),
        );
        handles.push(std::thread::spawn(move || {
            loop {
                if total_samples.load(Ordering::Relaxed) >= max_samples {
                    break;
                }
                let index = next_job.fetch_add(1, Ordering::Relaxed);
                let Some((group, name, enhanced, duel)) = jobs.get(index) else {
                    break;
                };
                let raw = if *duel { format!("{name}\n{name}") } else { name.clone() };
                let mut session = if rollout_ends {
                    let Ok(mut session) = ScoreTestSession::new_with_mode(&raw, Some(SearchConfig::default()), *enhanced) else {
                        continue;
                    };
                    session.set_legacy_opponents(true);
                    session
                } else {
                    let Ok(session) = ScoreTestSession::new_with_mode(&raw, None, *enhanced) else {
                        continue;
                    };
                    session
                };
                for _ in 0..args.games_per_job {
                    if total_samples.load(Ordering::Relaxed) >= max_samples {
                        break;
                    }
                    let rows: Vec<f32> = if rollout_ends {
                        // 滚出终点模式：估值调用点由 search_reward 内的采集钩子登记（战场克隆）。
                        // 标签 = 蒙特卡洛自标签：从该状态继续原版策略推演到终局的结局，
                        // 与真实战斗结局解耦，避免「平行宇宙」标签错配。
                        nn_eval::nn_dump_clear();
                        let Ok((_truncated, _winner)) = session.step_observed(|_runtime| {}) else {
                            continue;
                        };
                        done_games.fetch_add(1, Ordering::Relaxed);
                        let mut rows = Vec::new();
                        for (state, team, stage) in nn_eval::nn_dump_drain() {
                            let other = state
                                .entities
                                .iter()
                                .find(|(_, entity)| entity.runtime.team != team)
                                .map(|(_, entity)| entity.runtime.team)
                                .unwrap_or(team ^ 1);
                            let feats0 = state.nn_features(team);
                            let feats1 = state.nn_features(other);
                            let Some(label0) = mc_label(&state, team) else {
                                continue;
                            };
                            rows.extend_from_slice(&feats0);
                            rows.push(label0);
                            rows.push(f32::from(stage as u8));
                            rows.push(*group as f32);
                            rows.extend_from_slice(&feats1);
                            rows.push(-label0);
                            rows.push(f32::from(stage as u8));
                            rows.push(*group as f32);
                        }
                        rows
                    } else {
                        let mut team0 = usize::MAX;
                        let mut feats: Vec<f32> = Vec::new();
                        let mut rounds_seen: u64 = 0;
                        let result = session.step_observed(|runtime| {
                            rounds_seen += 1;
                            if rounds_seen % every != 0 {
                                return;
                            }
                            if feats.len() / (2 * dims) >= MAX_ROUND_PAIRS_PER_GAME {
                                return;
                            }
                            let Some(first) = runtime.entities.get(EntityIdx(0)) else {
                                return;
                            };
                            team0 = first.runtime.team;
                            let team1 = runtime
                                .entities
                                .iter()
                                .find(|(_, entity)| entity.runtime.team != team0)
                                .map(|(_, entity)| entity.runtime.team)
                                .unwrap_or(team0 ^ 1);
                            feats.extend_from_slice(&runtime.nn_features(team0));
                            feats.extend_from_slice(&runtime.nn_features(team1));
                        });
                        done_games.fetch_add(1, Ordering::Relaxed);
                        let Ok((truncated, winner)) = result else {
                            continue;
                        };
                        if truncated || team0 == usize::MAX {
                            continue;
                        }
                        let Some(winner) = winner else {
                            continue;
                        };
                        let lab0: f32 = if winner == team0 { 1.0 } else { -1.0 };
                        let pairs = feats.len() / (2 * dims);
                        if pairs == 0 {
                            continue;
                        }
                        let mut rows = Vec::with_capacity(2 * pairs * (dims + 3));
                        for pair in 0..pairs {
                            rows.extend_from_slice(&feats[pair * 2 * dims..(pair * 2 + 1) * dims]);
                            rows.push(lab0);
                            rows.push(0.0);
                            rows.push(*group as f32);
                            rows.extend_from_slice(&feats[(pair * 2 + 1) * dims..(pair * 2 + 2) * dims]);
                            rows.push(-lab0);
                            rows.push(0.0);
                            rows.push(*group as f32);
                        }
                        rows
                    };
                    if rows.is_empty() {
                        continue;
                    }
                    let n = (rows.len() / (dims + 3)) as u64;
                    total_samples.fetch_add(n, Ordering::Relaxed);
                    let mut w = writer.lock().unwrap();
                    for chunk in rows.chunks(4096) {
                        let bytes: Vec<u8> = chunk.iter().flat_map(|v| v.to_le_bytes()).collect();
                        w.write_all(&bytes).unwrap();
                    }
                    if done_games.load(Ordering::Relaxed) % 500 == 0 {
                        println!(
                            "已跑 {} 场，样本 {}",
                            done_games.load(Ordering::Relaxed),
                            total_samples.load(Ordering::Relaxed)
                        );
                    }
                }
            }
        }));
    }
    for handle in handles {
        handle.join().map_err(|_| "采集线程异常退出")?;
    }
    let total = total_samples.load(Ordering::Relaxed);
    {
        let mut w = writer.lock().unwrap();
        w.flush()?;
        w.get_mut().seek(SeekFrom::Start(12))?;
        w.get_mut().write_all(&total.to_le_bytes())?;
        w.flush()?;
    }
    println!(
        "采集完成：{} 场战斗，样本 {}，输出 {}",
        done_games.load(Ordering::Relaxed),
        total,
        out_path.display()
    );
    Ok(())
}
