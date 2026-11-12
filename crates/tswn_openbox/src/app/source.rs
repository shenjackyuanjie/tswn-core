//! GUI 文本输入源，统一内联文本与文件选择。
//!
//! 预览只读取前 16 KiB 并最多展示十行，允许有损 UTF-8 截断；实际运行时重新
//! 完整读取并严格验证 UTF-8，去除开头 BOM，因此预览不是任务输入的持久快照。
//! read_all 返回自有 String，不让后台任务借用控件状态；读失败返回含路径的错误。
//!
//! CSV 是可选读取方式，默认关闭：关闭时按整行读取，行为与纯文本文件完全一致。
//! 启用后使用本模块自带的手写解析取指定列，并按“每两行一组”的行筛选跳过表头或
//! 交替行；不引入第三方 CSV 依赖，也不改变后端的输入格式（仍然每行一组名字）。

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::style::Palette;
use super::widgets::multiline;

/// 高级选项里表格取样展示的行数上限；不影响实际读取。
const CSV_SAMPLE_ROWS: usize = 12;
/// 单元格最多显示的字符数，过长只截断显示，不改变提取结果。
const CSV_CELL_PREVIEW_CHARS: usize = 24;
/// 结果预览最多展示的行数。
const CSV_RESULT_PREVIEW_LINES: usize = 10;

/// CSV 读取选项；默认关闭，因此未使用该功能的输入不受影响。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvOptions {
    /// 是否按 CSV 解析。
    pub enabled: bool,
    /// 取第几列，0 基。
    pub column: usize,
    /// 从第几条非空记录开始算数据，1 基；表头常见填 2。
    pub start_row: usize,
    /// 每两行一组时，组内第一行是否算数据。
    pub include_first: bool,
    /// 每两行一组时，组内第二行是否算数据。
    pub include_second: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            column: 0,
            start_row: 1,
            include_first: true,
            include_second: true,
        }
    }
}

/// 高级选项表格用的取样结果，只保留开头若干行。
#[derive(Debug, Default, Clone)]
struct CsvSample {
    rows: Vec<Vec<String>>,
    columns: usize,
    total_rows: usize,
}

