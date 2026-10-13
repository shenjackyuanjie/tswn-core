use super::{
    ReplayClip, ReplayEventView, ReplayState, ReplayTextPart, ReplayTextPartKind, ReplayTone, STATUS_EXIT_TEXT_COLOR,
    build_replay_view_frame,
};
use crate::runtime::PlrId;
use crate::runtime::update::RunUpdate;
use std::collections::HashMap;

#[derive(Clone)]
struct TestState {
    id: PlrId,
    hp: i32,
    max_hp: i32,
    shield: i32,
    alive: bool,
}

impl ReplayState for TestState {
    fn id(&self) -> PlrId { self.id }

    fn hp(&self) -> i32 { self.hp }

    fn max_hp(&self) -> i32 { self.max_hp }

    fn alive(&self) -> bool { self.alive }

    fn shield(&self) -> i32 { self.shield }

    fn with_shield(&self, shield: i32) -> Self {
        Self {
            shield: if self.alive { shield } else { 0 },
            ..self.clone()
        }
    }

    fn with_hp_alive(&self, hp: i32, alive: bool) -> Self {
        Self {
            hp,
            alive,
            shield: if alive { self.shield } else { 0 },
            ..self.clone()
        }
    }
}

fn names() -> HashMap<PlrId, String> { [(0, "caster".to_string()), (1, "target".to_string())].into() }

fn state(id: PlrId, hp: i32) -> TestState {
    TestState {
        id,
        hp,
        max_hp: 100,
        shield: 0,
        alive: hp > 0,
    }
}

fn player_part(clip: &ReplayClip<TestState>) -> &ReplayTextPart {
    clip.parts.iter().find(|part| part.kind == ReplayTextPartKind::Player).unwrap()
}

fn player_parts(clip: &ReplayClip<TestState>) -> Vec<&ReplayTextPart> {
    clip.parts.iter().filter(|part| part.kind == ReplayTextPartKind::Player).collect()
}

#[test]
fn shield_changes_only_advance_on_the_first_visible_player_clip() {
    let updates = [
        RunUpdate::new("[0]发起攻击", 0, 1, 0),
        RunUpdate::new("[1]受到[2]点伤害", 0, 1, 0),
        RunUpdate::new("[1]挡住攻击", 0, 1, 0),
        RunUpdate::new("[0]还剩[2]点血", 0, 1, 100),
    ];
    let events = updates
        .iter()
        .enumerate()
        .map(|(index, update)| ReplayEventView {
            update,
            tone: if index == 1 { ReplayTone::Damage } else { ReplayTone::Normal },
            message_rendered: "可见消息",
        })
        .collect::<Vec<_>>();
    let previous = vec![state(0, 100), state(1, 100).with_shield(40)];
    let frame = vec![state(0, 100).with_shield(10), state(1, 100).with_shield(25)];
    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clips = &view.rows[0].clips;
    let pairs = clips
        .iter()
        .map(|clip| {
            clip.sidebar_previous_states
                .iter()
                .zip(&clip.sidebar_states)
                .map(|(before, after)| (before.shield, after.shield))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pairs,
        vec![
            vec![(0, 10), (40, 40)],
            vec![(10, 10), (40, 25)],
            vec![(10, 10), (25, 25)],
            vec![(10, 10), (25, 25)],
        ]
    );
    // 零体力伤害仍可消耗护盾，血条行为保持原样。
    assert!(!player_part(&clips[1]).show_hp);
    assert_eq!((player_part(&clips[1]).hp_before, player_part(&clips[1]).hp_after), (100, 100));
}

#[test]
fn newly_appeared_participant_shield_waits_for_its_visible_player_part() {
    let updates = [RunUpdate::new("[0]召唤目标", 0, 1, 0), RunUpdate::new("[1]开始行动", 1, 1, 0)];
    let events = updates
        .iter()
        .map(|update| ReplayEventView {
            update,
            tone: ReplayTone::Normal,
            message_rendered: "可见消息",
        })
        .collect::<Vec<_>>();
    let previous = vec![state(0, 100)];
    let frame = vec![state(0, 100), state(1, 100).with_shield(20)];
    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clips = &view.rows[0].clips;
    assert_eq!(clips[0].sidebar_states[1].shield, 0);
    assert_eq!(clips[1].sidebar_previous_states[1].shield, 0);
    assert_eq!(clips[1].sidebar_states[1].shield, 20);
}

#[test]
fn lethal_clip_clears_shield_and_following_death_clip_does_not_repeat_it() {
    let damage = RunUpdate::new("[1]受到[2]点伤害", 0, 1, 50);
    let death = RunUpdate::new("[1]被击倒了", 0, 1, 0);
    let events = [
        ReplayEventView {
            update: &damage,
            tone: ReplayTone::Damage,
            message_rendered: "伤害",
        },
        ReplayEventView {
            update: &death,
            tone: ReplayTone::Knockout,
            message_rendered: "击倒",
        },
    ];
    let previous = vec![state(0, 100), state(1, 50).with_shield(20)];
    let frame = vec![state(0, 100), state(1, 0)];
    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clips = &view.rows[0].clips;
    assert_eq!(clips[0].sidebar_previous_states[1].shield, 20);
    assert_eq!(clips[0].sidebar_states[1].shield, 0);
    assert!(!clips[0].sidebar_states[1].alive);
    assert_eq!(clips[1].sidebar_previous_states[1].shield, 0);
    assert_eq!(clips[1].sidebar_states[1].shield, 0);
    assert!(player_part(&clips[1]).death_effect);
    assert!(!player_part(&clips[1]).show_hp);
}

#[test]
fn hp_bar_only_shows_when_hp_changes() {
    let update = RunUpdate::new("[1]受到[2]点伤害", 0, 1, 30);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Damage,
        message_rendered: "target受到30点伤害",
    }];
    let previous = vec![state(0, 100), state(1, 50)];
    let frame = vec![state(0, 100), state(1, 20)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (50, 20));
    assert!(!player_part.death_effect);
}

