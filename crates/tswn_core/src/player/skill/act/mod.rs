//! # 主动技能 (act)
//!
//! 本模块定义所有主动技能，共 27 种。
//!
//! ## 技能列表
//!
//! | 技能名称   | 说明                          |
//! |------------|-------------------------------|
//! | `absorb`   | 吸血                          |
//! | `accumulate`| 积累                          |
//! | `assassinate`| 刺杀                        |
//! | `berserk`  | 狂暴                          |
//! | `charge`   | 充能                          |
//! | `charm`    | 魅惑                          |
//! | `clone`    | 克隆                          |
//! | `critical` | 暴击                          |
//! | `curse`    | 诅咒                          |
//! | `disperse` | 驱散                          |
//! | `exchange` | 交换                          |
//! | `fire`     | 火球                          |
//! | `half`     | 半血                          |
//! | `haste`    | 急速                          |
//! | `heal`     | 治疗                          |
//! | `ice`      | 冰冻                          |
//! | `iron`     | 铁壁                          |
//! | `minion`   | 召唤物                        |
//! | `poison`   | 中毒                          |
//! | `possess`  | 附身                          |
//! | `quake`    | 地震                          |
//! | `rapid`    | 连击                          |
//! | `revive`   | 复活                          |
//! | `shadow`   | 影分身                        |
//! | `slow`     | 减速                          |
//! | `summon`   | 召唤                          |
//! | `thunder`  | 雷击                          |
//!
//! ## 技能实现
//!
//! 每个技能都是一个独立的模块，实现技能的具体逻辑。
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::skill::act::fire;
//!
//! // 使用火球技能
//! fire::fire(/* args */);
//! ```

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
