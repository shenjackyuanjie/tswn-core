use super::*;

#[test]
fn runtime_dispatches_replay_renderers_in_registry_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let late = builder
        .register_replay_renderer("custom", "late", "custom.late_replay", SkillPriority(10))
        .expect("late replay renderer should register");
    let early = builder
        .register_replay_renderer("custom", "early", "custom.early_replay", SkillPriority(1))
        .expect("early replay renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_replay_renderer(late, render_update_count_replay);
    runtime.set_replay_renderer(early, render_first_message_replay);
    let frame = RuntimeFrame::single_damage(0, 0, 3);

    let rendered = runtime.render_replay_frame(&frame);

    assert_eq!(
        rendered,
        vec![
            RenderedReplay::new(ReplayRendererId(0), "[0]攻击[1]"),
            RenderedReplay::new(ReplayRendererId(1), "1")
        ]
    );
}

#[test]
fn runtime_dispatches_show_renderers_in_registry_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let show = builder
        .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
        .expect("show renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_show_renderer(show, render_first_message_show);
    let frame = RuntimeFrame::single_damage(0, 0, 3);

    let rendered = runtime.render_show_frame(&frame);

    assert_eq!(rendered, vec![RenderedShow::new(ShowRendererId(0), "[0]攻击[1]")]);
}

#[test]
fn runtime_dispatches_hp_marker_show_renderer_golden() {
    let mut builder = ExtensionRegistryBuilder::default();
    let show = builder
        .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(0))
        .expect("hp marker show renderer should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_show_renderer(show, render_hp_marker_bar_show);
    let mut updates = crate::runtime::update::RunUpdates::new();
    let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
    hp_report.param = Some(87);
    updates.add(hp_report);
    let frame = RuntimeFrame { updates };

    let rendered = runtime.render_show_frame(&frame);

    assert_eq!(
        rendered,
        vec![RenderedShow::new(ShowRendererId(0), "hp-bar:actor=0:value=87:text=0还剩87点血")]
    );
}

#[test]
fn runtime_frame_renders_core_replay_and_show_golden() {
    let mut frame = RuntimeFrame::single_damage(0, 1, 3);
    frame.updates.add(RuntimeFrame::replay_update(0, 1, "[0]属性上升", 0));

    assert_eq!(
        frame.render_core_replay(),
        vec![
            CoreReplayEvent {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 3,
            },
            CoreReplayEvent {
                message: "[0]属性上升".to_owned(),
                caster: 0,
                target: 1,
                targets: Vec::new(),
                param: None,
                score: 0,
            },
        ]
    );
    assert_eq!(
        frame.render_core_show(),
        vec![
            CoreShowEvent {
                text: "0攻击1".to_owned(),
                score: 3,
            },
            CoreShowEvent {
                text: "0属性上升".to_owned(),
                score: 0,
            },
        ]
    );
}

#[test]
fn runtime_frame_renders_hp_marker_core_show_golden() {
    let mut updates = crate::runtime::update::RunUpdates::new();
    let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
    hp_report.param = Some(87);
    updates.add(hp_report);
    let frame = RuntimeFrame { updates };

    assert_eq!(
        frame.render_core_replay(),
        vec![CoreReplayEvent {
            message: "[0]还剩[2]点血".to_owned(),
            caster: 0,
            target: 0,
            targets: Vec::new(),
            param: Some(87),
            score: 0,
        }]
    );
    assert_eq!(
        frame.render_core_show(),
        vec![CoreShowEvent {
            text: "0还剩87点血".to_owned(),
            score: 0,
        }]
    );
}
