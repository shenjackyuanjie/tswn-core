//! 数值槽登记、presence 投影与类别映射。
//!
//! [`required_normalization`] 列出构造编码器所需的逐字段变换声明；实体与模板投影
//! 按固定槽序提取 f64 原值，尚不执行归一化。可空字段用独立 presence 区分缺失与真实零，
//! 固定枚举映射则保留 0 给 padding，有效类别从 1 起。

use super::*;

/// `entity_num` 的 24 个槽位：`(槽位, 校准字段路径)`；顺序即规格第 3 节的字段顺序。
pub const ENTITY_NUM_SLOTS: [(usize, &str); 24] = [
    (0, "entity.runtime.hp"),
    (1, "entity.runtime.attack"),
    (2, "entity.runtime.magic"),
    (3, "entity.runtime.magic_point"),
    (4, "entity.runtime.wisdom"),
    (5, "entity.runtime.speed"),
    (6, "entity.runtime.defense"),
    (7, "entity.runtime.resistance"),
    (8, "entity.runtime.agility"),
    (9, "entity.runtime.at_boost"),
    (10, "entity.runtime.attr_sum"),
    (11, "entity.runtime.atk_sum"),
    (12, "entity.runtime.attract"),
    (13, "entity.runtime.shield"),
    (14, "entity.runtime.protect_pre_defend_skill_count"),
    (15, "entity.runtime.move_state.speed_points"),
    (16, "entity.runtime.charge.step"),
    (17, "entity.runtime.accumulate.acc"),
    (18, "entity.runtime.accumulate.charge_bonus"),
    (19, "entity.runtime.hide.level"),
    (20, "entity.runtime.hide.attract"),
    (21, "entity.runtime.hide.agility"),
    (22, "entity.runtime.hide.defense"),
    (23, "entity.runtime.hide.resistance"),
];

/// `template_num` 的 31 个槽位；`[17..30]` 由 `clone_build` 整体 presence 控制。
pub const TEMPLATE_NUM_SLOTS: [(usize, &str); 31] = [
    (0, "template.max_hp"),
    (1, "template.attack"),
    (2, "template.magic"),
    (3, "template.magic_point"),
    (4, "template.wisdom"),
    (5, "template.speed"),
    (6, "template.defense"),
    (7, "template.resistance"),
    (8, "template.agility"),
    (9, "template.at_boost"),
    (10, "template.attr_sum"),
    (11, "template.atk_sum"),
    (12, "template.attract"),
    (13, "template.reserved_player_ids_before_spawn"),
    (14, "template.identity.boss_action_prob_count"),
    (15, "template.identity.boost_immune_threshold"),
    (16, "template.move_state.speed_points"),
    (17, "template.clone_build.name_factor"),
    (18, "template.clone_build.child_name_factor"),
    (19, "template.clone_build.adjustments.max_hp"),
    (20, "template.clone_build.adjustments.attack"),
    (21, "template.clone_build.adjustments.magic"),
    (22, "template.clone_build.adjustments.wisdom"),
    (23, "template.clone_build.adjustments.speed"),
    (24, "template.clone_build.adjustments.defense"),
    (25, "template.clone_build.adjustments.resistance"),
    (26, "template.clone_build.adjustments.agility"),
    (27, "template.clone_build.adjustments.at_boost_delta"),
    (28, "template.clone_build.adjustments.attr_sum"),
    (29, "template.clone_build.adjustments.atk_sum"),
    (30, "template.clone_build.adjustments.attract_delta"),
];

/// `immunity_num` 的九个固定 status 轴（与 model_state 投影顺序一致）。
pub const IMMUNITY_AXIS: [&str; 9] = [
    "assassinate",
    "charm",
    "berserk",
    "half",
    "curse",
    "exchange",
    "slow",
    "ice",
    "fire",
];

