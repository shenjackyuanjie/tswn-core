mod battle;
mod config;
mod matching;
mod storage;
mod updator;

#[cfg(test)]
use battle::run_battle_winner;
use config::*;
use matching::*;
use storage::connect_postgres;

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
const K_HANDICAP: f64 = 6.0;
const HANDICAP_DECAY: f64 = 0.97;
const MATCHCOUNT_DECAY: f64 = 0.9999;
const PERTURBATION_RANGE: f64 = 40.0;
const DROPRATE: f64 = 0.9996;
const MEMORY_LIMIT_MB: u64 = 256;
const HANDICAP_PRECISION: u32 = 2;
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
const HANDICAP_SPARSE_PERSIST: bool = true;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let client = connect_postgres(&config.database_url).await?;
    let mut updator = Updator::new(client, config);
    if updator.config.run_once {
        updator.run_round().await?;
        return Ok(());
    }

    updator.run_forever().await
}

fn finite_or(value: f64, fallback: f64) -> f64 { if value.is_finite() { value } else { fallback } }

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
}
