//! 东方角色的共享选择、属性、召唤状态与行动结算。
//! 来源：KODU16（`kodu16/main`，`cf460c84`）。
use super::super::*;
use crate::runtime::entity::RosmonticRuntime;

impl CombatRuntime {
    pub fn ros_state(&mut self, actor: EntityIdx) -> &mut RosmonticRuntime {
        self.entities.get_mut(actor).unwrap().runtime.boss_mut().rosmontic.get_or_insert_default()
    }

    pub fn ros_atp(&mut self, actor: EntityIdx, magic: bool) -> f64 {
        self.entities.get(actor).unwrap().runtime.get_at(magic, &mut self.rng)
    }

    pub fn ros_enemies(&mut self, actor: EntityIdx, count: usize) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        let mut targets: Vec<_> = self
            .world
            .flat_alive()
            .iter()
            .copied()
            .filter(|id| self.plain_effective_team(*id) != team && !self.quilt_untargetable(*id))
            .collect();
        self.rng.sort_list(&mut targets);
        targets.truncate(count);
        targets
    }

    pub fn ros_message(&self, actor: EntityIdx, target: EntityIdx, text: &'static str, updates: &mut RunUpdates) {
        updates.add_newline();
        updates.add(RunUpdate::new(text, actor.0 as usize, target.0 as usize, 1));
    }

    /// 自定义技能仍走引擎的防御、受伤、死亡和召唤物链。
    pub fn ros_hit(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        magic: bool,
        atp: f64,
        sure: bool,
        effect: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        if !self.entities.get(actor).is_some_and(|e| e.is_active())
            || !self.entities.get(target).is_some_and(|e| e.runtime.hp > 0)
        {
            return 0;
        }
        if sure {
            self.drain_plain_attack_after_dodge_into(actor, target, magic, atp, DamageKind::Default, None, effect, updates)
        } else {
            self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, magic, atp, DamageKind::Default, effect, updates)
        }
    }

    /// 同时写入当前值与属性覆盖，避免后续疾走或迟缓刷新丢失变化。
    pub fn ros_stat(&mut self, actor: EntityIdx, index: usize, factor: f64, delta: i32) {
        let state = self.ros_state(actor);
        let old = f64::from_bits(state.stat_scale_bits[index]);
        let old = if old == 0.0 { 1.0 } else { old };
        state.stat_scale_bits[index] = (old * factor).to_bits();
        state.stat_add[index] = (f64::from(state.stat_add[index]) * factor) as i32 + delta;
        let r = &mut self.entities.get_mut(actor).unwrap().runtime;
        let value = match index {
            0 => &mut r.attack,
            1 => &mut r.defense,
            2 => &mut r.speed,
            3 => &mut r.agility,
            4 => &mut r.magic,
            5 => &mut r.resistance,
            _ => &mut r.wisdom,
        };
        *value = (f64::from(*value) * factor) as i32 + delta;
    }

    pub fn ros_default(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if self.is_rosmontic_boss(actor, "marisa") {
            let add = (self.rng.next_u8() & 3) + 1;
            let s = self.ros_state(actor);
            s.energy = (s.energy + add).min(8);
            self.rosmontic_message(actor, "[0]为[八卦炉]填充了灵力", updates);
            return;
        }
        let Some(target) = self.select_plain_default_attack_target(actor, true) else {
            return;
        };
        let r = &mut self.entities.get_mut(actor).unwrap().runtime;
        let cost = (r.magic - r.attack).max(0) / 4;
        let magic = r.magic > r.attack && r.magic_point >= cost;
        if magic {
            r.magic_point -= cost;
        }
        self.drain_plain_default_attack_into(actor, target, magic, updates);
    }

    /// 回合前只推进真正获行动的角色；时停和梦想计数不受其他人的回合影响。
    pub fn begin_rosmontic_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(e) = self.entities.get_mut(actor) else {
            return;
        };
        let is_sakuya = e.template.name == "sakuya" && e.runtime.flags.contains(PlayerKindFlags::BOSS);
        let boost_agility = is_sakuya && e.runtime.agility < 310;
        let Some(s) = e.runtime.boss.as_deref_mut().and_then(|b| b.rosmontic.as_mut()) else {
            return;
        };
        if s.dream > 0 {
            s.dream = s.dream.saturating_add(1);
        }
        if is_sakuya {
            if boost_agility {
                s.stat_add[3] += 21;
            }
            let stop_ended = if s.time_stop > 0 {
                s.time_stop += 1;
                if s.time_stop > 4 {
                    s.time_stop = 0;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if boost_agility {
                e.runtime.agility += 21;
            }
            if stop_ended {
                self.rosmontic_message(actor, "[0]的[世界]结束了", updates);
            }
        }
        self.begin_pcb_action(actor, updates);
    }

    /// 结界四次自身行动后解除；燃烧的剩余威力分两轮衰减并走真实伤害链。
    pub fn end_rosmontic_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(s) = self.entities.get_mut(actor).and_then(|e| e.runtime.boss_mut().rosmontic.as_mut()) else {
            return;
        };
        if s.barrier_turns > 0 {
            s.barrier_turns -= 1;
            if s.barrier_turns == 0 {
                self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points -= 128;
                self.rosmontic_message(actor, "[0]的[结界]解除了", updates);
            }
        }
        let (caster, atp, turns) = {
            let s = self
                .entities
                .get(actor)
                .unwrap()
                .runtime
                .boss
                .as_deref()
                .and_then(|b| b.rosmontic.as_ref())
                .unwrap();
            (s.burn_owner, f64::from_bits(s.burn_atp_bits), s.burn_turns)
        };
        if let Some(caster) = caster
            && turns > 0
            && self.entities.get(actor).unwrap().runtime.hp > 0
        {
            let part = atp * (1.0 + f64::from(turns - 1) * 0.1) / f64::from(turns);
            let amount = (part / f64::from(self.entities.get(actor).unwrap().runtime.magic + 64)).ceil() as i32;
            let s = self.ros_state(actor);
            s.burn_turns -= 1;
            s.burn_atp_bits = (atp - part).to_bits();
            if s.burn_turns == 0 {
                s.burn_owner = None;
            }
            self.ros_message(caster, actor, "[1]被[烧伤]了", updates);
            if self.apply_plain_attack_damage_into(caster, actor, amount, updates) {
                self.drain_plain_lethal_damage_into(caster, actor, updates);
            }
        }
        self.end_pcb_action(actor, updates);
    }

    /// 驱散只清除可解除的结界和气场，不重置已经消耗的招式或五行进度。
    pub fn dispel_rosmontic_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(s) = self.entities.get_mut(target).and_then(|e| e.runtime.boss_mut().rosmontic.as_mut()) else {
            return;
        };
        let changed = s.aura || s.barrier_turns > 0 || s.divine_turns > 0;
        let divine = std::mem::take(&mut s.divine_turns) > 0;
        let aura_or_barrier = s.aura || s.barrier_turns > 0;
        s.aura = false;
        s.aura_bonus = false;
        s.barrier_turns = 0;
        s.scarlet = 0;
        if changed {
            self.entities.get_mut(target).unwrap().apply_runtime_stats_from_template(false);
            if aura_or_barrier {
                self.rosmontic_message(target, "[0]的[结界／气场]被打破了", updates);
            }
            if divine {
                self.rosmontic_message(target, "[0]从[神现]中解除", updates);
            }
        }
    }

    pub fn ros_immune(&mut self, target: EntityIdx, key: &str) -> bool {
        let e = self.entities.get(target).unwrap();
        let threshold = if e.runtime.flags.contains(PlayerKindFlags::BOSS) {
            crate::namerena::boss_immune_threshold(&e.template.name, key)
        } else if e.runtime.flags.contains(PlayerKindFlags::BOOST) {
            crate::namerena::boost_value(&e.template.name) as i32
        } else {
            return false;
        };
        threshold > 0 && i32::from(self.rng.next_u8()) < threshold
    }

    /// 支援清除所有负面状态，包含本批新增的燃烧，不清除友方增益。
    pub fn ros_cleanse(&mut self, target: EntityIdx) {
        let e = self.entities.get_mut(target).unwrap();
        let keys: Vec<_> = e
            .states
            .entries()
            .iter()
            .filter(|s| s.is_negative_state())
            .map(|s| s.legacy_order_key)
            .collect();
        let had_negative_state = !keys.is_empty();
        for key in keys {
            e.states.clear_legacy_key(key);
        }
        if let Some(s) = e.runtime.boss_mut().rosmontic.as_mut() {
            s.burn_owner = None;
            s.burn_atp_bits = 0;
            s.burn_turns = 0;
        }
        if had_negative_state {
            e.refresh_runtime_stats_from_template();
        }
    }

    pub fn drain_touhou_action_into(&mut self, actor: EntityIdx, smart: bool, updates: &mut RunUpdates) {
        if !self.entities.get(actor).is_some_and(EntityRecord::is_active) {
            return;
        }
        let child_kind = self.ros_state(actor).child_kind;
        if child_kind != 0 {
            if child_kind <= 3 {
                self.ros_child_action(actor, child_kind, updates);
            } else {
                self.pcb_child_action(actor, child_kind, updates);
            }
            return;
        }
        let name = self.entities.get(actor).unwrap().template.name.clone();
        // 被动技能不参与逐槽概率选择。
        let order: &[usize] = match name.as_str() {
            "reimu" => &[2, 1, 3, 4],
            "marisa" => &[1, 4, 0, 3, 2],
            "daiyousei" => &[0, 1, 2, 3],
            "hong" => &[3, 1, 2, 4],
            "koakuma" => &[1, 2, 4, 3],
            "patchouli" => &[4, 9, 7, 2, 0, 10, 8, 1, 5, 3, 6],
            "sakuya" => &[0, 3, 1, 2],
            "remilia" => &[3, 4, 1, 2],
            "flandre" => &[0, 2, 1, 3],
            "letty" => &[2, 5, 3, 4],
            "chen" => &[4, 3, 1, 2],
            "alice" => &[0, 7, 3, 5, 1, 10, 8, 4, 2, 6, 9],
            "phantom" => &[2, 0, 3, 1],
            "youmu" => &[1, 2, 0, 3],
            "yuyuko" => &[0, 2, 1, 3],
            "ran" => &[0, 2, 3, 1],
            _ => return,
        };
        if let Some(skill) = self.ros_state(actor).charged_skill.take() {
            let targets = if matches!(name.as_str(), "flandre" | "yuyuko") {
                self.ros_skill_targets(actor, skill, smart)
            } else {
                self.ros_enemies(actor, 1)
            };
            self.ros_cast(actor, skill, targets, updates);
            return;
        }
        let cost = i32::from(self.rng.next_u8() & 15) + 8;
        if self.entities.get(actor).unwrap().runtime.magic_point >= cost {
            for &skill in order {
                if !self.ros_skill_allowed(actor, skill, smart) {
                    continue;
                }
                let level = self.ros_state(actor).levels[skill];
                if level <= 0 || self.rng.r127() as i32 >= level {
                    continue;
                }
                let targets = self.ros_skill_targets(actor, skill, smart);
                if targets.is_empty() {
                    continue;
                }
                self.entities.get_mut(actor).unwrap().runtime.magic_point -= cost;
                self.ros_cast(actor, skill, targets, updates);
                return;
            }
        }
        self.ros_default(actor, updates);
    }

    pub fn ros_skill_allowed(&mut self, actor: EntityIdx, skill: usize, smart: bool) -> bool {
        let e = self.entities.get(actor).unwrap();
        let s = e.runtime.boss.as_deref().and_then(|b| b.rosmontic.as_ref()).unwrap();
        match e.template.name.as_str() {
            "reimu" => match skill {
                1 => s.dream == 0 || s.dream > 6,
                2 => s.dream == 0 && s.barrier_turns == 0,
                _ => s.dream == 0,
            },
            "marisa" => match skill {
                2 => s.energy > 0 && s.energy <= 2 + (self.rng.next_u8() & 3),
                3 => s.energy >= 2 && s.energy <= 4 + (self.rng.next_u8() & 3),
                4 => s.energy >= 4,
                _ => true,
            },
            "daiyousei" => skill != 2 || !smart || e.runtime.hp >= 400,
            "hong" => skill != 1 || (!s.aura && (!smart || (e.runtime.hp >= 120 && !e.runtime.accumulate.active))),
            "koakuma" => skill != 1 || e.runtime.hp <= 450,
            "patchouli" => {
                if skill == 10 {
                    return s.elements == 31;
                }
                let elem = skill / 2;
                let dependency = [4, 2, 0, 1, 3][elem];
                s.elements & (1 << elem) == 0 && ((s.elements & (1 << dependency) != 0) == (skill % 2 == 1))
            }
            "sakuya" => {
                skill != 1
                    || ((!smart || s.time_stop == 0)
                        && s.child.is_none_or(|id| !self.entities.get(id).is_some_and(|e| e.runtime.hp > 0)))
            }
            "remilia" => skill != 1 || !smart || e.runtime.hp >= 100 + 100 * s.scarlet as i32,
            "flandre" => skill != 0 || !smart || e.runtime.hp >= 4,
            "letty" => skill != 3 || e.runtime.hp * 100 <= e.template.max_hp * 49,
            "chen" => !(s.cat && matches!(skill, 2..=4)) && (skill != 3 || s.levels[3] <= 100),
            "alice" => {
                let dolls = self.pcb_dolls(actor);
                if skill == 1 {
                    return dolls.len() >= 3 && e.runtime.hp * 10 <= e.template.max_hp * 3;
                }
                if skill == 2 {
                    return dolls.len() >= 5 && e.runtime.hp * 10 >= e.template.max_hp * 3;
                }
                let role = match skill {
                    0 => 18,
                    3 => 10,
                    4 => 11,
                    5 => 12,
                    6 => 13,
                    7 => 17,
                    8 => 15,
                    9 => 16,
                    _ => 14,
                };
                !dolls.into_iter().any(|id| {
                    self.entities
                        .get(id)
                        .unwrap()
                        .runtime
                        .boss
                        .as_deref()
                        .and_then(|b| b.rosmontic.as_ref())
                        .unwrap()
                        .child_kind
                        == role
                })
            }
            "phantom" => {
                if skill == 0 {
                    s.phantom_members == 7
                } else {
                    s.phantom_members & (1 << (skill - 1)) != 0
                }
            }
            "youmu" => skill != 0 || s.guard != 2,
            "yuyuko" => match skill {
                0 => !smart || !e.states.entries().iter().any(|x| matches!(x.payload, StatePayload::Poison { .. })),
                1 => !s.butterfly_prepared && !s.butterfly_revived,
                _ => true,
            },
            "ran" => skill != 2 || s.divine_turns == 0,
            _ => true,
        }
    }

    fn ros_skill_targets(&mut self, actor: EntityIdx, skill: usize, smart: bool) -> Vec<EntityIdx> {
        let name = self.entities.get(actor).unwrap().template.name.as_str();
        match (name, skill) {
            ("flandre", 0) | ("chen", 1) | ("youmu", 0) | ("yuyuko", 1) | ("ran", 0 | 1 | 3) => vec![actor],
            ("alice", 1) | ("ran", 2) => self.world.flat_alive().to_vec(),
            ("alice", 2) => self.ros_enemies(actor, self.pcb_dolls(actor).len()),
            ("alice", _) => vec![actor],
            ("phantom", _) => {
                let mut targets = self.world.flat_alive().to_vec();
                self.rng.sort_list(&mut targets);
                targets.truncate(3);
                targets
            }
            ("letty", 2) | ("chen", 4) | ("youmu", 3) => self.ros_enemies(actor, 2),
            ("letty", 5) => self.ros_enemies(actor, 4),
            ("chen", 3) => {
                let count = 1 + (self.rng.next_u8() % 5) as usize;
                self.ros_enemies(actor, count)
            }
            ("yuyuko", 0 | 2) => self.ros_enemies(actor, 3),
            ("flandre", 1) if smart => {
                let targets = self.ros_enemies(actor, 999);
                targets
                    .into_iter()
                    .filter(|id| {
                        let e = self.entities.get(*id).unwrap();
                        e.runtime.hp >= 80 && e.states.entry(78).and_then(StateEntry::slow_value).unwrap_or(0) <= 1
                    })
                    .take(1)
                    .collect()
            }
            ("reimu", 2) | ("daiyousei", 2) | ("hong", 1 | 4) | ("koakuma", 1) | ("sakuya", 1) | ("remilia", 1) => vec![actor],
            ("daiyousei", 0) => self.select_plain_revive_targets(actor, smart).into_iter().collect(),
            ("daiyousei", 1) => self.select_plain_heal_targets(actor, smart).into_iter().collect(),
            ("daiyousei", 3) => self.select_plain_haste_targets(actor, smart).into_iter().collect(),
            ("reimu", 1) | ("sakuya", 0) => self.ros_enemies(actor, 999),
            ("reimu", 4) => self.ros_enemies(actor, 4),
            ("marisa", 3) => {
                let count = 2 + (self.rng.next_u8() & 1) as usize;
                self.ros_enemies(actor, count)
            }
            ("marisa", 4) => {
                let count = 4 + (self.rng.next_u8() & 1) as usize;
                self.ros_enemies(actor, count)
            }
            ("hong", 3) => self.ros_enemies(actor, 5),
            ("koakuma", 2) | ("remilia", 3) => self.ros_enemies(actor, 2),
            ("patchouli", 1 | 8 | 9) => self.ros_enemies(actor, 4),
            ("patchouli", 10) => self.ros_enemies(actor, 5),
            _ => self.ros_enemies(actor, 1),
        }
    }

    /// 独立招式入口也供行为回归直接覆盖各阶段，不依赖概率碰运气。
    pub fn ros_cast(&mut self, actor: EntityIdx, skill: usize, targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        if targets.is_empty() || !self.entities.get(actor).is_some_and(|e| e.is_active()) {
            return;
        }
        let name = self.entities.get(actor).unwrap().template.name.clone();
        match name.as_str() {
            "reimu" => self.ros_reimu(actor, skill, targets, updates),
            "marisa" => self.ros_marisa(actor, skill, targets, updates),
            "daiyousei" => self.ros_daiyousei(actor, skill, targets, updates),
            "hong" => self.ros_hong(actor, skill, targets, updates),
            "koakuma" => self.ros_koakuma(actor, skill, targets, updates),
            "patchouli" => self.ros_patchouli(actor, skill, targets, updates),
            "sakuya" => self.ros_sakuya(actor, skill, targets, updates),
            "remilia" => self.ros_remilia(actor, skill, targets, updates),
            "flandre" => self.ros_flandre(actor, skill, targets, updates),
            "letty" => self.ros_letty(actor, skill, targets, updates),
            "chen" => self.ros_chen(actor, skill, targets, updates),
            "alice" => self.ros_alice(actor, skill, targets, updates),
            "phantom" => self.ros_phantom(actor, skill, targets, updates),
            "youmu" => self.ros_youmu(actor, skill, targets, updates),
            "yuyuko" => self.ros_yuyuko(actor, skill, targets, updates),
            "ran" => self.ros_ran(actor, skill, targets, updates),
            _ => {}
        }
    }
}