impl CsvSample {
    fn of(content: &str) -> Self {
        let mut rows = Vec::new();
        let mut columns = 0;
        let mut total_rows = 0;
        for row in csv_rows(content) {
            total_rows += 1;
            columns = columns.max(row.len());
            if rows.len() < CSV_SAMPLE_ROWS {
                rows.push(row);
            }
        }
        Self {
            rows,
            columns,
            total_rows,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextSource {
    inline: String,
    from_file: bool,
    file_path: Option<PathBuf>,
    preview: String,
    error: Option<String>,
    csv: CsvOptions,
    advanced_open: bool,
    sample: CsvSample,
}

impl TextSource {
    pub fn inline(text: impl Into<String>) -> Self {
        Self {
            inline: text.into(),
            from_file: false,
            file_path: None,
            preview: String::new(),
            error: None,
            csv: CsvOptions::default(),
            advanced_open: false,
            sample: CsvSample::default(),
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
                if self.file_path.is_some() && ui.button("高级选项").clicked() {
                    self.advanced_open = true;
                    self.reload_sample();
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
                ui.colored_label(Palette::of(ui).emphasis, error);
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

    /// 渲染 CSV 高级选项窗口；关闭时不绘制，也不读取任何文件。
    ///
    /// 同一页可能有多个文本来源，`id` 与输入框的滚动 id 相同，窗口之间互不干扰。
    pub fn advanced_window(&mut self, ctx: &egui::Context, id: &'static str) {
        if !self.advanced_open {
            return;
        }
        let mut open = true;
        egui::Window::new("文本来源 · 高级选项")
            .id(egui::Id::unique((id, "advanced")))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(720.0)
            .default_height(620.0)
            .show(ctx, |ui| self.advanced_ui(ui, id));
        self.advanced_open = open;
    }

    /// 仅供界面截图与测试构造：直接绑定文件并设定 CSV 选项，不经过文件对话框。
    #[cfg(feature = "ui_capture")]
    pub fn from_csv_file(path: PathBuf, csv: CsvOptions, advanced_open: bool) -> Self {
        let mut source = Self::inline("");
        source.from_file = true;
        source.csv = csv;
        source.advanced_open = advanced_open;
        source.file_path = Some(path);
        source.reload_sample();
        source.refresh_preview();
        source
    }

    pub fn read_all(&self) -> Result<String, String> {
        if !self.from_file {
            return Ok(self.inline.clone());
        }
        let path = self.file_path.as_ref().ok_or_else(|| "请选择输入文件。".to_string())?;
        let content = read_text_file(path)?;
        if self.csv.enabled {
            csv_extract(&content, &self.csv)
        } else {
            Ok(content)
        }
    }

    fn advanced_ui(&mut self, ui: &mut egui::Ui, id: &'static str) {
        let path_label = self
            .file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未选择输入文件".to_string());
        ui.label(path_label);
        if self.file_path.is_none() {
            super::style::hint(ui, "先勾选“从文件中读取”并选好文件，再在这里调整读取方式。");
            return;
        }

        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut self.csv.enabled, "按 CSV 解析").changed();
            if ui.button("刷新").clicked() {
                changed = true;
            }
            if ui.button("恢复默认").clicked() {
                self.csv = CsvOptions::default();
                changed = true;
            }
        });
        if changed {
            self.reload_sample();
            self.refresh_preview();
        }
        if !self.csv.enabled {
            super::style::hint(ui, "未启用 CSV：仍按整行读取，与旧版本行为一致。");
            if let Some(error) = &self.error {
                ui.colored_label(Palette::of(ui).emphasis, error);
            }
            return;
        }

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("从第");
            changed |= ui.add(egui::DragValue::new(&mut self.csv.start_row).range(1..=99_999).speed(1)).changed();
            ui.label("行开始");
            changed |= ui.checkbox(&mut self.csv.include_first, "包含第一行").changed();
            changed |= ui.checkbox(&mut self.csv.include_second, "包含第二行").changed();
        });
        ui.horizontal(|ui| {
            ui.label("取第");
            let mut column = self.csv.column + 1;
            if ui.add(egui::DragValue::new(&mut column).range(1..=999).speed(1)).changed() {
                self.csv.column = column - 1;
                changed = true;
            }
            ui.label(format!(
                "列（共 {} 列，{} 行）",
                self.sample.columns.max(1),
                self.sample.total_rows
            ));
        });
        super::style::hint(
            ui,
            "两个“包含”都勾选时取整份文件；只勾一个时每两行取该行，“从第 N 行开始”用来跳过表头。行号后带 × 的行不参与提取。",
        );
        if changed {
            self.refresh_preview();
        }

        ui.separator();
        ui.label(egui::RichText::new("文本预览（点列头或单元格选择该列）").strong());
        let sample_rows = &self.sample.rows;
        let columns = self.sample.columns.max(1);
        let mut options = self.csv.clone();
        egui::ScrollArea::both().id_salt((id, "csv_table")).max_height(220.0).show(ui, |ui| {
            egui::Grid::new((id, "csv_table_grid")).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.weak("行");
                for column in 0..columns {
                    if ui.selectable_label(options.column == column, format!("列 {}", column + 1)).clicked() {
                        options.column = column;
                    }
                }
                ui.end_row();
                for (index, row) in sample_rows.iter().enumerate() {
                    let row_number = index + 1;
                    let included = row_number >= options.start_row && csv_row_selected(row_number - options.start_row, &options);
                    if included {
                        ui.weak(row_number.to_string());
                    } else {
                        ui.weak(format!("{row_number} ×"));
                    }
                    for column in 0..columns {
                        let text = row.get(column).map(|value| clip_cell(value)).unwrap_or_default();
                        // 选中列保持正常前景，其余列弱化，避免深色主题下整张表都像被选中。
                        let text = if options.column == column {
                            egui::RichText::new(text)
                        } else {
                            egui::RichText::new(text).weak()
                        };
                        if ui.selectable_label(options.column == column, text).clicked() {
                            options.column = column;
                        }
                    }
                    ui.end_row();
                }
            });
        });
        let column_changed = options.column != self.csv.column;
        self.csv = options;
        if column_changed {
            self.refresh_preview();
        }

