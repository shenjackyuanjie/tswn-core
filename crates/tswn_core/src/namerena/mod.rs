//! 从 namerena 文本到可导入 Runtime 的角色构建数据。
//!
//! [`NamerenaInput`] 解析对战队伍与 seed，[`PlayerSpec`] 解析单个名字的 `@`、武器和
//! DIY/OL 后缀；[`PreparedRoster::build`] 再构建属性、技能与团队强化。
//! 战斗推进与状态生命周期由 [`crate::runtime`] 接续处理。
//!
//! # 队伍与阵营各自的作用
//!
//! 有空行时，连续名字组成一支对战队伍；没有空行时，每行各自成队。
//! `name@clan` 的 `clan` 影响角色构建和阵营机制，不直接决定输入分组。
//! `seed:` 行作为本场初始化材料提取。
//!
//! ```
//! use tswn_core::namerena::{NamerenaInput, PlayerSpec};
//!
//! let input = NamerenaInput::parse("a@red\nb@red\n\nc@blue\nseed:demo@!")?;
//! assert_eq!(input.groups.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
//! assert_eq!(input.seed, ["seed:demo@!"]);
//! let player = PlayerSpec::parse(r#"a@red+ol:{"skills":{"shield":80}}"#)?;
//! assert!(player.overlay.is_some());
//! # Ok::<(), tswn_core::namerena::PlayerSpecError>(())
//! ```
//!
//! # 主要类型与工具
//!
//! | 入口 | 作用 |
//! | --- | --- |
//! | [`PreparedPlayer`] / [`PreparedRoster`] | 一次构建的属性、技能与阵容数据 |
//! | [`PreparedMinionBlueprint`] | 召唤物派生与重施策略 |
//! | [`PlayerStats`] | 构建属性容器，供 Runtime 导入 |
//! | [`SkillLoadoutSpec`] / [`SkillBoost`] | 待导入的技能顺序、等级与强化信息 |
//! | [`overlay`] | 玩家与幻影/使魔/丧尸的显式覆盖输入 |
//! | [`eval_name`] | UTF-16 名字评估及 `eval_rq` 默认值 |
//! | [`icon`] / [`icon_render`] | 身份图标数据与 RGBA / PNG 输出 |
//!
//! UTF-8 长度校验、UTF-16 字符评估和实际技能等级各有度量语义。
//! 解析规则同时承载角色身份、随机材料与输入兼容信息。

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
