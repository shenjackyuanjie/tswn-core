use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::act::charm::CharmState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use smallvec::SmallVec;

#[derive(Debug, Clone, Default)]
pub struct HideSkill {
    pub on_pre_action: Option<()>,
    pub on_update_state: Option<()>,
}

impl HideSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for HideSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HideSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        let _ = (dmg, caster);
        if level == 0 || self.on_update_state.is_some() {
            return;
        }
        let owner_active = args.3.get_player(&args.0).map(|x| x.active()).unwrap_or(false);
        let (alive_group_snapshot, effective_group_snapshot, owner_charmed) =
            args.3.get_player(&args.0).map_or((None, None, false), |owner| {
                owner
                    .get_state::<CharmState>()
                    .map(|charm| {
                        (
                            charm
                                .effective_team_idx
                                .and_then(|team_idx| args.3.alive_group_at(team_idx))
                                .or_else(|| args.3.alive_group_at_team_of(charm.group_id)),
                            charm
                                .effective_team_idx
                                .and_then(|team_idx| args.3.get_group(team_idx))
                                .or_else(|| args.3.group_containing(charm.group_id)),
                            true,
                        )
                    })
                    .unwrap_or_else(|| (args.3.alive_group_at_team_of(args.0), args.3.group_containing(args.0), false))
            });
        let mut alive_candidates: SmallVec<[PlrId; 8]> = SmallVec::new();
        if let Some(group) = alive_group_snapshot {
            alive_candidates.extend(
                group
                    .iter()
                    .copied()
                    .filter(|id| args.3.get_player(id).map(|p| p.alive()).unwrap_or(false)),
            );
        }
        // JS 的同队 alive 视图会在同一 action 的后半段立刻看到：
        // 1. owner 当前 action 内刚 addNew 出来的 pending spawn
        // 2. 已经 queue_revival、但还没 sync 回 alive_group 的旧成员
        // 否则像 f250 里 Mira 先复活 Light、随后 poison/post_damage 再打回自己时，
        // Hide 的 r63 检定会少吃 1 byte。
        //
        // 这里不能直接把 roster 里所有 `alive()==true` 的成员都补进来；storage 里还可能
        // 暂留一些“状态已变但并非 JS 同拍可见”的实体，那会把 Hide 的触发窗口放大，反而引入新 diff。
        for id in effective_group_snapshot
            .into_iter()
            .flat_map(|group| args.3.iter_pending_revival_ids_for_group(group))
        {
            if !alive_candidates.contains(&id) && args.3.get_player(&id).map(|p| p.alive()).unwrap_or(false) {
                alive_candidates.push(id);
            }
        }
        if !owner_charmed {
            for id in args.3.iter_pending_spawn_ids_for_owner(args.0) {
                if !alive_candidates.contains(&id) && args.3.get_pending_spawn_player(id).map(|p| p.alive()).unwrap_or(false) {
                    alive_candidates.push(id);
                }
            }
        }
        let alive_allies = alive_candidates.len();
        if owner_active && alive_allies > 1 && args.1.r63() < level {
            self.on_update_state = Some(());
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get hide owner from storage")
                .update_states();
            args.2.add(RunUpdate::new("[0]发动[隐匿]", args.0, args.0, 10));
        }
    }

    fn pre_action(&mut self, args: SkillArgs) {
        if self.on_update_state.is_some() {
            self.on_update_state = None;
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get hide owner from storage")
                .update_states();
        }
    }

    fn pre_action_clear_forced(&mut self, _smart: bool, args: SkillArgs) -> bool {
        args.3
            .get_player(&args.0)
            .map(|owner| owner.has_state::<crate::player::skill::berserk::BerserkState>())
            .unwrap_or(false)
    }

    fn pre_action_accumulate_with_level(
        &mut self,
        _level: u32,
        _current_forced: Option<usize>,
        _self_key: usize,
        _smart: bool,
        args: SkillArgs,
    ) -> Option<usize> {
        self.pre_action(args);
        None
    }

    fn update_state_with_level(&mut self, level: u32, args: SkillArgs) {
        if self.on_update_state.is_none() {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get hide owner from storage");
        owner.mul_attract(0.10000000149011612);
        if level > 63 {
            let boost = (level - 63) as i32;
            owner.add_agility(boost);
            owner.add_defense(boost);
            owner.add_resistance(boost);
        }
    }

    fn update_state_inline(&mut self, level: u32, status: &mut crate::player::PlayerStatus) {
        if self.on_update_state.is_none() {
            return;
        }
        status.attract *= 0.10000000149011612;
        if level > 63 {
            let boost = (level - 63) as i32;
            status.agility += boost;
            status.defense += boost;
            status.resistance += boost;
        }
    }

    fn dynamic_update_state_enabled(&self) -> bool { self.on_update_state.is_some() }

    fn proc_kinds(&self) -> &'static [ProcKind] { &[ProcKind::PostDamage, ProcKind::PreAction, ProcKind::UpdateState] }
}
