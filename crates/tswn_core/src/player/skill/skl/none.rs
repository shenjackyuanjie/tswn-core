use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

/// 真的就是啥都没有啊喂
#[derive(Debug, Clone, Default)]
pub struct NoneSkill;

impl NoneSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for NoneSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(NoneSkill::new()) }
}

impl SkillTrait for NoneSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    /// 对齐 Dart `SklVoid`: act 直接 return。
    fn act(&mut self, _targets: &[PlrId], _smart: bool, _args: SkillArgs) {}

    /// 占位技能，不应被当作正常技能参与流程。
    fn is_normal_skill(&self) -> bool { false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::storage::Storage;
    use crate::engine::update::RunUpdates;
    use crate::player::OnDamageFunc;
    use crate::rc4::RC4;

    fn dummy_on_damage(
        _caster: PlrId,
        _target: PlrId,
        _dmg: i32,
        _r: &mut RC4,
        _updates: &mut RunUpdates,
        _storage: &std::sync::Arc<Storage>,
    ) {
    }

    #[test]
    fn none_skill_is_noop() {
        let storage = Storage::new_arc();
        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();
        let mut skill = NoneSkill::new();

        skill.act(&[1], true, (0, &mut randomer, &mut updates, &storage));
        assert!(updates.updates.is_empty());

        let atp = skill.pre_defend(
            123.5,
            1,
            true,
            &(dummy_on_damage as OnDamageFunc),
            (0, &mut randomer, &mut updates, &storage),
        );
        assert_eq!(atp, 123.5);

        let dmg = skill.post_defend(
            42,
            1,
            &(dummy_on_damage as OnDamageFunc),
            (0, &mut randomer, &mut updates, &storage),
        );
        assert_eq!(dmg, 42);

        assert!(!skill.is_normal_skill());
    }
}
