//! 东方角色的人偶、分身与式神沿用原生召唤物归属和死亡链。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
use crate::runtime::entity::RosmonticRuntime;
impl CombatRuntime {
    pub fn pcb_spawn(
        &mut self,
        actor: EntityIdx,
        role: u8,
        name: &str,
        attrs: [i32; 8],
        points: i32,
        attract: f64,
        updates: &mut RunUpdates,
    ) -> EntityIdx {
        let owner = self.entities.get(actor).unwrap();
        let mut template = self
            .preview_plain_minion_blueprint(actor, crate::namerena::MinionKind::Shadow)
            .unwrap_or_else(|| {
                let kind = self.registry.player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT).unwrap();
                PlayerTemplate::with_kind(0, "shadow", kind, owner.runtime.team, attrs[0].max(1), attrs[1].max(0))
            });
        template.name = format!("{}?shadow", owner.template.name);
        template.display_name = name.to_owned();
        template.id_key_name = format!("{}?{}@!", owner.template.name, self.entities.len());
        template.skills = SkillLoadout::default();
        template.policy_overrides.damage_share = Some(DamageSharePolicy::None);
        // 前鬼保留原生守护被动，行动仍由专属招式接管。
        if role == 20 {
            let protect = self.registry.skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT).unwrap();
            template.skills = SkillLoadout::from_skill_levels([(protect, 100)]);
        }
        template.max_hp = attrs[0].max(1);
        template.attack = attrs[1];
        template.defense = attrs[2];
        template.speed = attrs[3];
        template.agility = attrs[4];
        template.magic = attrs[5];
        template.resistance = attrs[6];
        template.wisdom = attrs[7];
        template.magic_point = attrs[7] / 2;
        template.move_state.speed_points = points;
        template.attract_bits = attract.to_bits();
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template: Box::new(template),
            message: std::borrow::Cow::Borrowed("召唤出[1]"),
        });
        self.drain_effects_into(updates);
        let child = self.entities.get_mut(idx).unwrap();
        let mut state = RosmonticRuntime {
            child_kind: role,
            ..Default::default()
        };
        if role == 4 {
            state.levels = vec![70, 140];
        }
        if role == 22 {
            state.levels = vec![0, 50, 60, 0, 50];
        }
        child.runtime.boss_mut().rosmontic = Some(Box::new(state));
        idx
    }

    /// 人偶每次行动重新选择目标，治疗人偶只面向仍存活的友军。
    pub fn pcb_child_action(&mut self, actor: EntityIdx, role: u8, updates: &mut RunUpdates) {
        if role == 14 {
            let e = self.entities.get_mut(actor).unwrap();
            e.runtime.hp = e.template.max_hp;
            self.rosmontic_message(actor, "[0]恢复了全部生命", updates);
            return;
        }
        if role == 16 {
            if let Some(target) = self.select_plain_heal_targets(actor, true).into_iter().next() {
                self.ros_message(actor, target, "[0]为[1]使用[春之治愈]", updates);
                self.apply_plain_absorb_heal_into(actor, target, 240, updates);
                self.ros_cleanse(target);
            }
            return;
        }
        if role == 22 {
            for skill in [2, 1, 4] {
                let level = self.ros_state(actor).levels[skill];
                if (self.rng.r127() as i32) < level {
                    let targets = self.ros_enemies(actor, if skill == 4 { 2 } else { 1 });
                    if !targets.is_empty() {
                        self.ros_chen(actor, skill, targets, updates);
                        return;
                    }
                }
            }
            self.ros_default(actor, updates);
            return;
        }
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        match role {
            4 => {
                if (self.rng.r127() as i32) < self.ros_state(actor).levels[0] {
                    self.pcb_caged_bird(actor, target, false, updates);
                } else if (self.rng.r127() as i32) < self.ros_state(actor).levels[1] {
                    self.pcb_forbidden_game(actor, target, false, updates);
                } else {
                    self.ros_default(actor, updates);
                }
            }
            10 => {
                self.ros_message(actor, target, "[0]使用[急速激光]", updates);
                let atp = self.ros_atp(actor, false) * 1.5;
                self.ros_hit(actor, target, true, atp, true, PlainAttackOnDamage::None, updates);
            }
            11 | 18 => {
                self.rosmontic_message(
                    actor,
                    if role == 11 {
                        "[0]使用[广域激光]"
                    } else {
                        "[0]使用[试验型激光]"
                    },
                    updates,
                );
                let atp = self.ros_atp(actor, true) * if role == 11 { 1.2 } else { 4.0 };
                for t in self.ros_enemies(actor, 4) {
                    self.ros_hit(actor, t, true, atp, false, PlainAttackOnDamage::None, updates);
                }
            }
            12 => {
                self.ros_message(actor, target, "[0]使用[赤之十]", updates);
                let atp = self.ros_atp(actor, false);
                self.ros_hit(actor, target, true, atp * 0.7, true, PlainAttackOnDamage::None, updates);
                self.ros_hit(actor, target, true, atp * 0.9, false, PlainAttackOnDamage::None, updates);
            }
            15 => {
                self.ros_message(actor, target, "[0]使用[雾中束缚]", updates);
                let magic = self.entities.get(actor).unwrap().runtime.magic;
                let resistance = self.entities.get(target).unwrap().runtime.resistance;
                if !self.ros_immune(target, "slow") && !self.roll_plain_dodge(target, magic, resistance, false) {
                    let layers = if self.entities.get(target).unwrap().states.entry(78).is_some() {
                        3
                    } else {
                        2
                    };
                    self.pcb_slow(actor, target, layers, 1024, updates);
                }
            }
            17 => {
                self.ros_message(actor, target, "[0]使用[白垩冰枪]", updates);
                let atp = self.ros_atp(actor, false) * 1.4;
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
            20 if self.rng.r127() < 120 => {
                self.ros_message(actor, target, "[0]使用[前鬼之击]", updates);
                let atp = self.ros_atp(actor, false) * 0.85;
                self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 720;
            }
            21 => {
                self.ros_message(actor, target, "[0]使用[后鬼之击]", updates);
                let atp = self.ros_atp(actor, false) * 1.8;
                self.ros_hit(actor, target, false, atp, false, PlainAttackOnDamage::None, updates);
            }
            _ => self.ros_default(actor, updates),
        }
    }
}
