//! 柯南（conan）Boss：推理阶段选敌与秒杀处决。
use super::super::*;
use crate::runtime::lang::keys;

impl CombatRuntime {
    /// 使用柯南的智能模式尝试抽取一组目标。
    pub fn select_plain_conan_targets(&mut self, actor: EntityIdx) -> PreparedTargetList {
        self.select_plain_conan_targets_in_mode(actor, true)
    }

    pub fn select_plain_conan_action_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let mut targets = self.select_plain_conan_targets_in_mode(actor, smart);
        if targets.is_empty() {
            let team = self.plain_effective_team(actor);
            // 原版只在初次选敌返回空时转为智能重抽。扩展玩法可以只剩战斗使魔，
            // 这种情况下没有合法目标，返回空以免一直重试。
            if self.world.flat_alive().iter().any(|&enemy| {
                self.entities
                    .get(enemy)
                    .is_some_and(|entity| entity.runtime.team != team && !entity.runtime.is_combat_minion())
            }) {
                while targets.is_empty() {
                    targets = self.select_plain_conan_targets_in_mode(actor, true);
                }
            }
        }
        targets
    }

    /// 普通模式抽样 3 个目标并掷评分，智能模式抽样 4 个目标。
    /// 抽到战斗使魔只计入无效次数，分身仍是合法目标。
    fn select_plain_conan_targets_in_mode(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let select_count = if smart { 4 } else { 3 };
        let actor_team = self.plain_effective_team(actor);
        let target_pool = Self::quilt_target_pool(&self.entities, &self.world);
        let all_alive = target_pool.as_ref();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
        }
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                self.entities
                    .get(*candidate)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<smallvec::SmallVec<[usize; 8]>>();
        let mut selected = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut duplicate_count = 0usize;
        let mut invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if ally_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &ally_skip_indices)
            };
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            if self.entities.get(target).is_some_and(|entity| entity.runtime.is_combat_minion()) {
                invalid_count += 1;
                continue;
            }
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return PreparedTargetList::new();
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_default_enemy_target(target, smart)))
            .collect::<smallvec::SmallVec<[(EntityIdx, f64); 8]>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// 设置柯南的推理阶段值；推理状态必须已存在，否则 panic。
    fn set_conan_thinking(&mut self, actor: EntityIdx, value: i32) {
        let entity = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime conan actor: {}", actor.0));
        assert!(
            entity.runtime.conan_thinking.is_some(),
            "runtime conan thinking state disappeared"
        );
        entity.runtime.conan_thinking = Some(value);
    }

    /// `SklConan`：先推理再秒杀，单目标行动会推进推理阶段。
    ///
    /// `fx` 初始为 -1。单目标且处于 -1 时先提示发现尸体并转入推理；
    /// 推理期间每回合只输出消息；归零后把目标的当前血量直接清零并走死亡链。
    /// 击杀后自身速度按目标队伍剩余人数增加，最多 3000。
    pub fn drain_plain_conan_action_into(&mut self, actor: EntityIdx, targets: PreparedTargetList, updates: &mut RunUpdates) {
        let Some(&target) = targets.first() else {
            return;
        };
        let mut thinking = self
            .entities
            .get(actor)
            .and_then(|entity| entity.runtime.conan_thinking)
            .unwrap_or_else(|| panic!("runtime conan actor lacks thinking state: {}", actor.0));
        if thinking == -1 && targets.len() == 1 {
            thinking = 1;
            self.set_conan_thinking(actor, thinking);
            updates.add_with(|| {
                RunUpdate::new_lang(keys::SKL_CONAN_KILL_UNKNOWN, actor.0 as usize, actor.0 as usize, 0).without_replay_target()
            });
            updates.add_newline();
        }
        if thinking > 0 {
            self.set_conan_thinking(actor, thinking - 1);
            updates.add_with(|| {
                RunUpdate::new_lang(keys::SKL_CONAN_THINKING, actor.0 as usize, actor.0 as usize, 0).without_replay_target()
            });
            return;
        }

        let old_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime conan target: {}", target.0))
            .runtime
            .hp;
        self.entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime conan target: {}", target.0))
            .runtime
            .hp = 0;
        let kill_key = if targets.len() == 1 && thinking == 0 {
            updates.add_with(|| {
                RunUpdate::new_lang(keys::SKL_CONAN_THINKING_FINISH, actor.0 as usize, actor.0 as usize, 0)
                    .without_replay_target()
            });
            updates.add_with(|| {
                let mut only_truth = RunUpdate::new_lang(keys::SKL_CONAN_THINKING_FINISH2, actor.0 as usize, actor.0 as usize, 0)
                    .without_replay_target();
                only_truth.delay1 = 2000;
                only_truth
            });
            updates.add_with(|| {
                RunUpdate::new_lang(keys::SKL_CONAN_THINKING_FINISH3, actor.0 as usize, actor.0 as usize, 0)
                    .without_replay_target()
            });
            keys::SKL_CONAN_KILL_LAST
        } else {
            self.set_conan_thinking(actor, 1);
            keys::SKL_CONAN_KILL
        };
        // 处决本身结算体力；后续击倒只播放退场效果，死亡钩子仍按原顺序执行。
        updates.add_with(|| {
            let kill_score = (old_hp + 80).max(0) as u32;
            RunUpdate::new_lang(kill_key, actor.0 as usize, target.0 as usize, kill_score).with_execution_damage(old_hp)
        });
        let target_team = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime conan target: {}", target.0))
            .runtime
            .team;
        self.drain_plain_lethal_damage_into(actor, target, updates);
        let group_alive = self.world.team_alive(target_team).map_or(0, |alive| alive.len()) as i32;
        let entity = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime conan actor: {}", actor.0));
        entity.runtime.move_state.speed_points = (entity.runtime.move_state.speed_points + group_alive * 1000).min(3000);
    }
}
