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
