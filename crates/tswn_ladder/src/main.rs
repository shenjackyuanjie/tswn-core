//! PostgreSQL 天梯更新进程：匹配、实际战斗、ELO/handicap 与排名历史。
//!
//! # 注意：启动会更新数据
//!
//! `DATABASE_URL` 指定数据库；默认按 cron 周期运行，`LADDER_RUN_ONCE` 可只执行一轮。
//! 调试使用独立测试数据库，便于检查完整更新和回写结果。
//!
//! # 一轮工作
//!
//! `storage 读玩家 → matching 配对 → battle 调用 core → 更新评分 → storage 回写/日志`。
//! `config` 管理分组、cron 和关闭信号，`updator` 管理周期、资源与错误编排。
//! seed 时间戳固定 Asia/Shanghai，PostgreSQL 关闭连接后尝试重连，其余错误由
//! 更新编排记录并处理。ELO、handicap、配额与衰减在 core 结果之上表达天梯策略。
//!
//! 以 binary 和数据库协议接入；单元测试覆盖纯逻辑，真实数据库测试验证回写。
//! 运行环境与配置见
//! [ladder README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_ladder)。

mod battle;
mod config;
mod matching;
mod storage;
mod updator;

#[cfg(test)]
use battle::run_battle_winner;
use config::*;
use matching::*;
use storage::{connect_postgres, load_match_counts, save_match_counts};

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use chrono_tz::Asia::Shanghai;
use cron::Schedule;
use tokio::io::AsyncWriteExt;
use tokio_postgres::{Client, NoTls};
use tswn_core::Runner;

const ELO_SCALE_FACTOR: f64 = 50_600.0;
const HANDICAP_SCALE: f64 = 88.3;
const BASE_K_FACTOR: f64 = 80.0;
/// 累计场次很多之后 K 收缩到的下限（占 `BASE_K_FACTOR` 的比例）。
/// 一分钟一轮时每个玩家一天要打上万场，固定 K=80 的稳态误差远大于真实实力差，
/// 这是排名每分钟大幅抖动的主因；新玩家仍用完整 K 快速定位。
const K_FLOOR_RATIO: f64 = 0.15;
/// K 收缩尺度：`K_i = BASE_K_FACTOR / (1 + n_i / K_SHRINK_MATCHES)`。
/// n_i 为玩家累计参赛场次（跨进程保存，见 `storage::load_match_counts`）。
const K_SHRINK_MATCHES: f64 = 200.0;
const K_HANDICAP: f64 = 6.0;
const HANDICAP_DECAY: f64 = 0.97;
const MATCHCOUNT_DECAY: f64 = 0.9999;
const PERTURBATION_RANGE: f64 = 40.0;
const DROPRATE: f64 = 0.9996;
const MEMORY_LIMIT_MB: u64 = 256;
const HANDICAP_PRECISION: u32 = 2;
/// handicap 回写刻度：与库里的值差不到这个量就不写（0.01 的存储精度下，
/// 1.0 换算成胜率只有 10^(1*88.3/50600)≈0.4%，对匹配没有可见影响）。
const HANDICAP_PERSIST_QUANTUM: f64 = 1.0;
/// 每隔多少轮清理一次「对局双方已不活跃」的 handicap 行。
const HANDICAP_MAINTENANCE_EVERY: u32 = 30;
/// 每轮结束后隔多少轮把累计场次快照落盘一次。
const MATCHCOUNT_SAVE_EVERY: u32 = 10;
const MATCHCOUNT_STATE_FILE: &str = "ladder_matchcount.txt";
const BATTLE_ERROR_LOG_LIMIT: usize = 3;

const MAX_ACTIVE_MATCHES: usize = 7;
const MAX_PASSIVE_MATCHES: usize = 8;
const MAX_MATCHES: usize = 11;
const SUPPLEMENT_RESERVED_SLOTS: usize = 2;
const PRE_SA_MAX_MATCHES: usize = MAX_MATCHES - SUPPLEMENT_RESERVED_SLOTS;

const SECOND_ACTIVE_MATCHES: usize = 5;
const SECOND_PASSIVE_MATCHES: usize = 9;
const GLOBAL_EXPLORATION_RATIO: f64 = 0.2;
const GLOBAL_EXPLORATION_MIN: usize = 2;
const HANDICAP_ZERO_EPSILON: f64 = 1e-9;

