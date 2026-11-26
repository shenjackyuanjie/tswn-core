use super::*;

use crate::namerena::BossActionSkill;
use crate::runtime::lang::keys;
use crate::runtime::update::RunUpdateHp;

#[derive(Clone, Copy)]
enum BossMergeSlot {
    Normal(usize),
    Action(BossActionSkill),
    Unleveled,
}

impl CombatRuntime {
    /// 吞噬按原版 Boss 技能数组的位置取等级，包含不参与主动扫描的 0 级槽。
    pub fn plain_boss_merge_levels(&self, target: EntityIdx) -> Option<smallvec::SmallVec<[u32; 3]>> {
        use BossActionSkill::{Fire, IceAge, IkarugaAttack, MarioGet, YuriControl};
        use BossMergeSlot::{Action, Normal, Unleveled};

        let template = &self.entities.get(target)?.template;
        if !template.native_boss_skills {
            return None;
        }
        let name = crate::namerena::boss_combat_name(&template.name);
        let slots: &[BossMergeSlot] = match name {
            "aokiji" => &[Unleveled, Action(IceAge), Normal(1)],
            "ikaruga" => &[Unleveled, Action(IkarugaAttack)],
            "mario" => &[Action(MarioGet), Action(Fire), Unleveled],
            "mosquito" => &[Normal(4)],
            "sonic" => &[Normal(6), Normal(7), Normal(30)],
            "thanos" => &[Normal(29)],
            "wheel" => &[Normal(30)],
            "yuri" => &[Action(YuriControl), Normal(25), Normal(27)],
            "covid" | "lazy" | "saitama" | "slime" => &[Unleveled],
            "conan" => &[],
            _ => return None,
        };
        let skills = &template.skills;
        Some(
            slots
                .iter()
                .map(|&slot| match slot {
                    Normal(key) => skills
                        .levels()
                        .iter()
                        .enumerate()
                        .find_map(|(lane, &level)| (skills.fixed_lane_key_at(lane) == Some(key)).then_some(level))
                        .unwrap_or(0),
                    Action(Fire) => self.plain_mario_fire_level(target),
                    Action(skill) => crate::namerena::boss_action_skills(name)
                        .iter()
                        .find(|&&(action, _)| action == skill)
                        .map_or(0, |&(_, level)| level),
                    Unleveled => 0,
                })
                .collect(),
        )
    }

