use crate::player::{
    PlrPtr,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct FireSkill {
    pub fire_mag: f64,
}

impl FireSkill {
    pub fn new() -> Self { Self { fire_mag: 0.0 } }
}

impl SkillExt for FireSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(FireSkill::new()) }
}

impl SkillTrait for FireSkill {
    fn destroy(&self, plr: PlrPtr, args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn act(&mut self, targets: Vec<PlrPtr>, smart: bool, args: SkillArgs) {
        let target_ptr = targets[0];
        let target = args.3.just_get_player_mut(target_ptr).expect("cannot get player in the storage");
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get owner from storage");
        let atp = owner.get_at(true, args.1) * (1.5 + self.fire_mag);
        
    }
}
