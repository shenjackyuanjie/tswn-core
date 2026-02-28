use crate::engine::update::RunUpdate;
use crate::player::{
    PlayerStateStore, PlayerType, PlrId, StateTrait,
    skill::act::minion::{MinionKind, MinionRuntimeState, is_combat_minion},
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
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
        if args.1.r63() >= level {
            return false;
        }
        if args.3.get_player(&target).map(is_combat_minion).unwrap_or(false) {
            return false;
        }
        let Some(owner) = args.3.get_player(&args.0).cloned() else {
            args.3
                .just_get_player_mut(target)
                .expect("cannot get zombie target from storage")
                .set_state(ZombieState {
                    target: Some(target),
                    owner: None,
                });
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
            .set_state(ZombieState {
                target: Some(target),
                owner: Some(args.0),
            });

        let mut zombie = owner.clone();
        zombie.id = args.3.new_plr_id();
        zombie.name = "丧尸".to_string();
        zombie.player_type = PlayerType::Clone;
        zombie.sort_int = 0;
        zombie.state = PlayerStateStore::default();
        zombie.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Zombie,
        });
        zombie.status.attack = 0;
        zombie.status.wisdom = 0;
        zombie.status.max_hp = (owner.get_status().max_hp / 2).max(1);
        zombie.status.hp = zombie.status.max_hp;
        zombie.status.set_alive(true);
        zombie.status.set_frozen(false);
        zombie.status.move_point = args.1.r255() as i32 * 4;
        zombie.skills = crate::player::skill::store::SkillStorage::new();
        zombie.skills.update_proc();
        let zombie_id = zombie.as_ptr();
        args.3.queue_spawn(args.0, zombie);

        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][召唤亡灵]", args.0, target, 60));
        let mut zombied = RunUpdate::new("[2]变成了[1]", args.0, zombie_id, 0);
        zombied.targets.push(target);
        args.2.add(zombied);
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZombieState {
    pub target: Option<PlrId>,
    pub owner: Option<PlrId>,
}

impl StateTrait for ZombieState {
    fn meta_type(&self) -> i32 { 0 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}