    /// 0.5.5 Boss `k4` 扫描：按插入顺序尝试各主动技能的概率与目标。
    ///
    /// 每个技能先调用 `au`（无条件消耗随机字节，马里奥采菇与吸血术的
    /// smart 低血量门槛除外），通过后再用技能自己的 `aa` 选目标；
    /// `aa` 返回 null 时继续尝试下一个技能，空数组（马里奥采菇）也算选中。
    pub fn scan_plain_boss_action_skills(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBossSkillAction> {
        let name = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime boss action-scan actor: {}", actor.0))
            .template
            .name
            .clone();
        for &(skill, base_level) in crate::namerena::boss_action_skills(&name) {
            // 马里奥的火球术等级由采菇状态改写，初始为 0。
            let level = if matches!(skill, BossActionSkill::Fire) {
                self.plain_mario_fire_level(actor)
            } else {
                base_level
            };
            if !self.plain_boss_action_skill_probability(actor, skill, level, smart) {
                continue;
            }
            let targets = match skill {
                BossActionSkill::Fire | BossActionSkill::Absorb | BossActionSkill::Critical => {
                    self.select_plain_default_enemy_targets(actor, smart)
                }
                BossActionSkill::Ice => self.select_plain_ice_targets(actor, smart),
                BossActionSkill::Rapid => self.select_plain_rapid_targets(actor, smart),
                BossActionSkill::IceAge | BossActionSkill::IkarugaAttack => {
                    self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
                }
                BossActionSkill::YuriControl => self.select_plain_yuri_control_targets(actor, smart),
                BossActionSkill::MarioGet
                | BossActionSkill::Squall
                | BossActionSkill::Quilt
                | BossActionSkill::Smdd
                | BossActionSkill::Xjy
                | BossActionSkill::Yi
                | BossActionSkill::Tiger
                | BossActionSkill::Fish => PreparedTargetList::new(),
                // `SklSaitama.au` 的基础概率在等级 0 时恒为 false，`aa` 不会执行。
                BossActionSkill::Saitama => continue,
            };
            if targets.is_empty() && !matches!(skill, BossActionSkill::MarioGet) {
                continue;
            }
            return Some(PreparedBossSkillAction {
                selected: skill,
                targets,
            });
        }
        None
    }

    /// 复刻 Boss 主动技能的 `au`。只有马里奥采菇与吸血术会跳过随机字节。
    pub fn plain_boss_action_skill_probability(
        &mut self,
        actor: EntityIdx,
        skill: BossActionSkill,
        level: u32,
        smart: bool,
    ) -> bool {
        match skill {
            BossActionSkill::MarioGet => {
                let (grow, lives) = {
                    let entity = self
                        .entities
                        .get(actor)
                        .unwrap_or_else(|| panic!("unknown runtime mario-get probability actor: {}", actor.0));
                    let grow = entity
                        .states
                        .entry(PLAIN_MARIO_BOSS_STATE_KEY)
                        .and_then(StateEntry::mario_boss_value)
                        .map_or(0, |(grow, _)| grow);
                    (grow, entity.runtime.mario_reraise_lives.unwrap_or(3))
                };
                if grow >= 2 {
                    if lives >= 2 { false } else { self.rng.next_u8() < 7 }
                } else {
                    self.rng.c50()
                }
            }
            BossActionSkill::Absorb if smart => {
                let entity = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime absorb probability actor: {}", actor.0));
                if entity.template.max_hp - entity.runtime.hp < 32 {
                    false
                } else {
                    self.rng.r127() < level
                }
            }
            _ => self.rng.r127() < level,
        }
    }

    /// 读取马里奥 Boss 状态中记录的火系技能等级；没有该状态时返回 0。
    fn plain_mario_fire_level(&self, actor: EntityIdx) -> u32 {
        self.entities
            .get(actor)
            .and_then(|entity| entity.states.entry(PLAIN_MARIO_BOSS_STATE_KEY))
            .and_then(StateEntry::mario_boss_value)
            .map_or(0, |(_, fire_level)| fire_level)
    }

    /// 结算扫描选中的 Boss 主动技能；通用技能沿用现有内置实现。
    pub fn drain_plain_boss_skill_into(&mut self, actor: EntityIdx, prepared: PreparedBossSkillAction, updates: &mut RunUpdates) {
        match prepared.selected {
            BossActionSkill::Squall => self.drain_squall_action_into(actor, updates),
            BossActionSkill::Quilt => self.drain_quilt_action_into(actor, updates),
            BossActionSkill::Smdd => self.drain_smdd_action_into(actor, updates),
            BossActionSkill::Xjy => self.drain_xjy_action_into(actor, updates),
            BossActionSkill::Yi => self.drain_yi_action_into(actor, updates),
            BossActionSkill::Tiger => self.drain_tiger_action_into(actor, updates),
            BossActionSkill::Fish => self.drain_fish_action_into(actor, updates),
            BossActionSkill::Fire => {
                let target = prepared.targets[0];
                self.drain_plain_fire_skill_into(actor, target, updates);
            }
            BossActionSkill::Ice => {
                let target = prepared.targets[0];
                self.drain_plain_ice_skill_into(actor, target, updates);
            }
            BossActionSkill::Absorb => {
                let target = prepared.targets[0];
                self.drain_plain_absorb_skill_into(actor, target, updates);
            }
            BossActionSkill::Critical => {
                let target = prepared.targets[0];
                self.drain_plain_critical_skill_into(actor, target, updates);
            }
            BossActionSkill::Rapid => {
                self.drain_plain_rapid_skill_into(actor, prepared.targets, updates);
            }
            BossActionSkill::IceAge => {
                self.drain_plain_boss_ice_age_into(actor, prepared.targets, updates);
            }
            BossActionSkill::IkarugaAttack => {
                self.drain_plain_boss_ikaruga_attack_into(actor, prepared.targets, updates);
            }
            BossActionSkill::MarioGet => {
                self.drain_plain_mario_get_into(actor, updates);
            }
            BossActionSkill::YuriControl => {
                let target = prepared.targets[0];
                self.drain_plain_yuri_control_into(actor, target, updates);
            }
            BossActionSkill::Saitama => {
                unreachable!("runtime saitama boss skill never enters k4 selection");
            }
        }
    }

    /// `SklAokijiIceAge`：目标列表确定后只算一次攻击力，逐目标全额攻击。
    pub fn drain_plain_boss_ice_age_into(&mut self, actor: EntityIdx, targets: PreparedTargetList, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        updates.add_with(|| {
            RunUpdate::new_lang(keys::SKL_AOKIJI_ICE_AGE, actor.0 as usize, targets[0].0 as usize, 1)
                .with_animation_targets(targets.iter().map(|target| target.0 as usize))
        });
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime ice-age actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 2.5
            / (targets.len() as f64 + 0.5);
        for target in targets {
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0);
            if !target_alive {
                continue;
            }
            updates.add_newline();
            self.drain_plain_attack_with_atp_and_on_damage_into(
                actor,
                target,
                true,
                atp,
                DamageKind::Ice,
                PlainAttackOnDamage::Ice,
                updates,
            );
        }
    }

