//! 通过真实 CLI 对局校验已向 Vite 公开对象核实的 Boss 回放边界。

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

fn frames(raw: &str) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tswn-cli"))
        .args(["fight", "--jsonl"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(raw.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|entry| entry["type"] == "frame")
        .map(|mut entry| entry["data"].take())
        .collect()
}

fn events(frame: &Value) -> impl Iterator<Item = (&Value, &Value)> {
    let updates = frame["updates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|update| !update["message_template"].as_str().unwrap().trim().is_empty() && update["is_win"] == false);
    let clips = frame["rows"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row["clips"].as_array().unwrap())
        .filter(|clip| clip["winner"] == false);
    updates.zip(clips)
}

fn event<'a>(frame: &'a Value, message: &str) -> (&'a Value, &'a Value) {
    events(frame).find(|(update, _)| update["message_template"] == message).unwrap()
}

fn state<'a>(frame: &'a Value, id: &Value) -> &'a Value {
    frame["states"].as_array().unwrap().iter().find(|player| player["id"] == *id).unwrap()
}

fn verify_clip_hp(clip: &Value, id: &Value, before: i32, after: i32, show_hp: bool, death: bool) {
    let part = clip["parts"].as_array().unwrap().iter().find(|part| part["player_id"] == *id).unwrap();
    assert_eq!(part["hp_before"], before, "{part}");
    assert_eq!(part["hp_after"], after, "{part}");
    assert_eq!(part["show_hp"], show_hp, "{part}");
    assert_eq!(part["death_effect"], death, "{part}");
    for (key, hp) in [("sidebar_previous_states", before), ("sidebar_states", after)] {
        let item = clip[key].as_array().unwrap().iter().find(|player| player["id"] == *id).unwrap();
        assert_eq!(item["hp"], hp, "{key}: {item}");
        assert_eq!(item["alive"], hp > 0, "{key}: {item}");
    }
}

#[test]
fn conan_public_executions_show_hp_before_knockout_and_revive() {
    let clone_fight = frames("conan@!\n\nfighter22");
    let mario_fight = frames("conan@!\n\nmario@!");
    for (frame, message, name, hp) in [
        (&clone_fight[2], "[0]在一间密室中发现了[1]的尸体", "fighter22?0@fighter22", 141),
        (&clone_fight[6], "[1]", "fighter22", 141),
        (&mario_fight[5], "[1]", "mario@!", 386),
    ] {
        let (execution, clip) = event(frame, message);
        let id = &execution["target_id"];
        assert_eq!(state(frame, id)["id_key_name"], name);
        // 处决先显示体力结算，随后击倒只播放退场效果。
        verify_clip_hp(clip, id, hp, 0, true, false);
        let (knockout, clip) = event(frame, "[1]被击倒了");
        assert_eq!(knockout["target_id"], *id);
        verify_clip_hp(clip, id, 0, 0, false, true);
        assert_eq!(execution["score"], hp + 80);
        assert_eq!(execution["hp_delta"], -hp);
        assert!(execution["param"].is_null(), "旧数值参数不改为对象");
        assert_eq!(execution["param_payload"]["damage"], hp);
    }

    let frame = &mario_fight[5];
    for (message, delay1) in [("[0]推理完毕", 100), ("真相只有一个", 2000), ("凶手就是你", 100)] {
        let (update, clip) = event(frame, message);
        assert!(update["target_id"].is_null());
        assert_eq!(clip["target_ids"], serde_json::json!([]));
        assert_eq!(update["delay1"], delay1);
    }
    let (truth, _) = event(frame, "真相只有一个");
    // Vite 原版无 score；规范与旧数值 API 保持既有的零分映射。
    assert_eq!(truth["score"], 0);
    let (_, after_truth) = event(frame, "凶手就是你");
    assert_eq!(after_truth["delay"], 2000);
    let (revive, clip) = event(frame, "[0]满血复活");
    assert_eq!(revive["delay0"], 3000);
    assert_eq!(clip["delay"], 3000);
    assert!(revive["target_id"].is_null());
    verify_clip_hp(clip, &revive["caster_id"], 0, 386, true, false);
    let (life, _) = event(frame, "[0]还剩[2]条命");
    assert!(life["target_id"].is_null());
    assert_eq!(life["param"], 2);
}

