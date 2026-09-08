use super::*;

#[test]
fn summon_explode_defers_self_round_removal_until_after_target_death() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.kind.test-summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "anchor", 0, 10, 1),
            PlayerTemplate::new(2, "victim", 1, 3, 3).with_def_res(0, 0),
            PlayerTemplate::new(3, "middle", 1, 10, 1),
            PlayerTemplate::new(4, "tail", 1, 10, 1),
            PlayerTemplate::with_kind(5, "summon", summon_kind, 0, 5, 1).with_magic(80),
        ],
        builder.build(),
    ));
    let summon = EntityIdx(4);
    let victim = EntityIdx(1);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(0)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(victim));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(2)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(3)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(summon));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: summon,
        target: victim,
        fire_state_key: 91,
    });

    let frame = runtime
        .flush_effects()
        .expect("summon explode should emit lethal target and self-death updates");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::runtime::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[0]使用[自爆]", "[1]受到[2]点伤害", "[1]被击倒了", "[1]消失了"]
    );
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(2), EntityIdx(3)]);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(3)));
}
