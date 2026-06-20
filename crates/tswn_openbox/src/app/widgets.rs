//! 通用 egui 控件封装。

use std::path::{PathBuf, absolute};

use eframe::egui;

use tswn_openbox::backend::OutputMode;

use super::state::{AccuracyPreset, CountMode};

#[derive(Debug, Clone, Default)]
pub struct OptionalFileOutput {
    pub enabled: bool,
    pub path: Option<PathBuf>,
}

impl OptionalFileOutput {
    pub fn selected_path(&self) -> Option<PathBuf> { if self.enabled { self.path.clone() } else { None } }

    pub fn path(&self) -> Option<PathBuf> { self.path.clone() }
}

#[derive(Debug, Clone)]
pub struct BenchOutputConfig {
    pub file_output: OptionalFileOutput,
    pub mode: OutputMode,
    pub min_screen: String,
    pub min_file: String,
    pub precision: usize,
}

pub fn optional_file_output_controls(ui: &mut egui::Ui, output: &mut OptionalFileOutput, default_name: &str) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut output.enabled, "写入文件");
        if output.enabled {
            if ui.button("选择输出文件").clicked()
                && let Some(path) = pick_output_file(default_name)
            {
                output.path = Some(path);
            }
            if output.path.is_some() && ui.button("清空").clicked() {
                output.path = None;
            }
        }
    });

    if output.enabled {
        let label = output
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未选择输出文件".to_string());
        ui.label(label);
    }
}

pub fn bench_output_controls(
    ui: &mut egui::Ui,
    output: &mut BenchOutputConfig,
    default_name: &str,
    show_jsonl: bool,
    show_precision: bool,
) {
    ui.horizontal(|ui| {
        ui.label("输出文件");
        if ui.button("选择输出文件").clicked()
            && let Some(path) = pick_output_file(default_name)
        {
            output.file_output.enabled = true;
            output.file_output.path = Some(path);
        }
        if output.file_output.path.is_some() && ui.button("清空").clicked() {
            output.file_output.path = None;
        }
    });

    let label = output
        .file_output
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "未选择输出文件".to_string());
    ui.label(label);

    ui.horizontal(|ui| {
        ui.label("文件格式");
        ui.radio_value(&mut output.mode, OutputMode::Log, "分数 名字");
        if show_jsonl {
            ui.radio_value(&mut output.mode, OutputMode::Jsonl, "JSONL (--log)");
        }
        ui.radio_value(&mut output.mode, OutputMode::Pure, "名字 (--pure)");
    });

    ui.horizontal(|ui| {
        ui.label("日志阈值");
        ui.add(egui::TextEdit::singleline(&mut output.min_screen).desired_width(72.0));
        ui.label("文件阈值");
        ui.add(egui::TextEdit::singleline(&mut output.min_file).desired_width(72.0));
        if show_precision {
            ui.label("保留小数点后 X 位");
            ui.add(egui::DragValue::new(&mut output.precision).range(0..=9).speed(1));
        }
    });
}

pub fn accuracy_controls(ui: &mut egui::Ui, accuracy: &mut AccuracyPreset) {
    ui.horizontal(|ui| {
        ui.label("精确度");
        for preset in AccuracyPreset::ALL {
            ui.radio_value(accuracy, preset, preset.label());
        }
    });
}

pub fn count_mode_controls(ui: &mut egui::Ui, mode: &mut CountMode, accuracy: &mut AccuracyPreset, count: &mut usize) {
    ui.horizontal(|ui| {
        ui.radio_value(mode, CountMode::Accuracy, "精确度");
        ui.radio_value(mode, CountMode::Manual, "场数");
    });
    match mode {
        CountMode::Accuracy => accuracy_controls(ui, accuracy),
        CountMode::Manual => {
            ui.horizontal(|ui| {
                ui.label("场数");
                ui.add(egui::DragValue::new(count).range(1..=10_000_000).speed(100));
            });
        }
    }
}

pub fn thread_controls(ui: &mut egui::Ui, auto_threads: &mut bool, threads: &mut usize) {
    ui.horizontal(|ui| {
        ui.checkbox(auto_threads, "系统线程 * 1.5");
        ui.label("线程");
        ui.add_enabled(!*auto_threads, egui::DragValue::new(threads).range(0..=256).speed(1));
    });
}

pub fn multiline(ui: &mut egui::Ui, id: &'static str, text: &mut String, rows: usize) {
    egui::ScrollArea::both().id_salt(id).max_height(rows as f32 * 20.0).show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(text)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(rows),
        );
    });
}

fn pick_output_file(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_directory(current_dir())
        .set_file_name(default_name)
        .save_file()
}

pub fn pick_named_output_file(default_name: &str) -> Option<PathBuf> { pick_output_file(default_name) }

fn current_dir() -> PathBuf {
    std::env::current_dir()
        .ok()
        .and_then(|path| absolute(path).ok())
        .unwrap_or_else(|| PathBuf::from("."))
}
