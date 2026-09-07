//! Ordinary battle and Runtime diagnostic command entry points.
mod driver;
mod runtime;
mod trace;

pub use driver::{run, run_diff};
pub use runtime::run_runtime_normalized;
