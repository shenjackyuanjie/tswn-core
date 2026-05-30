//! tswn-core — 星际战争命名器核心战斗引擎库。
//!
//! 提供 [`Runner`]（一次性战斗执行器）、[`PreparedRunner`]（预解析的批量执行器）
//! 及底层 RC4 随机数、玩家/技能/Boss 模型等全部核心类型。

#[cfg(feature = "mimalloc_alloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "no_debug"))]
pub mod debug;

#[cfg(feature = "no_debug")]
pub mod debug {
    #[inline(always)]
    pub fn debug_action() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_action_matches(_name: &str) -> bool { false }
    #[inline(always)]
    pub const fn debug_stats() -> bool { false }
    #[inline(always)]
    pub const fn debug_world() -> bool { false }
    #[inline(always)]
    pub const fn debug_tick() -> bool { false }
    #[inline(always)]
    pub const fn debug_pick() -> bool { false }
    #[inline(always)]
    pub const fn debug_dodge() -> bool { false }
    #[inline(always)]
    pub const fn debug_dodge_all() -> bool { false }
    #[inline(always)]
    pub fn debug_die() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_state() -> bool { false }
    #[inline(always)]
    pub const fn debug_post_action() -> bool { false }
    #[inline(always)]
    pub const fn debug_forced_skill() -> bool { false }
    #[inline(always)]
    pub const fn debug_covid() -> bool { false }
    #[inline(always)]
    pub const fn debug_fire() -> bool { false }
    #[inline(always)]
    pub const fn debug_heal() -> bool { false }
    #[inline(always)]
    pub fn debug_upgrade() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_reflect() -> bool { false }
    #[inline(always)]
    pub const fn debug_damage() -> bool { false }
    #[inline(always)]
    pub const fn trace_rc4() -> bool { false }

    macro_rules! debug_println {
        ($condition:expr, $($arg:tt)*) => {{}};
    }

    pub(crate) use debug_println;
}

pub mod case_gen;
pub mod cli_api;
pub mod engine;
pub mod error;
pub mod player;
pub mod rc4;
pub mod win_rate;

/// 核心对局入口。
///
/// - [`Runner`] 表示一场具体可运行的对局
/// - [`PreparedRunner`] 表示一份可复用的预构建模板，适合同一输入下按不同 seed 批量构造 `Runner`
///
/// 当你只需要跑单局时，通常直接使用 [`Runner`] 即可；
/// 当你需要对同一组输入重复跑很多局（如 win-rate / benchmark）时，优先考虑先构造 [`PreparedRunner`] 再复用。
pub use engine::runners::{PreparedRunner, Runner};
pub use engine::update::{RunUpdate, RunUpdates};

#[inline]
pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }
