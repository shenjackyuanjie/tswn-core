//! # 主动技能 (act)
//!
//! 本模块定义所有主动技能，共 27 种。每个正式主动技能都在独立文件中实现对应的
//! [`SkillTrait`](crate::player::skill::SkillTrait) 钩子，并通过技能 id / 名称注册到上层分发逻辑。
//!
//! `minion` 不是一个可直接装备的技能，而是幻影、使魔、丧尸等主动技能共享的召唤物运行时辅助模块。
//!
//! ## 技能列表
//!
//! | 技能名称 | 说明 |
//! | --- | --- |
//! | `absorb` | 吸血，将造成的部分伤害转化为自身回复 |
//! | `accumulate` | 聚气，蓄积能量后释放高倍率攻击 |
//! | `assassinate` | 刺杀，优先打击低生命目标 |
//! | `berserk` | 狂暴，提升攻击并改变后续行动/防御表现 |
//! | `charge` | 蓄力，暂停普通攻击后释放强化伤害 |
//! | `charm` | 魅惑，使目标下一次行动攻击己方 |
//! | `clone` | 克隆，以自身模板创建分身召唤物 |
//! | `critical` | 暴击，按概率造成高倍率伤害 |
//! | `curse` | 诅咒，施加持续伤害和治疗削弱 |
//! | `disperse` | 驱散，清除目标身上的状态效果 |
//! | `exchange` | 交换，与目标互换当前生命值 |
//! | `fire` | 火焰，施加可叠加的燃烧伤害 |
//! | `half` | 半血，直接削减目标当前生命值 |
//! | `haste` | 急速，增加行动次数或行动节奏 |
//! | `heal` | 治疗，恢复生命并处理部分异常状态 |
//! | `ice` | 冰冻，限制目标行动并影响受伤表现 |
//! | `iron` | 铁壁，强化防御并牺牲行动速度 |
//! | `minion` | 召唤物公共运行时辅助，不作为可装备技能 |
//! | `poison` | 中毒，施加可叠加的持续生命流失 |
//! | `possess` | 附身，接管目标身份执行攻击 |
//! | `quake` | 地震，对敌方群体造成随机范围伤害 |
//! | `rapid` | 连击，同一回合内连续攻击目标 |
//! | `revive` | 复活，主动拉起死亡目标或召唤物 |
//! | `shadow` | 幻术，生成幻影召唤物 |
//! | `slow` | 减速，降低目标行动优先级 |
//! | `summon` | 血祭/使魔召唤，创建或刷新使魔召唤物 |
//! | `thunder` | 雷击，造成闪电伤害并可能链式传递 |
//!
//! ## 维护边界
//!
//! 本模块只组织主动技能的模块边界。具体的随机数消耗、状态写入、消息模板和召唤物继承规则
//! 都应留在各技能文件内，方便和 JS 产物逐项对照。

pub mod absorb;
pub mod accumulate;
pub mod assassinate;
pub mod berserk;
pub mod charge;
pub mod charm;
pub mod clone;
pub mod critical;
pub mod curse;
pub mod disperse;
pub mod exchange;
pub mod fire;
pub mod half;
pub mod haste;
pub mod heal;
pub mod ice;
pub mod iron;
pub mod minion;
pub mod poison;
pub mod possess;
pub mod quake;
pub mod rapid;
pub mod revive;
pub mod shadow;
pub mod slow;
pub mod summon;
pub mod thunder;
