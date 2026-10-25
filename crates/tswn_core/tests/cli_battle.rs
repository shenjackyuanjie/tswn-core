//! CLI 进程的流式输出契约：JSONL 事件、stdin、人类可读输出、诊断命令与非法输入。
//!
//! 测试直接运行本包构建出的 `tswn-cli` 二进制，检查真实进程状态与 stdout/stderr 分流。
//! 战斗样例只读现有 runtime_stress 输入，不生成或覆盖 golden/corpus；流式刷新与写入失败
//! 的细粒度行为由二进制内的 writer 单测覆盖。参数失败案例不应创建输出文件或推进对局。

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

fn fixture(name: &str) -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../tswn_test/cases/runtime_stress").join(name) }

fn run(args: &[&str], raw: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tswn-cli"));
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(raw) = raw {
        command.stdin(Stdio::piped());
        let mut child = command.spawn().expect("启动 CLI");
        child.stdin.as_mut().expect("stdin 管道").write_all(raw.as_bytes()).expect("写入 stdin");
        child.wait_with_output().expect("等待 CLI")
    } else {
        command.output().expect("运行 CLI")
    }
}

fn stdout(output: &Output) -> String { String::from_utf8_lossy(&output.stdout).into_owned() }

fn stderr(output: &Output) -> String { String::from_utf8_lossy(&output.stderr).into_owned() }

#[test]
fn jsonl_streams_initial_frames_and_result() {
    for name in [
        "1v1-0f92cb76cc37fdc5.txt",
        "2v2-554f4128af707167.txt",
        "ffa_8-16d11de1ebe1df41.txt",
        "3v3v3-0ace5df17b84e26a.txt",
    ] {
        let path = fixture(name);
        assert!(path.is_file(), "缺少 fixture {}", path.display());
        for limit in [1usize, 20_000] {
            let output = run(
                &[
                    "fight",
                    "--jsonl",
                    "--max-rounds",
                    &limit.to_string(),
                    "-f",
                    path.to_str().unwrap(),
                ],
                None,
            );
            assert!(output.status.success(), "{name} limit={limit}: {}", stderr(&output));
            assert!(stderr(&output).is_empty(), "{name} limit={limit} 不应有 stderr");
            let events: Vec<serde_json::Value> = stdout(&output)
                .lines()
                .map(|line| serde_json::from_str(line).expect("JSONL 事件"))
                .collect();
            assert_eq!(events.first().unwrap()["type"], "initial", "{name}");
            assert_eq!(events.last().unwrap()["type"], "result", "{name}");
            assert!(
                events[1..events.len() - 1].iter().all(|event| event["type"] == "frame"),
                "{name} 中间事件必须是 frame"
            );
            let result = &events.last().unwrap()["data"];
            assert_eq!(
                result["frames_emitted"].as_u64().unwrap(),
                (events.len() - 2) as u64,
                "{name} 可见帧数必须与事件数一致"
            );
            assert!(result["rounds_advanced"].as_u64().unwrap() <= limit as u64, "{name}");
            let status = result["status"].as_str().unwrap();
            assert!(matches!(status, "finished" | "truncated"), "{name} 状态 {status}");
            let indices: Vec<u64> = events[1..events.len() - 1]
                .iter()
                .map(|event| event["data"]["frame_index"].as_u64().unwrap())
                .collect();
            assert_eq!(indices, (0..indices.len() as u64).collect::<Vec<_>>(), "{name}");
        }
    }
}

#[test]
fn invalid_fight_inputs_fail_on_stderr_only() {
    let cases: [&[&str]; 2] = [
        &["fight", "--jsonl", "--max-rounds", "0", "-r", "a\\n\\nb"],
        &["fight", "--jsonl", "-r", ""],
    ];
    for args in cases {
        let output = run(args, None);
        assert!(!output.status.success(), "{args:?} 必须失败");
        assert!(!stderr(&output).is_empty(), "{args:?} 必须在 stderr 报错");
        assert!(stdout(&output).is_empty(), "{args:?} 不得写 stdout");
    }
}

#[test]
fn invalid_metric_thresholds_and_paths_fail_before_output() {
    for spec in [
        "pp:NaN",
        "pp:inf",
        "pp:1e999",
        "pp::out.txt:NaN",
        "pp::out.txt:inf",
        r"pp::C:\scores\out.txt:stream",
        r"pp::C:\scores\out.txt:stream:1",
        r"pp::\\server\share\out.txt:stream",
    ] {
        let output = run(&["namer-pf", "-r", "mario", "--metric", spec], None);
        assert_eq!(output.status.code(), Some(2), "{spec}: {}", stderr(&output));
        assert!(!stderr(&output).is_empty(), "{spec} 必须在 stderr 报错");
        assert!(stdout(&output).is_empty(), "{spec} 不得写 stdout");
    }
}

#[test]
fn stdin_human_output_and_diagnostics() {
    let piped = run(&["fight", "--jsonl", "--max-rounds", "1"], Some("a\n\nb"));
    assert!(piped.status.success(), "{}", stderr(&piped));
    let first: serde_json::Value = serde_json::from_str(stdout(&piped).lines().next().unwrap()).unwrap();
    assert_eq!(first["type"], "initial");

    let human = run(&["fight", "--max-rounds", "1", "-r", "a\\n\\nb"], None);
    assert!(human.status.success(), "{}", stderr(&human));
    assert!(stdout(&human).contains("对局截断"), "人类可读输出应说明截断");

    let diagnostic = run(&["runtime", "diff", "-r", "a\\n\\nb"], None);
    assert!(diagnostic.status.success(), "{}", stderr(&diagnostic));
    assert!(!stdout(&diagnostic).is_empty(), "诊断命令应有输出");
    assert!(!stdout(&diagnostic).contains("欢迎"), "诊断输出不应包含欢迎语");
}
