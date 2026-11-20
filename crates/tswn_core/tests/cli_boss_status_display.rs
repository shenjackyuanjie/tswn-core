//! 用真实公开回放验证 Boss 局的原版状态文案参数与普通局的旧投影。

use tswn_core::cli_api::{BattleOptions, battle_replay};

#[test]
fn accumulate_uses_target_placeholder_only_in_boss_battles() {
    for (raw, expected) in [
        ("slime@!\n\nAlice", "[1]攻击力上升"),
        ("notaboss@!\n\nAlice\nfighter1", "[0]攻击力上升"),
    ] {
        let replay = battle_replay(raw, BattleOptions::default()).unwrap();
        let alice = replay.initial_states.iter().find(|p| p.id_key_name == "Alice").unwrap().id;
        let frame = replay
            .frames
            .iter()
            .find(|frame| frame.updates.iter().any(|update| update.message_template == "[0]开始[聚气]"))
            .unwrap();
        let pair = frame.updates.windows(2).find(|pair| pair[0].message_template == "[0]开始[聚气]").unwrap();
        assert_eq!(pair[1].message_template, expected);
        assert_eq!(pair[1].caster_id, Some(alice));
        assert_eq!(pair[1].target_id, Some(alice));
        assert_eq!(pair[1].message_rendered, "Alice攻击力上升");
        assert_eq!((pair[1].delay0, pair[1].delay1), (1000, 100));
    }
}

#[test]
fn iron_expiry_keeps_original_boss_template_and_event_delay_without_rewriting_mario_growth() {
    for (raw, expected, delay1) in [
        ("mario@!\n\nparity17a\nparity17b\n\nparity17c", "[0]从[铁壁]中解除", 500),
        ("parity17a\nparity17b\n\nparity17c", "[1]从[铁壁]中解除", 100),
    ] {
        let replay = battle_replay(raw, BattleOptions::default()).unwrap();
        let owner = replay.initial_states.iter().find(|p| p.id_key_name == "parity17c").unwrap().id;
        let (frame, expiry) = replay
            .frames
            .iter()
            .find_map(|frame| {
                frame
                    .updates
                    .iter()
                    .find(|update| update.message_template.ends_with("从[铁壁]中解除"))
                    .map(|update| (frame, update))
            })
            .unwrap();
        assert_eq!(expiry.message_template, expected);
        assert_eq!(expiry.message_rendered, "parity17c从[铁壁]中解除");
        assert_eq!(expiry.caster_id, Some(owner));
        assert_eq!(expiry.target_id, Some(owner));
        assert_eq!((expiry.delay0, expiry.delay1), (1000, delay1));
        let clip = frame
            .rows
            .iter()
            .flat_map(|row| &row.clips)
            .find(|clip| clip.tone == "status_exit")
            .unwrap();
        assert_eq!(clip.parts[0].player_id, Some(owner));
        assert_eq!(clip.parts[1].text, "从");
        assert_eq!(clip.parts[2].text, "铁壁");

        if let Some(mario) = replay.initial_states.iter().find(|p| p.id_key_name == "mario@!") {
            let growth = replay
                .frames
                .iter()
                .flat_map(|frame| &frame.updates)
                .find(|update| update.caster_id == Some(mario.id) && update.message_rendered.ends_with("攻击力上升"))
                .unwrap();
            assert_eq!(growth.message_template, "[0]攻击力上升");
            assert_eq!(growth.target_id, None);
        }
    }
}
