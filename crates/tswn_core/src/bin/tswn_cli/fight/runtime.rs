//! CLI 侧的主 Runtime 诊断入口。
//!
//! diff 收集消息文本后统一输出，normalized-run 返回完整的诊断 JSON；两者都先运行对局，
//! 不承担普通 `fight --jsonl` 的逐事件刷新契约。输入构建、执行与序列化错误转换成中文上下文，
//! 由命令入口写入 stderr 并以非零状态退出。

use super::trace::collect_runtime_diff_lines;
use tswn_core::cli_api::{self as core_cli_api, CliApiError, JsonRuntimeNormalizedRun};

/// `runtime diff` 入口：以默认回合上限生成 diff 行并打印；失败时打印错误并以退出码 1 结束。
pub(super) fn run_runtime_diff(raw: String) {
    match runtime_diff_lines(&raw, 20_000) {
        Ok(lines) if !lines.is_empty() => println!("{}", lines.join("\n")),
        Ok(_) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

/// 跑一局自定义 runtime 对局，把完整的规范化诊断 JSON 打印到 stdout；
/// 输入构建、执行或序列化失败时在 stderr 报告中文错误并以退出码 1 终止。
pub fn run_runtime_normalized(raw: String, max_rounds: usize) {
    match runtime_normalized_json(&raw, max_rounds) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

/// 用默认 custom runtime 混合 runner 生成 diff 行集合。
fn runtime_diff_lines(raw: &str, max_rounds: usize) -> Result<Vec<String>, String> {
    let mut runner = core_cli_api::default_custom_runtime_mixed_runner(raw).map_err(cli_api_error)?;
    let (lines, _, _) = collect_runtime_diff_lines(&mut runner, max_rounds, true);
    Ok(lines)
}

/// 运行 normalized-run 并序列化为美化 JSON。
fn runtime_normalized_json(raw: &str, max_rounds: usize) -> Result<String, String> {
    let run = core_cli_api::default_custom_runtime_normalized_run(raw, max_rounds).map_err(cli_api_error)?;
    serde_json::to_string_pretty(&JsonRuntimeNormalizedRun::from(run)).map_err(|err| format!("序列化 runtime JSON 失败: {err}"))
}

/// 把 `CliApiError` 转为面向用户的错误文本，按变体补充上下文。
fn cli_api_error(err: CliApiError) -> String {
    match err {
        CliApiError::InvalidInput(message)
        | CliApiError::InvalidArgument(message)
        | CliApiError::UnsupportedOption(message)
        | CliApiError::Internal(message) => message,
        CliApiError::RunnerInit(err) => format!("构建 runtime 对局失败: {err}"),
        CliApiError::Runtime(message) => format!("运行 runtime 对局失败: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "left@red\n\nright@blue\nseed:42@!";

    #[test]
    fn normalized_run_is_valid_json() {
        let json = runtime_normalized_json(RAW, 8).expect("normalized run should serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("normalized run should be JSON");
        assert!(
            value
                .get("rounds")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|rounds| !rounds.is_empty())
        );
    }

    #[test]
    fn diff_output_is_deterministic() {
        assert_eq!(runtime_diff_lines(RAW, 64).unwrap(), runtime_diff_lines(RAW, 64).unwrap());
    }
}
