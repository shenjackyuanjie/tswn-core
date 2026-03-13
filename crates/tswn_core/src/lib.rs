#[allow(dead_code)]
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
    pub const fn trace_rc4() -> bool { false }

    macro_rules! debug_println {
        ($condition:expr, $($arg:tt)*) => {{}};
    }

    pub(crate) use debug_println;
}

#[allow(dead_code)]
pub mod engine;
#[allow(dead_code)]
pub mod error;
#[allow(dead_code)]
pub mod player;
#[allow(dead_code)]
pub mod rc4;

pub use engine::runners::Runner;
pub use engine::update::{RunUpdate, RunUpdates};

#[inline]
pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }
