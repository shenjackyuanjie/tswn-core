//! # 调试工具模块 (debug)
//!
//! 本模块提供统一的调试环境变量管理。
//!
//! 常规开关在首次查询时一起读取环境，初始化完成前并发查询会等待；之后只读取缓存标志。
//! bool 开关按变量是否存在启用，因此值为 `0` 仍表示开启。ACTION、DIE、UPGRADE 额外要求
//! 值是有效 Unicode，启用后每次查询仍读取当前文本；调用方应在启动工作线程前设置环境。
//! `TSWN_PROBE_*` 等文本探针则各自按首次查询缓存，后续环境变更不生效。
//!
//! ## 支持的环境变量
//!
//! | 环境变量 | 说明 |
//! |----------|------|
//! | `TSWN_DEBUG_ACTION` | 调试特定玩家的行动（值为玩家名） |
//! | `TSWN_DEBUG_STATS` | 调试玩家属性计算 |
//! | `TSWN_DEBUG_WORLD` | 调试世界状态同步 |
//! | `TSWN_DEBUG_TICK` | 调试每个 tick 的执行 |
//! | `TSWN_DEBUG_PICK` | 调试目标选择逻辑 |
//! | `TSWN_DEBUG_DODGE` | 调试闪避逻辑 |
//! | `TSWN_DEBUG_DODGE_ALL` | 调试所有玩家的闪避 |
//! | `TSWN_DEBUG_DIE` | 调试死亡处理 |
//! | `TSWN_DEBUG_STATE` | 调试状态系统（状态设置/清除/追踪） |
//! | `TSWN_DEBUG_POST_ACTION` | 调试 post_action 链顺序 |
//! | `TSWN_DEBUG_FORCED_SKILL` | 调试 pre_action 强制技能选择 |
//! | `TSWN_DEBUG_COVID` | 调试 COVID Boss 相关逻辑 |
//! | `TSWN_DEBUG_FIRE` | 调试火焰技能 |
//! | `TSWN_DEBUG_HEAL` | 调试治疗技能 |
//! | `TSWN_DEBUG_UPGRADE` | 调试升级技能 |
//! | `TSWN_DEBUG_REFLECT` | 调试反射技能 |
//! | `TSWN_DEBUG_DAMAGE` | 调试伤害计算 |
//! | `TSWN_TRACE_RC4` | 追踪 RC4 随机数状态 |

