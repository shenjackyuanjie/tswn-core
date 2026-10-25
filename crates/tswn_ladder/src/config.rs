//! 运行配置、cron 解析和关闭信号。
//!
//! 启动时从环境读取连接串、分组与调度表达式；本模块只解析参数，不连接数据库。
//! 五字段 cron 会补上固定为 0 的秒字段，六/七字段交给 cron 库校验；实际调度按 UTC
//! 计算，与用于战斗 seed 的上海时间戳无关。关闭信号只在调度等待期间被轮询。

use super::*;

#[derive(Debug, Clone)]
pub(super) struct Config {
    pub(super) database_url: String,
    pub(super) cron_schedule: String,
    pub(super) groups: Vec<String>,
    pub(super) run_once: bool,
    pub(super) rank_log_dir: PathBuf,
    /// 运行状态目录（累计场次快照等），默认与 rank 日志同目录。
    pub(super) state_dir: PathBuf,
    /// 自适应 K 的收缩尺度与下限（可用 `LADDER_K_SHRINK_MATCHES` /
    /// `LADDER_K_FLOOR` 覆盖，便于线上不改二进制就调参）。
    pub(super) k_shrink_matches: f64,
    pub(super) k_floor: f64,
    /// handicap 回写刻度（`LADDER_HANDICAP_PERSIST_QUANTUM`）。
    pub(super) handicap_persist_quantum: f64,
    /// 只算不写：跑完一整轮但不执行任何 UPDATE/INSERT/DELETE（`LADDER_DRY_RUN`）。
    /// 用于在线上库上做真实负载的干跑验证。
    pub(super) dry_run: bool,
    /// 换号后保持满 K 的轮数（`LADDER_K_BOOST_ROUNDS`，0 = 关闭）。
    pub(super) k_boost_rounds: u32,
}

impl Config {
    pub(super) fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let cron_schedule = env::var("LADDER_CRON_SCHEDULE").unwrap_or_else(|_| "* * * * *".to_string());
        let groups = env::var("LADDER_GROUPS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|groups| !groups.is_empty())
            .unwrap_or_else(|| DEFAULT_GROUPS.iter().map(|x| x.to_string()).collect());
        let run_once = read_env_bool("LADDER_RUN_ONCE", false);
        let rank_log_dir = env::var("LADDER_RANK_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let state_dir = env::var("LADDER_STATE_DIR").map(PathBuf::from).unwrap_or_else(|_| rank_log_dir.clone());
        let k_shrink_matches = read_env_f64("LADDER_K_SHRINK_MATCHES", K_SHRINK_MATCHES);
        let k_floor = read_env_f64("LADDER_K_FLOOR", BASE_K_FACTOR * K_FLOOR_RATIO);
        let handicap_persist_quantum = read_env_f64("LADDER_HANDICAP_PERSIST_QUANTUM", HANDICAP_PERSIST_QUANTUM);
        let dry_run = read_env_bool("LADDER_DRY_RUN", false);
        let k_boost_rounds = read_env_f64("LADDER_K_BOOST_ROUNDS", K_BOOST_ROUNDS as f64).clamp(0.0, 100_000.0) as u32;

        Ok(Self {
            database_url,
            cron_schedule,
            groups,
            run_once,
            rank_log_dir,
            state_dir,
            k_shrink_matches,
            k_floor,
            handicap_persist_quantum,
            dry_run,
            k_boost_rounds,
        })
    }
}

/// 接受常见五字段 cron 或库支持的完整表达式，返回可复用的调度表。
///
/// 五字段输入表示按分钟触发，秒固定为 0；无效字段或表达式返回带原输入的解析错误。
/// 时区由调用调度表的一方决定，此函数不读取主机时区。
fn read_env_f64(name: &str, default: f64) -> f64 {
    match env::var(name).ok().and_then(|value| value.trim().parse::<f64>().ok()) {
        Some(value) if value.is_finite() => value,
        _ => default,
    }
}

pub(super) fn parse_cron_schedule(value: &str) -> Result<Schedule> {
    let parts = value.split_whitespace().count();
    if parts == 5 {
        let with_seconds = format!("0 {value}");
        return Schedule::from_str(&with_seconds).with_context(|| format!("parse cron schedule {value:?} / {with_seconds:?}"));
    }

    Schedule::from_str(value).with_context(|| format!("parse cron schedule {value:?}"))
}

pub(super) fn read_env_bool(name: &str, default: bool) -> bool {
    match env::var(name).ok().map(|x| x.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on") => true,
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "n" | "off") => false,
        _ => default,
    }
}

pub(super) async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate()).context("listen for SIGTERM")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("listen for ctrl-c")?;
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.context("listen for ctrl-c")?;
    }

    Ok(())
}
