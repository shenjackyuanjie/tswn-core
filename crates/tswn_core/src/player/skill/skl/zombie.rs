//! `召唤亡灵` 被动技能实现。
//!
//! 本模块负责在击杀后把目标标记为丧尸尸体，并按需要生成可作战的丧尸召唤物。

use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlayerStateStore, PlayerType, PlrId,
    skill::act::minion::{
        MinionKind, MinionRuntimeState, alloc_minion_name, apply_child_minion_overlay, apply_minion_attrs,
        apply_minion_skill_overlay, is_combat_minion, owner_minion_overlay, prepare_combat_minion,
    },
    skill::corpse::CorpseState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait, store::SkillStorage},
};

#[derive(Debug, Clone, Default)]
pub struct ZombieSkill;

impl ZombieSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ZombieSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ZombieSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { self.kill_with_level(32, target, args) }

    fn kill_with_level(&mut self, level: u32, target: PlrId, args: SkillArgs) -> bool {
        if args.3.get_player(&target).map(is_combat_minion).unwrap_or(false) {
            return false;
        }
        if args.1.r63() >= level {
            return false;
        }
        let Some(owner_clan) = args.3.get_player(&args.0).map(|owner| owner.clan_name()) else {
            args.3
                .just_get_player_mut(target)
                .expect("cannot get zombie target from storage")
                .set_state(CorpseState::zombie());
            return true;
        };
        if !args
            .3
            .just_get_player_mut(args.0)
            .expect("cannot get zombie owner from storage")
            .mp_ready(args.1)
        {
            return false;
        }

        args.3
            .just_get_player_mut(target)
            .expect("cannot get zombie target from storage")
            .set_state(CorpseState::zombie());

        let minion_overlay = owner_minion_overlay(args.3, args.0, MinionKind::Zombie);
        let seed_name = format!(
            "{}?zombie",
            args.3.get_player(&args.0).expect("cannot get zombie owner").base_name()
        );
        let mut zombie =
            Player::new_minion_and_init(Some(owner_clan), seed_name, None, args.3.clone()).expect("cannot init zombie minion");
        prepare_combat_minion(&mut zombie);
        zombie.build();
        zombie.id = args.3.new_plr_id();
        zombie.set_id_name_override(Some(alloc_minion_name(args.3, args.0)));
        zombie.set_display_name_override(Some("丧尸".to_string()));
        if !apply_minion_attrs(&mut zombie, minion_overlay.as_ref()) {
            zombie.attr[0] = 0;
            zombie.attr[6] = 0;
            zombie.attr[7] = (zombie.attr[7] >> 1).max(1);
        }
        apply_child_minion_overlay(&mut zombie, minion_overlay.as_ref());
        zombie.init_values();
        zombie.player_type = PlayerType::Clone;
        zombie.sort_int = 0;
        zombie.state = PlayerStateStore::default();
        zombie.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Zombie,
            share_damage_owner: None,
        });
        zombie.status.set_alive(true);
        zombie.status.set_frozen(false);
        zombie.status.move_point = args.1.r255() as i32 * 4;
        if !apply_minion_skill_overlay(&mut zombie, minion_overlay.as_ref()) {
            zombie.skills = SkillStorage::new();
            zombie.skills.update_proc();
        }
        let zombie_id = zombie.as_ptr();
        args.3.queue_spawn(args.0, zombie);

        args.2.add(RunUpdate::new_newline());
        let mut summon_update = RunUpdate::new("[0][召唤亡灵]", args.0, target, 60);
        summon_update.delay0 = 1500;
        args.2.add(summon_update);
        let mut zombied = RunUpdate::new("[2]变成了[1]", args.0, zombie_id, 0);
        zombied.targets.push(target);
        args.2.add(zombied);
        true
    }

    fn proc_kinds(&self) -> &'static [ProcKind] { &[ProcKind::PostKill] }
}
