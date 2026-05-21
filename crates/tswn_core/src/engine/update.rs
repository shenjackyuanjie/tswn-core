//! # 运行更新帧 (update)
//!
//! 本模块定义战斗过程中产生的事件消息结构体。
//!
//! ## 设计说明
//!
//! 每一次战斗事件（发动技能、造成伤害、角色死亡等）都会生成一个 [`RunUpdate`]，
//! 并被追加到当前回合的 [`RunUpdates`] 集合中。
//!
//! 消息模板中使用三种占位符（与 JS 产物一致）：
//! - `[0]` — 施法者的显示名
//! - `[1]` — 目标的显示名
//! - `[2]` — 参数值（数值 或 多目标 ID 拼接字符串）
//!
//! 调用 [`RunUpdate::msg()`] 可将 caster/target ID 及 param 替换进模板后得到最终字符串。
//!
//! ## 推荐用法
//!
//! ```rust,ignore
//! // 直接使用硬编码字符串（向后兼容）
//! RunUpdate::new("[0]使用[火球术]", caster, target, 1)
//!
//! // 推荐：使用语言包键（便于维护多语言）
//! RunUpdate::new_lang(lang::keys::SKL_FIRE, caster, target, 1)
//! ```

use crate::player::PlrId;
use std::borrow::Cow;
use std::cell::Cell;

thread_local! {
    /// 每线程自增 ID，用于为每批 [`RunUpdates`] 分配标识。
    ///
    /// 对战推进本身是单线程持有一份 `RunUpdates`；胜率并行时每个 worker
    /// 也只在本线程内比较批次 ID。使用线程本地计数可以避免 benchmark 热路径
    /// 在每个可见事件批次上做跨线程原子自增。
    static RUN_UPDATES_ID: Cell<u64> = const { Cell::new(1) };
}

#[inline]
fn next_run_updates_id() -> u64 {
    RUN_UPDATES_ID.with(|counter| {
        let id = counter.get();
        counter.set(id.wrapping_add(1));
        id
    })
}

/// 战斗事件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateType {
    /// 胜负已分（某队伍获胜）。
    Win,
    /// 无实际内容的占位帧（跳过渲染）。
    None,
    /// 换行分隔符，用于战斗日志分段。
    NextLine,
}

/// 单条战斗事件消息帧。
///
/// 对应 JS 产物中的 `RunUpdate` 对象，携带：
/// - 消息模板字符串（含占位符 `[0]`/`[1]`/`[2]`）
/// - 施法者与目标的 [`PlrId`]
/// - 分值（用于 UI 评分展示）
/// - 延迟时间（用于 web 端动画）
#[derive(Debug, Clone)]
pub struct RunUpdate {
    /// 视觉分值（用于 UI 展示，影响动画强度）。
    pub score: u32,
    /// 可选数值参数，对应消息中 `[2]` 的纯数字情形（优先于 targets 列表）。
    pub param: Option<u32>,
    /// 动画延迟 0（毫秒，JS 端使用）。
    pub delay0: i32,
    /// 动画延迟 1（毫秒，JS 端使用）。
    pub delay1: i32,
    /// 消息模板字符串，可包含 `[0]`/`[1]`/`[2]` 占位符。
    pub message: Cow<'static, str>,
    /// 施法者 PlrId（替换 `[0]`）。
    pub caster: PlrId,
    /// 目标 PlrId（替换 `[1]`）。
    pub target: PlrId,
    /// 多目标列表（替换 `[2]`，与 `param` 二选一）。
    pub targets: smallvec::SmallVec<[PlrId; 2]>,
    /// 事件类型标记。
    pub update_type: UpdateType,
}

impl RunUpdate {
    /// 创建一个空白占位帧（`UpdateType::None`）。
    pub fn new_dummy() -> RunUpdate {
        RunUpdate {
            score: 0,
            param: None,
            delay0: 0,
            delay1: 0,
            message: Cow::Borrowed("\n"),
            caster: 0,
            target: 0,
            targets: smallvec::SmallVec::new(),
            update_type: UpdateType::None,
        }
    }

    /// 创建一个换行分隔帧（`UpdateType::NextLine`）。
    ///
    /// 用于在战斗日志中插入视觉换行，对应 JS 的 `RunUpdate_init(null, ...)` 形式。
    pub fn new_newline() -> RunUpdate {
        RunUpdate {
            score: 0,
            param: None,
            delay0: 0,
            delay1: 0,
            message: Cow::Borrowed("\n"),
            caster: 0,
            target: 0,
            targets: smallvec::SmallVec::new(),
            update_type: UpdateType::NextLine,
        }
    }

