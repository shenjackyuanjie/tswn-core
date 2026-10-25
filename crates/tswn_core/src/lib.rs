//! 名字竞技场的角色构建、确定性战斗与数据投影引擎。
//!
//! 从 namerena 文本生成属性和技能，再由同一套 [`runtime`] 执行战斗；CLI、Python、
//! C ABI 和 WASM 通过适配层共享这套战斗规则。
//!
//! # 先选择入口
//!
//! | 需求 | 推荐入口 | 返回内容 |
//! | --- | --- | --- |
//! | 播放一场战斗、逐帧更新界面 | [`cli_api::battle::BattleSession`] | 初始/当前快照、可见帧、终止结果 |
//! | 一次取完整回放 | [`cli_api::battle::battle_replay`] | 规范的 rows / clips / states |
//! | 同一阵容重复跑不同 seed | [`PreparedRunner`]、[`win_rate`] | 精简的批量统计 |
//! | 研究技能、状态或自定义规则 | [`Runner`]、[`runtime::CombatRuntime`] | 可访问的运行时数据与 hook |
//! | 生成学习数据 | [`cli_api::battle::BattleModelSession`] | 帧末机制状态与边界 |
//! | 将机制状态转成张量 | [`encoder::encode::FeatureEncoder`] | 固定容量张量与精确旁路 |
//!
//! # 最小流式对局
//!
//! ```
//! use tswn_core::cli_api::battle::{BattleOptions, BattleSession};
//!
//! let raw = "left@red\n\nright@blue\nseed:demo@!";
//! let mut session = BattleSession::new(raw, BattleOptions::default())?;
//! assert_eq!(session.initial_states().len(), 2);
//! while let Some(frame) = session.next_frame()? {
//!     // 每帧描述一次可见 Runtime 回合，states 提供该回合末展示值。
//!     println!("帧 {}，包含 {} 条消息", frame.frame_index, frame.updates.len());
//! }
//! let result = session.result().expect("正常终止应提供结果");
//! assert!(result.finished || result.truncated);
//! # Ok::<(), tswn_core::cli_api::CliApiError>(())
//! ```
//!
//! `max_rounds` 限制实际推进回合数；正常胜利和安全截断都产生结果。
//! Runtime 错误则返回 `Err` 并保存 sticky failure，供调用方单独处理失败。
//!
//! # 模块阅读地图
//!
//! - [`namerena`]：输入、名字评估、DIY/OL、构建属性、技能描述和图标。
//! - [`runtime`]：实体、世界视图、行动调度、技能/状态 hook、效果队列及对局复用。
//! - [`cli_api`]：面向应用的参数校验、统一错误与规范战斗 DTO。
//! - [`replay_view`]：把消息和帧边界快照转换成展示片段，不推进战斗。
//! - [`rc4`]：与原实现对齐的游戏确定性随机流。
//! - [`win_rate`]：批量胜率入口；名字评分与 CQP 的实现入口在 [`runtime`]。
//! - [`encoder`]：纯 Rust 特征编码；Parquet 由 PWP 处理，训练与推理另行接入。
//! - [`bench_sched`]、[`case_gen`]：批任务调度与复现输入辅助。
//!
//! # 数据各有用途
//!
//! 空行划分对战队伍，`@` 后的阵营参与名字构建。实体 ID 用于关联机制与事件，
//! 显示名用于阅读；输入队伍索引记录来源，运行时编号记录当前队伍。
//! prepared 模板固化阵容与名字评估参数，各对局独占 RNG 和可变状态；seed 是初始化材料，
//! 随机方法调用顺序、技能顺序和实体遍历同样属于可复现契约。模型/展示查询不推进随机流，
//! 复现跨版本差异时仍需记录原输入、seed 与规则版本，而不只保存一个 seed。
//! [`runtime::PlayerTemplate`] 提供当前构造基底，吞噬等机制会更新它。
//! 展示快照服务 UI，完整 Runtime 服务执行，学习状态与张量服务数据处理。
//!
//! # Features 与完整指南
//!
//! | Feature | 用途 |
//! | --- | --- |
//! | `png_render`（默认） | 图标 PNG / Base64 输出 |
//! | `mimalloc_alloc`（默认） | 原生全局分配器；嵌入应用需考虑分配器选择 |
//! | `no_debug` | 编译期移除调试探针与 trace 路径，不切换战斗规则 |
//! | `aux_bins` | 额外诊断 binary |
//! | `battle-test-support` | 绑定测试用故障注入，不用于生产 |
//!
//! 面向熟悉游戏的读者，从仓库的
//! [项目与 core 原理指南](https://github.com/shenjackyuanjie/tswn-core/tree/main/docs/core-guide)
//! 开始；安装、CLI 参数和发布工具见各 crate README。API 细节以本 rustdoc 的类型和签名为准。

#[cfg(feature = "mimalloc_alloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "no_debug"))]
pub mod debug;

#[cfg(feature = "no_debug")]
pub mod debug {
    #[inline(always)]
    pub fn debug_action() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_action_matches(_name: &str) -> bool { false }
    #[inline(always)]
    pub const fn debug_stats() -> bool { false }
    #[inline(always)]
    pub const fn debug_world() -> bool { false }
    #[inline(always)]
    pub const fn debug_tick() -> bool { false }
    #[inline(always)]
    pub const fn debug_pick() -> bool { false }
    #[inline(always)]
    pub const fn debug_dodge() -> bool { false }
    #[inline(always)]
    pub const fn debug_dodge_all() -> bool { false }
    #[inline(always)]
    pub fn debug_die() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_state() -> bool { false }
    #[inline(always)]
    pub const fn debug_post_action() -> bool { false }
    #[inline(always)]
    pub const fn debug_forced_skill() -> bool { false }
    #[inline(always)]
    pub const fn debug_covid() -> bool { false }
    #[inline(always)]
    pub const fn debug_fire() -> bool { false }
    #[inline(always)]
    pub const fn debug_heal() -> bool { false }
    #[inline(always)]
    pub fn debug_upgrade() -> Option<String> { None }
    #[inline(always)]
    pub const fn debug_reflect() -> bool { false }
    #[inline(always)]
    pub const fn debug_damage() -> bool { false }
    #[inline(always)]
    pub const fn trace_rc4() -> bool { false }
    #[inline(always)]
    pub const fn probe_step() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_action() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_default_attack() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_refresh() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_rc4() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_protect() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_counter() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_hide() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_possess() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_charm() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_exchange() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_heal() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn probe_minion_blueprint() -> Option<&'static str> { None }
    #[inline(always)]
    pub const fn debug_tick_order() -> Option<&'static str> { None }
}

pub mod bench_sched;
pub mod case_gen;
pub mod cli_api;
pub mod encoder;
pub mod namerena;
pub mod rc4;
pub mod replay_view;
pub mod runtime;
pub(crate) mod time;
pub mod win_rate;

/// 主 Runtime 对局入口。
///
/// - [`Runner`] 表示一场具体可运行的对局
/// - [`PreparedRunner`] 表示一份可复用的预构建模板，适合同一输入下按不同 seed 批量构造 `Runner`
///
/// 当你只需要跑单局时，通常直接使用 [`Runner`] 即可；
/// 当你需要对同一组输入重复跑很多局（如 win-rate / benchmark）时，优先考虑先构造 [`PreparedRunner`] 再复用。
pub use runtime::{PreparedRuntimeRunner as PreparedRunner, RuntimeRunner as Runner};

pub use runtime::update::{RunUpdate, RunUpdates};

#[inline]
/// 当前核心 crate 的版本；包装层版本应另行查询。
pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }
