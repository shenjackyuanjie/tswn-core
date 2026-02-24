use super::*;

impl Player {
    pub fn update_player(&mut self) {
        self.init_skills();
        self.update_states();
    }

    /// 每回合中的玩家行动
    ///
    /// 包括 pre, main, post
    pub fn step(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        if !self.status.alive() {
            return;
        }
        self.update_states();
        let ptr = self.as_ptr();
        self.skills.update_state((ptr, randomer, updates, storage));
        let mut stp = self.status.speed * randomer.r3() as i32;
        stp = self.apply_pre_step_states(stp, updates);
        stp = self.skills.pre_step(stp, (ptr, randomer, updates, storage));
        self.status.move_point += stp;
        if self.check_move() {
            self.status.move_point -= MOVE_POINT_THRESHOLD;
            // 主动作
            self.action(randomer, updates, storage, targets);
        }
        // 结束
    }

    pub fn action(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        use crate::player::skill::berserk::BerserkState;

        let smart = self.status.wisdom > randomer.r63() as i32;
        let req_mp = randomer.r15() as i32 + 8;
        let ptr = self.as_ptr();
        let forced_skill = self.skills.pre_action(smart, (ptr, randomer, updates, storage));
        if self.status.frozed() {
            return;
        }

        let mut acted = false;
        let mut selected_skill_key: Option<usize> = forced_skill;
        if self.has_state::<BerserkState>() {
            self.default_attack(false, randomer, updates, storage, &targets.enemy_alive);
            acted = true;
        } else if selected_skill_key.is_none() && self.status.mp >= req_mp {
            self.status.mp -= req_mp;
            let skill_keys = self.skills.skill.clone();
            for key in skill_keys {
                let should_cast = {
                    let skill = self.skills.skill_by_id(key);
                    skill.level() > 0 && skill.has_action_impl() && skill.prob(smart, (ptr, randomer, updates, storage))
                };
                if should_cast {
                    selected_skill_key = Some(key);
                    break;
                }
            }
        }

        if let Some(skill_key) = selected_skill_key {
            let self_candidates = [ptr];
            let selected_targets = {
                let skill = self.skills.skill_by_id(skill_key);
                let candidates: &[PlrId] = match skill.target_domain() {
                    SkillTargetDomain::EnemyAlive => targets.enemy_alive.as_slice(),
                    SkillTargetDomain::AllyAlive => targets.ally_alive.as_slice(),
                    SkillTargetDomain::AllyAny => targets.ally_all.as_slice(),
                    SkillTargetDomain::AllyDead => targets.ally_dead.as_slice(),
                    SkillTargetDomain::SelfOnly => &self_candidates,
                    SkillTargetDomain::AllAlive => targets.all_alive.as_slice(),
                };
                skill.select_targets(candidates, smart, (ptr, randomer, updates, storage))
            };
            if !selected_targets.is_empty() {
                let skill = self.skills.skill_by_id_mut(skill_key);
                skill.act(selected_targets, smart, (ptr, randomer, updates, storage));
                acted = true;
            }
        }

        if !acted {
            self.default_attack(smart, randomer, updates, storage, &targets.enemy_alive);
        }

        let recover_threshold = (self.status.wisdom + 64).clamp(0, 127) as u32;
        if randomer.r127() < recover_threshold {
            self.status.mp += 16;
        }
        updates.add(RunUpdate::new_newline());
        self.skills.post_action((ptr, randomer, updates, storage));
        self.apply_post_action_states(randomer, updates, storage);
    }

    fn pick_target(targets: &[PlrId], randomer: &mut RC4) -> Option<PlrId> { randomer.pick(targets).map(|idx| targets[idx]) }

    fn default_attack(
        &mut self,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &[PlrId],
    ) {
        let Some(target_id) = Self::pick_target(targets, randomer) else {
            return;
        };

        if smart && self.status.magic > self.status.attack {
            let req_mp = (self.status.magic - self.status.attack) >> 2;
            if self.status.mp >= req_mp {
                self.status.mp -= req_mp;
                let atp = self.get_at(true, randomer);
                updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
                storage
                    .just_get_player_mut(target_id)
                    .expect("cannot get default-attack target from storage")
                    .attacked(atp, true, self.as_ptr(), noop_on_damage, randomer, updates, storage);
                return;
            }
        }

        let atp = self.get_at(false, randomer);
        updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
        storage
            .just_get_player_mut(target_id)
            .expect("cannot get default-attack target from storage")
            .attacked(atp, false, self.as_ptr(), noop_on_damage, randomer, updates, storage);
    }

    /// 当前玩家是否可行动
    #[inline]
    pub fn active(&self) -> bool { self.status.hp > 0 && !self.status.frozed() }
    /// 活着呢吧?
    #[inline]
    pub fn alive(&self) -> bool { self.status.alive() }

