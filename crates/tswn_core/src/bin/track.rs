use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("错误: {err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<String>>();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        print_usage();
        return Ok(());
    }

    let target = args.remove(0);
    let bin = match target.as_str() {
        "test" => "track_test",
        "miner" => "track_case_miner",
        "diy" | "diy-roundtrip" => "track_diy_roundtrip",
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

fn print_usage() {
    println!(
        r#"用法:
  track <test|miner|diy> [args...]

对应关系:
  track test   -> track_test
  track miner  -> track_case_miner
  track diy    -> track_diy_roundtrip
"#
    );
}

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
