//! 大妖精的支援与牺牲、红美铃的气和连拳、小恶魔的逆召唤。
use super::super::*;
use crate::runtime::entity::RosmonticRuntime;
impl CombatRuntime {
    /// 使用同名幻影蓝图派生属性，但移除普通幻术／血祭的技能与分摊策略。
    pub(super) fn ros_spawn(&mut self, actor: EntityIdx, role: u8, updates: &mut RunUpdates) -> EntityIdx {
        let owner = self.entities.get(actor).unwrap();
        let hp = match role {
            1 => owner.template.max_hp / 12,
            2 => 666,
            _ => 1,
        };
        let mut template = self
            .preview_plain_minion_blueprint(actor, crate::namerena::MinionKind::Shadow)
            .unwrap_or_else(|| {
                let kind = self.registry.player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT).unwrap();
                PlayerTemplate::with_kind(0, "shadow", kind, owner.runtime.team, hp.max(1), owner.runtime.attack.max(0))
            });
        template.name = format!("{}?shadow", owner.template.name);
        template.display_name = match role {
            1 => "妖精",
            2 => "帕秋莉？",
            _ => "残影",
        }
        .to_owned();
        template.id_key_name = format!("{}?{}@!", owner.template.name, self.entities.len());
        template.skills = SkillLoadout::default();
        template.max_hp = hp.max(1);
        template.move_state.speed_points = match role {
            1 => -2048,
            2 => 2048,
            _ => 0,
        };
        if role == 2 {
            template.attack = 66;
            template.defense = 99;
            template.speed = 66;
            template.agility = 66;
            template.magic = 99;
            template.resistance = 99;
            template.wisdom = 66;
        }
        if role == 3 {
            template.agility = 1;
            template.defense = 1;
            template.magic = 1;
        }
        let idx = self.entities.next_spawn_idx(&template);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "召唤出[1]".to_owned(),
        });
        self.drain_effects_into(updates);
        let child = self.entities.get_mut(idx).unwrap();
        child.runtime.policies.damage_share = DamageSharePolicy::None;
        child.runtime.rosmontic = Some(Box::new(RosmonticRuntime {
            child_kind: role,
            ..Default::default()
        }));
        idx
    }

    pub(super) fn ros_child_action(&mut self, actor: EntityIdx, role: u8, updates: &mut RunUpdates) {
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        match role {
            2 => {
                self.ros_message(actor, target, "[0]使用[「姆Q」]", updates);
                let fail = (self.rng.next_u8() & 3) > 1;
                let magic = self.entities.get(actor).unwrap().runtime.magic;
                let wisdom = self.entities.get(target).unwrap().runtime.wisdom;
                if self.ros_immune(target, "charm") || self.roll_plain_dodge(target, magic, wisdom, false) || fail {
                    return;
                }
                let team = self.plain_effective_team(actor);
                let id = self.registry.state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT).unwrap();
                let priority = self.registry.state(id).unwrap().priority;
                let step = self
                    .entities
                    .get(target)
                    .unwrap()
                    .states
                    .entry(76)
                    .and_then(StateEntry::charm_value)
                    .map_or(1, |(_, _, _, _, s)| s + 1);
                let e = self.entities.get_mut(target).unwrap();
                e.states.clear_legacy_key(76);
                e.states.add_entry(StateEntry::charm(
                    76,
                    id,
                    actor.0 as usize,
                    Some(team),
                    Some(team),
                    Some(target.0),
                    step,
                    priority,
                ));
                e.refresh_runtime_stats_from_template();
                self.ros_message(actor, target, "[1]被[魅惑]了", updates);
            }
            3 => {
                self.ros_message(actor, target, "[0]对[1]发起佯攻", updates);
                self.ros_hit(actor, target, false, 0.0, false, PlainAttackOnDamage::None, updates);
            }
            _ => self.ros_default(actor, updates),
        }
    }

    pub(super) fn ros_daiyousei(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        let target = targets[0];
        match skill {
            0 => {
                // 原版的赋值条件使苏生量恒为目标最大 HP；施法者随后灵力散尽。
                self.ros_state(actor).levels[0] = (self.ros_state(actor).levels[0] + 1) / 2;
                let e = self.entities.get_mut(target).unwrap();
                if e.runtime.alive {
                    return;
                }
                e.runtime.hp = e.template.max_hp;
                e.runtime.alive = true;
                let team = e.runtime.team;
                self.world.revive_round_actor(target);
                self.world.revive_alive(target, team);
                self.ros_message(actor, target, "[0]使用[捏造「科廷利的花仙子」]，[1]满血复活了", updates);
                if self.is_rosmontic_boss(target, "cirno")
                    || self.entities.get(target).unwrap().template.id_key_name == "lilywhite@!"
                {
                    for i in 0..6 {
                        self.ros_stat(target, i, 2.0, 0);
                    }
                    self.ros_message(actor, target, "[0]：[1]！这是我最后的灵力了！", updates);
                }
                self.entities.get_mut(actor).unwrap().runtime.hp = 0;
                self.drain_summon_explode_self_death_into(actor, updates);
            }
            1 => {
                let level = self.ros_state(actor).levels[1];
                if level > 8 {
                    self.ros_state(actor).levels[1] -= 1;
                }
                let heal = (self.ros_atp(actor, true) / 40.0).ceil() as i32;
                let e = self.entities.get_mut(target).unwrap();
                let heal = heal.min(e.template.max_hp - e.runtime.hp).max(0);
                e.runtime.hp += heal;
                self.ros_message(actor, target, "[0]发动[「妖精的祈愿」]", updates);
                let mut u = RunUpdate::new("[1]回复体力[2]点", actor.0 as usize, target.0 as usize, heal as u32);
                u.param = Some(heal as u32);
                updates.add(u);
                self.ros_cleanse(target);
            }
            2 => {
                self.ros_state(actor).levels[2] = (f64::from(self.ros_state(actor).levels[2]) * 0.5).round() as i32;
                self.rosmontic_message(actor, "[0]发动[「妖精的声援」]", updates);
                self.ros_spawn(actor, 1, updates);
                self.ros_spawn(actor, 1, updates);
            }
            _ => {
                self.ros_message(actor, target, "[0]发动[「妖精的策动」]", updates);
                let speed = self.entities.get(actor).unwrap().runtime.speed;
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += speed;
                let id = self.registry.state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT).unwrap();
                let priority = self.registry.state(id).unwrap().priority;
                let e = self.entities.get_mut(target).unwrap();
                if let Some((f, effective, step)) =
                    e.states.entry(PLAIN_HASTE_STATE_KEY).and_then(StateEntry::haste_runtime_value)
                {
                    e.states.set_payload(
                        PLAIN_HASTE_STATE_KEY,
                        StatePayload::Haste {
                            faster: f,
                            effective_faster: effective,
                            step: step + 4,
                        },
                    );
                } else {
                    e.states.add_entry(StateEntry::haste_with_effective_faster(
                        PLAIN_HASTE_STATE_KEY,
                        id,
                        2,
                        2,
                        3,
                        priority,
                    ));
                }
                self.ros_cleanse(target);
            }
        }
    }

    pub(super) fn ros_hong(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        let target = targets[0];
        match skill {
            1 => {
                let charge = self.entities.get(actor).unwrap().runtime.charge.active;
                let s = self.ros_state(actor);
                s.aura = true;
                s.aura_bonus = charge;
                self.entities.get_mut(actor).unwrap().refresh_runtime_stats_from_template();
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += 400 + if charge { 500 } else { 0 };
                self.rosmontic_message(actor, "[0]使用[气符「猛虎内劲」]，[气势如虹！]", updates);
            }
            2 => {
                let bits = self.ros_state(actor).qi_bits;
                let qi = if bits == 0 { 1.0 } else { f64::from_bits(bits) };
                self.ros_state(actor).qi_bits = (qi * 1.2).to_bits();
                self.ros_message(actor, target, "[0]使用[彩华「虹色太极拳」]", updates);
                self.ros_hit(
                    actor,
                    target,
                    false,
                    (qi * 1.5).round(),
                    false,
                    PlainAttackOnDamage::None,
                    updates,
                );
            }
            3 => {
                self.rosmontic_message(actor, "[0]使用[星气「星脉地转弹」]", updates);
                let atp = self.ros_atp(actor, false) * 3.0 / (targets.len() + 1) as f64;
                for t in targets {
                    self.ros_hit(actor, t, false, atp, false, PlainAttackOnDamage::None, updates);
                }
            }
            _ => {
                if self.ros_state(actor).levels[4] > 0 {
                    self.ros_state(actor).levels[4] = 0;
                    self.ros_state(actor).charged_skill = Some(4);
                    self.rosmontic_message(actor, "[0][蓄气]", updates);
                    return;
                }
                self.ros_message(actor, target, "[0]使用[炽击「大鹏坠击拳」]", updates);
                for _ in 0..3 {
                    if !self.entities.get(target).unwrap().is_active() || !self.entities.get(actor).unwrap().is_active() {
                        break;
                    }
                    let r = &self.entities.get(actor).unwrap().runtime;
                    let accuracy = 100 + r.agility;
                    let t = &self.entities.get(target).unwrap().runtime;
                    let dodge = t.resistance + t.agility;
                    if self.roll_plain_dodge(target, accuracy, dodge, false) {
                        break;
                    }
                    let atp = self.ros_atp(actor, false) * 1.8;
                    self.ros_hit(actor, target, true, atp, true, PlainAttackOnDamage::None, updates);
                }
            }
        }
    }

    pub(super) fn ros_koakuma(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        match skill {
            1 => {
                self.ros_state(actor).levels[1] = 0;
                self.rosmontic_message(actor, "[0]发动[「逆恶魔召唤」]", updates);
                let child = self.ros_spawn(actor, 2, updates);
                self.ros_state(actor).child = Some(child);
            }
            2 => {
                self.ros_state(actor).actions += 1;
                if self.ros_state(actor).actions >= 2 {
                    self.ros_state(actor).levels[2] = 0;
                }
                self.rosmontic_message(actor, "[0]使用[「梦中的死灵之书」]", updates);
                for t in targets {
                    let r = &self.entities.get(t).unwrap().runtime;
                    let atp = 100.0 * f64::from(r.resistance + r.defense);
                    self.ros_hit(actor, t, true, atp, false, PlainAttackOnDamage::None, updates);
                    self.ros_hit(actor, t, false, atp, false, PlainAttackOnDamage::None, updates);
                }
            }
            _ => {
                let t = targets[0];
                let physical = skill == 3;
                self.ros_message(
                    actor,
                    t,
                    if physical {
                        "[0]使用[粉碎「帕西菲斯冲击」]"
                    } else {
                        "[0]使用[升华「墨瓦蜡泥加冲击」]"
                    },
                    updates,
                );
                let atp = self.ros_atp(actor, false) * 1.35;
                let magic = self.entities.get(actor).unwrap().runtime.magic;
                let dodge = if physical {
                    self.entities.get(t).unwrap().runtime.defense
                } else {
                    self.entities.get(t).unwrap().runtime.resistance
                };
                if self.roll_plain_dodge(t, magic, dodge, false) {
                    return;
                }
                self.ros_stat(t, if physical { 5 } else { 1 }, 0.8, 0);
                self.ros_hit(actor, t, !physical, atp, true, PlainAttackOnDamage::None, updates);
            }
        }
    }
}
