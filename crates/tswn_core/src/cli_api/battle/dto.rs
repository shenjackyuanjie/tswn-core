use crate::replay_view::ReplayState;
use crate::runtime::{BINDING_COMPLETION_MAX_ROUNDS, PlrId};
use serde::Serialize;

/// 面向用户的完整战斗回放选项。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleOptions {
    /// 有限的名字评估参数，默认普通 fight 口径。
    pub eval_rq: f64,
    /// 是否在快照中附 PNG Base64；默认关闭以减少计算和载荷。
    ///
    /// 展示会话需要 `png_render` feature，否则请求 true 返回 `UnsupportedOption`；
    /// 模型会话不构造图标，始终忽略此选项。
    pub include_icons: bool,
    /// 正整数的 Runtime 推进回合上限，包含空回合；默认 20,000。
    pub max_rounds: usize,
    /// 玩家受控队伍：该队伍的本体与分身必须等前端下指令，否则战斗暂停。
    pub controlled_team: Option<usize>,
    /// 开局体力倍率（如 1.25 表示以 125% 体力开场）；只作用于受控队伍。
    pub start_hp_pct: Option<f64>,
}

impl Default for BattleOptions {
    fn default() -> Self {
        Self {
            eval_rq: crate::namerena::eval_name::DEFAULT_EVAL_RQ,
            include_icons: false,
            max_rounds: BINDING_COMPLETION_MAX_ROUNDS,
            controlled_team: None,
            start_hp_pct: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 完整收集同一 [`super::BattleSession`] 的规范回放。
pub struct BattleReplay {
    /// 玩家受控单位把战斗逼停时，这里是等待指令的实体 id。
    pub paused_actor: Option<usize>,
    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub finished: bool,
    pub truncated: bool,
    pub initial_states: Vec<BattlePlayerState>,
    pub frames: Vec<BattleReplayFrame>,
    pub final_states: Vec<BattlePlayerState>,
    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,
    pub state_granularity: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 面向界面的只读投影；整理角色显示属性、关系和人类可读状态标签。
pub struct BattlePlayerState {
    /// 本场实体 ID；允许预留空洞，适合作为状态映射的主键。
    pub id: usize,
    /// 当前运行时队伍编号。
    pub team_index: usize,
    /// 对应原输入队伍，召唤物按根主人映射。
    pub input_team_index: Option<usize>,
    /// 直接主人实体 ID，无主人时为 None。
    pub owner_id: Option<usize>,
    /// 根来源实体 ID；与直接 owner 可能不同。
    pub source_id: Option<usize>,
    /// 面向兼容接口的身份名，不作为本场主键。
    pub id_name: String,
    /// 名字推导使用的身份键。
    pub id_key_name: String,
    /// 图标生成键；昵称编辑时继续使用此身份键。
    pub icon_key: String,
    /// 展示名，可能与其他实体重复。
    pub display_name: String,
    /// 分身等同名实体的展示序号。
    pub display_index: usize,
    pub base_name: String,
    pub player_type: String,
    pub minion_kind: Option<&'static str>,
    pub icon_png_base64: Option<String>,
    /// 当前体力，死亡状态可能保留非正数。
    pub hp: i32,
    /// 当前模板体力上限，吞噬等机制后可能变化。
    pub max_hp: i32,
    /// 剩余吸收护盾，合计常规蓄盾与 ShieldValue；不含铁壁，死亡时为 0。
    pub shield: i32,
    /// 当前护盾技能熟练度，随吞噬等技能变化更新；不是实际盾值的硬上限。
    pub max_shield: u32,
    pub magic_point: i32,
    pub move_point: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub agility: i32,
    pub magic: i32,
    pub resistance: i32,
    pub wisdom: i32,
    pub point: u32,
    pub all_sum: u32,
    pub name_factor: f64,
    pub at_boost: f64,
    pub attract: f64,
    pub frozen: bool,
    /// Runtime 的存活登记标记。
    pub alive: bool,
    /// 当前是否存活且体力大于 0。
    pub active: bool,
    /// 人类可读的状态描述；完整载荷可从机制状态查阅。
    pub status_labels: Vec<String>,
    /// 当前技能熟练度（导出名, 等级）；分身等派生单位是衰减后的值。
    pub skills: Vec<(String, u32)>,
}

impl ReplayState for BattlePlayerState {
    fn id(&self) -> PlrId { self.id }

    fn hp(&self) -> i32 { self.hp }

    fn max_hp(&self) -> i32 { self.max_hp }

    fn alive(&self) -> bool { self.alive }

    fn shield(&self) -> i32 { self.shield }

    fn with_shield(&self, shield: i32) -> Self {
        Self {
            shield: if self.alive { shield.max(0) } else { 0 },
            skills: self.skills.clone(),
            ..self.clone()
        }
    }

    fn with_hp_alive(&self, hp: i32, alive: bool) -> Self {
        Self {
            hp,
            alive,
            shield: if alive { self.shield } else { 0 },
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 一次可见回合的消息、展示行和帧末快照。
pub struct BattleReplayFrame {
    /// 从 0 开始的可见帧索引。
    pub frame_index: usize,
    /// 从 0 开始的 Runtime 回合索引；包含空回合造成的间隔。
    pub round_index: usize,
    pub finished: bool,
    pub winner_ids: Vec<usize>,
    pub updates: Vec<BattleUpdate>,
    pub rows: Vec<BattleReplayRow>,
    pub states: Vec<BattlePlayerState>,
    pub total_delay: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 已执行事件的展示消息；模板、替换文本、参与实体和播放延时分开保存。
pub struct BattleUpdate {
    pub update_type: &'static str,
    pub tone: &'static str,
    pub message_template: String,
    pub message_rendered: String,
    pub caster_id: Option<usize>,
    pub target_id: Option<usize>,
    pub target_ids: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub hp_delta: Option<i32>,
    pub is_win: bool,
    pub is_next_line: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 一行展示内容，按 clips 顺序播放。
pub struct BattleReplayRow {
    pub indent: bool,
    pub clips: Vec<BattleReplayClip>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 播放片段及 sidebar 前后值；部分数值由消息与边界快照投影。
pub struct BattleReplayClip {
    /// 毫秒级的播放等待时间。
    pub delay: i32,
    pub color: String,
    pub tone: &'static str,
    pub parts: Vec<BattleReplayTextPart>,
    pub caster_ids: Vec<usize>,
    pub target_ids: Vec<usize>,
    pub sidebar_states: Vec<BattlePlayerState>,
    pub sidebar_previous_states: Vec<BattlePlayerState>,
    pub winner: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// 已拆分的文字、玩家引用与数值片段；前端按 player_id 关联实体。
pub struct BattleReplayTextPart {
    pub kind: &'static str,
    pub text: String,
    pub player_id: Option<usize>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
}

#[deprecated(note = "use BattleOptions")]
pub type BattleReplayOptions = BattleOptions;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// 正常会话状态；sticky Runtime failure 使用独立的 is_failed 查询。
pub enum BattleStatus {
    Running,
    Finished,
    Truncated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[derive(serde::Deserialize)]
/// 正常终止原因；Winner 表示胜利，MaxRounds 和 NoProgress 表示安全截断。
pub enum BattleStopReason {
    Winner,
    MaxRounds,
    NoProgress,
    Paused,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
/// 正常 terminal 结果，包含胜利或截断信息；调用失败通过错误通道处理。
pub struct BattleResult {
    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,
    pub finished: bool,
    pub truncated: bool,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,
    pub final_states: Vec<BattlePlayerState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_api::CliApiError;

    #[test]
    fn public_enum_values_and_error_codes_are_stable() {
        for (value, expected) in [
            (BattleStatus::Running, "running"),
            (BattleStatus::Finished, "finished"),
            (BattleStatus::Truncated, "truncated"),
        ] {
            assert_eq!(serde_json::to_value(value).unwrap(), expected);
        }
        for (value, expected) in [
            (BattleStopReason::Winner, "winner"),
            (BattleStopReason::MaxRounds, "max_rounds"),
            (BattleStopReason::NoProgress, "no_progress"),
        ] {
            assert_eq!(serde_json::to_value(value).unwrap(), expected);
        }
        for (error, expected) in [
            (CliApiError::InvalidInput("input".into()), "INVALID_INPUT"),
            (CliApiError::InvalidArgument("argument".into()), "INVALID_ARGUMENT"),
            (CliApiError::UnsupportedOption("option".into()), "UNSUPPORTED_OPTION"),
            (CliApiError::RunnerInit("init".into()), "RUNNER_INIT_FAILED"),
            (CliApiError::Runtime("runtime".into()), "RUNTIME_FAILED"),
            (CliApiError::Internal("internal".into()), "INTERNAL_ERROR"),
        ] {
            assert_eq!(error.code().as_str(), expected);
            assert_eq!(serde_json::to_value(error.code()).unwrap(), expected);
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn battle_options_defaults_match_binding_contract() {
        let options = BattleOptions::default();
        assert_eq!(options.max_rounds, 20_000);
        assert!(!options.include_icons);
        assert_eq!(options.eval_rq, crate::namerena::eval_name::DEFAULT_EVAL_RQ);
    }
}
