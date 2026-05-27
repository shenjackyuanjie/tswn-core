//! # 被动/防御技能 (skl)
//!
//! 本模块定义所有被动、防御与占位技能，共 12 种。
//!
//! 这些技能通常挂在防御前后、伤害后、死亡后或击杀后等阶段，不会像主动技能那样直接参与
//! “选择一个动作”的流程。技能触发顺序由 [`SkillStorage`](crate::player::skill::store::SkillStorage)
//! 在注册阶段整理。
//!
//! ## 技能列表
//!
//! | 技能名称 | 说明 |
//! | --- | --- |
//! | `corpse` | 尸体状态，供融合、丧尸等死亡后逻辑读取 |
//! | `counter` | 反击，受击后按概率回击攻击者 |
//! | `defend` | 防御，受击前后减少最终伤害 |
//! | `hide` | 隐匿，影响敌方目标选择并在特定行为后解除 |
//! | `merge` | 融合，死亡时吞并尸体并尝试复生 |
//! | `none` | 无技能，占位空技能槽 |
//! | `protect` | 保护，替队友承受或抵消致命伤害 |
//! | `reflect` | 反射，将受到的部分伤害反弹给攻击者 |
//! | `reraise` | 被动复活，死亡时按规则恢复生命 |
//! | `shield` | 护盾，优先消耗护盾值抵扣伤害 |
//! | `upgrade` | 升级，生命阈值触发后的永久属性强化 |
//! | `zombie` | 召唤亡灵，击杀后标记尸体并生成丧尸召唤物 |
//!
//! ## 维护边界
//!
//! 本模块只声明和分组子模块。技能自己的状态结构、随机数消耗和消息模板应留在对应文件中。

pub mod corpse;
pub mod counter;
pub mod defend;
pub mod hide;
pub mod merge;
pub mod none;
pub mod protect;
pub mod reflect;
pub mod reraise;
pub mod shield;
pub mod upgrade;
pub mod zombie;
