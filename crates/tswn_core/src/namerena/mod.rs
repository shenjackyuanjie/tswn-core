//! 名字竞技场输入与构建数据层。
//!
//! 这里仅保存从文本输入到 Runtime 模板所需的纯数据，不承载战斗期状态。

mod build;
mod input;
mod skill;
mod weapon;

pub use build::{PreparedPlayer, PreparedRoster, PreparedRosterError};
pub use input::{
    NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerSpec, PlayerSpecError, SEED_PREFIX, TEAM_MAX_LEN, is_seed_line,
    raw_namerena_to_id_name,
};
pub use skill::{BuiltinSkillRef, SkillEntrySpec, SkillLoadoutSpec};
