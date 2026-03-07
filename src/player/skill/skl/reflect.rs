use crate::engine::update::RunUpdate;
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ReflectSkill;

impl ReflectSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ReflectSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReflectSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn pre_defend_with_level(
        &mut self,
        level: u32,
        atp: f64,
        caster: PlrId,
        _is_mag: bool,
        on_damage: &OnDamageFunc,
        args: SkillArgs,
    ) -> f64 {
        let debug_reflect = std::env::var("TSWN_DEBUG_REFLECT").is_ok();
        // JS: if (caster.dead) return atp; dead 仅由 hp<=0 判定。
        if args.3.get_player(&caster).map(|p| p.get_status().hp <= 0).unwrap_or(true) {
            return atp;
        }
        if debug_reflect {
            let owner_name = args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_default();
            let caster_name = args.3.get_player(&caster).map(|p| p.id_name()).unwrap_or_default();
            eprintln!(
                "[reflect] start owner={owner_name} caster={caster_name} atp={atp} level={level} rc4=({}, {})",
                args.1.i, args.1.j
            );
        }
        let reflect_atp = {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get reflect owner from storage");
            if args.1.r255() >= level || !args.1.c50() || !owner.mp_ready(args.1) {
                if debug_reflect {
                    eprintln!("[reflect] check failed rc4=({}, {})", args.1.i, args.1.j);
                }
                return atp;
            }
            if debug_reflect {
                eprintln!("[reflect] checks passed rc4=({}, {})", args.1.i, args.1.j);
            }
            let mut reflect_atp = owner.get_at(true, args.1) * 0.5;
            if debug_reflect {
                eprintln!(
                    "[reflect] after get_at reflect_atp={reflect_atp} rc4=({}, {})",
                    args.1.i, args.1.j
                );
            }
            if reflect_atp > atp {
                reflect_atp = atp;
            }
            owner.set_move_point(owner.move_point() - 480);
            reflect_atp
        };
        args.2.add(RunUpdate::new("[0]使用[伤害反弹]", args.0, caster, 20));
        if debug_reflect {
            eprintln!(
                "[reflect] before reflected_attacked reflect_atp={reflect_atp} rc4=({}, {})",
                args.1.i, args.1.j
            );
        }
        args.3
            .just_get_player_mut(caster)
            .expect("cannot get reflect caster from storage")
            .attacked(reflect_atp, true, args.0, *on_damage, args.1, args.2, args.3);
        if debug_reflect {
            eprintln!("[reflect] after reflected_attacked rc4=({}, {})", args.1.i, args.1.j);
        }
        0.0
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PreDefend] }
}