        ui.separator();
        ui.label(egui::RichText::new(format!("提取结果（前 {CSV_RESULT_PREVIEW_LINES} 行）")).strong());
        if let Some(error) = &self.error {
            ui.colored_label(Palette::of(ui).emphasis, error);
        } else {
            egui::ScrollArea::vertical()
                .id_salt((id, "csv_extract"))
                .max_height(120.0)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(self.preview.as_str()).monospace()).wrap());
                });
        }
    }

    fn pick_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            // 换文件时按扩展名重新判断是否默认启用 CSV，用户仍可在高级选项里关掉。
            self.csv = CsvOptions {
                enabled: looks_like_csv(&path),
                ..CsvOptions::default()
            };
            self.file_path = Some(path);
            self.sample = CsvSample::default();
            self.refresh_preview();
        }
    }

    fn refresh_preview(&mut self) {
        let Some(path) = self.file_path.clone() else {
            self.preview.clear();
            self.error = None;
            return;
        };
        if self.csv.enabled {
            match read_text_file(&path) {
                Ok(content) => {
                    self.sample = CsvSample::of(&content);
                    match csv_extract(&content, &self.csv) {
                        Ok(extracted) => {
                            self.preview = head_lines(&extracted, CSV_RESULT_PREVIEW_LINES);
                            self.error = None;
                        }
                        Err(err) => {
                            self.preview.clear();
                            self.error = Some(err);
                        }
                    }
                }
                Err(err) => {
                    self.preview.clear();
                    self.sample = CsvSample::default();
                    self.error = Some(err);
                }
            }
            return;
        }
        match fs::File::open(&path).and_then(read_preview) {
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

    fn reload_sample(&mut self) {
        if !self.csv.enabled {
            return;
        }
        let Some(path) = self.file_path.as_ref() else {
            return;
        };
        if let Ok(content) = read_text_file(path) {
            self.sample = CsvSample::of(&content);
        }
    }
}

/// 按 CSV 规则取出指定列，每行一个值供后端按“每行一组”解析。
///
/// 空行与空单元格会被跳过；列越界或所有行都被排除时返回错误而不是空输入，
/// 让调用方在启动任务前就能看到原因。
pub fn csv_extract(content: &str, options: &CsvOptions) -> Result<String, String> {
    let rows = csv_rows(content);
    if rows.is_empty() {
        return Ok(String::new());
    }
    if !options.include_first && !options.include_second {
        return Err("CSV 已排除所有行：请至少勾选包含第一行或包含第二行。".to_string());
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut picked = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index + 1 < options.start_row {
            continue;
        }
        if !csv_row_selected(index + 1 - options.start_row, options) {
            continue;
        }
        let Some(value) = row.get(options.column) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        picked.push(value);
    }
    if picked.is_empty() {
        return Err(format!(
            "CSV 第 {} 列（共 {columns} 列）从第 {} 行起没有可用内容。",
            options.column + 1,
            options.start_row
        ));
    }
    Ok(picked.join("\n"))
}

/// 每两行一组：相对索引 0 属于组内第一行，1 属于组内第二行。
fn csv_row_selected(relative_index: usize, options: &CsvOptions) -> bool {
    if relative_index.is_multiple_of(2) {
        options.include_first
    } else {
        options.include_second
    }
}

fn csv_rows(content: &str) -> Vec<Vec<String>> {
    content.lines().filter(|line| !line.trim().is_empty()).map(split_csv_line).collect()
}

/// 按逗号拆分一行，支持双引号包裹字段与 `""` 转义；没有引号时等价于简单拆分。
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

fn looks_like_csv(path: &Path) -> bool { path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("csv")) }

