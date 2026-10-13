//! tswn-pwp 命令行入口；解析 Clap 参数后调用库的统一执行门面。
//!
//! 生成、统计、校准、manifest 和编码的职责见 [`tswn_pwp`]。
//! 错误打印到 stderr，并以非零状态退出；生成失败不产生伪造的监督标签。

use clap::Parser;

fn main() {
    if let Err(error) = tswn_pwp::run(tswn_pwp::Cli::parse()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
