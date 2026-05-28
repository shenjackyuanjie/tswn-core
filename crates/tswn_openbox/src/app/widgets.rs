//! 通用 egui 控件封装。
//!
//! 提供文件输出选择控件（`optional_file_output_controls`、`bench_output_controls`）、
//! 场数/线程控件（`bench_controls`）及带滚动的多行文本编辑框（`multiline`）。

use std::path::PathBuf;

use eframe::egui;

use crate::backend::OutputMode;

#[derive(Debug, Clone, Default)]
pub(crate) struct OptionalFileOutput {
    pub(crate) enabled: bool,
    pub(crate) path: Option<PathBuf>,
}

impl OptionalFileOutput {
    pub(crate) fn selected_path(&self) -> Option<PathBuf> { if self.enabled { self.path.clone() } else { None } }

    pub(crate) fn path(&self) -> Option<PathBuf> { self.path.clone() }
}

#[derive(Debug, Clone)]
pub(crate) struct BenchOutputConfig {
    pub(crate) file_output: OptionalFileOutput,
    pub(crate) mode: OutputMode,
    pub(crate) min_screen: String,
    pub(crate) min_file: String,
    pub(crate) precision: usize,
}

pub(crate) fn optional_file_output_controls(ui: &mut egui::Ui, output: &mut OptionalFileOutput, default_name: &str) {
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

pub(crate) fn bench_output_controls(ui: &mut egui::Ui, output: &mut BenchOutputConfig, default_name: &str) {
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
        ui.radio_value(&mut output.mode, OutputMode::Jsonl, "JSONL (--log)");
        ui.radio_value(&mut output.mode, OutputMode::Pure, "名字 (--pure)");
    });

    ui.horizontal(|ui| {
        ui.label("日志阈值");
        ui.add(egui::TextEdit::singleline(&mut output.min_screen).desired_width(72.0));
        ui.label("文件阈值");
        ui.add(egui::TextEdit::singleline(&mut output.min_file).desired_width(72.0));
        ui.label("小数");
        ui.add(egui::DragValue::new(&mut output.precision).range(0..=9).speed(1));
    });
}

pub(crate) fn bench_controls(ui: &mut egui::Ui, count: &mut usize, threads: &mut usize) {
    ui.horizontal(|ui| {
        ui.label("场数");
        ui.add(egui::DragValue::new(count).range(1..=10_000_000).speed(100));
        ui.label("线程");
        ui.add(egui::DragValue::new(threads).range(0..=256).speed(1));
    });
}

pub(crate) fn multiline(ui: &mut egui::Ui, id: &'static str, text: &mut String, rows: usize) {
    egui::ScrollArea::vertical().id_salt(id).max_height(rows as f32 * 22.0).show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(text)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(rows),
        );
    });
}

fn pick_output_file(default_name: &str) -> Option<PathBuf> { rfd::FileDialog::new().set_file_name(default_name).save_file() }

pub(crate) fn pick_named_output_file(default_name: &str) -> Option<PathBuf> { pick_output_file(default_name) }
