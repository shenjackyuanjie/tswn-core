//! Squall 重制：按实际行动采样，固定两次出勤动作，其余招式使用正式战斗链。
use super::super::*;
/// 取自本地聊天记录的测试/模型相关原句，只用于演出台词。
pub const SAMPLE_QUOTES: [&str; 5] = [
    "其实不是判断出来，其实就是测出来的",
    "我提一嘴，不要对模型的准确性抱有任何信心",
    "我还没有进行漏率的检查",
    "只是拟合模型",
    "转换不成实际战力的话看着烧心",
];

/// 红温台词取自本地聊天记录中唯一出现“红温”的原话，发送于 2026-11-06 00:50:43。
pub const RED_QUOTE: &str = "我已经要被 6.1sol 整红温了";

/// 禁言的固定魔力下限。
pub const TYRANNY_MP_FLOOR: i32 = -64;

impl CombatRuntime {
    /// 派生独立的演出随机数，读取当前检查点但不消费战斗 RNG，也不窥视后续随机序列。
    pub fn next_squall_sample_quote(&mut self, actor: EntityIdx) -> &'static str {
        let s = self.entities.get(actor).unwrap().runtime.squall.as_ref().unwrap();
        let mut value = ((self.rng.i as u64) << 32)
            ^ (self.rng.j as u64)
            ^ u64::from(s.turn).wrapping_mul(0x9e3779b97f4a7c15)
            ^ u64::from(actor.0);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        let index = s.last_sample_quote.map_or((value % 5) as u8, |last| (last + 1 + (value % 4) as u8) % 5);
        self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().last_sample_quote = Some(index);
        SAMPLE_QUOTES[index as usize]
    }

    /// 每条发动文案后立即插入独立台词段，效果明细随后沿用原版战报。
    fn squall_say(
        &self,
        actor: EntityIdx,
        message: impl Into<std::borrow::Cow<'static, str>>,
        quote: &'static str,
        updates: &mut RunUpdates,
    ) {
        updates.add_newline();
        updates.add(RunUpdate::new(message, actor.0 as usize, actor.0 as usize, 1).without_replay_target());
        updates.add_newline();
        updates.add(
            RunUpdate::new(quote, actor.0 as usize, actor.0 as usize, 0)
                .as_dialogue()
                .without_replay_target(),
        );
        updates.add_newline();
    }

    fn squall_enemies(&self, actor: EntityIdx, main_only: bool) -> Vec<EntityIdx> {
        let team = self.plain_effective_team(actor);
        self.world
            .flat_alive()
            .iter()
            .copied()
            .filter(|idx| {
                self.entities.get(*idx).is_some_and(|e| {
                    e.runtime.active() && self.plain_effective_team(*idx) != team && (!main_only || !e.runtime.is_minion())
                })
            })
            .collect()
    }

    /// 采集正式结算造成的实际生命损失，过量伤害不计；召唤单位不冒充本体输出。
    pub fn record_squall_damage(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32) {
        if self.squall_bosses.is_empty() || amount <= 0 {
            return;
        }
        let Some(source) = self.entities.get(caster) else {
            return;
        };
        if source.runtime.is_minion() {
            return;
        }
        let source_team = self.plain_effective_team(caster);
        let target_team = self.plain_effective_team(target);
        for boss in self.squall_bosses.clone() {
            let boss_team = self.plain_effective_team(boss);
            if source_team == boss_team || target_team != boss_team {
                continue;
            }
            let Some(e) = self.entities.get_mut(boss) else {
                continue;
            };
            if !e.runtime.active() {
                continue;
            }
            if let Some(s) = &mut e.runtime.squall {
                if let Some((_, damage)) = s.damage.iter_mut().find(|(idx, _)| *idx == caster) {
                    *damage += i64::from(amount);
                } else {
                    s.damage.push((caster, i64::from(amount)));
                }
            }
        }
    }

    /// 每次真正行动才推进出勤与呼号器倒计时，冰冻期间不会偷偷跳过阶段。
    pub fn begin_squall_action(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(s) = self.entities.get_mut(actor).and_then(|e| e.runtime.squall.as_mut()) else {
            return;
        };
        s.turn = s.turn.saturating_add(1);
        let enemies = self.squall_enemies(actor, true);
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        if let Some((target, _)) = s.damage.iter().filter(|(idx, _)| enemies.contains(idx)).max_by_key(|(_, amount)| *amount) {
            s.marked = Some(*target);
        }
        s.damage.clear();
        self.try_squall_red(actor, updates);
    }

    /// 出勤双防可被驱散；红温和样本不是普通增益，净化不重置它们。
    pub fn dispel_squall_effects(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some(s) = self.entities.get_mut(actor).and_then(|e| e.runtime.squall.as_mut())
            && s.attendance
        {
            s.attendance = false;
            self.entities.get_mut(actor).unwrap().refresh_runtime_stats_from_template();
            self.squall_say(actor, "[0]的[出勤]双防增益解除了", "出勤回来了。", updates);
        }
    }

    /// 双防直接修正属性；不叠加旧科研铁壁或易伤。
    pub fn apply_squall_defense(&self, _target: EntityIdx, _value: &mut RuntimeDefendValue) {}

    /// HP 首次低于 50% 时一次性进入红温：全属性永久提高 50%，持续整场，净化不能解除。
    pub fn try_squall_red(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(e) = self.entities.get_mut(actor) else {
            return;
        };
        if !e.runtime.active()
            || (i64::from(e.runtime.hp) * 2 >= i64::from(e.template.max_hp)
                && !e.runtime.squall.as_ref().is_some_and(|s| s.force_red_at.is_some_and(|at| s.turn >= at)))
        {
            return;
        }
        let Some(s) = &mut e.runtime.squall else {
            return;
        };
        if s.red {
            return;
        }
        s.red = true;
        // 永久倍率由覆盖层保存，后续疾走、隐匿和出勤结束重算时不丢失。
        for i in 0..7 {
            self.ros_stat(actor, i, 1.5, 0);
        }
        self.squall_say(actor, "[0]进入[红温]", RED_QUOTE, updates);
        updates.add(RunUpdate::new("[0]的全属性上升50%", actor.0 as usize, actor.0 as usize, 0));
    }

    /// 每个敌方本体行动各给一层样本；不合并同一间隔内多人的贡献。
    pub fn finish_squall_action(&mut self, actor: Option<EntityIdx>, updates: &mut RunUpdates) {
        for boss in self.squall_bosses.clone() {
            self.try_squall_red(boss, updates);
            if !self.entities.get(boss).is_some_and(|e| e.runtime.active()) {
                continue;
            }
            let Some(actor) = actor else {
                continue;
            };
            if actor == boss {
                let s = self.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
                if s.turn >= 2 && s.attendance {
                    s.attendance = false;
                    self.entities.get_mut(boss).unwrap().refresh_runtime_stats_from_template();
                    updates.add(RunUpdate::new("[0]的[出勤]双防增益结束了", boss.0 as usize, boss.0 as usize, 0));
                }
                continue;
            }
            if self.plain_effective_team(actor) == self.plain_effective_team(boss)
                || self.entities.get(actor).is_none_or(|e| e.runtime.is_minion())
            {
                continue;
            }
            let s = self.entities.get_mut(boss).unwrap().runtime.squall.as_mut().unwrap();
            if s.samples < 6 {
                s.samples += 1;
                let quote = self.next_squall_sample_quote(boss);
                self.squall_say(boss, "[0]开始测试，获得1层测试样本", quote, updates);
            }
        }
    }

    fn squall_can_continue(&self, actor: EntityIdx) -> bool { self.entities.get(actor).is_some_and(|e| e.runtime.active()) }

    fn squall_attack(&mut self, actor: EntityIdx, target: EntityIdx, scale: f64, updates: &mut RunUpdates) {
        self.squall_hit(actor, target, true, scale, false, updates);
    }

    /// 伤害倍率独立取样，强命只跳过回避，仍结算防御、护盾和死亡。
    fn squall_hit(&mut self, actor: EntityIdx, target: EntityIdx, magic: bool, scale: f64, sure: bool, updates: &mut RunUpdates) {
        if !self.squall_can_continue(actor)
            || !self.entities.get(target).is_some_and(|e| e.runtime.active())
            || self.plain_effective_team(actor) == self.plain_effective_team(target)
        {
            return;
        }
        let atp = self.entities.get(actor).unwrap().runtime.get_at(magic, &mut self.rng) * scale;
        updates.add_newline();
        if sure {
            self.drain_plain_attack_after_dodge_into(
                actor,
                target,
                magic,
                atp,
                DamageKind::Default,
                None,
                PlainAttackOnDamage::None,
                updates,
            );
        } else {
            self.drain_plain_attack_with_atp_into(actor, target, magic, atp, updates);
        }
    }

    pub fn squall_take_amplify(&mut self, _actor: EntityIdx) -> f64 { 1.0 }

    /// 第一、第二次行动固定为出勤/回来；50% 只影响是否带回猎豹。
    pub fn drain_squall_action_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        if let Some((owner, role, _)) = self.entities.get(actor).and_then(|e| e.runtime.squall_helper) {
            self.squall_helper_action(actor, owner, role, updates);
            return;
        }
        if !self.squall_can_continue(actor) {
            return;
        }
        let s = self.entities.get(actor).unwrap().runtime.squall.as_ref().unwrap();
        match s.turn {
            1 => {
                self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().attendance = true;
                self.entities.get_mut(actor).unwrap().runtime.shield += 100;
                self.entities.get_mut(actor).unwrap().refresh_runtime_stats_from_template();
                self.squall_say(actor, "[0]使用[出勤]", "出勤去了。", updates);
                updates.add(RunUpdate::new(
                    "[0]获得100点护盾，防御与抗性翻倍至下回合结束",
                    actor.0 as usize,
                    actor.0 as usize,
                    0,
                ));
                return;
            }
            2 => {
                self.squall_say(actor, "[0][正在出勤回来的路上]", "正在出勤回来的路上。", updates);
                if self.rng.next_u8() < 128 {
                    let count = 1 + (self.rng.next_u8() & 1);
                    for _ in 0..count {
                        self.tiger_spawn_leopard(actor, updates);
                    }
                }
                return;
            }
            _ => {}
        }
        let mut choices = vec![1, 3, 4, 6];
        let s = self.entities.get(actor).unwrap().runtime.squall.as_ref().unwrap();
        if s.samples >= 2 {
            choices.push(0);
        }
        if s.samples > 0 {
            choices.push(5);
        }
        if !s.summoned {
            choices.push(2);
        }
        let selected = choices[usize::from(self.rng.next_u8()) % choices.len()];
        match selected {
            0 => self.squall_weighted(actor, updates),
            1 => self.squall_pager(actor, updates),
            2 => self.squall_team(actor, updates),
            3 => self.squall_weaken(actor, updates),
            4 => self.squall_spray(actor, updates),
            5 => self.squall_barrage(actor, updates),
            _ => self.squall_tyranny(actor, updates),
        }
    }

    pub fn squall_weighted(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let enemies = self.squall_enemies(actor, true);
        if enemies.is_empty() {
            return;
        }
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        s.samples -= 2;
        let target = s.marked.filter(|idx| enemies.contains(idx)).unwrap_or(enemies[0]);
        let other = enemies.iter().copied().find(|idx| *idx != target).unwrap_or(target);
        let amp = self.squall_take_amplify(actor);
        self.squall_say(
            actor,
            "[0]使用[建立模型]",
            "你这个分数很高，但实际打下来不是这么回事。",
            updates,
        );
        updates.add(RunUpdate::new("[0]消耗2层[测试样本]", actor.0 as usize, actor.0 as usize, 0));
        for picked in [target, target, other] {
            if !self.squall_can_continue(actor) {
                break;
            }
            let t = if self.entities.get(picked).is_some_and(|e| e.runtime.active()) {
                Some(picked)
            } else {
                self.squall_enemies(actor, false).first().copied()
            };
            if let Some(t) = t {
                self.squall_attack(actor, t, 1.74 * amp, updates);
            }
        }
    }

    pub fn squall_team(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().summoned = true;
        self.squall_say(actor, "[0]使用[三人科研]", "三个人的关系，并没有三个相加那么简单。", updates);
        let owner = self.entities.get(actor).unwrap();
        let hp = 168;
        let team = owner.runtime.team;
        let kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
            .expect("core summon kind missing");
        for (role, name) in [(0, "高位辅助"), (1, "低位配件")] {
            let mut template = PlayerTemplate::with_kind(0, name, kind, team, hp, 128);
            template.magic = 128;
            template.defense = 128;
            template.resistance = 128;
            template.speed = 212;
            template.agility = 128;
            template.wisdom = 128;
            template.magic_point = 64;
            // 高位辅助的嘲讽要真正拉走攻击；低位配件靠极高潜行躲开索敌。
            template.attract_bits = if role == 0 { 1.0e12_f64 } else { 1024.0_f64 }.to_bits();
            if role == 1
                && let Some(hide) = self.registry.skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT)
            {
                template = template.with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 200)]));
            }
            template.id_key_name = format!("Squall·{}@Squall", name);
            template.display_name = name.to_owned();
            let idx = self.entities.next_spawn_idx(&template);
            self.effects.push(QueuedEffect::SpawnWithMessage {
                caster: actor,
                template,
                message: "召唤出[1]".to_owned(),
            });
            self.drain_effects_into(updates);
            let helper = self.entities.get_mut(idx).expect("research helper missing");
            // 召唤即满血。
            helper.runtime.hp = helper.template.max_hp;
            helper.runtime.squall = None;
            helper.runtime.squall_helper = Some((actor, role, 0));
            helper.runtime.policies.damage_share = DamageSharePolicy::None;
        }
    }

    fn squall_helper_action(&mut self, actor: EntityIdx, owner: EntityIdx, role: u8, updates: &mut RunUpdates) {
        // 助手只继承角色分工，不复制科研、烧库或召唤能力。
        let (_, _, turn) = self.entities.get(actor).unwrap().runtime.squall_helper.unwrap();
        self.entities.get_mut(actor).unwrap().runtime.squall_helper = Some((owner, role, turn + 1));
        if role == 0
            && self
                .entities
                .get(owner)
                .is_some_and(|e| e.runtime.active() && self.plain_effective_team(actor) == self.plain_effective_team(owner))
        {
            if turn % 2 == 0 {
                self.drain_plain_iron_skill_into(actor, updates);
            } else {
                self.entities.get_mut(owner).unwrap().runtime.protect_from.retain(|p| p.owner != actor);
                self.entities
                    .get_mut(owner)
                    .unwrap()
                    .runtime
                    .protect_from
                    .push(ProtectLinkRuntime { owner: actor, level: 64 });
                self.entities.get_mut(actor).unwrap().runtime.protect_to = Some(owner);
                updates.add(RunUpdate::new("[0]开始[守护][1]", actor.0 as usize, owner.0 as usize, 0));
            }
        } else if role == 1 {
            let targets = self.squall_enemies(actor, false);
            if turn % 2 == 0 {
                for t in targets {
                    self.squall_attack(actor, t, 0.6, updates);
                }
            } else if let Some(t) = targets.first() {
                self.drain_plain_slow_skill_into(actor, *t, updates);
            }
        } else if let Some(t) = self.squall_enemies(actor, false).first().copied() {
            self.squall_attack(actor, t, 1.0, updates);
        }
    }

    /// 呼号器对全体角色加速；下一次自身行动前仍未红温则强制触发。
    pub fn squall_pager(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.squall_say(actor, "[0]使用[发布测号器]", "只是拟合模型。", updates);
        let ids: Vec<_> = self.world.flat_alive().to_vec();
        for id in ids {
            self.ros_stat(id, 2, 1.0, 50);
        }
        let s = self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap();
        if !s.red {
            s.force_red_at = Some(s.turn + 1);
        }
    }

    /// 对最高七围目标统一处理，不包含对尚未重制的豹星的特判。
    pub fn squall_weaken(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let Some(target) = self.squall_enemies(actor, false).into_iter().max_by_key(|id| {
            let r = &self.entities.get(*id).unwrap().runtime;
            i64::from(r.attack)
                + i64::from(r.defense)
                + i64::from(r.speed)
                + i64::from(r.agility)
                + i64::from(r.magic)
                + i64::from(r.resistance)
                + i64::from(r.wisdom)
        }) else {
            return;
        };
        self.squall_say(actor, "[0]使用[再卖弱把你头薅下来]", "再卖弱把你头薅下来。", updates);
        // 覆盖层保存永久减半，后续迟缓/隐匿刷新不会恢复原值。
        for i in 0..7 {
            self.ros_stat(target, i, 0.5, 0);
        }
        let amount = self.entities.get(target).unwrap().runtime.hp.max(0) / 2;
        if self.apply_plain_attack_damage_into(actor, target, amount, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
        if self.squall_can_continue(actor) {
            let e = self.entities.get_mut(actor).unwrap();
            let heal = amount.min(e.template.max_hp - e.runtime.hp).max(0);
            e.runtime.hp += heal;
            updates.add(RunUpdate::new(
                "[0]吸取体力[2]点",
                actor.0 as usize,
                actor.0 as usize,
                heal as u32,
            ));
        }
    }

    pub fn squall_spray(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.squall_say(actor, "[0]使用[开喷]", "开喷。", updates);
        let mut enemies = self.squall_enemies(actor, false);
        self.rng.sort_list(&mut enemies);
        enemies.truncate(enemies.len().div_ceil(2));
        for target in enemies {
            self.squall_hit(actor, target, false, 1.52, false, updates);
            self.squall_hit(actor, target, true, 0.76, false, updates);
        }
    }

    pub fn squall_barrage(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.squall_say(
            actor,
            "[0]使用[我就端这么个东西上来你爱用不用]",
            "我就端这么个东西上来，你爱用不用。",
            updates,
        );
        let count = std::mem::take(&mut self.entities.get_mut(actor).unwrap().runtime.squall.as_mut().unwrap().samples);
        for _ in 0..count {
            if !self.squall_can_continue(actor) {
                break;
            }
            let mut targets = self.squall_enemies(actor, false);
            self.rng.sort_list(&mut targets);
            if let Some(target) = targets.first() {
                self.squall_hit(actor, *target, true, 1.2, true, updates);
            }
        }
    }

    /// 固定 MP、行动条与冰冻标记，攻击 +30 属于永久属性变化。
    pub fn squall_tyranny(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        self.squall_say(actor, "[0]使用[暴政一下]", "所以情商低，下一个", updates);
        let mut targets = self.squall_enemies(actor, false);
        self.rng.sort_list(&mut targets);
        let count = 1 + usize::from(self.rng.next_u8()) % 3;
        targets.truncate(count);
        for target in targets {
            let e = self.entities.get_mut(target).unwrap();
            e.runtime.magic_point = TYRANNY_MP_FLOOR;
            e.runtime.move_state.speed_points -= 2048;
            e.states.add_ice_frozen_step(PLAIN_ICE_STATE_KEY, 4096);
            e.refresh_runtime_stats_from_template();
            updates.add_newline();
            updates.add(RunUpdate::new(
                "[1]被[0]禁言1分钟，行动延后并附加4096点冰冻标记",
                actor.0 as usize,
                target.0 as usize,
                0,
            ));
        }
        self.ros_stat(actor, 0, 1.0, 30);
    }
}