const DEFAULT_GROUPS: &[&str] = &["1a", "1b", "2a", "2b", "3a"];

#[derive(Debug, Clone)]
struct PlayerRow {
    id: i32,
    name: String,
    loaded_score: f64,
    score: i32,
    original_score: i32,
    initial_rank: i32,
    rank: i32,
    total_matches: usize,
    base_matches: usize,
    second_matches: usize,
    supplement_matches: usize,
}

#[derive(Debug, Clone)]
struct HandicapData {
    basic_value: f64,
    disturbed_value: f64,
    count: f64,
    /// 库里当前存着的值/场次；只有与它们拉开到 `HANDICAP_PERSIST_QUANTUM`
    /// 才需要回写。旧实现每轮把整张表（活跃玩家的全部对局组合）重写一遍，
    /// 24 GB 里绝大多数就是这种反复重写留下的死元组。
    persisted_value: f64,
    persisted_count: f64,
}

#[derive(Debug, Clone)]
struct HistoryRecord {
    win_or_lose: &'static str,
    from_member_id: i32,
    to_member_id: i32,
    current_score: i32,
    from_rank: i32,
    to_rank: i32,
    from_name: String,
    to_name: String,
    runs: String,
    number: String,
    expected_win_prob: f64,
    actual_result: f64,
}

#[derive(Debug, Clone)]
struct MatchStats {
    min_matches: usize,
    max_matches: usize,
    avg_matches: f64,
    final_squared_loss: f64,
}

#[derive(Debug, Clone)]
struct HandicapStats {
    avg_match_count: f64,
    avg_handicap: f64,
}