    #[inline]
    pub fn revive_with_hp(&mut self, hp: i32) {
        self.status.hp = hp.clamp(1, self.status.max_hp.max(1));
        self.status.set_alive(true);
        self.status.set_frozen(false);
    }

    #[inline]
    pub fn set_state<T: StateTrait + 'static>(&mut self, state: T) { self.state.set(state); }

    #[inline]
    pub fn get_state<T: StateTrait + 'static>(&self) -> Option<&T> { self.state.get::<T>() }

    #[inline]
    pub fn get_state_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> { self.state.get_mut::<T>() }

    #[inline]
    pub fn has_state<T: StateTrait + 'static>(&self) -> bool { self.state.has::<T>() }

    #[inline]
    pub fn clear_state<T: StateTrait + 'static>(&mut self) { self.state.clear::<T>(); }

    #[inline]
    pub fn clear_negative_states(&mut self) { self.state.clear_negative_states(); }

    #[inline]
    pub fn clear_positive_states(&mut self) { self.state.clear_positive_states(); }

    pub(super) fn apply_update_state_effects(&mut self) {
        use crate::player::skill::{curse::CurseState, haste::HasteState, ice::IceState, slow::SlowState};

        if let Some(haste) = self.get_state::<HasteState>() {
            self.status.speed *= haste.faster;
        }
        if self.has_state::<SlowState>() {
            self.status.speed /= 2;
        }
        if self.has_state::<CurseState>() {
            self.status.atk_sum *= 4;
        }
        if self.has_state::<IceState>() {
            self.status.set_frozen(true);
        }
    }

    pub(super) fn apply_pre_step_states(&mut self, mut step: i32, updates: &mut RunUpdates) -> i32 {
        use crate::player::skill::ice::IceState;

        let mut clear_ice = false;
        let move_point = self.status.move_point;
        if let Some(ice) = self.get_state_mut::<IceState>()
            && step > 0
        {
            if ice.frozen_step > 0 {
                ice.frozen_step -= step;
                step = 0;
            } else if step + move_point >= MOVE_POINT_THRESHOLD {
                clear_ice = true;
                step = 0;
            }
        }
        if clear_ice {
            self.clear_state::<IceState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[冰冻]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }
        step
    }

    fn apply_post_action_states(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        use crate::player::skill::{
            berserk::BerserkState, charm::CharmState, haste::HasteState, poison::PoisonState, slow::SlowState,
        };

        let mut clear_poison = false;
        let mut clear_haste = false;
        let mut clear_slow = false;
        let mut clear_berserk = false;
        let mut clear_charm = false;
        let mut poison_tick: Option<(PlrId, i32)> = None;
        let magic = self.status.magic;

        if self.alive()
            && let Some(poison) = self.get_state_mut::<PoisonState>()
        {
            let atpp = poison.atp * (1.0 + (poison.count - 1) as f64 * 0.1) / poison.count as f64;
            poison.atp -= atpp;
            let dmg = (atpp / (magic + 64) as f64).ceil() as i32;
            poison.count -= 1;
            clear_poison = poison.count <= 0;
            poison_tick = Some((poison.caster.unwrap_or(self.as_ptr()), dmg));
        }
        if let Some((caster, dmg)) = poison_tick {
            updates.add(RunUpdate::new("[1][毒性发作]", caster, self.as_ptr(), 0));
            self.damage(dmg, caster, noop_on_damage, randomer, updates, storage);
        }
        if clear_poison {
            self.clear_state::<PoisonState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[中毒]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(haste) = self.get_state_mut::<HasteState>() {
            haste.step -= 1;
            clear_haste = haste.step <= 0;
        }
        if clear_haste {
            self.clear_state::<HasteState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[疾走]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(slow) = self.get_state_mut::<SlowState>() {
            slow.step -= 1;
            clear_slow = slow.step <= 0;
        }
        if clear_slow {
            self.clear_state::<SlowState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[迟缓]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(berserk) = self.get_state_mut::<BerserkState>() {
            berserk.step -= 1;
            clear_berserk = berserk.step <= 0;
        }
        if clear_berserk {
            self.clear_state::<BerserkState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[狂暴]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }

        if let Some(charm) = self.get_state_mut::<CharmState>() {
            charm.step -= 1;
            clear_charm = charm.step <= 0;
        }
        if clear_charm {
            self.clear_state::<CharmState>();
            if self.alive() {
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[魅惑]中解除", self.as_ptr(), self.as_ptr(), 0));
            }
        }
    }

    fn apply_pre_defend_states(
        &mut self,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        use crate::player::skill::protect::ProtectState;

        let target_id = self.as_ptr();
        let links = self
            .get_state::<ProtectState>()
            .map(|state| state.protect_from.clone())
            .unwrap_or_default();
        if links.is_empty() {
            return atp;
        }

        let mut stale_owners = Vec::new();
        for link in links {
            let protector_alive = storage.get_player(&link.owner).map(|p| p.alive()).unwrap_or(false);
            if !protector_alive {
                stale_owners.push(link.owner);
                continue;
            }
            if randomer.r127() >= link.level {
                continue;
            }
            let protector_ready = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.mp_ready(randomer)
            };
            if !protector_ready {
                continue;
            }

            updates.add(RunUpdate::new("[0][守护][1]", link.owner, target_id, 40));
            let redirected_atp = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage)
            };
            if redirected_atp == 0.0 {
                return 0.0;
            }
            let mut redirected_dmg = {
                let protector = storage.get_player(&link.owner).expect("cannot get protect owner from storage");
                (redirected_atp * 0.5 / protector.get_df(is_mag) as f64).floor() as i32
            };
            redirected_dmg = {
                let protector = storage.just_get_player_mut(link.owner).expect("cannot get protect owner from storage");
                protector.post_defend(redirected_dmg, caster, on_damage, randomer, updates, storage)
            };
            storage
                .just_get_player_mut(link.owner)
                .expect("cannot get protect owner from storage")
                .damage(redirected_dmg, caster, on_damage, randomer, updates, storage);
            return 0.0;
        }

        if !stale_owners.is_empty() {
            let mut clear_state = false;
            if let Some(state) = self.get_state_mut::<ProtectState>() {
                state.protect_from.retain(|entry| !stale_owners.contains(&entry.owner));
                clear_state = state.protect_from.is_empty();
            }
            if clear_state {
                self.clear_state::<ProtectState>();
            }
        }

        atp
    }

