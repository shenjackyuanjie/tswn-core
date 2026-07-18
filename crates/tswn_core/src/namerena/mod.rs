//! 名字竞技场输入与构建数据层。
//!
//! 这里仅保存从文本输入到 Runtime 模板所需的纯数据，不承载战斗期状态。

mod build;
pub mod eval_name;
pub mod icon;
pub mod icon_render;
mod input;
pub mod overlay;
mod rules;
mod skill;
mod stats;
mod weapon;

pub(crate) use build::score_profile_team_rng;
pub use build::{MinionKind, PreparedMinionBlueprint, PreparedPlayer, PreparedRoster, PreparedRosterError};
pub use input::{
    NAME_MAX_LEN, NamerenaInput, PlayerClass, PlayerSpec, PlayerSpecError, SEED_PREFIX, TEAM_MAX_LEN, is_seed_line,
    raw_namerena_to_id_name,
};
pub(crate) use input::{raw_namerena_to_id_name_into, trim_js_line_end};
pub use overlay::{MinionOverlay, PlayerOverlay};
pub use rules::{
    BOOST_NAMES, BOSS_NAMES, boost_value, boss_action_prob_count, boss_append_attr, boss_display_name, boss_immune_threshold,
    median,
};
pub use skill::{
    BuiltinSkillRef, CLASSIFIED_SKILL_SLOT_COUNT, ClassifiedSkillRef, PHANTOM_POSSESS_SKILL_KEY, SUMMON_EXPLODE_SKILL_KEY,
    SUMMON_FIRE1_SKILL_KEY, SUMMON_FIRE2_SKILL_KEY, SUMMON_MINION_NORMAL_SKILL_KEY_BASE, SUMMON_SHARE_DAMAGE_SKILL_KEY,
    SkillBoost, SkillEntrySpec, SkillLoadoutSpec, classified_player_skill_name_for_export,
    classified_summon_minion_skill_name_for_export, parse_prefixed_classified_skill_name, phantom_skill_ref_from_name,
    player_classified_skill_ref_from_name, skill_name_for_export, skill_name_to_id, summon_slot_skill_ref_from_name,
};
pub use stats::PlayerStats;
