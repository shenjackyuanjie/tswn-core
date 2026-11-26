//! 蓄力与聚气技能 handler：行动后推进蓄力步数，或激活聚气运行时并播报消息。

use super::*;

/// 蓄力技能的行动后处理：推进蓄力步数，归零时撤销倍率。
pub fn run_charge_post_action_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.tick_owner_charge_post_action().expect("charge post_action owner should exist");
}

/// 聚气技能：激活聚气运行时并播报开始与攻击力上升两条消息。
pub fn run_accumulate_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    if !context.activate_owner_accumulate_runtime().expect("accumulate owner should exist") {
        return;
    }

    let owner = context.owner_idx();
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]开始[聚气]",
        owner.0 as usize,
        owner.0 as usize,
        1,
    ));
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]攻击力上升",
        owner.0 as usize,
        owner.0 as usize,
        0,
    ));
}
