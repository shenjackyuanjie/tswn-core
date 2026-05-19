//! # 被动/防御技能 (skl)
//!
//! 本模块定义所有被动/防御技能，共 12 种。
//!
//! ## 技能列表
//!
//! | 技能名称   | 说明                          |
//! |------------|-------------------------------|
//! | `corpse`   | 尸体                          |
//! | `counter`  | 反击                          |
//! | `defend`   | 防御                          |
//! | `hide`     | 隐匿                          |
//! | `merge`    | 融合                          |
//! | `none`     | 无技能                        |
//! | `protect`  | 保护                          |
//! | `reflect`  | 反射                          |
//! | `reraise`  | 复活                          |
//! | `shield`   | 护盾                          |
//! | `upgrade`  | 升级                          |
//! | `zombie`   | 僵尸                          |
//!
//! ## 技能实现
//!
//! 每个技能都是一个独立的模块，实现技能的具体逻辑。
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::skill::skl::defend;
//!
//! // 使用防御技能
//! defend::defend(/* args */);
//! ```

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
