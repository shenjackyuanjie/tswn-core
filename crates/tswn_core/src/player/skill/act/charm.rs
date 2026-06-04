//! 魅惑主动技能实现。
//!
//! 维护 `CharmState`，对目标施加魅惑效果，使其在下一行动时攻击己方队友。

use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct CharmSkill;

impl CharmSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CharmSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CharmSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if !smart {
            return true;
        }
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        if let Some(charm) = target_plr.get_state::<CharmState>() {
            charm.step <= 1
        } else {
            true
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            let alive_group_count = args.3.alive_group_count();
            let target_alive_group_len = args.3.alive_group_len_containing(target);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_hi_hp(status.hp) * status.attr_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.has_state::<CharmState>() || target_plr.has_state::<crate::player::skill::berserk::BerserkState>() {
            score /= 2.0;
        }
        score
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[魅惑]", args.0, target_id, 1));

        let (owner_magic, charge_active) = {
            let owner = args.3.get_player(&args.0).expect("cannot get charm owner from storage");
            (owner.get_status().magic, owner.get_status().at_boost >= 3.0)
        };
        // Dart 比较的是 owner.allyGroup（队伍对象）与 charmState.grp（队伍对象）。
        // Rust 在 group_id 中保留魅惑来源玩家 ID，但还需要记录已解析出的
        // 有效队伍，避免连锁魅惑时又坍缩回来源玩家的原始队伍。
        let caster_effective_team_idx = {
            let owner = args.3.get_player(&args.0).expect("cannot get charm owner from storage");
            if let Some(caster_charm) = owner.get_state::<CharmState>() {
                caster_charm.effective_team_idx.or_else(|| args.3.group_index_of(caster_charm.group_id))
            } else {
                args.3.group_index_of(args.0)
            }
        };
        let target = args.3.just_get_player_mut(target_id).expect("cannot get charm target from storage");
        if target.check_immune("charm", args.1)
            || (target.active()
                && Player::dodge(
                    owner_magic,
                    target.get_status().agility + target.get_status().resistance,
                    args.1,
                ))
        {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        if let Some(state) = target.get_state_mut::<CharmState>() {
            // JS 的 recharm 逻辑：
            //   s = caster.z          (caster 当前 allyGroup — 即 caster 的有效队伍)
            //   if (s != charm.r) charm.r = s   (不同队 → 替换 .r)
            //   else charm.z += 1               (同队 → 叠层数)
            //
            // 关键：JS 只更新 charm.r（用于下次 recharm 比较和 ar() 源），
            // 但 player.z（实际用于 target selection 的缓存）只在 F() 中由 ar() 设置，
            // 而 recharm 不会触发 F()。因此 recharm 后 player.z 仍然是首次 charm 时的 group。
            //
            // Rust 对应：
            // - group_id 对应 JS 的 charm.r 中存储的 caster PlrId（用于 select_targets 兜底）
            // - source_team_idx 对应 JS 的 charm.r 解析后的 team index（用于 recharm 比较）
            //   当 caster 本身被 charm 时，group_index_of(group_id) 返回 caster 原始队伍，
            //   而 JS 的 charm.r 存储的是 caster 的有效队伍。source_team_idx 修复了这个差异。
            // - effective_team_idx 对应 JS 的 player.z（用于 select_targets，首次 charm 后不变）
            // - recharm 更新 group_id 和 source_team_idx，但不更新 effective_team_idx，
            //   使 select_targets 继续使用首次 charm 时缓存的 team index。
            let existing_team_idx = state.source_team_idx.or_else(|| args.3.group_index_of(state.group_id));
            if existing_team_idx == caster_effective_team_idx {
                state.step += 1;
            } else {
                state.group_id = args.0;
                state.source_team_idx = caster_effective_team_idx;
            }
            // 不更新 effective_team_idx — 它保持首次 charm 时由 set_state → update_states 设定的值，
            // 与 JS 中 F()/ar() 只在 aP() 时运行一次、recharm 不触发 F() 的行为一致。
            if charge_active {
                state.step += 3;
            }
        } else {
            target.set_state(CharmState {
                group_id: args.0,
                effective_team_idx: caster_effective_team_idx,
                source_team_idx: caster_effective_team_idx,
                target: Some(target_id),
                on_post_action: None,
                step: if charge_active { 4 } else { 1 },
            });
        }
        args.2.add(RunUpdate::new("[1]被[魅惑]了", args.0, target_id, 120));
    }
}

/// 魅惑状态。
///
/// 字段语义对照 JS (`md5.js`)：
/// - `group_id`          → JS `charm.r` 中的 caster PlrId（select_targets 兜底用）
/// - `source_team_idx`   → JS `charm.r` 解析后的有效 team index（recharm 同队/异队判定用）
/// - `effective_team_idx` → JS `player.z`（被 charm 后的有效队伍，用于 select_targets，首次设置后不变）
/// - `step`              → JS `charm.z`（剩余回合数）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharmState {
    pub group_id: usize,
    pub effective_team_idx: Option<usize>,
    /// 上一次 charm/recharm 时 caster 的有效 team index，用于 recharm 时判断"同队叠层 vs 异队替换"。
    /// 对应 JS 中 `charm.r` 存储的 group 引用的 team index。
    /// 当 caster 本身被 charm 时，其有效队伍可能与原始队伍不同，
    /// 因此不能用 `group_index_of(group_id)` 代替。
    pub source_team_idx: Option<usize>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl StateTrait for CharmState {
    fn meta_type(&self) -> i32 { -1 }

    // JS 中 CharmState 和 SlowState 都通过 PostActionImpl 包装，ga4() = Infinity，
    // 即同优先级，实际执行顺序由注册时机决定。Rust 侧必须与 SlowState 保持同层(210)，
    // 否则会出现固定的消息顺序反转（如"从魅惑中解除"与"从迟缓中解除"顺序不一致）。
    fn post_action_priority(&self) -> i32 { 210 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut crate::rc4::RC4,
        updates: &mut crate::engine::update::RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        self.step -= 1;
        if self.step > 0 {
            return false;
        }
        if alive {
            updates.add_newline();
            updates.emit(|| RunUpdate::new("[1]从[魅惑]中解除", owner, owner, 0));
        }
        true
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}