    fn apply_post_defend_states(&mut self, mut dmg: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates) -> i32 {
        use crate::player::skill::{curse::CurseState, shield::ShieldState};

        if let Some(shield) = self.get_state_mut::<ShieldState>() {
            if shield.shield > 0 {
                if dmg > shield.shield {
                    dmg -= shield.shield;
                    shield.shield = 0;
                } else {
                    shield.shield -= dmg;
                    dmg = 0;
                }
            }
        }
        if dmg > 0
            && let Some(curse) = self.get_state::<CurseState>()
            && randomer.r63() < curse.prob as u32
        {
            updates.add(RunUpdate::new("[诅咒]使伤害加倍", caster, self.as_ptr(), 0));
            dmg *= curse.multiply;
        }
        dmg
    }

    /// 蓝条是不是够用
    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        if !self.active() {
            return false;
        }
        let require_mp = randomer.r3x3() as i32;
        if self.status.mp >= require_mp {
            self.status.mp -= require_mp;
            return true;
        }
        false
    }

    // 用于兼容 namerena 的各种名字调用
    #[inline]
    pub fn id_name(&self) -> String { self.name.clone() }
    #[inline]
    pub fn display_name(&self) -> String { self.name.split(" ").next().unwrap_or_default().to_string() }
    #[inline]
    pub fn clan_name(&self) -> String { self.team.clone().unwrap_or(self.name.clone()) }
    #[inline]
    pub fn base_name(&self) -> String { self.name.clone() }

    #[inline]
    pub fn is_seed_plr(&self) -> bool { matches!(self.player_type, PlayerType::Boost) }

    #[inline]
    pub fn cmp_by_id_name(&self, other: &Self) -> std::cmp::Ordering { self.id_name().cmp(&other.id_name()) }

    #[inline]
    pub fn cmp_for_sort(&self, other: &Self) -> std::cmp::Ordering { self.p_cmp(other) }

    pub(super) fn p_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.sort_int.cmp(&other.sort_int) {
            Ordering::Equal => self.id_name().cmp(&other.id_name()),
            ord => ord,
        }
    }

    /// getAt
    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.status.magic } else { self.status.attack };
        let a = {
            let mut temp = [
                randomer.r127() as i32,
                randomer.r127() as i32,
                randomer.r127() as i32,
                atk + 64,
                atk,
            ];
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        a * b * self.status.at_boost
    }

    /// getDf
    pub fn get_df(&self, use_mag: bool) -> i32 {
        if use_mag {
            self.status.resistance + 64
        } else {
            self.status.defense + 64
        }
    }

    pub fn dodge(al_a: i32, al_d: i32, randomer: &mut RC4) -> bool {
        let ch = {
            let temp = 24 + al_d - al_a;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= ch
    }

    /// preDefend
    pub fn pre_defend(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        atp = self.apply_pre_defend_states(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if atp == 0.0 {
            return 0.0;
        }
        self.skills
            .pre_defend(atp, is_mag, caster, on_damage, (self.as_ptr(), randomer, updates, storage))
    }

    /// postDefend
    pub fn post_defend(
        &mut self,
        mut dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        dmg = self.apply_post_defend_states(dmg, caster, randomer, updates);
        self.skills
            .post_defend(dmg, caster, &on_damage, (self.as_ptr(), randomer, updates, storage))
    }

    pub fn attacked(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        atp = self.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if atp == 0.0 {
            return 0;
        }
        let (accure, dodgeval) = {
            let caster_plr = storage.get_player(&caster).expect("faild to get caster player");
            if is_mag {
                (
                    caster_plr.status.magic + caster_plr.status.agility,
                    self.status.resistance + self.status.agility,
                )
            } else {
                (
                    caster_plr.status.attack + caster_plr.status.agility,
                    self.status.defense + self.status.agility,
                )
            }
        };
        if self.active() && Self::dodge(accure, dodgeval, randomer) {
            let update = RunUpdate::new("[0][回避]了攻击", self.as_ptr(), caster, 20);
            updates.add(update);
            return 0;
        }
        self.defned(atp, is_mag, caster, on_damage, randomer, updates, storage)
    }

    pub fn defned(
        &mut self,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let dfp = self.get_df(is_mag);
        let mut dmg = (atp / dfp as f64).ceil() as i32;
        dmg = self.post_defend(dmg, caster, on_damage, randomer, updates, storage);
        self.damage(dmg, caster, on_damage, randomer, updates, storage)
    }

    pub fn damage(
        &mut self,
        dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        if dmg < 0 {
            let _old_hp = self.status.hp;
            self.status.hp -= dmg;
            if self.status.hp > self.status.max_hp {
                self.status.hp = self.status.max_hp;
            }
            let update = RunUpdate::new("[1]回复体力[2]点", caster, self.as_ptr(), dmg.unsigned_abs());
            updates.add(update);
            return 0;
        }
        if dmg == 0 {
            let update = RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", self.as_ptr(), self.as_ptr(), 10);
            updates.add(update);
            return 0;
        }
        let old_hp = self.status.hp;
        self.status.hp -= dmg;
        if self.status.hp < 0 {
            self.status.hp = 0;
        }
        let mut msg = "[0]受到[2]点伤害".to_string();
        if dmg >= 160 {
            msg.push_str("[s_dmg160]");
        } else if dmg >= 120 {
            msg.push_str("[s_dmg120]");
        }
        let mut update = RunUpdate::new(msg, caster, self.as_ptr(), dmg as u32);
        update.delay0 = if dmg > 250 { 1500 } else { 1000 + dmg * 2 };
        updates.add(update);
        on_damage(caster, self.as_ptr(), dmg, randomer, updates);
        self.on_damaged(dmg, old_hp, caster, randomer, updates, storage)
    }

    pub fn on_damaged(
        &mut self,
        dmg: i32,
        old_hp: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let post_damaged_indices: Vec<_> = self.skills.post_damage.to_vec();
        for skill_idx in post_damaged_indices {
            let ptr = self.as_ptr();
            let skill = self.skills.skill_by_id_mut(skill_idx);
            skill.post_damage(dmg, caster, (ptr, randomer, updates, storage));
        }
        if self.status.hp <= 0 {
            if caster != self.as_ptr()
                && let Some(killer) = storage.just_get_player_mut(caster)
            {
                killer.skills.kill(self.as_ptr(), (caster, randomer, updates, storage));
            }
            self.on_die(old_hp, caster, randomer, updates, storage);
            old_hp
        } else {
            dmg
        }
    }

    fn get_die_message(&self) -> &'static str { "[1]被击倒了" }

    pub fn on_die(&mut self, old_hp: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        use crate::player::skill::act::minion::MinionRuntimeState;

        if self.status.hp > 0 {
            return;
        }

        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(self.get_die_message(), caster, self.as_ptr(), 50));

        let ptr = self.as_ptr();
        self.skills.die(old_hp, caster, (ptr, randomer, updates, storage));
        if self.status.hp > 0 {
            return;
        }
        self.status.hp = 0;
        self.status.set_alive(false);

        let owner_id = self.as_ptr();
        let linked_minions = storage
            .all_player_ids()
            .into_iter()
            .filter(|id| *id != owner_id)
            .filter(|id| {
                storage
                    .get_player(id)
                    .and_then(|player| player.get_state::<MinionRuntimeState>())
                    .map(|state| state.owner == Some(owner_id))
                    .unwrap_or(false)
            })
            .collect::<Vec<PlrId>>();
        for minion_id in linked_minions {
            if let Some(minion) = storage.just_get_player_mut(minion_id)
                && minion.alive()
            {
                minion.status.hp = 0;
                minion.status.set_alive(false);
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]消失了", owner_id, minion_id, 30));
            }
            storage.queue_remove_player(minion_id);
        }
    }
}