    /// 创建一条标准事件消息帧。
    ///
    /// # 参数
    /// - `msg`：消息模板（可含 `[0]`/`[1]`/`[2]` 占位符），也可直接传入已展开的字符串。
    /// - `caster`：施法者 ID（替换 `[0]`）。
    /// - `target`：目标 ID（替换 `[1]`）。
    /// - `score`：UI 显示分值。
    pub fn new(msg: impl Into<Cow<'static, str>>, caster: PlrId, target: PlrId, score: u32) -> Self {
        RunUpdate {
            score,
            param: None,
            delay0: 0,
            delay1: 0,
            message: msg.into(),
            caster,
            target,
            targets: smallvec::SmallVec::new(),
            update_type: UpdateType::None,
        }
    }

    /// 从语言包键创建一条事件消息帧。
    ///
    /// 内部调用 [`crate::engine::lang::get_lang`] 查找对应的中文模板字符串，
    /// 再转交 [`RunUpdate::new`] 构造。若键不存在，消息为空字符串。
    ///
    /// # 参数
    /// - `key`：语言包键名，建议使用 [`crate::engine::lang::keys`] 中定义的常量。
    /// - `caster`：施法者 ID。
    /// - `target`：目标 ID。
    /// - `score`：UI 显示分值。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// use tswn_core::engine::update::RunUpdate;
    /// use tswn_core::engine::lang::keys;
    ///
    /// let upd = RunUpdate::new_lang(keys::SKL_FIRE, caster_id, target_id, 1);
    /// // upd.message == "[0]使用[火球术]"
    /// ```
    pub fn new_lang(key: &str, caster: PlrId, target: PlrId, score: u32) -> Self {
        Self::new(crate::engine::lang::get_lang(key), caster, target, score)
    }

    /// 将消息模板中的占位符替换为实际值，返回最终显示字符串。
    ///
    /// 替换规则：
    /// - `[0]` → `caster` ID 的字符串形式
    /// - `[1]` → `target` ID 的字符串形式
    /// - `[2]` → `param`（若有）的字符串，否则为 `targets` 列表的逗号拼接
    pub fn msg(&self) -> String {
        let mut msg = self.message.to_string();
        msg = msg.replace("[0]", &self.caster.to_string());
        msg = msg.replace("[1]", &self.target.to_string());
        let param_str = if let Some(p) = self.param {
            p.to_string()
        } else {
            self.targets.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(",")
        };
        msg = msg.replace("[2]", &param_str);
        msg
    }
}

/// 单回合内所有事件消息帧的集合容器。
///
/// 每调用一次 `main_round`（即处理到有输出事件为止）都会产生一批新的 `RunUpdates`。
/// `id` 字段用于在调试时区分不同批次。
///
/// ## 特殊字段
///
/// `on_update_end` 是一个待回调列表：本回合结束时，引擎会对列表中每个玩家
/// 调用 `on_update_end`，用于处理持续性效果的结算（如中毒、冰冻计时等）。
#[derive(Debug, Clone)]
pub struct RunUpdates {
    /// 批次唯一 ID（自增，从 1 开始）。
    pub id: u64,
    /// 本批次内所有事件帧，按时间顺序排列。
    pub updates: smallvec::SmallVec<[RunUpdate; 8]>,
    /// 本批次结束后需要触发 `on_update_end` 回调的玩家列表。
    pub on_update_end: smallvec::SmallVec<[PlrId; 8]>,
    /// 是否缓存详细帧内容（benchmark 高速路径可关闭）。
    pub capture_updates: bool,
    /// 本批次是否出现过事件（无论是否缓存详细帧）。
    has_activity: bool,
}

impl RunUpdates {
    fn new_with_capture(capture_updates: bool) -> RunUpdates {
        RunUpdates {
            id: next_run_updates_id(),
            updates: smallvec::SmallVec::new(),
            on_update_end: smallvec::SmallVec::new(),
            capture_updates,
            has_activity: false,
        }
    }

    /// 创建一个新的空批次，分配唯一 `id`。
    pub fn new() -> RunUpdates { Self::new_with_capture(true) }

    /// 创建不缓存详细事件帧的批次（仍可判断是否发生过事件）。
    pub fn new_no_capture() -> RunUpdates { Self::new_with_capture(false) }

    /// 清理批次内容，复用分配。
    pub fn reset(&mut self) {
        self.id = next_run_updates_id();
        self.updates.clear();
        self.on_update_end.clear();
        self.has_activity = false;
    }

    /// 本批次是否发生过有效事件。
    pub fn had_updates(&self) -> bool { self.has_activity }

    /// 追加一条事件帧。
    pub fn add(&mut self, update: RunUpdate) {
        self.has_activity = true;
        if self.capture_updates {
            self.updates.push(update);
        }
    }

    /// 延迟构建事件帧：仅在 `capture_updates=true` 时执行构建闭包。
    pub fn emit<F>(&mut self, build: F)
    where
        F: FnOnce() -> RunUpdate,
    {
        self.has_activity = true;
        if self.capture_updates {
            self.updates.push(build());
        }
    }

    // /// 批量追加事件帧（从切片复制）。
    // pub fn add_all(&mut self, updates: &mut [RunUpdate]) { self.updates.extend_from_slice(updates); }
}

impl Default for RunUpdates {
    fn default() -> Self { Self::new() }
}