#[test]
fn thanos_public_snap_does_not_attribute_linked_minion_exit_to_owner() {
    let frames = frames("thanos@!\n\nfighter25");
    let frame = &frames[1];
    let (ash, _) = event(frame, "[1]化成了灰烬");
    assert_eq!(ash["score"], 318);
    assert_eq!(state(frame, &ash["target_id"])["id_key_name"], "fighter25");
    let (exit, clip) = event(frame, "[1]消失了");
    let id = &exit["target_id"];
    assert_eq!(state(frame, id)["id_key_name"], "fighter25?0@fighter25");
    assert!(exit["caster_id"].is_null());
    assert_eq!(exit["score"], 50);
    assert_eq!(clip["caster_ids"], serde_json::json!([]));
    assert_eq!(clip["target_ids"], serde_json::json!([id]));
    assert!(clip["parts"][0]["death_effect"].as_bool().unwrap());
}

#[test]
fn boss_group_skill_start_reports_ordered_animation_targets() {
    for (raw, index, message) in [
        ("aokiji@!\n\na\nb", 0, "[0]使用[冰河时代]"),
        ("ikaruga@!\n\na\nb", 3, "[0]使用[能量释放]"),
    ] {
        let frames = frames(raw);
        let frame = &frames[index];
        let (update, clip) = event(frame, message);
        assert!(update["target_id"].is_null());
        assert_eq!(update["target_ids"], serde_json::json!([]), "动画目标不替换占位符 [2]");
        let ids = update["animation_target_ids"].as_array().unwrap();
        assert_eq!(
            ids.iter().map(|id| state(frame, id)["id_key_name"].as_str().unwrap()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(clip["target_ids"], update["animation_target_ids"]);
        assert_eq!(update["delay0"], 1000);
        assert_eq!(update["delay1"], 100);
    }
}

#[test]
fn boss_self_announcements_do_not_claim_a_target() {
    let mario = frames("mario@!\n\na\nb");
    for (index, messages) in [
        (11, ["[0]得到[蘑菇]", "[0]攻击力上升"]),
        (13, ["[0]得到[火焰花]", "[0]学会[火球术]"]),
    ] {
        for message in messages {
            let (update, clip) = event(&mario[index], message);
            assert_eq!(state(&mario[index], &update["caster_id"])["id_key_name"], "mario@!");
            assert!(update["target_id"].is_null(), "{message}");
            assert_eq!(clip["target_ids"], serde_json::json!([]));
        }
    }
    for (raw, index, message, name, score) in [
        ("ikaruga@!\n\na\nb", 2, "[0][吸收]所有奇数伤害", "ikaruga@!", 35),
        ("yuri@!\n\naokiji@!", 1, "[0][吸收]所有冰冻伤害", "aokiji@!", 195),
    ] {
        let fight = frames(raw);
        let frame = &fight[index];
        let (absorb, clip) = event(frame, message);
        assert_eq!(state(frame, &absorb["caster_id"])["id_key_name"], name);
        assert!(absorb["target_id"].is_null());
        assert_eq!(absorb["score"], score);
        assert_eq!(clip["caster_ids"], serde_json::json!([absorb["caster_id"]]));
        assert_eq!(clip["target_ids"], serde_json::json!([]));
    }
}

#[test]
fn wheel_hp_exchange_exposes_inline_emoji_without_html() {
    let frames = frames("wheel@!\n\nslime@!");
    let (update, clip) = event(&frames[19], "[1]的体力值与[0]互换");
    assert!(!update["message_rendered"].as_str().unwrap().contains('<'));
    let part = clip["parts"].as_array().unwrap().iter().find(|part| part["text"] == "互换").unwrap();
    assert_eq!(part["emoji"], "s_exchange");
}