fn clip_cell(value: &str) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let head: String = chars.by_ref().take(CSV_CELL_PREVIEW_CHARS).collect();
    if chars.next().is_some() { format!("{head}…") } else { head }
}

fn head_lines(text: &str, limit: usize) -> String {
    let mut lines = text.lines();
    let mut preview = lines.by_ref().take(limit).collect::<Vec<_>>().join("\n");
    if lines.next().is_some() {
        preview.push_str("\n...");
    }
    preview
}

fn read_text_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|mut content| {
            let prefix = content.len() - content.trim_start_matches('﻿').len();
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
    let mut lines = content.trim_start_matches('﻿').lines();
    let mut preview = lines.by_ref().take(10).collect::<Vec<_>>().join("\n");
    if truncated || lines.next().is_some() {
        preview.push_str("\n...");
    }
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("tswn_openbox_source_{name}.txt"));
        fs::write(&path, content).unwrap();
        path
    }

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

    #[test]
    fn split_csv_line_handles_quotes_and_escapes() {
        assert_eq!(split_csv_line("a,b,c"), vec!["a", "b", "c"]);
        assert_eq!(split_csv_line("a,\"b,c\",d"), vec!["a", "b,c", "d"]);
        assert_eq!(split_csv_line("\"say \"\"hi\"\"\",x"), vec!["say \"hi\"", "x"]);
        assert_eq!(split_csv_line(""), vec![""]);
    }

    #[test]
    fn csv_extract_keeps_every_row_by_default() {
        assert_eq!(csv_extract("甲,1\n乙,2\n丙,3\n", &CsvOptions::default()).unwrap(), "甲\n乙\n丙");
    }

    #[test]
    fn csv_extract_skips_header_and_can_take_one_row_per_pair() {
        let content = "名字,分\n甲,1\n乙,2\n丙,3\n";
        let header_skipped = CsvOptions {
            start_row: 2,
            ..CsvOptions::default()
        };
        assert_eq!(csv_extract(content, &header_skipped).unwrap(), "甲\n乙\n丙");

        let first_only = CsvOptions {
            start_row: 2,
            include_first: true,
            include_second: false,
            ..CsvOptions::default()
        };
        assert_eq!(csv_extract(content, &first_only).unwrap(), "甲\n丙");

        let second_only = CsvOptions {
            start_row: 2,
            include_first: false,
            include_second: true,
            ..CsvOptions::default()
        };
        assert_eq!(csv_extract(content, &second_only).unwrap(), "乙");
    }

    #[test]
    fn csv_extract_reports_missing_column_and_empty_selection() {
        let content = "甲,1\n乙,2\n";
        assert!(
            csv_extract(
                content,
                &CsvOptions {
                    column: 5,
                    ..CsvOptions::default()
                }
            )
            .is_err()
        );
        assert!(
            csv_extract(
                content,
                &CsvOptions {
                    include_first: false,
                    include_second: false,
                    ..CsvOptions::default()
                }
            )
            .is_err()
        );
        assert!(csv_extract("\n\n", &CsvOptions::default()).unwrap().is_empty());
    }

    #[test]
    fn file_mode_reads_whole_lines_until_csv_is_enabled() {
        let mut source = TextSource::inline("");
        source.from_file = true;
        source.file_path = Some(temp_file("plain", "甲\n乙\n"));
        assert_eq!(source.read_all().unwrap(), "甲\n乙\n");

        source.file_path = Some(temp_file("columns", "\u{feff}甲,1\n乙,2\n"));
        source.csv = CsvOptions {
            enabled: true,
            column: 1,
            ..CsvOptions::default()
        };
        assert_eq!(source.read_all().unwrap(), "1\n2");
    }
}
