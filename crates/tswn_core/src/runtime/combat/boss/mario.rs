//! 马里奥（mario）Boss：采菇成长、火球术等级与死亡复活。
//!
//! 成长层数与火球术等级保存在 `MarioBoss` 离散状态载荷里，随状态清除一起移除；
//! 复活次数挂在运行时字段上，不受状态清除影响。
use super::super::*;
use crate::runtime::lang::keys;
use crate::runtime::update::RunUpdateHp;

impl CombatRuntime {
    /// 读取马里奥 Boss 状态中记录的火系技能等级；没有该状态时返回 0。
    pub fn plain_mario_fire_level(&self, actor: EntityIdx) -> u32 {
        self.entities
            .get(actor)
            .and_then(|entity| entity.states.entry(PLAIN_MARIO_BOSS_STATE_KEY))
            .and_then(StateEntry::mario_boss_value)
            .map_or(0, |(_, fire_level)| fire_level)
    }

    /// `SklMarioGet`：按成长次数依次发放蘑菇、火焰花和奖命蘑菇。
    ///
    /// 成长层数与火球术等级保存在 `MarioBoss` 载荷中；离散状态清除会连同
    /// 它们一起移除，与 0.5.5 的 meta 销毁一致。复活次数挂在运行时字段上，
    /// 不受状态清除影响。
    pub fn drain_plain_mario_get_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (grow, fire_level) = self
            .entities
            .get(actor)
            .and_then(|entity| entity.states.entry(PLAIN_MARIO_BOSS_STATE_KEY))
            .and_then(StateEntry::mario_boss_value)
            .unwrap_or((0, 0));
        let next_grow = grow + 1;
        let next_fire_level = if next_grow == 2 { 120 } else { fire_level };
        let entity = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime mario-get actor: {}", actor.0));
        if entity.states.entry(PLAIN_MARIO_BOSS_STATE_KEY).is_some() {
            assert!(
                entity.states.set_payload(
                    PLAIN_MARIO_BOSS_STATE_KEY,
                    StatePayload::MarioBoss {
                        grow: next_grow,
                        fire_level: next_fire_level,
                    },
                ),
                "runtime mario state disappeared during growth"
            );
        } else {
            assert!(
                entity
                    .states
                    .add_entry(StateEntry::mario_boss(PLAIN_MARIO_BOSS_STATE_KEY, next_grow, next_fire_level)),
                "runtime mario state should be inserted"
            );
        }
        let announcement = |key| RunUpdate::new_lang(key, actor.0 as usize, actor.0 as usize, 0).without_replay_target();
        match next_grow {
            1 => {
                updates.add_with(|| announcement(keys::BOSS_MARIO_GROW_10));
                entity.refresh_runtime_stats_from_template();
                updates.add_with(|| announcement(keys::BOSS_MARIO_GROW_11));
            }
            2 => {
                updates.add_with(|| announcement(keys::BOSS_MARIO_GROW_20));
                updates.add_with(|| announcement(keys::BOSS_MARIO_GROW_21));
            }
            _ => {
                updates.add_with(|| announcement(keys::BOSS_MARIO_GROW_30));
                let lives = entity.runtime.mario_reraise_lives.unwrap_or(0) + 1;
                entity.runtime.mario_reraise_lives = Some(lives);
                updates.add_with(|| {
                    let mut life_update = announcement(keys::BOSS_MARIO_LIFE);
                    life_update.param = Some(lives);
                    life_update
                });
            }
        }
        entity.runtime.move_state.speed_points += 2000;
    }

    /// 0.5.5 `SklMarioReraise.b1`：死亡时消耗一条命并满血复活。
    ///
    /// 复活在体力仍为 0 时静默清除负 metaType 状态（legacy 的解除消息要求
    /// `fx > 0`，因此这里没有解除文本），随后补满体力并执行
    /// `SklMarioGet.K` 的成长与火球等级重置。
    pub fn drain_plain_mario_reraise_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(entity) = self.entities.get_mut(target) else {
            return false;
        };
        if !entity.runtime.flags.contains(PlayerKindFlags::BOSS) || entity.template.name != "mario" {
            return false;
        }
        if entity.runtime.hp > 0 {
            return false;
        }
        let Some(lives) = entity.runtime.mario_reraise_lives else {
            return false;
        };
        let remaining = lives.saturating_sub(1);
        entity.runtime.mario_reraise_lives = Some(remaining);
        if remaining == 0 {
            return false;
        }
        let negative_keys: smallvec::SmallVec<[u32; 8]> = entity
            .states
            .entries()
            .iter()
            .filter(|entry| entry.is_negative_state())
            .map(|entry| entry.legacy_order_key)
            .collect();
        for key in negative_keys {
            entity.states.clear_legacy_key(key);
        }
        entity.runtime.hp = entity.template.max_hp;
        entity.states.clear_legacy_key(PLAIN_MARIO_BOSS_STATE_KEY);
        entity.refresh_runtime_stats_from_template();

        updates.add_with(|| {
            let mut revive_update = RunUpdate::new_lang(keys::BOSS_MARIO_REVIVE, target.0 as usize, target.0 as usize, 0)
                .without_target()
                .with_hp_changes([RunUpdateHp {
                    player: target.0 as usize,
                    before: 0,
                    after: entity.runtime.hp,
                }]);
            revive_update.delay0 = 3000;
            revive_update
        });
        updates.add_with(|| {
            let mut life_update =
                RunUpdate::new_lang(keys::BOSS_MARIO_LIFE, target.0 as usize, target.0 as usize, 0).without_replay_target();
            life_update.param = Some(remaining);
            life_update
        });
        true
    }
}
