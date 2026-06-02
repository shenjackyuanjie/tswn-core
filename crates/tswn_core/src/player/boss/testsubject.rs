//! 测试专用 Boss（TestSubject）实现。
//!
//! 提供行为可配置的测试目标 Boss，用于引擎内部单元测试，不作为正式 Boss 参与排行。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    ActionTargets, OnDamageFunc, Player, PlrId, StateTrait, noop_on_damage,
    skill::act::{fire, slow::SlowState},
};
use crate::rc4::RC4;

const MAX_REVIVES: i32 = 2;

/// 实验体 #C8 的核心 Boss 状态。
///
/// revives_used:
/// - 0 = 第一阶段
/// - 1 = 第二阶段
/// - 2 = 第三阶段
#[derive(Clone, Debug)]
pub struct TestSubjectState {
    pub revives_used: i32,
    pub phase_action_count: i32,
    pub multi_claw_used: i32,
}

impl TestSubjectState {
    #[inline]
    pub fn phase(&self) -> i32 { self.revives_used + 1 }
}

impl StateTrait for TestSubjectState {
    fn meta_type(&self) -> i32 { 0 }

    /// 尽量让复活逻辑在死亡记录前执行。
    fn post_damage_priority(&self) -> i32 { i32::MIN }

    fn on_post_damage(
        &mut self,
        owner: PlrId,
        dmg: i32,
        _caster: PlrId,
        _randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        if dmg <= 0 {
            return;
        }

        let Some(player) = storage.just_get_player_mut(owner) else {
            return;
        };

        if player.get_status().hp > 0 {
            return;
        }

        if self.revives_used >= MAX_REVIVES {
            return;
        }

        self.revives_used += 1;
        self.phase_action_count = 0;
        self.multi_claw_used = 0;

        apply_test_subject_phase_attrs(player, self.revives_used);

        updates.add_newline();
        updates.emit(|| RunUpdate::new("[1]发生了异常再生", owner, owner, 80));

        match self.phase() {
            2 => {
                updates.emit(|| RunUpdate::new("[1]长出了一个新的头颅", owner, owner, 0));
            }
            3 => {
                updates.emit(|| RunUpdate::new("[1]长出了一个新的头颅", owner, owner, 0));
            }
            _ => {}
        }
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// 「颅骨重击」附加状态：易伤。
///
/// 规则：
/// - 易伤强度固定为受到伤害 +50%；
/// - stacks 只表示持续时间，不提高易伤倍率；
/// - 没有易伤时，颅骨重击施加 2 层持续时间；
/// - 已有易伤时，颅骨重击追加 1 层持续时间；
/// - 目标每次行动结束后减少 1 层；
/// - 层数归零后移除状态。
#[derive(Clone, Copy, Debug)]
pub struct SkullFractureState {
    pub stacks: i32,
}

impl StateTrait for SkullFractureState {
    fn meta_type(&self) -> i32 { -1 }

    fn post_defend_priority(&self) -> i32 { 10000 }

    fn on_post_defend(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        _randomer: &mut RC4,
        updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        if *dmg <= 0 || self.stacks <= 0 {
            return false;
        }

        updates.emit(|| RunUpdate::new("[易伤]使伤害提高", caster, owner, 0));
        *dmg = ((*dmg as f64) * 1.5).ceil() as i32;

        false
    }

    fn post_action_priority(&self) -> i32 { 1000 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut RC4,
        updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        if !alive {
            return false;
        }

        if self.stacks > 0 {
            self.stacks -= 1;
        }

        if self.stacks <= 0 {
            updates.emit(|| RunUpdate::new("[1]从[易伤]中解除", owner, owner, 0));
            true
        } else {
            false
        }
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

/// 第三阶段的无实体状态。
///
/// 设计目标：
/// - 类似铁壁，受到攻击时伤害降低为 1；
/// - 无法被净化；
/// - 无法被打消；
/// - 由实验体自己的奇偶行动机制控制进入和解除。
///
/// 因此 meta_type 设为 0，而不是正面状态。
#[derive(Clone, Copy, Debug)]
pub struct TestSubjectNoEntityState;

impl StateTrait for TestSubjectNoEntityState {
    fn meta_type(&self) -> i32 { 0 }

    fn post_defend_priority(&self) -> i32 { 10 }

    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        dmg: &mut i32,
        _caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &Arc<Storage>,
    ) -> bool {
        if *dmg > 1 {
            *dmg = 1;
        }

        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

pub fn init_test_subject_state(player: &mut Player) {
    player.set_state(TestSubjectState {
        revives_used: 0,
        phase_action_count: 0,
        multi_claw_used: 0,
    });

    apply_test_subject_phase_attrs(player, 0);
}

pub fn test_subject_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let phase = player.get_state::<TestSubjectState>().map(|state| state.phase()).unwrap_or(1);

    match phase {
        1 => phase_one_action(player, smart, randomer, updates, storage, targets),
        2 => phase_two_action(player, smart, randomer, updates, storage, targets),
        _ => phase_three_action(player, smart, randomer, updates, storage, targets),
    }

    after_test_subject_action(player, updates);
}

fn phase_one_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    if roll_skill(64, randomer) {
        skull_smash(player, randomer, updates, storage, targets);
    } else {
        test_subject_default_attack(player, smart, randomer, updates, storage, targets);
    }
}

fn phase_two_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    if roll_skill(128, randomer) {
        multi_claw(player, smart, randomer, updates, storage, targets);
    } else {
        test_subject_default_attack(player, smart, randomer, updates, storage, targets);
    }
}

fn phase_three_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    if roll_skill(21, randomer) {
        rupture(player, smart, randomer, updates, storage, targets);
    } else if roll_skill(64, randomer) {
        great_pounce(player, smart, randomer, updates, storage, targets);
    } else if roll_skill(128, randomer) {
        burning_roar(player, randomer, updates, storage, targets);
    } else {
        test_subject_default_attack(player, smart, randomer, updates, storage, targets);
    }
}

/// 一阶段：颅骨重击。
///
/// 对所有敌人造成 80% 物理伤害，
/// 若造成有效伤害，则附加易伤。
fn skull_smash(
    player: &mut Player,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();
    let targets_snapshot = targets.enemy_alive.iter().copied().collect::<Vec<_>>();
    let scale = 0.8 * phase_one_damage_scale(player);

    updates.add(RunUpdate::new("[0]使用[颅骨重击]", boss_id, boss_id, 1));

    for target_id in targets_snapshot {
        if !target_is_alive(storage, target_id) {
            continue;
        }

        attack_target(
            player,
            target_id,
            scale,
            false,
            "",
            on_skull_smash_damage as OnDamageFunc,
            randomer,
            updates,
            storage,
        );
    }
}

/// 二阶段：多段爪击。
///
/// 造成 50% 物理伤害 N + 3 次，
/// N 为该技能已使用次数。
///
/// 如果本次行动总计造成了 > 0 的伤害，
/// 则给目标附加一层减速。
fn multi_claw(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    let boss_id = player.as_ptr();

    let used_count = player.get_state::<TestSubjectState>().map(|state| state.multi_claw_used).unwrap_or(0);

    let hit_count = used_count + 3;

    updates.add(RunUpdate::new("[0]使用[多段爪击]", boss_id, target_id, 1));

    let mut total_damage = 0;

    for _ in 0..hit_count {
        if !target_is_alive(storage, target_id) {
            break;
        }

        let dmg = attack_target(player, target_id, 0.6, false, "", noop_on_damage, randomer, updates, storage);

        total_damage += dmg.max(0);
    }

    if let Some(state) = player.get_state_mut::<TestSubjectState>() {
        state.multi_claw_used += 1;
    }

    if total_damage > 0 {
        apply_slow_layer(boss_id, target_id, randomer, updates, storage);
    }
}

/// 三阶段：割裂。
///
/// 造成 50% 物理伤害 3 次。
fn rupture(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    let boss_id = player.as_ptr();

    updates.add(RunUpdate::new("[0]使用[割裂]", boss_id, target_id, 1));

    for _ in 0..3 {
        if !target_is_alive(storage, target_id) {
            break;
        }

        attack_target(player, target_id, 0.6, false, "", noop_on_damage, randomer, updates, storage);
    }
}

/// 三阶段：大猛扑。
///
/// 造成 225% 物理伤害。
fn great_pounce(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    let boss_id = player.as_ptr();

    updates.add(RunUpdate::new("[0]使用[大猛扑]", boss_id, target_id, 1));

    attack_target(player, target_id, 2.25, false, "", noop_on_damage, randomer, updates, storage);
}

/// 三阶段：燃烧咆哮。
///
/// 对所有敌人造成 80% 魔法伤害，
/// 并使用火球术一致的 on_fire 回调附加灼烧。
fn burning_roar(
    player: &mut Player,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();
    let targets_snapshot = targets.enemy_alive.iter().copied().collect::<Vec<_>>();

    updates.add(RunUpdate::new("[0]使用[燃烧咆哮]", boss_id, boss_id, 1));

    for target_id in targets_snapshot {
        if !target_is_alive(storage, target_id) {
            continue;
        }

        attack_target(
            player,
            target_id,
            0.8,
            true,
            "",
            fire::on_fire as OnDamageFunc,
            randomer,
            updates,
            storage,
        );
    }
}

fn test_subject_default_attack(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    let scale = if current_phase(player) == 1 {
        phase_one_damage_scale(player)
    } else {
        1.0
    };

    attack_target(
        player,
        target_id,
        scale,
        false,
        "[0]发起攻击",
        noop_on_damage,
        randomer,
        updates,
        storage,
    );
}

/// 每次 Boss 行动后的阶段机制。
///
/// 这里特别注意 Rust 借用规则：
/// 先在内部代码块里修改 TestSubjectState，
/// 把需要的信息复制出来，结束对 player 的 mutable borrow，
/// 然后再调用 player.has_state / set_state / clear_state。
fn after_test_subject_action(player: &mut Player, updates: &mut RunUpdates) {
    let boss_id = player.as_ptr();

    let enter_no_entity = {
        let Some(state) = player.get_state_mut::<TestSubjectState>() else {
            return;
        };

        state.phase_action_count += 1;

        if state.phase() != 3 {
            return;
        }

        state.phase_action_count % 2 == 1
    };

    if enter_no_entity {
        if !player.has_state::<TestSubjectNoEntityState>() {
            player.set_state_no_update(TestSubjectNoEntityState);
            updates.add(RunUpdate::new("[1]进入[无实体]状态", boss_id, boss_id, 0));
        }
    } else if player.has_state::<TestSubjectNoEntityState>() {
        player.clear_state::<TestSubjectNoEntityState>();
        updates.add(RunUpdate::new("[1]从[无实体]状态中解除", boss_id, boss_id, 0));
    }
}

fn on_skull_smash_damage(
    caster: PlrId,
    target: PlrId,
    dmg: i32,
    _randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    if dmg <= 0 {
        return;
    }

    let Some(target_player) = storage.just_get_player_mut(target) else {
        return;
    };

    if target_player.get_status().hp <= 0 {
        return;
    }

    if let Some(state) = target_player.get_state_mut::<SkullFractureState>() {
        state.stacks += 1;
    } else {
        target_player.set_state(SkullFractureState { stacks: 2 });
    }

    updates.emit(|| RunUpdate::new("[1]进入[易伤]状态", caster, target, 60));
}

fn apply_slow_layer(caster: PlrId, target_id: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    let Some(target) = storage.just_get_player_mut(target_id) else {
        return;
    };

    if target.get_status().hp <= 0 {
        return;
    }

    if target.check_immune("slow", randomer) {
        return;
    }

    let reduce_move_point = target.get_status().speed + 64;
    target.set_move_point(target.move_point() - reduce_move_point);

    if let Some(state) = target.get_state_mut::<SlowState>() {
        state.step += 2;
    } else {
        target.set_state(SlowState {
            owner: Some(caster),
            target: Some(target_id),
            on_post_action: None,
            step: 2,
        });
    }

    updates.add(RunUpdate::new("[1]进入[迟缓]状态", caster, target_id, 60));
}

#[allow(clippy::too_many_arguments)]
fn attack_target(
    player: &mut Player,
    target_id: PlrId,
    scale: f64,
    use_mag: bool,
    message: &'static str,
    on_damage: OnDamageFunc,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) -> i32 {
    let boss_id = player.as_ptr();
    let atp = player.get_at(use_mag, randomer) * scale;

    if !message.is_empty() {
        updates.add(RunUpdate::new(message, boss_id, target_id, 0));
    }

    let Some(target) = storage.just_get_player_mut(target_id) else {
        return 0;
    };

    target.attacked(atp, use_mag, boss_id, on_damage, randomer, updates, storage)
}

/// 按阶段强制设置实验体属性。
///
/// revives_used = 0:
///   HP 100，其余 100
///
/// revives_used = 1:
///   HP 200，其余 150
///
/// revives_used = 2:
///   HP 300，其余 200
fn apply_test_subject_phase_attrs(player: &mut Player, revives_used: i32) {
    let revives_used = revives_used.max(0) as u32;

    let attr_value = 80 + revives_used * 40;
    let hp_value = 100 + revives_used * 100;

    player.name_factor = 0.0;
    player.attr = [
        attr_value, // 攻
        attr_value, // 防
        attr_value, // 速
        attr_value, // 敏
        attr_value, // 魔
        attr_value, // 抗
        attr_value, // 智
        hp_value,   // HP
    ];

    player.update_states();

    player.status.hp = player.status.max_hp;
    player.status.magic_point = 1000;
    player.status.set_alive(true);
}

fn current_phase(player: &Player) -> i32 { player.get_state::<TestSubjectState>().map(|state| state.phase()).unwrap_or(1) }

fn phase_one_damage_scale(player: &Player) -> f64 {
    let action_count = player.get_state::<TestSubjectState>().map(|state| state.phase_action_count).unwrap_or(0);

    1.0 + action_count as f64 * 0.1
}

fn roll_skill(level: u32, randomer: &mut RC4) -> bool { randomer.r127() < level }

fn target_is_alive(storage: &Arc<Storage>, target_id: PlrId) -> bool {
    storage
        .get_player(&target_id)
        .map(|target| target.alive() && target.get_status().hp > 0)
        .unwrap_or(false)
}
