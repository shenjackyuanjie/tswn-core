//! 实验体 Boss 的阶段计数与本回合待执行招式。

/// 实验体在概率阶段选中的招式；普攻（未选中任何技能）时为 `None`。
///
/// 概率阶段与结算阶段分开：`scan_plain_boss_action_skills` 先按阶段抽签并记录
/// 待执行招式，`drain_plain_boss_skill_into` 随后原样读取，避免同一批随机字节被
/// 抽两次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TestSubjectSkill {
    /// 颅骨重击：全体敌人 80% 物理，有效伤害附加易伤。
    SkullSmash,
    /// 多段爪击：60% 物理 ×（该技能已用次数 + 3），累计伤害为正时附加迟缓。
    MultiClaw,
    /// 割裂：60% 物理 × 3。
    Rupture,
    /// 大猛扑：225% 物理。
    GreatPounce,
    /// 燃烧咆哮：全体敌人 80% 魔法，命中附加灼烧。
    BurningRoar,
}

/// 实验体的阶段计数。
///
/// `revives_used` 0/1/2 分别对应一/二/三阶段；复活时清零后两个计数，
/// 使每阶段都从初始倍率与初始段数重新开始。
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestSubjectRuntime {
    /// 已使用的异常再生次数，同时决定当前阶段。
    pub revives_used: i32,
    /// 当前阶段已行动次数，驱动一阶段普攻与颅骨重击的倍率爬升。
    pub phase_action_count: i32,
    /// 多段爪击在当前阶段已使用次数，决定下一发的段数。
    pub multi_claw_used: i32,
    /// 本回合概率阶段选中的招式；普攻为 `None`。
    pub pending_skill: Option<TestSubjectSkill>,
}

impl TestSubjectRuntime {
    /// 当前阶段序号，从 1 起算。
    pub fn phase(&self) -> i32 { self.revives_used + 1 }

    /// 一阶段伤害倍率：`1.0 + 0.1 × 本阶段已行动次数`。
    pub fn phase_one_damage_scale(&self) -> f64 { 1.0 + self.phase_action_count as f64 * 0.1 }
}
