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
fn boss_replay_projects_ash_before_revive_and_keeps_snap_targetless() {
    // 原版 Vite 的第七个可见批次先把 327 HP 的马里奥化为灰烬，再满血复活为 386 HP。
    let output = run(&["fight", "--jsonl"], Some("thanos@!\naaa\nbbb\nccc\nmario@!"));
    assert!(output.status.success(), "{}", stderr(&output));
    let frames: Vec<serde_json::Value> = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|event: &serde_json::Value| event["type"] == "frame")
        .collect();
    let frame = frames
        .iter()
        .map(|entry| &entry["data"])
        .find(|frame| {
            frame["updates"]
                .as_array()
                .unwrap()
                .iter()
                .any(|update| update["message_template"] == "[1]化成了灰烬" && update["score"] == 377)
        })
        .expect("应打出 327 HP 的马里奥灰烬事件");
    let updates = frame["updates"].as_array().unwrap();
    let snap = updates.iter().find(|update| update["message_template"] == "[0]打了个[响指]").unwrap();
    assert!(snap["target_id"].is_null(), "响指起手没有单体目标");
    let clips: Vec<_> = frame["rows"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row["clips"].as_array().unwrap())
        .collect();
    let ash = clips
        .iter()
        .position(|clip| {
            clip["parts"].as_array().unwrap().iter().any(|part| part["text"] == "化成了灰烬")
                && clip["parts"].as_array().unwrap().iter().any(|part| part["text"] == "马里奥")
        })
        .unwrap();
    let ash_part = &clips[ash]["parts"][0];
    let revived = &clips[ash + 1]["parts"][0];
    assert_eq!(ash_part["text"], "马里奥");
    assert_eq!(
        (ash_part["hp_before"].as_i64(), ash_part["hp_after"].as_i64()),
        (Some(327), Some(0))
    );
    assert_eq!(ash_part["show_hp"], true);
    assert_eq!(ash_part["death_effect"], true);
    assert_eq!(revived["player_id"], ash_part["player_id"]);
    assert_eq!(
        (revived["hp_before"].as_i64(), revived["hp_after"].as_i64()),
        (Some(0), Some(386))
    );
    assert_eq!(revived["show_hp"], true);
    assert_eq!(revived["death_effect"], false);
    for (index, alive) in [(ash, false), (ash + 1, true)] {
        let states = clips[index]["sidebar_states"].as_array().unwrap();
        let mario = states.iter().find(|state| state["id"] == ash_part["player_id"]).unwrap();
        assert_eq!(mario["alive"], alive);
        assert_eq!(mario["active"], alive);
    }
}

#[test]
fn boss_replay_projects_split_children_and_both_sides_of_exchange() {
    let output = run(&["fight", "--jsonl"], Some("wheel@!\nslime@!"));
    assert!(output.status.success(), "{}", stderr(&output));
    let frames: Vec<serde_json::Value> = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|event: &serde_json::Value| event["type"] == "frame")
        .collect();
    let updates = || frames.iter().flat_map(|entry| entry["data"]["updates"].as_array().unwrap());
    let split = updates().find(|update| update["message_template"] == "[0][分裂]").unwrap();
    assert!(split["target_id"].is_null(), "分裂起手不是对自己的伤害");
    let clips = || {
        frames
            .iter()
            .flat_map(|entry| entry["data"]["rows"].as_array().unwrap())
            .flat_map(|row| row["clips"].as_array().unwrap())
    };
    let find_clip = |text| {
        clips()
            .find(|clip| clip["parts"].as_array().unwrap().iter().any(|part| part["text"] == text))
            .unwrap()
    };
    let pairs = |clip: &serde_json::Value| {
        clip["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|part| part["show_hp"] == true)
            .map(|part| (part["hp_before"].as_i64().unwrap(), part["hp_after"].as_i64().unwrap()))
            .collect::<Vec<_>>()
    };
    assert_eq!(pairs(find_clip("分成了")), [(218, 218), (218, 218)]);
    assert_eq!(pairs(find_clip("互换")), [(218, 117), (117, 218)]);
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
