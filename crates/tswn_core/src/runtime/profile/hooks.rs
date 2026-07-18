use super::*;

pub fn run_defend_post_defend_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let damage = context.defend_damage().expect("runtime defend skill must run during POST_DEFEND");
    let level = context.skill_level(entry);
    if context.rng_r255() >= level {
        return;
    }
    if !context.owner_mp_ready().expect("runtime defend skill owner must exist") {
        return;
    }
    let caster = context.defend_caster().expect("runtime defend skill must receive incoming caster");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0][防御]",
        context.owner_idx().0 as usize,
        caster.0 as usize,
        40,
    ));
    context.set_defend_damage(damage / 2);
}

pub fn run_reflect_pre_defend_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let atp = context.defend_atp().expect("runtime reflect skill must run during PRE_DEFEND");
    if !context.defend_caster_active().expect("runtime reflect incoming caster must exist") {
        return;
    }

    let level = context.skill_level(entry);
    if context.rng_r255() >= level
        || !context.rng_c50()
        || !context.owner_mp_ready().expect("runtime reflect skill owner must exist")
    {
        return;
    }

    let caster = context.defend_caster().expect("runtime reflect skill must receive incoming caster");
    let reflect_atp = (context.owner_attack_power(true).expect("runtime reflect skill owner must exist") * 0.5).min(atp);
    let mut update =
        crate::runtime::update::RunUpdate::new("[0]使用[伤害反弹]", context.owner_idx().0 as usize, caster.0 as usize, 20);
    update.delay0 = 1500;
    context.add_update(update);
    context.set_defend_atp(0.0);
    context.push_nested(QueuedEffect::ReflectedAttack {
        caster: context.owner_idx(),
        target: caster,
        atp_bits: reflect_atp.to_bits(),
        on_damage: context.defend_on_damage(),
    });
}

pub fn run_shield_pre_action_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    let shield = context.owner_shield().expect("runtime shield skill owner must exist");
    if (level as i32) < shield {
        return;
    }
    let max = (1 + (level as i32 * 3 / 4)).max(1);
    let add = context.rng_next_i32(max) + 1;
    context.set_owner_shield(shield + add).expect("runtime shield skill owner must exist");
}

pub fn run_protect_post_action_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    context
        .refresh_owner_protect_target(level)
        .expect("runtime protect post-action context must access allies");
}

pub fn run_plain_passive_noop_skill(_: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {}

pub fn run_merge_kill_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    let roll = context.rng_r63();
    if roll >= level {
        return;
    }
    let Some(target) = context.selected_target() else {
        return;
    };
    context.push_nested(QueuedEffect::Merge {
        caster: context.owner_idx(),
        target,
    });
}

pub fn run_reraise_die_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    if context.rng_r127() >= level {
        return;
    }
    let hp = context.rng_r16() as i32;
    context.reraise_owner(entry, hp).expect("runtime reraise owner must exist");
    let mut reraise_update = crate::runtime::update::RunUpdate::new(
        "[0]使用[护身符]抵挡了一次死亡",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        80,
    );
    reraise_update.delay0 = 1500;
    context.add_update(reraise_update);
    let mut recover_update = crate::runtime::update::RunUpdate::new(
        "[1]回复体力[2]点",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    );
    recover_update.param = Some(hp as u32);
    context.add_update(recover_update);
}
