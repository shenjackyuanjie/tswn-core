use super::super::*;

use crate::namerena::BossActionSkill;
use crate::runtime::lang::keys;

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
                | BossActionSkill::Fish
                | BossActionSkill::TestSubject => PreparedTargetList::new(),
                // `SklSaitama.au` 的基础概率在等级 0 时恒为 false，`aa` 不会执行。
                BossActionSkill::Saitama => continue,
            };
            // 马里奥采菇与实验体行动表的目标由结算阶段自行选取，空列表同样算选中。
            if targets.is_empty() && !matches!(skill, BossActionSkill::MarioGet | BossActionSkill::TestSubject) {
                continue;
            }
            return Some(PreparedBossSkillAction {
                selected: skill,
                targets,
                smart,
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
            // 实验体的抽签链按阶段变化（三阶段最多连抽三次），必须走专属入口。
            BossActionSkill::TestSubject => self.roll_plain_testsubject_phase_skill(actor),
            _ => self.rng.r127() < level,
        }
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
            BossActionSkill::TestSubject => self.drain_testsubject_action_into(actor, prepared.smart, updates),
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
}