#[test]
fn damage_to_zero_shows_hp_without_death_effect() {
    let update = RunUpdate::new("[1] took [2] damage", 0, 1, 50);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Damage,
        message_rendered: "target took 50 damage",
    }];
    let previous = vec![state(0, 100), state(1, 50)];
    let frame = vec![state(0, 100), state(1, 0)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (50, 0));
    assert!(!player_part.death_effect);
}

#[test]
fn percent_hp_damage_uses_actual_score_for_hp_bar() {
    let mut update = RunUpdate::new("[1]体力减少[2]%", 0, 1, 9);
    update.param = Some(47);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Damage,
        message_rendered: "target体力减少47%",
    }];
    let previous = vec![state(0, 100), state(1, 20)];
    let frame = vec![state(0, 100), state(1, 11)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (20, 11));
    assert!(clip.parts.iter().any(|part| part.kind == ReplayTextPartKind::Data && part.text == "47"));
}

#[test]
fn status_exit_clip_uses_status_exit_highlight_color() {
    let update = RunUpdate::new("[1]从[狂暴]中解除", 0, 1, 0);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::StatusExit,
        message_rendered: "target从狂暴中解除",
    }];
    let previous = vec![state(0, 100), state(1, 50)];
    let frame = vec![state(0, 100), state(1, 50)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    assert_eq!(clip.color, STATUS_EXIT_TEXT_COLOR);
    assert_eq!(clip.tone, ReplayTone::StatusExit);
    assert!(
        clip.parts
            .iter()
            .any(|part| { part.kind == ReplayTextPartKind::Highlight && part.text == "狂暴" })
    );
}

#[test]
fn new_entity_uses_current_hp_as_before_hp() {
    let update = RunUpdate::new("[1]出现了", 0, 2, 0);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Normal,
        message_rendered: "summon出现了",
    }];
    let previous = vec![state(0, 100)];
    let frame = vec![state(0, 100), state(2, 60)];
    let mut names = names();
    names.insert(2, "summon".to_string());

    let view = build_replay_view_frame(&events, &previous, &frame, &names, false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (60, 60));
    assert!(!player_part.death_effect);
}

#[test]
fn hp_swap_shows_hp_for_both_players() {
    let update = RunUpdate::new("[1]\u{7684}\u{4f53}\u{529b}\u{503c}\u{4e0e}[0]\u{4e92}\u{6362}", 0, 1, 100);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Normal,
        message_rendered: "target\u{7684}\u{4f53}\u{529b}\u{503c}\u{4e0e}caster\u{4e92}\u{6362}",
    }];
    let previous = vec![state(0, 30), state(1, 80)];
    let frame = vec![state(0, 80), state(1, 30)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];
    let parts = player_parts(clip);

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].player_id, Some(1));
    assert!(parts[0].show_hp);
    assert_eq!((parts[0].hp_before, parts[0].hp_after), (80, 30));
    assert_eq!(parts[1].player_id, Some(0));
    assert!(parts[1].show_hp);
    assert_eq!((parts[1].hp_before, parts[1].hp_after), (30, 80));
}

