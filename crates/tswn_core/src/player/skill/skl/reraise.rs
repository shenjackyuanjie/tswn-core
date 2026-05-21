use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ReraiseSkill {
    pub sort_id: f64,
}

impl Default for ReraiseSkill {
    fn default() -> Self { Self { sort_id: 10.0 } }
}

impl ReraiseSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ReraiseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReraiseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn die_with_level(&mut self, level: &mut u32, _oldhp: i32, _caster: PlrId, args: SkillArgs) -> bool {
        if args.1.r127() >= *level {
            return false;
        }
        // Dart: level = (level+1) ~/ 2
        *level = (*level).div_ceil(2);
        let hp = args.1.r16() as i32;
        let mut reraise_update = RunUpdate::new("[0]使用[护身符]抵挡了一次死亡", args.0, args.0, 80);
        reraise_update.delay0 = 1500;
        args.2.add(reraise_update);
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get reraise owner from storage")
            .revive_with_hp(hp);
        // JS 的 SklReraise.b1() 只会在当前死亡链里直接回 hp，不会再额外排一次 revive/sync。
        // 这里按这个语义保持不调用 queue_revival，并且已经重新跑过全量 1w2 case
        // (1v1, 2v2, 3v3v3, ffa)：没有引入回归，但也没有额外修掉剩余 failed case。
        // 除非后面拿到新的 JS 对照证据，证明 reraise 之后还会再走一次独立 revival sync，
        // 否则不要轻易把 queue_revival 加回来。
        // args.3.queue_revival(args.0);
        let mut recover_update = RunUpdate::new("[1]回复体力[2]点", args.0, args.0, 0);
        recover_update.param = Some(hp as u32);
        args.2.add(recover_update);
        true
    }

    fn proc_kinds(&self) -> &'static [ProcKind] { &[ProcKind::PostDeath] }
}