use std::sync::{
    Once, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static DEBUG_ACTION: AtomicBool = AtomicBool::new(false);
static DEBUG_STATS: AtomicBool = AtomicBool::new(false);
static DEBUG_WORLD: AtomicBool = AtomicBool::new(false);
static DEBUG_TICK: AtomicBool = AtomicBool::new(false);
static DEBUG_PICK: AtomicBool = AtomicBool::new(false);
static DEBUG_DODGE: AtomicBool = AtomicBool::new(false);
static DEBUG_DODGE_ALL: AtomicBool = AtomicBool::new(false);
static DEBUG_DIE: AtomicBool = AtomicBool::new(false);
static DEBUG_STATE: AtomicBool = AtomicBool::new(false);
static DEBUG_POST_ACTION: AtomicBool = AtomicBool::new(false);
static DEBUG_FORCED_SKILL: AtomicBool = AtomicBool::new(false);
static DEBUG_COVID: AtomicBool = AtomicBool::new(false);
static DEBUG_FIRE: AtomicBool = AtomicBool::new(false);
static DEBUG_HEAL: AtomicBool = AtomicBool::new(false);
static DEBUG_UPGRADE: AtomicBool = AtomicBool::new(false);
static DEBUG_REFLECT: AtomicBool = AtomicBool::new(false);
static DEBUG_DAMAGE: AtomicBool = AtomicBool::new(false);
static TRACE_RC4: AtomicBool = AtomicBool::new(false);
static INITIALIZED: Once = Once::new();

fn init_once() {
    INITIALIZED.call_once(|| {
        DEBUG_ACTION.store(std::env::var("TSWN_DEBUG_ACTION").is_ok(), Ordering::Relaxed);
        DEBUG_STATS.store(std::env::var_os("TSWN_DEBUG_STATS").is_some(), Ordering::Relaxed);
        DEBUG_WORLD.store(std::env::var_os("TSWN_DEBUG_WORLD").is_some(), Ordering::Relaxed);
        DEBUG_TICK.store(std::env::var_os("TSWN_DEBUG_TICK").is_some(), Ordering::Relaxed);
        DEBUG_PICK.store(std::env::var_os("TSWN_DEBUG_PICK").is_some(), Ordering::Relaxed);
        DEBUG_DODGE.store(std::env::var_os("TSWN_DEBUG_DODGE").is_some(), Ordering::Relaxed);
        DEBUG_DODGE_ALL.store(std::env::var_os("TSWN_DEBUG_DODGE_ALL").is_some(), Ordering::Relaxed);
        DEBUG_DIE.store(std::env::var("TSWN_DEBUG_DIE").is_ok(), Ordering::Relaxed);
        DEBUG_STATE.store(std::env::var_os("TSWN_DEBUG_STATE").is_some(), Ordering::Relaxed);
        DEBUG_POST_ACTION.store(std::env::var_os("TSWN_DEBUG_POST_ACTION").is_some(), Ordering::Relaxed);
        DEBUG_FORCED_SKILL.store(std::env::var_os("TSWN_DEBUG_FORCED_SKILL").is_some(), Ordering::Relaxed);
        DEBUG_COVID.store(std::env::var_os("TSWN_DEBUG_COVID").is_some(), Ordering::Relaxed);
        DEBUG_FIRE.store(std::env::var_os("TSWN_DEBUG_FIRE").is_some(), Ordering::Relaxed);
        DEBUG_HEAL.store(std::env::var_os("TSWN_DEBUG_HEAL").is_some(), Ordering::Relaxed);
        DEBUG_UPGRADE.store(std::env::var("TSWN_DEBUG_UPGRADE").is_ok(), Ordering::Relaxed);
        DEBUG_REFLECT.store(std::env::var_os("TSWN_DEBUG_REFLECT").is_some(), Ordering::Relaxed);
        DEBUG_DAMAGE.store(std::env::var_os("TSWN_DEBUG_DAMAGE").is_some(), Ordering::Relaxed);
        TRACE_RC4.store(std::env::var_os("TSWN_TRACE_RC4").is_some(), Ordering::Relaxed);
    });
}

#[inline]
pub fn debug_action() -> Option<String> {
    init_once();
    if DEBUG_ACTION.load(Ordering::Relaxed) {
        std::env::var("TSWN_DEBUG_ACTION").ok()
    } else {
        None
    }
}

#[inline]
pub fn debug_action_matches(name: &str) -> bool {
    init_once();
    if DEBUG_ACTION.load(Ordering::Relaxed) {
        std::env::var("TSWN_DEBUG_ACTION")
            .map(|needle| needle == name || name.contains(&needle))
            .unwrap_or(false)
    } else {
        false
    }
}

#[inline]
pub fn debug_stats() -> bool {
    init_once();
    DEBUG_STATS.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_world() -> bool {
    init_once();
    DEBUG_WORLD.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_tick() -> bool {
    init_once();
    DEBUG_TICK.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_pick() -> bool {
    init_once();
    DEBUG_PICK.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_dodge() -> bool {
    init_once();
    DEBUG_DODGE.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_dodge_all() -> bool {
    init_once();
    DEBUG_DODGE_ALL.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_die() -> Option<String> {
    init_once();
    if DEBUG_DIE.load(Ordering::Relaxed) {
        std::env::var("TSWN_DEBUG_DIE").ok()
    } else {
        None
    }
}

#[inline]
pub fn debug_state() -> bool {
    init_once();
    DEBUG_STATE.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_post_action() -> bool {
    init_once();
    DEBUG_POST_ACTION.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_forced_skill() -> bool {
    init_once();
    DEBUG_FORCED_SKILL.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_covid() -> bool {
    init_once();
    DEBUG_COVID.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_fire() -> bool {
    init_once();
    DEBUG_FIRE.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_heal() -> bool {
    init_once();
    DEBUG_HEAL.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_upgrade() -> Option<String> {
    init_once();
    if DEBUG_UPGRADE.load(Ordering::Relaxed) {
        std::env::var("TSWN_DEBUG_UPGRADE").ok()
    } else {
        None
    }
}

#[inline]
pub fn debug_reflect() -> bool {
    init_once();
    DEBUG_REFLECT.load(Ordering::Relaxed)
}

#[inline]
pub fn debug_damage() -> bool {
    init_once();
    DEBUG_DAMAGE.load(Ordering::Relaxed)
}

#[inline]
pub fn trace_rc4() -> bool {
    init_once();
    TRACE_RC4.load(Ordering::Relaxed)
}

/// 缓存一次性的 `TSWN_PROBE_*` / `TSWN_DEBUG_*` 探针取值。
///
/// 每个探针独立缓存首次查询的 Unicode 字符串，未设置或非 Unicode 的值缓存为 `None`。
/// 返回值借用进程生命周期内的缓存，不复制字符串，也不在后续查询时重新读取环境；
/// 环境值应在启动查询线程前配置。此处只定义取值方式，打印时机由调用点决定。
macro_rules! cached_probe {
    ($(#[$meta:meta])* $name:ident, $var:literal) => {
        $(#[$meta])*
        #[inline]
        pub fn $name() -> Option<&'static str> {
            static CACHE: OnceLock<Option<String>> = OnceLock::new();
            CACHE.get_or_init(|| std::env::var($var).ok()).as_deref()
        }
    };
}

cached_probe!(
    /// `TSWN_PROBE_STEP`：逐 tick 打印调度器步进。
    probe_step,
    "TSWN_PROBE_STEP"
);
cached_probe!(
    /// `TSWN_PROBE_ACTION`：打印指定角色的普通行动准备过程。
    probe_action,
    "TSWN_PROBE_ACTION"
);
cached_probe!(
    /// `TSWN_PROBE_DEFAULT_ATTACK`：打印默认攻击选目标过程。
    probe_default_attack,
    "TSWN_PROBE_DEFAULT_ATTACK"
);
cached_probe!(
    /// `TSWN_PROBE_REFRESH`：打印待生效加速状态。
    probe_refresh,
    "TSWN_PROBE_REFRESH"
);
cached_probe!(
    /// `TSWN_PROBE_RC4`：追踪 RC4 状态。
    probe_rc4,
    "TSWN_PROBE_RC4"
);
cached_probe!(
    /// `TSWN_PROBE_PROTECT`：打印保护链判定。
    probe_protect,
    "TSWN_PROBE_PROTECT"
);
cached_probe!(
    /// `TSWN_PROBE_COUNTER`：打印反击结算计划。
    probe_counter,
    "TSWN_PROBE_COUNTER"
);
cached_probe!(
    /// `TSWN_PROBE_HIDE`：打印潜行结算。
    probe_hide,
    "TSWN_PROBE_HIDE"
);
cached_probe!(
    /// `TSWN_PROBE_POSSESS`：打印附身结算。
    probe_possess,
    "TSWN_PROBE_POSSESS"
);
cached_probe!(
    /// `TSWN_PROBE_CHARM`：打印魅惑选目标与结算。
    probe_charm,
    "TSWN_PROBE_CHARM"
);
cached_probe!(
    /// `TSWN_PROBE_EXCHANGE`：打印交换结算。
    probe_exchange,
    "TSWN_PROBE_EXCHANGE"
);
cached_probe!(
    /// `TSWN_PROBE_HEAL`：打印治疗结算。
    probe_heal,
    "TSWN_PROBE_HEAL"
);
cached_probe!(
    /// `TSWN_PROBE_MINION_BLUEPRINT`：打印召唤物蓝图初始化。
    probe_minion_blueprint,
    "TSWN_PROBE_MINION_BLUEPRINT"
);
cached_probe!(
    /// `TSWN_DEBUG_TICK_ORDER`：打印世界行动顺序。
    debug_tick_order,
    "TSWN_DEBUG_TICK_ORDER"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_first_queries_observe_initialized_environment() {
        const CHILD_ENV: &str = "TSWN_DEBUG_INIT_TEST_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            let barrier = std::sync::Barrier::new(32);
            std::thread::scope(|scope| {
                for _ in 0..32 {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        assert!(trace_rc4(), "首次并发查询必须等待全部标志初始化");
                        for query in [
                            debug_stats,
                            debug_world,
                            debug_tick,
                            debug_pick,
                            debug_dodge,
                            debug_dodge_all,
                            debug_state,
                            debug_post_action,
                            debug_forced_skill,
                            debug_covid,
                            debug_fire,
                            debug_heal,
                            debug_reflect,
                            debug_damage,
                        ] {
                            assert!(query());
                        }
                        assert_eq!(debug_action().as_deref(), Some("测试角色"));
                        assert!(debug_action_matches("测试角色分身"));
                        assert_eq!(debug_die().as_deref(), Some("测试角色"));
                        assert_eq!(debug_upgrade().as_deref(), Some("测试角色"));
                    });
                }
            });
            return;
        }

        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child.args([
            "--exact",
            "debug::tests::concurrent_first_queries_observe_initialized_environment",
            "--nocapture",
        ]);
        child.env(CHILD_ENV, "1");
        for name in [
            "TSWN_DEBUG_ACTION",
            "TSWN_DEBUG_STATS",
            "TSWN_DEBUG_WORLD",
            "TSWN_DEBUG_TICK",
            "TSWN_DEBUG_PICK",
            "TSWN_DEBUG_DODGE",
            "TSWN_DEBUG_DODGE_ALL",
            "TSWN_DEBUG_DIE",
            "TSWN_DEBUG_STATE",
            "TSWN_DEBUG_POST_ACTION",
            "TSWN_DEBUG_FORCED_SKILL",
            "TSWN_DEBUG_COVID",
            "TSWN_DEBUG_FIRE",
            "TSWN_DEBUG_HEAL",
            "TSWN_DEBUG_UPGRADE",
            "TSWN_DEBUG_REFLECT",
            "TSWN_DEBUG_DAMAGE",
            "TSWN_TRACE_RC4",
        ] {
            child.env(name, "测试角色");
        }
        let output = child.output().expect("应能运行隔离环境的测试子进程");
        assert!(
            output.status.success(),
            "并发首次查询失败：\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
