#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod app;
mod backend;

fn main() -> eframe::Result<()> { app::run() }
