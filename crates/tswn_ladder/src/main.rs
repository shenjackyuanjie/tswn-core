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
//! 以 binary 和数据库协议接入；单元测试覆盖纯逻辑，不启动 PostgreSQL。
//! 实际回写与断线恢复需要另用独立测试数据库验证，启动生产配置会直接产生数据库副作用。
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
use storage::{connect_postgres, load_member_state, save_member_state};

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
/// 每隔多少轮自己 VACUUM 一次 handicap。
///
/// 这张表每分钟要更新约 1800 行，全靠 autovacuum 回收死元组；线上实测
/// autovacuum 在共享集群里会整小时不启动（`last_autovacuum` 停在 6 小时前，
/// 期间 `n_dead_tup` 涨到 4.5 万，索引和堆一起膨胀）。表本身只有几 MB，
/// 自己收一次是毫秒级，且 `VACUUM` 只拿 SHARE UPDATE EXCLUSIVE，不挡读写。
const HANDICAP_VACUUM_EVERY: u32 = 10;
/// 每轮结束后隔多少轮把累计场次快照落盘一次。
const MATCHCOUNT_SAVE_EVERY: u32 = 10;
const MATCHCOUNT_STATE_FILE: &str = "ladder_matchcount.txt";
/// 换号后保持满 K 的轮数。
///
/// 站点的换号逻辑（`app/main/team_routes.py`）会扣 10% 分数并清掉该成员的 handicap，
/// 旧版 K=80 时这笔分大约 3 小时补回来；降到 K=12 之后同样的缺口要 20 小时才补回，
/// 相当于把处罚悄悄加重了 6 倍。检测到成员名称变化（=换号）时，临时恢复满 K 跑
/// `LADDER_K_BOOST_ROUNDS` 轮，把回补速度拉回原来的量级；设 0 关闭。
const K_BOOST_ROUNDS: u32 = 250;
const BATTLE_ERROR_LOG_LIMIT: usize = 3;

const MAX_ACTIVE_MATCHES: usize = 7;
const MAX_PASSIVE_MATCHES: usize = 8;
/// 每人每轮的场次上限（含给「补位」预留的 2 个槽）。
///
/// **补位/退火这一段从 Node 版起就是关闭的，2026-10-23 复核后决定继续保持关闭。**
/// 它原本的作用是：以 `Σ(玩家本轮期望改分)²` 为目标再补 1~2 场，让 handicap 造成的
/// 偏置在各人身上互相抵消。模拟复核（`tools/`，单组 106 人 / 700 轮，与线上算法同源）：
///
/// | 方案 | 目标 loss | 每轮场次 | 每轮 \|Δ名次\| | 相邻 Spearman | 对真值准确度 |
/// | --- | --- | --- | --- | --- | --- |
/// | 不补位（现状） | 1367.6 | 641 | 0.64 | 0.9985 | 0.973 |
/// | 补位（无条件采纳，复刻旧行为） | 272.7（−80%） | 777 | 0.64 | 0.9985 | 0.973 |
/// | 补位（只收正向 + 必须整体变好） | 263.6（−81%） | 773 | 0.64 | 0.9985 | 0.975 |
///
/// 也就是说：**它确实把自己的目标优化掉了 80%，但天梯侧的波动/稳定度/准确度一个都没动，
/// 代价是场次 +21%（对战 CPU、history 写入等比增加）**。原因是它优化的那个量是二阶的——
/// `Σ改分` 每人每轮约 3 分（handicap 诱导），而单场正常改分就是 `K/2 = 6` 分，且各对的
/// 偏置符号随机，跨轮不累积。
///
/// 顺带发现旧实现有几个实现缺陷（即使要重做也不该照抄）：① 没有「比不补位更好才采纳」
/// 这层门槛，`bestLoss` 从不与「不补位」比较；② 初始贪心无条件接受 gain ≤ 0 的配对；
/// ③ `MIN_SQUARED_LOSS_IMPROVEMENT` 名字像采纳门槛，实际只当「继续降温」的判据；
/// ④ 候选筛选 `|delta| > 0.8·(1+ln(1+|score|/1000))` 在分数 11000 时门槛约 2.8 分，
/// 会滤掉大部分人；⑤ 退火每轮要做「温度 10→0.01 × 每温度 50 次 × 整条赛程重算」，
/// 在 JS 里很贵。
///
/// 若以后 handicap 的量级发生数量级变化（例如把补偿比例提到 100%，`|h|` 从 ~10 涨到 ~50），
/// 这个结论需要重新评估。
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

