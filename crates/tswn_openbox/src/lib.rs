//! Openbox 桌面工具箱的 Rust 门面；GUI 和无头 CLI 共用独立计算后端。
//!
//! # 从哪一层接入
//!
//! - 本 crate 的 [`backend`] 与 [`presets`] 是兼容再导出。
//! - 无界面 Rust 应用可直接依赖 `tswn_openbox_backend`，使用精简的计算接口。
//! - GUI binary `tswn_openbox` 用 eframe/egui 展示任务、结构化结果和 DS4 工作流。
//! - binary `openbox-cli` 复用同一任务接口，数据行走 stdout，进度走 stderr。
//!
//! [`backend`] 提供 DIY、namer-pf、批量胜率与配对任务，战斗与评分复用 core。
//! 预设默认从当前工作目录 `setting/` 读取，缺失时按需写入内嵌默认资源。
//! 这类预设构造具有同步 I/O 副作用；后台任务的输入所有权、取消和回调约定见 [`backend`]。
//!
//! # Features
//!
//! `gui` 默认启用，包含界面与 DS4 集成；`--no-default-features` 可构建无头 CLI。
//! `mimalloc_alloc` 是原生分配器选项，`ui_capture` 仅用于界面截图验收。
//! GUI 通过独立 binary 使用，库门面用于共用业务接口。启动、配置与命令说明见
//! [Openbox README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_openbox)。

pub use tswn_openbox_backend::{backend, presets};
