use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::act::charm::CharmState,
    skill::{InlineCtx, ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use smallvec::SmallVec;

#[derive(Debug, Clone, Default)]
pub struct HideSkill {
    pub on_pre_action: Option<()>,
    pub on_update_state: Option<()>,
}

impl HideSkill {
    pub fn new() -> Self { Self::default() }

    fn count_alive_allies(
        owner_id: PlrId,
        owner_charm: Option<(PlrId, Option<usize>)>,
        storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> usize {
        let (alive_group_snapshot, effective_group_snapshot) = owner_charm
            .map(|(group_id, effective_team_idx)| {
                (
                    effective_team_idx
                        .and_then(|team_idx| storage.alive_group_at(team_idx))
                        .or_else(|| storage.alive_group_at_team_of(group_id)),
                    effective_team_idx
                        .and_then(|team_idx| storage.get_group(team_idx))
                        .or_else(|| storage.group_containing(group_id)),
                )
            })
            .unwrap_or_else(|| (storage.alive_group_at_team_of(owner_id), storage.group_containing(owner_id)));

        let mut alive_candidates: SmallVec<[PlrId; 8]> = SmallVec::new();
        if let Some(group) = alive_group_snapshot {
            alive_candidates.extend(
                group
                    .iter()
                    .copied()
                    .filter(|id| storage.get_player(id).map(|player| player.alive()).unwrap_or(false)),
            );
        }

        for id in effective_group_snapshot
            .into_iter()
            .flat_map(|group| storage.iter_pending_revival_ids_for_group(group))
        {
            if !alive_candidates.contains(&id) && storage.get_player(&id).map(|player| player.alive()).unwrap_or(false) {
                alive_candidates.push(id);
            }
        }
        for id in storage.iter_pending_spawn_ids_for_owner(owner_id) {
            if !alive_candidates.contains(&id) && storage.get_pending_spawn_player(id).map(|player| player.alive()).unwrap_or(false)
            {
                alive_candidates.push(id);
            }
        }
        alive_candidates.len()
    }

    fn trigger_from_damage(
        &mut self,
        level: u32,
        owner_id: PlrId,
        owner_active: bool,
        owner_charm: Option<(PlrId, Option<usize>)>,
        randomer: &mut crate::rc4::RC4,
        updates: &mut crate::engine::update::RunUpdates,
        storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        if level == 0 || self.on_update_state.is_some() {
            return false;
        }

        let alive_allies = Self::count_alive_allies(owner_id, owner_charm, storage);
        if owner_active && alive_allies > 1 && randomer.r63() < level {
            self.on_update_state = Some(());
            updates.add(RunUpdate::new("[0]发动[隐匿]", owner_id, owner_id, 10));
            return true;
        }
        false
    }
}

impl SkillExt for HideSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HideSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        let _ = (dmg, caster);
        // JS 的同队 alive 视图会在同一 action 的后半段立刻看到：
        // 1. owner 当前 action 内刚 addNew 出来的 pending spawn
        // 2. 已经 queue_revival、但还没 sync 回 alive_group 的旧成员
        // 否则像 f250 里 Mira 先复活 Light、随后 poison/post_damage 再打回自己时，
        // Hide 的 r63 检定会少吃 1 byte。
        //
        // 这里不能直接把 roster 里所有 `alive()==true` 的成员都补进来；storage 里还可能
        // 暂留一些“状态已变但并非 JS 同拍可见”的实体，那会把 Hide 的触发窗口放大，反而引入新 diff。
        let owner = args.3.get_player(&args.0);
        let owner_active = owner.map(|player| player.active()).unwrap_or(false);
        let owner_charm = owner.and_then(|player| {
            player
                .get_state::<CharmState>()
                .map(|charm| (charm.group_id, charm.effective_team_idx))
        });
        if self.trigger_from_damage(level, args.0, owner_active, owner_charm, args.1, args.2, args.3) {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get hide owner from storage")
                .update_states();
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

    fn has_inline_pre_action(&self) -> bool { true }

    fn pre_action_inline(&mut self, ctx: &mut InlineCtx) {
        if self.on_update_state.is_some() {
            self.on_update_state = None;
            ctx.mark_update_states();
        }
    }

    fn has_inline_post_damage(&self) -> bool { true }

    fn post_damage_inline(&mut self, level: u32, ctx: &mut InlineCtx) {
        let owner_charm = ctx
            .owner
            .get_state::<CharmState>()
            .map(|charm| (charm.group_id, charm.effective_team_idx));
        if self.trigger_from_damage(level, ctx.ptr, ctx.owner.active(), owner_charm, ctx.randomer, ctx.updates, ctx.storage)
            || self.on_update_state.is_some()
        {
            ctx.mark_update_states();
        }
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

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDamage, ProcKind::PreAction, ProcKind::UpdateState] }
}
