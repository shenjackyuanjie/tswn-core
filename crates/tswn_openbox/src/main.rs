//! tswn openbox — 带 GUI 界面的本地工具箱。
//!
//! 基于 `eframe`/`egui` 构建的原生桌面应用，集成 to-diy、namer-pf、batch-rate、pair
//! 四种工具，支持从文件或内联文本输入，结果可写入文件或展示在日志区。

#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod backend;

fn main() -> eframe::Result<()> { app::run() }
