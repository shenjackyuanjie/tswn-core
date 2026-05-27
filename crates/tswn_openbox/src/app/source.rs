//! 文本输入源抽象。
//!
//! [`TextSource`] 支持内联文本与从文件读取两种模式，
//! 提供统一的 `read_all()`接口供后端调用，以及带预览的 egui 控件渲染（`ui()`）。

use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui;

use super::widgets::multiline;

#[derive(Debug, Clone)]
pub(crate) struct TextSource {
    inline: String,
    from_file: bool,
    file_path: Option<PathBuf>,
    preview: String,
    error: Option<String>,
}

impl TextSource {
    pub(crate) fn inline(text: impl Into<String>) -> Self {
        Self {
            inline: text.into(),
            from_file: false,
            file_path: None,
            preview: String::new(),
            error: None,
        }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, label: &str, id: &'static str, rows: usize) {
        ui.horizontal(|ui| {
            ui.label(label);
            let was_from_file = self.from_file;
            if ui.checkbox(&mut self.from_file, "从文件中读取").changed() {
                if self.from_file && !was_from_file {
                    self.pick_file();
                }
                if !self.from_file {
                    self.error = None;
                }
            }
            if self.from_file {
                if ui.button("选择文件").clicked() {
                    self.pick_file();
                }
                if self.file_path.is_some() && ui.button("刷新预览").clicked() {
                    self.refresh_preview();
                }
            } else if ui.button("清空").clicked() {
                self.inline.clear();
            }
        });

        if self.from_file {
            let path = self
                .file_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "未选择文件".to_string());
            ui.label(path);
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
            }
            let mut preview = self.preview.clone();
            egui::ScrollArea::vertical().id_salt(id).max_height(rows as f32 * 22.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut preview)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(rows)
                        .interactive(false),
                );
            });
        } else {
            multiline(ui, id, &mut self.inline, rows);
        }
    }

    pub(crate) fn read_all(&self) -> Result<String, String> {
        if !self.from_file {
            return Ok(self.inline.clone());
        }
        let path = self.file_path.as_ref().ok_or_else(|| "请选择输入文件。".to_string())?;
        read_text_file(path)
    }

    fn pick_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.file_path = Some(path);
            self.refresh_preview();
        }
    }

    fn refresh_preview(&mut self) {
        let Some(path) = &self.file_path else {
            self.preview.clear();
            self.error = None;
            return;
        };
        match read_text_file(path) {
            Ok(content) => {
                let line_count = content.lines().count();
                let mut preview = content.lines().take(10).collect::<Vec<_>>().join("\n");
                if line_count > 10 {
                    preview.push_str("\n...");
                }
                self.preview = preview;
                self.error = None;
            }
            Err(err) => {
                self.preview.clear();
                self.error = Some(err);
            }
        }
    }
}

fn read_text_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|content| content.trim_start_matches('\u{feff}').to_string())
        .map_err(|err| format!("读取文件失败: {}: {err}", path.display()))
}
