use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlayerStateStore, PlayerType, PlrId,
    skill::store::SkillStorage,
    skill::{Skill, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};
use crate::rc4::RC4;

use super::minion::{MinionKind, MinionRuntimeState};

const SUMMON_SHARE_DAMAGE_SKILL_KEY: usize = 255;

fn ensure_summon_share_damage_skill(skills: &mut SkillStorage, enabled: bool) {
    if !skills.store.contains_key(&SUMMON_SHARE_DAMAGE_SKILL_KEY) {
        skills.store.insert(
            SUMMON_SHARE_DAMAGE_SKILL_KEY,
            Skill::new(1, Box::new(SummonShareDamageSkill::new())),
        );
    }
    skills
        .store
        .get_mut(&SUMMON_SHARE_DAMAGE_SKILL_KEY)
        .expect("summon share-damage skill missing")
        .set_level(if enabled { 1 } else { 0 });
}

#[derive(Debug, Clone, Default)]
pub struct SummonSkill {
    pub summoned: Option<PlrId>,
}

impl SummonSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for SummonSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for SummonSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get summon owner from storage");
            if owner.get_status().hp < 80 {
                return false;
            }
        }
        if let Some(summoned) = self.summoned
            && args.3.get_player(&summoned).map(|p| p.alive()).unwrap_or(false)
        {
            return false;
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        args.2.add(RunUpdate::new("[0]使用[血祭]", args.0, args.0, 60));
        let owner = args.3.get_player(&args.0).expect("cannot get summon owner from storage").clone();
        let charge_active = owner.get_status().at_boost >= 3.0;
        if let Some(summoned_id) = self.summoned
            && let Some(summoned) = args.3.just_get_player_mut(summoned_id)
            && !summoned.alive()
        {
            // JS SklSummon.v(): after the first creation, recasts reuse the same dead summon
            // object and only rerun bP()/bs()/cn(). That means action boosted flags persist
            // across recasts, so the second cast boosts the next still-unboosted action.
            ensure_summon_share_damage_skill(&mut summoned.skills, !charge_active);
            summoned.skills.boost_last();
            summoned.skills.update_proc();
            summoned.init_values();
            summoned.status.set_alive(true);
            summoned.status.move_point = args.1.r255() as i32 * 4;
            if charge_active {
                summoned.status.move_point = 2048;
            }
            args.3.queue_revival(summoned_id);
            args.2.add(RunUpdate::new("召唤出[1]", args.0, summoned_id, 0));
            return;
        }
        let summon_team = owner.clan_name();
        let summon_name = format!("{}?summon", owner.base_name());
        let mut summoned =
            crate::player::Player::new_and_init(Some(summon_team.clone()), summon_name.clone(), None, args.3.clone())
                .expect("cannot init summon minion");
        summoned.build();
        summoned.attr[7] = (summoned.attr[7] / 3).max(1);
        summoned.attr[0] = 0;
        summoned.attr[1] = owner.attr[1];
        summoned.attr[4] = 0;
        summoned.attr[5] = owner.attr[5];
        summoned.update_states();
        summoned.status.hp = summoned.status.max_hp;
        summoned.status.mp = summoned.status.wisdom >> 1;

        summoned.id = args.3.new_plr_id();
        summoned.name = "使魔".to_string();
        summoned.player_type = PlayerType::Clone;
        summoned.sort_int = 0;
        summoned.state = PlayerStateStore::default();
        summoned.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Summon,
        });
        summoned.status.set_alive(true);
        summoned.status.set_frozen(false);

        let skill_level_from_slot = |slot: usize| -> u32 {
            let base = 64 + slot * 4;
            if base + 3 >= summoned.name_base.len() {
                return 0;
            }
            let minv = summoned.name_base[base..base + 4].iter().copied().min().unwrap_or(0);
            minv.saturating_sub(10) as u32
        };
        let mut skill_order = [0usize, 1, 2];
        let team_bytes = [0_u8].iter().chain(summon_team.as_bytes()).copied().collect::<Vec<u8>>();
        let name_bytes = [0_u8].iter().chain(summon_name.as_bytes()).copied().collect::<Vec<u8>>();
        let mut skill_rand = RC4::new(&team_bytes, 1);
        skill_rand.update(&name_bytes, 2);
        skill_rand.sort_list(&mut skill_order);
        let mut skills = SkillStorage::new();
        skills.add_skill(Skill::new_with_id(0, 0));
        skills.add_skill(Skill::new_with_id(0, 0));
        skills.add_skill(Skill::new(0, Box::new(SummonExplodeSkill::new())));
        // JS PlrSummon keeps k1 fixed as [fire, fire, explode] and only shuffles k2.
        // Merge compares the fixed k1 slots, so we must preserve the stable slot keys here
        // and only use `skills.skill` to model the shuffled action order.
        for (slot, skill_key) in skill_order.iter().copied().enumerate() {
            let level = skill_level_from_slot(slot);
            let skill = skills.skill_by_id_mut(skill_key);
            skill.set_level(level);
            // JS Plr.dm(): if computed level > 0, check the *original* (raw) hash;
            // if raw min - 10 <= 0, mark skill as already boosted so boost_last skips it.
            if level > 0 {
                let raw_base = 64 + slot * 4;
                if raw_base + 3 < summoned.raw_name_base.len() {
                    let raw_min = summoned.raw_name_base[raw_base..raw_base + 4].iter().copied().min().unwrap_or(0);
                    if raw_min <= 10 {
                        skill.boosted = true;
                    }
                }
            }
        }
        skills.skill = skill_order.to_vec();
        ensure_summon_share_damage_skill(&mut skills, !charge_active);
        skills.boost_last();
        summoned.skills = skills;
        summoned.skills.update_proc();

        // JS: this_.fr.l = a8.n() * 4 (无条件消耗 r255)
        // 然后如果 charge: this_.fr.l = 2048 (覆盖)
        summoned.status.move_point = args.1.r255() as i32 * 4;
        if charge_active {
            summoned.status.move_point = 2048;
        }
        let summoned_id = summoned.as_ptr();
        self.summoned = Some(summoned_id);
        args.3.queue_spawn(args.0, summoned);
        args.2.add(RunUpdate::new("召唤出[1]", args.0, summoned_id, 0));
    }
}