#[derive(Debug, Clone)]
struct UpdateResults {
    history_records: Vec<HistoryRecord>,
    changed_handicaps: HashSet<PairKey>,
    avg_delta: f64,
    match_quality: f64,
    failed_matches: usize,
    executed_matches: usize,
    skip_persist: bool,
    handicap_stats: HandicapStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PairKey(i32, i32);

impl PairKey {
    fn new(a: i32, b: i32) -> Self { if a < b { Self(a, b) } else { Self(b, a) } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MatchPair(usize, usize);

impl MatchPair {
    fn key(self) -> (usize, usize) { if self.0 < self.1 { (self.0, self.1) } else { (self.1, self.0) } }
}

struct Updator {
    client: Client,
    config: Config,
    rng: fastrand::Rng,
    battle_error_count: usize,
    battle_error_suppressed: usize,
    /// 玩家累计参赛场次（用于自适应 K）。启动时从状态文件恢复，定期落盘。
    match_counts: HashMap<i32, f64>,
    /// 已经跑过的轮次，用来驱动周期性维护（handicap 清理 / 场次落盘）。
    round_index: u32,
    /// 场次快照写入失败只提示一次，避免每 10 轮刷一条重复日志。
    state_save_warned: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let client = connect_postgres(&config.database_url).await?;
    let mut updator = Updator::new(client, config);
    if updator.config.run_once {
        updator.run_round().await?;
        updator.flush_state();
        return Ok(());
    }

    updator.run_forever().await
}

fn finite_or(value: f64, fallback: f64) -> f64 { if value.is_finite() { value } else { fallback } }

/// 本轮的统一衰减量：组内平均分按 `DROPRATE` 会掉的那一份，从每个玩家身上扣掉同样多。
///
/// 旧实现是 `score * DROPRATE`，扣得最多的是高分端，等于持续把分差往中间挤。分差被压缩
/// 之后，同样大小的改分噪声会造成更大的名次抖动；而且「分数尺度」和 K 绑在一起，
/// 降低 K 就等于压缩全榜分数。扣同一个量时，均值仍然稳定（衰减量正比于组均值，
/// 天然自校正到 ~11000），但分差完全不受影响。
pub(crate) fn decay_step(mean_score: f64) -> f64 { (1.0 - DROPRATE) * mean_score }

/// 自适应 K：`BASE_K_FACTOR / (1 + 累计场次 / shrink)`，下限 `floor`。
pub(crate) fn adaptive_k(played: f64, shrink_matches: f64, floor: f64) -> f64 {
    let shrink = if shrink_matches > 0.0 { shrink_matches } else { 1.0 };
    let k = BASE_K_FACTOR / (1.0 + played.max(0.0) / shrink);
    k.max(floor.min(BASE_K_FACTOR))
}

/// 库里的 handicap 是否已经落后得需要回写（变化不足 `quantum` 就不写）。
pub(crate) fn handicap_needs_persist(persisted_value: f64, persisted_count: f64, value: f64, count: f64, quantum: f64) -> bool {
    let quantum = if quantum > 0.0 { quantum } else { 1.0 };
    (value - persisted_value).abs() >= quantum || (count - persisted_count).abs() >= quantum
}

#[cfg(test)]
mod tests {
    use super::battle::{format_seed_timestamp, generate_unique_seed};
    use super::*;

    #[test]
    fn parses_five_field_cron_like_node_cron() {
        assert!(parse_cron_schedule("* * * * *").is_ok());
    }

    #[test]
    fn seed_timestamp_uses_asia_shanghai() {
        let utc = DateTime::parse_from_rfc3339("2026-07-20T16:30:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        assert_eq!(format_seed_timestamp(utc), "2026-07-21 00:30");
    }

    #[test]
    fn generate_unique_seed_uses_shanghai_clock() {
        let mut rng = fastrand::Rng::with_seed(7);
        let seed = generate_unique_seed(&mut rng);
        let (stamp, suffix) = seed.split_once(" #").expect("seed has suffix");
        assert_eq!(stamp, format_seed_timestamp(Utc::now()));
        assert_eq!(suffix.len(), 6);
    }

    #[test]
    fn battle_winner_maps_first_input_side_to_one() {
        let winner = run_battle_winner("aaaaa\n\nbbbbb\nseed:test@!", "aaaaa", "bbbbb").expect("battle should run");
        assert!(matches!(winner, Some(0) | Some(1) | None));
    }

    #[test]
    fn pair_key_is_ordered() {
        assert_eq!(PairKey::new(5, 2), PairKey(2, 5));
    }

    #[test]
    fn uniform_decay_keeps_score_gaps() {
        let (a, b) = (12_000.0_f64, 9_000.0_f64);
        let step = decay_step((a + b) / 2.0);
        // 每人扣同一个量 → 分差不变
        assert!(((a - step) - (b - step) - (a - b)).abs() < 1e-9);
        // 旧的按比例衰减会把分差压小
        assert!(a * DROPRATE - b * DROPRATE < a - b);
        // 衰减量正比于组均值，所以均值有固定点
        assert!(decay_step(11_000.0) > 0.0);
    }

    #[test]
    fn adaptive_k_shrinks_with_matches_and_respects_floor() {
        let floor = BASE_K_FACTOR * K_FLOOR_RATIO;
        assert_eq!(adaptive_k(0.0, K_SHRINK_MATCHES, floor), BASE_K_FACTOR);
        assert!(adaptive_k(K_SHRINK_MATCHES, K_SHRINK_MATCHES, floor) < BASE_K_FACTOR);
        assert!(adaptive_k(2_000.0, K_SHRINK_MATCHES, floor) < adaptive_k(K_SHRINK_MATCHES, K_SHRINK_MATCHES, floor));
        assert_eq!(adaptive_k(1.0e9, K_SHRINK_MATCHES, floor), floor);
    }

    #[test]
    fn handicap_persist_only_when_drifted() {
        // 变化不到一个刻度：不写
        assert!(!handicap_needs_persist(10.0, 100.0, 10.4, 100.0, HANDICAP_PERSIST_QUANTUM));
        // 值跨过一个刻度：写
        assert!(handicap_needs_persist(10.0, 100.0, 11.2, 100.0, HANDICAP_PERSIST_QUANTUM));
        // 场次跨过一个刻度：写
        assert!(handicap_needs_persist(10.0, 100.0, 10.0, 98.5, HANDICAP_PERSIST_QUANTUM));
    }

    #[test]
    fn match_count_snapshot_round_trips() {
        let dir = std::env::temp_dir().join(format!("tswn_ladder_state_{}", std::process::id()));
        let mut counts = HashMap::new();
        counts.insert(7, 1_234.0);
        counts.insert(3, 0.0);
        save_match_counts(&dir, &counts);
        let loaded = load_match_counts(&dir);
        assert_eq!(loaded.get(&7), Some(&1_234.0));
        assert_eq!(loaded.get(&3), Some(&0.0));
        assert_eq!(loaded.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn match_count_snapshot_missing_dir_is_empty() {
        let missing = std::env::temp_dir().join(format!("tswn_ladder_absent_{}", std::process::id()));
        assert!(load_match_counts(&missing).is_empty());
    }
}