#[test]
fn hp_swap_forces_hp_for_unchanged_players() {
    let update = RunUpdate::new("[1]\u{7684}\u{4f53}\u{529b}\u{503c}\u{4e0e}[0]\u{4e92}\u{6362}", 0, 1, 100);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Normal,
        message_rendered: "target\u{7684}\u{4f53}\u{529b}\u{503c}\u{4e0e}caster\u{4e92}\u{6362}",
    }];
    let previous = vec![state(0, 50), state(1, 50)];
    let frame = vec![state(0, 50), state(1, 50)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];
    let parts = player_parts(clip);

    assert_eq!(parts.len(), 2);
    assert!(parts.iter().all(|part| part.show_hp));
    assert!(parts.iter().all(|part| (part.hp_before, part.hp_after) == (50, 50)));
}

#[test]
fn death_effect_only_renders_when_hp_stays_zero() {
    let update = RunUpdate::new("[1]被击倒", 0, 1, 50);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Knockout,
        message_rendered: "target被击倒",
    }];
    let previous = vec![state(0, 100), state(1, 0)];
    let frame = vec![state(0, 100), state(1, 0)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(!player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (0, 0));
    assert!(player_part.death_effect);
}

#[test]
fn death_effect_renders_for_disappear_sentence() {
    let update = RunUpdate::new("[1]\u{6d88}\u{5931}\u{4e86}", 0, 1, 50);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Knockout,
        message_rendered: "target\u{6d88}\u{5931}\u{4e86}",
    }];
    let previous = vec![state(0, 100), state(1, 0)];
    let frame = vec![state(0, 100), state(1, 0)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(!player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (0, 0));
    assert!(player_part.death_effect);
}

#[test]
fn hp_marker_report_forces_hp_bar_without_hp_delta() {
    let mut update = RunUpdate::new("[0]还剩[2]点血", 0, 0, 0);
    update.param = Some(87);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Normal,
        message_rendered: "caster还剩87点血",
    }];
    let previous = vec![state(0, 87), state(1, 100)];
    let frame = vec![state(0, 87), state(1, 100)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = clip.parts.iter().find(|part| part.kind == ReplayTextPartKind::Player).unwrap();
    assert!(player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (87, 87));
    let data_part = clip.parts.iter().find(|part| part.kind == ReplayTextPartKind::Data).unwrap();
    assert_eq!(data_part.text, "87");
}

#[test]
fn mechanism_death_sets_hp_to_zero_and_renders_death_effect() {
    let update = RunUpdate::new("[1]\u{6d88}\u{5931}\u{4e86}", 0, 1, 50);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Knockout,
        message_rendered: "target\u{6d88}\u{5931}\u{4e86}",
    }];
    let previous = vec![state(0, 100), state(1, 40)];
    let frame = vec![state(0, 100), state(1, 0)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let clip = &view.rows[0].clips[0];

    let player_part = player_part(clip);
    assert!(!player_part.show_hp);
    assert_eq!((player_part.hp_before, player_part.hp_after), (40, 0));
    assert!(player_part.death_effect);
}

