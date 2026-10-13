//! tswn openbox — 带 GUI 界面的本地工具箱。
//!
//! 基于 eframe/egui 的原生桌面应用，集成 DIY、namer-pf、批量胜率、pair 与 DS4。
//! 任务在后台执行，界面读取有界增量结果；计算复用 tswn_openbox_backend 和 core。
//! Windows 普通 GUI 不弹控制台；`ui_capture` 仅编译验收截图驱动。
//! 无头环境使用同 package 的 openbox-cli，不通过驱动 GUI 自动化计算。

#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result<()> {
    #[cfg(feature = "ui_capture")]
    if let Some(result) = app::capture::run_if_requested() {
        return result;
    }
    app::run()
}
