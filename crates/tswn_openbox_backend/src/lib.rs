//! Openbox GUI、openbox-cli 与 DS4 共用的无界面业务后端。
//!
//! # 任务边界
//!
//! [`backend`] 接收结构化输入，执行 DIY 导出、namer-pf、批量胜率和 pair，并按配置
//! 写入结果文件。`run_*_observed` 同时提供进度与结构化增量结果；旧 `run_*` 入口
//! 保留文本/channel 使用方式。计算是同步的，GUI 应在后台线程调用。
//!
//! [`backend::live`] 提供有界收件箱，使计算与 UI 消费解耦。结果显示使用输入索引，
//! 支持同名条目；展示缓存服务即时视图，最终业务结果保存到配置的输出文件。
//! [`presets`] 处理 `setting/` 中靶子、队友和技能基线配置，并按需写出内置默认资源。
//!
//! # 依赖方向
//!
//! `Openbox / DS4 → 本 crate → tswn_core`。GUI 和 DS4 直接调用此 library，
//! 外部模型由 DS4 编排，共用后端专注于 core 计算与业务结果。
//! 回调在调用期间借用，结构化更新拥有自己的数据；具体并发、取消与文件非事务
//! 语义见 [`backend`]。此 crate 不统一捕获 panic，GUI 在线程入口提供自己的保护边界。
//! 配置和输出口径见
//! [Openbox README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_openbox)。

pub mod backend;
pub mod presets;