#[derive(Debug, Clone, Default)]
struct SummonExplodeSkill;

impl SummonExplodeSkill {
    fn new() -> Self { Self }
}

impl SkillTrait for SummonExplodeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let fire_mag = args
            .3
            .get_player(&target_id)
            .expect("cannot get summon explode target from storage")
            .get_state::<super::fire::FireState>()
            .map(|state| state.fire_mag)
            .unwrap_or(0.0);
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get summon explode owner from storage")
            .get_at(true, args.1)
            * (4.0 + fire_mag);
        args.2.add(RunUpdate::new("[0]使用[自爆]", args.0, target_id, 0));
        let old_hp = {
            let owner = args
                .3
                .just_get_player_mut(args.0)
                .expect("cannot get mutable summon explode owner from storage");
            let old_hp = owner.get_status().hp;
            owner.status.hp = 0;
            old_hp
        };
        let _dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get mutable summon explode target from storage")
            .attacked(atp, true, args.0, super::fire::on_fire as OnDamageFunc, args.1, args.2, args.3);
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get mutable summon explode owner from storage")
            .on_die(old_hp, args.0, args.1, args.2, args.3);
    }
}

#[derive(Debug, Clone, Default)]
struct SummonShareDamageSkill;

impl SummonShareDamageSkill {
    fn new() -> Self { Self }
}

impl SkillTrait for SummonShareDamageSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        let owner_id = args
            .3
            .get_player(&args.0)
            .and_then(|player| player.get_state::<MinionRuntimeState>())
            .and_then(|state| state.owner);
        let Some(owner_id) = owner_id else {
            return;
        };
        // JS PlrSummon.aR: 在伤害分摊期间标记使魔，
        // 防止 owner 死亡时通过 linked minion 路径立即处理使魔的死亡。
        args.3.set_in_post_damage(args.0);
        if let Some(owner) = args.3.just_get_player_mut(owner_id) {
            owner.damage(dmg / 2, caster, on_summon_share_damage as OnDamageFunc, args.1, args.2, args.3);
        }
        args.3.clear_in_post_damage();
    }

    fn proc_kinds(&self) -> &[crate::player::skill::ProcKind] { &[crate::player::skill::ProcKind::PostDamage] }
}

fn on_summon_share_damage(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut RC4,
    _updates: &mut RunUpdates,
    _storage: &Arc<Storage>,
) {
}
