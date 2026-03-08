//! # 规则注册表 (rules)
//!
//! 本模块提供 [`RuleRegistry`]，目前作为规则数量的统计容器使用。
//!
//! ## 设计说明
//!
//! `RuleRegistry` 用于统计已注册的技能规则、武器规则和 Boss 规则数量，
//! 未来可扩展为真正的规则分发中心（如按 tag 查找技能处理器等）。
//! 目前引擎在几乎所有实际逻辑中并不依赖此结构，主要用于系统初始化计数。

/// 规则注册计数器。
/// 记录已注册的技能/武器/Boss 规则数量，可用于初始化检查。
#[derive(Default)]
pub struct RuleRegistry {
    pub skill_rules: usize,
    pub weapon_rules: usize,
    pub boss_rules: usize,
}

impl RuleRegistry {
    pub fn register_skill_rule(&mut self) {
        self.skill_rules += 1;
    }

    pub fn register_weapon_rule(&mut self) {
        self.weapon_rules += 1;
    }

    pub fn register_boss_rule(&mut self) {
        self.boss_rules += 1;
    }
}
