//! 规范战斗会话及跨语言共享的数据契约。
//!
//! [`BattleSession`] 支持初始快照 → 反复 [`BattleSession::next_frame`] →
//! [`BattleSession::result`]；[`battle_replay`] 只是将同一会话完整收集。
//! [`BattleModelSession`] 使用相同 driver，但仅报告边界并导出机制状态。
//!
//! # 帧、消息与状态
//!
//! 一个 Runtime 回合可没有可见消息，也可产生多条消息；`next_frame` 会越过空回合。
//! [`BattleReplayFrame::states`] 是帧末快照，`round_index` 和 `frame_index` 都从 0 开始，
//! 空回合使二者可能出现间隔。rows / clips 提供播放中的 HP/盾变化，完整机制由 Runtime 保存。
//!
//! # 正常结束与失败
//!
//! | 情况 | 状态 | 是否有 result |
//! | --- | --- | --- |
//! | 存在胜者 | [`BattleStatus::Finished`] | 是 |
//! | 达到回合上限或长期无可见进展 | [`BattleStatus::Truncated`] | 是，附截断原因与最终状态 |
//! | Runtime 校验失败 | 保持 [`BattleStatus::Running`]，`is_failed` 为 true | 否 |
//!
//! 失败会保存第一次错误，后续推进返回相同错误。调用方处理错误后释放会话。
//! 正常终止后的推进则幂等地返回 `None`。
//!
//! 同一回合的终止优先级是胜者、`max_rounds`、无可见进展；无进展判据为连续
//! 无可见更新的回合数达到 `max(当前实体数, 1) * 16`，不是对 HP 或机制状态完全不变的检测。
//! 查询机制状态不消费随机流；`BattleOptions::include_icons` 仅影响展示快照，
//! [`BattleModelSession`] 不生成图标或回放，prepared 构造还会沿用模板中固化的 `eval_rq`。
mod driver;
mod dto;
mod model_session;
mod replay;
mod session;

pub use dto::*;
pub use replay::battle_replay;

pub use session::BattleSession;

pub use model_session::{BattleModelFrame, BattleModelOutcome, BattleModelSession};
