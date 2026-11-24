//! 测试回归追踪工具的短命令入口。
//!
//! 将 `track test` 分发到相邻的 `track_test` 二进制；如果当前目录下没有
//! 已构建的 sibling executable，则回退到 `cargo run` 启动。

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// `track` 入口：转发到子命令实现，错误打印到 stderr 并以退出码 1 结束。
fn main() {
    if let Err(err) = try_main() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

/// 解析 `track` 子命令并转发执行：优先调用同目录的兄弟可执行文件，缺失时回退 `cargo run`，最终透传子进程退出码。
fn try_main() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<String>>();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        print_usage();
        return Ok(());
    }

    let target = args.remove(0);
    let bin = match target.as_str() {
        "test" => "track_test",
        other => return Err(format!("未知 track 子命令: {other}")),
    };

    let status = if let Some(path) = sibling_executable(bin) {
        Command::new(path).args(&args).status().map_err(|e| format!("启动 {bin} 失败: {e}"))?
    } else {
        Command::new("cargo")
            .args(["run", "-p", "tswn_core", "--bin", bin, "--"])
            .args(&args)
            .status()
            .map_err(|e| format!("通过 cargo 启动 {bin} 失败: {e}"))?
    };

    std::process::exit(status.code().unwrap_or(1));
}

/// 打印 `track` 用法与子命令对应关系。
fn print_usage() {
    println!(
        r#"用法:
  track test [args...]

对应关系:
  track test   -> track_test
"#
    );
}

/// 在当前可执行文件同目录查找指定二进制（Windows 追加 `.exe`），存在则返回路径。
fn sibling_executable(bin: &str) -> Option<PathBuf> {
    let current = env::current_exe().ok()?;
    let dir = current.parent()?;
    let exe = if cfg!(windows) {
        dir.join(format!("{bin}.exe"))
    } else {
        dir.join(bin)
    };
    if exe.is_file() { Some(exe) } else { None }
}
