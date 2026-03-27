//! # 调试工具模块 (debug)
//!
//! 本模块提供统一的调试环境变量管理。
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
//! | `TSWN_DEBUG_COVID` | 调试 COVID Boss 相关逻辑 |
//! | `TSWN_DEBUG_FIRE` | 调试火焰技能 |
//! | `TSWN_DEBUG_HEAL` | 调试治疗技能 |
//! | `TSWN_DEBUG_UPGRADE` | 调试升级技能 |
//! | `TSWN_DEBUG_REFLECT` | 调试反射技能 |
//! | `TSWN_DEBUG_DAMAGE` | 调试伤害计算 |
//! | `TSWN_TRACE_RC4` | 追踪 RC4 随机数状态 |

use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ACTION: AtomicBool = AtomicBool::new(false);
static DEBUG_STATS: AtomicBool = AtomicBool::new(false);
static DEBUG_WORLD: AtomicBool = AtomicBool::new(false);
static DEBUG_TICK: AtomicBool = AtomicBool::new(false);
static DEBUG_PICK: AtomicBool = AtomicBool::new(false);
static DEBUG_DODGE: AtomicBool = AtomicBool::new(false);
static DEBUG_DODGE_ALL: AtomicBool = AtomicBool::new(false);
static DEBUG_DIE: AtomicBool = AtomicBool::new(false);
static DEBUG_STATE: AtomicBool = AtomicBool::new(false);
static DEBUG_COVID: AtomicBool = AtomicBool::new(false);
static DEBUG_FIRE: AtomicBool = AtomicBool::new(false);
static DEBUG_HEAL: AtomicBool = AtomicBool::new(false);
static DEBUG_UPGRADE: AtomicBool = AtomicBool::new(false);
static DEBUG_REFLECT: AtomicBool = AtomicBool::new(false);
static DEBUG_DAMAGE: AtomicBool = AtomicBool::new(false);
static TRACE_RC4: AtomicBool = AtomicBool::new(false);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

fn init_once() {
    if INITIALIZED.load(Ordering::Relaxed) {
        return;
    }
    INITIALIZED.store(true, Ordering::Relaxed);

    DEBUG_ACTION.store(std::env::var("TSWN_DEBUG_ACTION").is_ok(), Ordering::Relaxed);
    DEBUG_STATS.store(std::env::var_os("TSWN_DEBUG_STATS").is_some(), Ordering::Relaxed);
    DEBUG_WORLD.store(std::env::var_os("TSWN_DEBUG_WORLD").is_some(), Ordering::Relaxed);
    DEBUG_TICK.store(std::env::var_os("TSWN_DEBUG_TICK").is_some(), Ordering::Relaxed);
    DEBUG_PICK.store(std::env::var_os("TSWN_DEBUG_PICK").is_some(), Ordering::Relaxed);
    DEBUG_DODGE.store(std::env::var_os("TSWN_DEBUG_DODGE").is_some(), Ordering::Relaxed);
    DEBUG_DODGE_ALL.store(std::env::var_os("TSWN_DEBUG_DODGE_ALL").is_some(), Ordering::Relaxed);
    DEBUG_DIE.store(std::env::var("TSWN_DEBUG_DIE").is_ok(), Ordering::Relaxed);
    DEBUG_STATE.store(std::env::var_os("TSWN_DEBUG_STATE").is_some(), Ordering::Relaxed);
    DEBUG_COVID.store(std::env::var_os("TSWN_DEBUG_COVID").is_some(), Ordering::Relaxed);
    DEBUG_FIRE.store(std::env::var_os("TSWN_DEBUG_FIRE").is_some(), Ordering::Relaxed);
    DEBUG_HEAL.store(std::env::var_os("TSWN_DEBUG_HEAL").is_some(), Ordering::Relaxed);
    DEBUG_UPGRADE.store(std::env::var("TSWN_DEBUG_UPGRADE").is_ok(), Ordering::Relaxed);
    DEBUG_REFLECT.store(std::env::var_os("TSWN_DEBUG_REFLECT").is_some(), Ordering::Relaxed);
    DEBUG_DAMAGE.store(std::env::var_os("TSWN_DEBUG_DAMAGE").is_some(), Ordering::Relaxed);
    TRACE_RC4.store(std::env::var_os("TSWN_TRACE_RC4").is_some(), Ordering::Relaxed);
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
        std::env::var("TSWN_DEBUG_ACTION").map(|v| v == name).unwrap_or(false)
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

macro_rules! debug_println {
    ($condition:expr, $($arg:tt)*) => {
        if $condition {
            eprintln!($($arg)*);
        }
    };
}

pub(crate) use debug_println;