/// 编码器消费的全部数值槽：`(校准路径, 变换类型)`。manifest 必须逐项声明且类型一致，
/// 缺一即 [`EncodeError::MissingCalibration`]，不允许用 0/1 顶替（规格第 5 节）。
pub fn required_normalization() -> Vec<(&'static str, TransformKind)> {
    let mut required: Vec<(&'static str, TransformKind)> = FIXED_COUNT_SLOTS
        .iter()
        .map(|(path, divisor)| (*path, TransformKind::FixedCount { divisor: *divisor }))
        .collect();
    required.push(("global.round", TransformKind::Fitted));
    required.push(("global.world.alive_group_count", TransformKind::Fitted));
    required.push(("global.world.round_pos", TransformKind::Fitted));
    required.extend(ENTITY_NUM_SLOTS.iter().map(|(_, path)| (*path, TransformKind::Fitted)));
    required.push(("entity.runtime.protect_from.level", TransformKind::Fitted));
    required.extend(TEMPLATE_NUM_SLOTS.iter().map(|(_, path)| (*path, TransformKind::Fitted)));
    required.push(("template.identity.immunity.threshold", TransformKind::Fitted));
    required.push(("state.priority", TransformKind::Fitted));
    required.extend([
        ("state.payload.fire_mag_half_steps", TransformKind::Fitted),
        ("state.payload.ice.frozen_step", TransformKind::Fitted),
        ("state.payload.shield_value", TransformKind::Fitted),
        ("state.payload.curse.prob", TransformKind::Fitted),
        ("state.payload.curse.multiply", TransformKind::Fitted),
        ("state.payload.poison.atp", TransformKind::Fitted),
        ("state.payload.poison.count", TransformKind::Fitted),
        ("state.payload.haste.faster", TransformKind::Fitted),
        ("state.payload.haste.effective_faster", TransformKind::Fitted),
        ("state.payload.haste.step", TransformKind::Fitted),
        ("state.payload.berserk.step", TransformKind::Fitted),
        ("state.payload.charm.step", TransformKind::Fitted),
        ("state.payload.slow.step", TransformKind::Fitted),
        ("state.payload.iron.protect", TransformKind::Fitted),
        ("state.payload.iron.step", TransformKind::Fitted),
    ]);
    required.push(("slot.minion_counter", TransformKind::Fitted));
    required.extend([
        (
            "template.clone_build.score_skill_boost_plan.slot_boosts.0.0",
            TransformKind::Fitted,
        ),
        (
            "template.clone_build.score_skill_boost_plan.slot_boosts.0.1",
            TransformKind::Fitted,
        ),
        (
            "template.clone_build.score_skill_boost_plan.slot_boosts.1.0",
            TransformKind::Fitted,
        ),
        (
            "template.clone_build.score_skill_boost_plan.slot_boosts.1.1",
            TransformKind::Fitted,
        ),
    ]);
    required.extend([
        ("lane.level", TransformKind::Fitted),
        ("lane.build_level", TransformKind::Fitted),
        ("lane.boost.base", TransformKind::Fitted),
        ("lane.boost.extra", TransformKind::Fitted),
    ]);
    required
}

/// `entity_num` 原始值；absent Option 的槽位填 0（presence 另行记录）。
pub(super) fn entity_num_values(runtime: &ModelPlayerRuntime) -> [f64; 24] {
    let mut values = [0.0f64; 24];
    values[0] = f64::from(runtime.hp);
    values[1] = f64::from(runtime.attack);
    values[2] = f64::from(runtime.magic);
    values[3] = f64::from(runtime.magic_point);
    values[4] = f64::from(runtime.wisdom);
    values[5] = f64::from(runtime.speed);
    values[6] = f64::from(runtime.defense);
    values[7] = f64::from(runtime.resistance);
    values[8] = f64::from(runtime.agility);
    values[9] = f64::from_bits(runtime.at_boost_bits);
    values[10] = f64::from(runtime.attr_sum);
    values[11] = f64::from(runtime.atk_sum);
    values[12] = f64::from_bits(runtime.attract_bits);
    values[13] = f64::from(runtime.shield);
    values[14] = runtime.protect_pre_defend_skill_count.map_or(0.0, |count| count as f64);
    values[15] = f64::from(runtime.move_state.speed_points);
    values[16] = f64::from(runtime.charge.step);
    values[17] = runtime.accumulate.acc();
    values[18] = runtime.accumulate.charge_bonus();
    if let Some(hide) = runtime.hide {
        values[19] = f64::from(hide.level);
        values[20] = f64::from_bits(hide.attract_bits);
        values[21] = f64::from(hide.agility);
        values[22] = f64::from(hide.defense);
        values[23] = f64::from(hide.resistance);
    }
    values
}

/// `entity_num_present`：只有 `[14]` 与 hide 的 `[19..23]` 跟随 Option，其余跟随实体行。
pub(super) fn entity_num_present(runtime: &ModelPlayerRuntime) -> [bool; 24] {
    let mut present = [true; 24];
    present[14] = runtime.protect_pre_defend_skill_count.is_some();
    for slot in present.iter_mut().skip(19).take(5) {
        *slot = runtime.hide.is_some();
    }
    present
}

/// `template_num` 原始值；`[17..30]` 在 `clone_build` 缺失时保持 0。
pub(super) fn template_num_values(template: &ModelTemplate) -> [f64; 31] {
    let mut values = [0.0f64; 31];
    values[0] = f64::from(template.max_hp);
    values[1] = f64::from(template.attack);
    values[2] = f64::from(template.magic);
    values[3] = f64::from(template.magic_point);
    values[4] = f64::from(template.wisdom);
    values[5] = f64::from(template.speed);
    values[6] = f64::from(template.defense);
    values[7] = f64::from(template.resistance);
    values[8] = f64::from(template.agility);
    values[9] = f64::from_bits(template.at_boost_bits);
    values[10] = f64::from(template.attr_sum);
    values[11] = f64::from(template.atk_sum);
    values[12] = f64::from_bits(template.attract_bits);
    values[13] = f64::from(template.reserved_player_ids_before_spawn);
    values[14] = template.identity.boss_action_prob_count as f64;
    values[15] = f64::from(template.identity.boost_immune_threshold);
    values[16] = f64::from(template.move_state.speed_points);
    if let Some(clone_build) = &template.clone_build {
        values[17] = f64::from_bits(clone_build.name_factor_bits);
        values[18] = f64::from_bits(clone_build.child_name_factor_bits);
        let adjustments = &clone_build.adjustments;
        values[19] = f64::from(adjustments.max_hp);
        values[20] = f64::from(adjustments.attack);
        values[21] = f64::from(adjustments.magic);
        values[22] = f64::from(adjustments.wisdom);
        values[23] = f64::from(adjustments.speed);
        values[24] = f64::from(adjustments.defense);
        values[25] = f64::from(adjustments.resistance);
        values[26] = f64::from(adjustments.agility);
        values[27] = f64::from_bits(adjustments.at_boost_delta_bits);
        values[28] = adjustments.attr_sum as f64;
        values[29] = f64::from(adjustments.atk_sum);
        values[30] = f64::from_bits(adjustments.attract_delta_bits);
    }
    values
}

/// `template_num_present`：`[0..16]`（含 `move_state` 槽 16）跟随模板行，
/// `[17..30]` 跟随 `clone_build` 整体存在性。
pub(super) fn template_num_present(template: &ModelTemplate) -> [bool; 31] {
    let mut present = [false; 31];
    for slot in present.iter_mut().take(17) {
        *slot = true;
    }
    if template.clone_build.is_some() {
        for slot in present.iter_mut().skip(17) {
            *slot = true;
        }
    }
    present
}

/// `RuntimeCorpseKind` 的固定词表：枚举 `None` 是真实类 1，不是缺失。
pub(super) fn corpse_kind_id(corpse: RuntimeCorpseKind) -> i32 {
    match corpse {
        RuntimeCorpseKind::None => 1,
        RuntimeCorpseKind::Merge => 2,
        RuntimeCorpseKind::Zombie => 3,
    }
}

/// `owner_resolution` 的固定词表（1 起，与规格第 4 节分类落域表一致）。
pub(super) fn owner_resolution_id(policy: OwnerResolutionPolicy) -> i32 {
    match policy {
        OwnerResolutionPolicy::SelfEntity => 1,
        OwnerResolutionPolicy::RootOwner => 2,
    }
}

/// `damage_share` 的固定词表。
pub(super) fn damage_share_id(policy: DamageSharePolicy) -> i32 {
    match policy {
        DamageSharePolicy::None => 1,
        DamageSharePolicy::ShareToOwner => 2,
        DamageSharePolicy::ShareToSummons => 3,
    }
}

/// `merge` 的固定词表。
pub(super) fn merge_id(policy: MergePolicy) -> i32 {
    match policy {
        MergePolicy::None => 1,
        MergePolicy::FixedLane => 2,
        MergePolicy::DropUnmappedSkills => 3,
    }
}