    /// `SklIkarugaAttack`：同冰河时代只算一次攻击力，但跳过闪避走防御换算。
    pub fn drain_plain_boss_ikaruga_attack_into(
        &mut self,
        actor: EntityIdx,
        targets: PreparedTargetList,
        updates: &mut RunUpdates,
    ) {
        if targets.is_empty() {
            return;
        }
        updates.add_with(|| {
            RunUpdate::new_lang(keys::SKL_IKARUGA_ATTACK, actor.0 as usize, targets[0].0 as usize, 1)
                .with_animation_targets(targets.iter().map(|target| target.0 as usize))
        });
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime ikaruga actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 2.5
            / (targets.len() as f64 + 0.5);
        for target in targets {
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0);
            if !target_alive {
                continue;
            }
            updates.add_newline();
            self.drain_plain_attack_from_defense_into(actor, target, true, atp, updates);
        }
    }

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

    /// 尤里的心灵控制目标：不看 smart，只要目标不在自身当前队伍、没有魅惑。
    pub fn select_plain_yuri_control_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let target_pool = Self::quilt_target_pool(&self.entities, &self.world);
        let all_alive = target_pool.as_ref();
        let mut candidates = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut enemy_skip_indices = smallvec::SmallVec::<[usize; 8]>::new();
        for (idx, target) in all_alive.iter().copied().enumerate() {
            if self.entities.get(target).is_some_and(|entity| entity.runtime.team == actor_team) {
                enemy_skip_indices.push(idx);
            } else {
                candidates.push(target);
            }
        }
        if candidates.is_empty() {
            return PreparedTargetList::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            let valid = target != actor
                && self
                    .entities
                    .get(target)
                    .is_some_and(|entity| entity.states.entry(PLAIN_CHARM_STATE_KEY).is_none());
            if !valid {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_charm_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// `SklYuriControl`：不做免疫与闪避判定，层数取目标队伍初始人数下限 3。
    pub fn drain_plain_yuri_control_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new_lang(
            keys::SKL_YURI_CONTROL,
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let step = self.plain_group_initial_player_count(target).max(3) as i32;
        let caster_effective_team_idx = self.plain_effective_team(actor);
        let existing = self
            .entities
            .get(target)
            .and_then(|entity| entity.states.entry(PLAIN_CHARM_STATE_KEY))
            .and_then(StateEntry::charm_value);
        if let Some((_group_id, _effective_team_idx, _source_team_idx, state_target, old_step)) = existing {
            assert!(
                self.entities.get_mut(target).unwrap().states.set_payload(
                    PLAIN_CHARM_STATE_KEY,
                    StatePayload::Charm {
                        group_id: actor.0 as usize,
                        effective_team_idx: Some(caster_effective_team_idx),
                        source_team_idx: Some(caster_effective_team_idx),
                        target: state_target,
                        step: old_step + step,
                    },
                ),
                "runtime yuri charm state disappeared during recharm"
            );
        } else {
            let charm_state_id = self
                .registry
                .state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT)
                .expect("default runtime profile must register core charm state");
            let charm_priority = self
                .registry
                .state(charm_state_id)
                .expect("default runtime core charm state disappeared")
                .priority;
            assert!(
                self.entities.get_mut(target).unwrap().states.add_entry(StateEntry::charm(
                    PLAIN_CHARM_STATE_KEY,
                    charm_state_id,
                    actor.0 as usize,
                    Some(caster_effective_team_idx),
                    Some(caster_effective_team_idx),
                    Some(target.0),
                    step,
                    charm_priority,
                )),
                "runtime yuri charm state should be inserted"
            );
            self.entities.get_mut(target).unwrap().refresh_runtime_stats_from_template();
        }
        updates.add(RunUpdate::new_lang(
            keys::SKL_CHARM_HIT,
            actor.0 as usize,
            target.0 as usize,
            120,
        ));
    }

    /// 目标所在队伍的初始成员数，对应 0.5.5 `group.initPlayers.length`。
    ///
    /// arena 里的初始成员都指向自己；分身、召唤与僵尸的 owner 指向召唤者，不会被计入。
    fn plain_group_initial_player_count(&self, target: EntityIdx) -> usize {
        let Some(entity) = self.entities.get(target) else {
            return 0;
        };
        let team = entity.runtime.team;
        self.entities
            .iter()
            .filter(|(idx, entity)| entity.runtime.team == team && entity.runtime.owner == *idx)
            .count()
    }
}
