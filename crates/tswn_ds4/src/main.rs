//! DS4 的统一 CLI 出口，流程实现由 library 提供。
//!
//! `run --root <目录>` 执行有文件副作用的工作流，局部命令见 `--help`。
//! Rust 应用使用 [`tswn_ds4::run`]，按 Result 处理宿主中的执行结果。

fn main() {
    if let Err(err) = tswn_ds4::run_cli(std::env::args().skip(1)) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
