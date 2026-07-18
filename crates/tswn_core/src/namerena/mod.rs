//! 名字竞技场输入与构建数据层。
//!
//! 这里仅保存从文本输入到 Runtime 模板所需的纯数据，不承载战斗期状态。

mod build;
mod input;
pub mod overlay;
mod skill;
mod weapon;

pub use build::{MinionKind, PreparedMinionBlueprint, PreparedPlayer, PreparedRoster, PreparedRosterError};
pub use input::{
    NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerSpec, PlayerSpecError, SEED_PREFIX, TEAM_MAX_LEN, is_seed_line,
    raw_namerena_to_id_name,
};
pub use overlay::{MinionOverlay, PlayerOverlay};
pub use skill::{
    BuiltinSkillRef, CLASSIFIED_SKILL_SLOT_COUNT, ClassifiedSkillRef, PHANTOM_POSSESS_SKILL_KEY, SUMMON_EXPLODE_SKILL_KEY,
    SUMMON_FIRE1_SKILL_KEY, SUMMON_FIRE2_SKILL_KEY, SUMMON_MINION_NORMAL_SKILL_KEY_BASE, SUMMON_SHARE_DAMAGE_SKILL_KEY,
    SkillBoost, SkillEntrySpec, SkillLoadoutSpec,
};