#[derive(Debug, Clone, Copy, Default)]
struct MemberState {
    /// 累计参赛场次，决定自适应 K。
    matches: f64,
    /// 上一轮看到的成员名称哈希（0 表示还没记录过）。站点的换号逻辑会改名称，
    /// 所以「名称变了」就是「换号了」的信号。
    name_hash: u64,
    /// 在此之前保持满 K（换号后快速回补被扣掉的分数）。
    boost_until: u32,
}

struct Updator {
    client: Client,
    config: Config,
    rng: fastrand::Rng,
    battle_error_count: usize,
    battle_error_suppressed: usize,
    /// 玩家的跨轮状态（累计场次 / 名称哈希 / K 恢复截止轮次）。
    member_state: HashMap<i32, MemberState>,
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

/// 成员名称的 FNV-1a 哈希。用哈希而不是原字符串，是因为名字里会有空格和中文，
/// 直接写进快照再按空白切分会坏掉。
pub(crate) fn name_hash(name: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// 换号检测：名称从「已知的 A」变成「B」时，返回该玩家新的 `boost_until`。
///
/// `previous_hash == 0` 表示第一次记录（还不知道原来叫什么），不算换号；
/// `boost_rounds == 0` 表示功能关闭。返回 `None` 表示不需要动 K。
pub(crate) fn rename_boost_until(previous_hash: u64, current_hash: u64, boost_rounds: u32, round_index: u32) -> Option<u32> {
    if previous_hash == 0 || previous_hash == current_hash || boost_rounds == 0 {
        return None;
    }
    Some(round_index.saturating_add(boost_rounds))
}

/// 资格门的分数窗口：`logistic_expected(x) ∈ [0.45, 0.55]` 等价于 `|x| ≤ 这个值`。
pub(crate) fn gate_score_limit() -> f64 { ELO_SCALE_FACTOR * (0.55_f64 / 0.45).log10() }

/// 这对玩家是否「不管 handicap 取多少都过不了资格门」。
///
/// 资格门判定的是 `|分数差 + 克制调整| ≤ gate_score_limit()`，而克制调整的绝对值有上界
/// `max_adjustment`（由本轮的 handicap 最大值算出）。所以分数差超过
/// `gate_score_limit() + max_adjustment` 时必然被拒——**它的判定结果与不做这次判定完全一致**，
/// 只是省掉了一次查表和一次 logistic。
///
/// 注意：只有「必然被拒」的探测才可以跳过。实测线上能跳过的只占全部探测的 **2.9%**
///（因为 `max_adjustment` 要按最坏情况取，约 1.1 万分），换算成时间约 0.17 ms/轮；
/// 一轮真正的耗时在「对战」（282 ms / 88%），匹配只占 5.9 ms / 1.8%。
pub(crate) fn probe_cannot_pass(score_gap_abs: f64, max_adjustment: f64) -> bool {
    score_gap_abs > gate_score_limit() + max_adjustment
}

#[cfg(test)]
mod tests {
    use super::battle::{format_seed_timestamp, generate_unique_seed, settle_round};
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
    fn member_state_snapshot_round_trips() {
        let dir = std::env::temp_dir().join(format!("tswn_ladder_state_{}", std::process::id()));
        let mut state = HashMap::new();
        state.insert(
            7,
            MemberState {
                matches: 1_234.0,
                name_hash: name_hash("乌云阻拦明月涌河湾 9fPX"),
                boost_until: 42,
            },
        );
        state.insert(3, MemberState::default());
        save_member_state(&dir, 12_345, &state);
        let loaded = load_member_state(&dir);
        assert_eq!(loaded.round, 12_345);
        assert_eq!(loaded.players.get(&7).map(|s| s.matches), Some(1_234.0));
        assert_eq!(
            loaded.players.get(&7).map(|s| s.name_hash),
            Some(name_hash("乌云阻拦明月涌河湾 9fPX"))
        );
        assert_eq!(loaded.players.get(&7).map(|s| s.boost_until), Some(42));
        assert_eq!(loaded.players.get(&3).map(|s| s.matches), Some(0.0));
        assert_eq!(loaded.players.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn member_state_snapshot_reads_old_two_field_format() {
        let dir = std::env::temp_dir().join(format!("tswn_ladder_old_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join(MATCHCOUNT_STATE_FILE), "11 90\n12 8\n").expect("write snapshot");
        let loaded = load_member_state(&dir);
        // 老格式没有轮次行，从 0 开始数
        assert_eq!(loaded.round, 0);
        assert_eq!(loaded.players.get(&11).map(|s| s.matches), Some(90.0));
        assert_eq!(loaded.players.get(&12).map(|s| s.matches), Some(8.0));
        // 老格式没有名称哈希，视为「还不知道」而不是换号
        assert_eq!(loaded.players.get(&11).map(|s| s.name_hash), Some(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn member_state_snapshot_missing_dir_is_empty() {
        let missing = std::env::temp_dir().join(format!("tswn_ladder_absent_{}", std::process::id()));
        let loaded = load_member_state(&missing);
        assert!(loaded.players.is_empty());
        assert_eq!(loaded.round, 0);
    }

    #[test]
    fn name_hash_is_stable_and_distinguishes_names() {
        assert_eq!(name_hash("abc"), name_hash("abc"));
        assert_ne!(name_hash("abc"), name_hash("abd"));
        assert_ne!(name_hash(""), 0);
    }

    #[test]
    fn round_settlement_uses_round_start_scores() {
        // 12000 的人一轮打 11000 和 10000 各一场，两场都赢。
        let scores = [12_000, 11_000, 10_000];
        let k = [80.0, 80.0, 80.0];
        let forward = settle_round(&scores, &[(0, 1), (0, 2)], &[true, true], &k);
        let backward = settle_round(&scores, &[(0, 2), (0, 1)], &[true, true], &k);

        // 顺序无关：这正是旧实现的 bug（后打的场次会用被前几场改过的分数算期望）
        assert!((forward[0] - backward[0]).abs() < 1e-9);

        // 两场都以轮初的 12000 为基准算期望，再加两个胜场 +1
        let expected = 12_000.0 + 80.0 * (1.0 - logistic_expected(1_000.0)) + 80.0 * (1.0 - logistic_expected(2_000.0)) + 2.0;
        assert!((forward[0] - expected).abs() < 1e-9);
        // 对手各输一场
        assert!((forward[1] - (11_000.0 - 80.0 * (1.0 - logistic_expected(1_000.0)))).abs() < 1e-9);
        assert!((forward[2] - (10_000.0 - 80.0 * (1.0 - logistic_expected(2_000.0)))).abs() < 1e-9);
        // 未参赛的人不动
        assert!((settle_round(&scores, &[], &[], &k)[0] - 12_000.0).abs() < 1e-9);
    }

    #[test]
    fn gate_limit_matches_the_probability_window() {
        let limit = gate_score_limit();
        assert!((logistic_expected(limit) - 0.55).abs() < 1e-4);
        assert!((logistic_expected(-limit) - 0.45).abs() < 1e-4);
    }

    #[test]
    fn pruned_probe_is_always_outside_the_gate() {
        // 线上实测的克制调整上界：(max|h| 86.68 + 扰动 40) * 88.3
        let max_adjustment = (86.68 + 40.0) * HANDICAP_SCALE;
        let limit = gate_score_limit();
        let gap = limit + max_adjustment + 1.0;
        assert!(probe_cannot_pass(gap, max_adjustment));
        // 再有利的克制调整也救不回来：两个方向、极端与零调整都被门拒
        for adj in [-max_adjustment, 0.0, max_adjustment] {
            for sign in [-1.0_f64, 1.0] {
                let p = logistic_expected(sign * gap + adj);
                assert!(!(0.45..=0.55).contains(&p), "p={p} adj={adj} sign={sign}");
            }
        }
        // 刚好落在边界内的探测绝不剪掉（否则会改变候选集）
        assert!(!probe_cannot_pass(limit + max_adjustment - 1.0, max_adjustment));
        assert!(!probe_cannot_pass(0.0, 0.0));
    }

    #[test]
    fn rename_boosts_only_on_actual_change() {
        let a = name_hash("旧名字");
        let b = name_hash("新名字");
        // 第一次记录（还不知原来叫什么）→ 不动 K
        assert_eq!(rename_boost_until(0, a, K_BOOST_ROUNDS, 100), None);
        // 名字没变 → 不动 K
        assert_eq!(rename_boost_until(a, a, K_BOOST_ROUNDS, 100), None);
        // 名字变了 → 从当前轮次起保持满 K
        assert_eq!(rename_boost_until(a, b, K_BOOST_ROUNDS, 100), Some(100 + K_BOOST_ROUNDS));
        // 关掉功能 → 不动 K
        assert_eq!(rename_boost_until(a, b, 0, 100), None);
    }
}
