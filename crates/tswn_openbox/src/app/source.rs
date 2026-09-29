//! 文本输入源抽象。
//!
//! [`TextSource`] 支持内联文本与从文件读取两种模式，
//! 提供统一的 `read_all()`接口供后端调用，以及带预览的 egui 控件渲染（`ui()`）。

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::widgets::multiline;

#[derive(Debug, Clone)]
pub struct TextSource {
    inline: String,
    from_file: bool,
    file_path: Option<PathBuf>,
    preview: String,
    error: Option<String>,
}

impl TextSource {
    pub fn inline(text: impl Into<String>) -> Self {
        Self {
            inline: text.into(),
            from_file: false,
            file_path: None,
            preview: String::new(),
            error: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, label: &str, id: &'static str, rows: usize) {
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
                ui.colored_label(super::style::Palette::of(ui).emphasis, error);
            }
            let mut preview = self.preview.as_str();
            egui::ScrollArea::both().id_salt(id).max_height(rows as f32 * 20.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut preview)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(rows)
                        .interactive(false),
                );
            });
        } else {
            multiline(ui, id, &mut self.inline, rows);
        }
    }

    pub fn read_all(&self) -> Result<String, String> {
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
        match fs::File::open(path).and_then(read_preview) {
            Ok(preview) => {
                self.preview = preview;
                self.error = None;
            }
            Err(err) => {
                self.preview.clear();
                self.error = Some(format!("读取文件预览失败: {}: {err}", path.display()));
            }
        }
    }
}

fn read_text_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|mut content| {
            let prefix = content.len() - content.trim_start_matches('\u{feff}').len();
            if prefix > 0 {
                content.drain(..prefix);
            }
            content
        })
        .map_err(|err| format!("读取文件失败: {}: {err}", path.display()))
}

fn read_preview(reader: impl Read) -> std::io::Result<String> {
    // 预览不能因单行超长或大文件而读入全部内容；运行时仍读取并验证完整 UTF-8 输入。
    const MAX_PREVIEW_BYTES: usize = 16 * 1024;
    let mut bytes = Vec::new();
    reader.take((MAX_PREVIEW_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    bytes.truncate(MAX_PREVIEW_BYTES);
    let content = String::from_utf8_lossy(&bytes);
    let mut lines = content.trim_start_matches('\u{feff}').lines();
    let mut preview = lines.by_ref().take(10).collect::<Vec<_>>().join("\n");
    if truncated || lines.next().is_some() {
        preview.push_str("\n...");
    }
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_caps_reading_even_for_a_single_huge_line() {
        let mut reader = std::io::Cursor::new("名".repeat(100_000).into_bytes());
        let preview = read_preview(&mut reader).unwrap();
        assert!(reader.position() <= 16 * 1024 + 1);
        assert!(preview.ends_with("\n..."));
        assert!(preview.len() < 17 * 1024);
    }

    #[test]
    fn preview_handles_bom_crlf_and_exactly_ten_lines() {
        assert_eq!(read_preview("\u{feff}甲\r\n乙\r\n".as_bytes()).unwrap(), "甲\n乙");
        let ten = (0..10).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(read_preview(ten.as_bytes()).unwrap(), ten);
        assert_eq!(read_preview(format!("{ten}\n10").as_bytes()).unwrap(), format!("{ten}\n..."));
    }
}