#[test]
fn reraise_knockout_does_not_show_hp_until_recover() {
    let knockout = RunUpdate::new("[1]被击倒了", 0, 1, 50);
    let reraise = RunUpdate::new("[0]使用[护身符]抵挡了一次死亡", 1, 1, 80);
    let mut recover = RunUpdate::new("[1]回复体力[2]点", 1, 1, 0);
    recover.param = Some(8);
    let events = [
        ReplayEventView {
            update: &knockout,
            tone: ReplayTone::Knockout,
            message_rendered: "target被击倒了",
        },
        ReplayEventView {
            update: &reraise,
            tone: ReplayTone::Normal,
            message_rendered: "target使用护身符抵挡了一次死亡",
        },
        ReplayEventView {
            update: &recover,
            tone: ReplayTone::Recover,
            message_rendered: "target回复体力8点",
        },
    ];
    let previous = vec![state(0, 100), state(1, 0)];
    let frame = vec![state(0, 100), state(1, 8)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let knockout_clip = &view.rows[0].clips[0];
    let reraise_clip = &view.rows[0].clips[1];
    let recover_clip = &view.rows[0].clips[2];

    let knockout_part = player_part(knockout_clip);
    assert!(!knockout_part.show_hp);
    assert_eq!((knockout_part.hp_before, knockout_part.hp_after), (0, 0));
    assert!(knockout_part.death_effect);
    let reraise_part = player_part(reraise_clip);
    assert!(!reraise_part.show_hp);
    assert_eq!((reraise_part.hp_before, reraise_part.hp_after), (0, 0));
    assert!(!reraise_part.death_effect);
    let recover_part = player_part(recover_clip);
    assert!(recover_part.show_hp);
    assert_eq!((recover_part.hp_before, recover_part.hp_after), (0, 8));
    assert!(!recover_part.death_effect);
}

#[test]
fn revive_then_recover_starts_from_zero_hp() {
    let revive = RunUpdate::new("[1][复活]了", 0, 1, 0);
    let mut recover = RunUpdate::new("[1]回复体力[2]点", 0, 1, 0);
    recover.param = Some(40);
    let events = [
        ReplayEventView {
            update: &revive,
            tone: ReplayTone::Normal,
            message_rendered: "target复活了",
        },
        ReplayEventView {
            update: &recover,
            tone: ReplayTone::Recover,
            message_rendered: "target回复体力40点",
        },
    ];
    // Runtime 帧快照仍可能保留上一个动作的 HP。
    // 复活更新必须在其恢复更新前重置回放状态。
    let previous = vec![state(0, 100), state(1, 66)];
    let frame = vec![state(0, 100), state(1, 40)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);
    let revive_part = player_part(&view.rows[0].clips[0]);
    let recover_part = player_part(&view.rows[0].clips[1]);

    assert!(!revive_part.show_hp);
    assert_eq!((revive_part.hp_before, revive_part.hp_after), (0, 0));
    assert!(recover_part.show_hp);
    assert_eq!((recover_part.hp_before, recover_part.hp_after), (0, 40));
}

#[test]
fn zombie_conversion_renders_corpse_as_player_part() {
    let mut update = RunUpdate::new("[2]变成了[1]", 0, 2, 0);
    update.targets.push(1);
    let events = [ReplayEventView {
        update: &update,
        tone: ReplayTone::Normal,
        message_rendered: "target变成了zombie",
    }];
    let previous = vec![state(0, 100), state(1, 0)];
    let frame = vec![state(0, 100), state(1, 0), state(2, 60)];
    let mut names = names();
    names.insert(2, "zombie".to_string());

    let view = build_replay_view_frame(&events, &previous, &frame, &names, false, &[]);
    let parts = player_parts(&view.rows[0].clips[0]);

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].player_id, Some(1));
    assert_eq!(parts[0].text, "target");
    assert!(!parts[0].show_hp);
    assert!(!parts[0].death_effect);
    assert_eq!(parts[1].player_id, Some(2));
    assert_eq!(parts[1].text, "zombie");
    assert!(parts[1].show_hp);
    assert_eq!((parts[1].hp_before, parts[1].hp_after), (60, 60));
}

#[test]
fn clip_delay_uses_priority_rules() {
    let attack = RunUpdate::new("[1]受到[2]点伤害", 0, 1, 30);
    let thunder = RunUpdate::new("[0]使用[雷击术]", 0, 1, 1);
    let thunder_damage = RunUpdate::new("[1]受到[2]点伤害", 0, 1, 20);
    let normal = RunUpdate::new("[0]发起攻击", 0, 1, 0);
    let newline = RunUpdate::new_newline();
    let events = [
        ReplayEventView {
            update: &attack,
            tone: ReplayTone::Damage,
            message_rendered: "target受到30点伤害",
        },
        ReplayEventView {
            update: &thunder,
            tone: ReplayTone::Normal,
            message_rendered: "caster使用雷击术",
        },
        ReplayEventView {
            update: &newline,
            tone: ReplayTone::Normal,
            message_rendered: "",
        },
        ReplayEventView {
            update: &thunder_damage,
            tone: ReplayTone::Damage,
            message_rendered: "target受到20点伤害",
        },
        ReplayEventView {
            update: &normal,
            tone: ReplayTone::Normal,
            message_rendered: "caster发起攻击",
        },
    ];
    let previous = vec![state(0, 100), state(1, 80)];
    let frame = vec![state(0, 100), state(1, 30)];

    let view = build_replay_view_frame(&events, &previous, &frame, &names(), false, &[]);

    assert_eq!(view.rows[0].clips[0].delay, 900);
    assert_eq!(view.rows[0].clips[1].delay, 500);
    assert_eq!(view.rows[1].clips[0].delay, 150);
    assert_eq!(view.rows[1].clips[1].delay, 500);
    assert_eq!(view.total_delay, 2050);
}
