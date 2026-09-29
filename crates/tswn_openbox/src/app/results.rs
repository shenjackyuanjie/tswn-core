//! 三种视图共用的结果模型；只格式化变化项，布局限于当前可见行。

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use tswn_openbox::backend::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultUpdate};

use super::log::{LogBuffer, LogKind};

const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
const ROW_HEIGHT: f32 = 20.0;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Text,
    #[default]
    Cards,
    Table,
}

struct DisplayEntry {
    data: ResultEntry,
    display: String,
}

struct Record {
    label: String,
    kind: ResultKind,
    precision: usize,
    entries: BTreeMap<usize, DisplayEntry>,
    finish: Option<ResultFinish>,
    top: Option<usize>,
    bytes: usize,
    summary: String,
}

impl Record {
    fn detail_order(&self) -> Vec<usize> {
        let mut indexes: Vec<_> = self.entries.keys().copied().collect();
        if self.kind == ResultKind::Pair {
            indexes.sort_by(|a, b| {
                self.entries[b]
                    .data
                    .value
                    .unwrap_or(0.0)
                    .total_cmp(&self.entries[a].data.value.unwrap_or(0.0))
                    .then(a.cmp(b))
            });
        }
        if let Some(top) = self.top {
            indexes.truncate(top);
        }
        indexes
    }

    fn update_summary(&mut self) {
        self.summary = if let Some(score) = self.finish.as_ref().and_then(|finish| finish.score) {
            format!("{score:.precision$}", precision = self.precision)
        } else if self.kind == ResultKind::Scores {
            self.entries
                .values()
                .filter(|entry| entry.data.index < 5)
                .map(|entry| format!("{} {}", entry.data.label, entry.display))
                .collect::<Vec<_>>()
                .join("  ")
        } else {
            String::new()
        };
    }
}

pub(crate) struct ResultsView {
    pub mode: ViewMode,
    pub follow: bool,
    records: HashMap<usize, Record>,
    order: VecDeque<usize>,
    expanded: HashSet<usize>,
    rows: Vec<(usize, Option<usize>)>,
    selected: Option<usize>,
    dirty: bool,
    rendered_mode: ViewMode,
    bytes: usize,
    pub trimmed: usize,
    terminal: Option<String>,
}

impl Default for ResultsView {
    fn default() -> Self {
        Self {
            mode: ViewMode::Cards,
            follow: true,
            records: HashMap::new(),
            order: VecDeque::new(),
            expanded: HashSet::new(),
            rows: Vec::new(),
            selected: None,
            dirty: true,
            rendered_mode: ViewMode::Cards,
            bytes: 0,
            trimmed: 0,
            terminal: None,
        }
    }
}

impl ResultsView {
    pub fn clear(&mut self) {
        let mode = self.mode;
        let follow = self.follow;
        *self = Self {
            mode,
            follow,
            ..Self::default()
        };
    }

    pub fn finish(&mut self, status: &str) { self.terminal = Some(status.to_owned()); }

    pub fn apply(&mut self, update: ResultUpdate, log: &mut LogBuffer) {
        let group = update.group;
        let prefix = format!("[#{} {}]", group + 1, update.label);
        for entry in &update.entries {
            let display = display_value(entry, update.precision);
            let noun = match update.kind {
                ResultKind::Rate => "靶子",
                ResultKind::Pair => "队友",
                _ => "项目",
            };
            log.append(
                &format!("{prefix} 预览 · {noun} #{} {}: {display}", entry.index + 1, entry.label),
                log_kind(entry.kind),
            );
        }
        if let Some(finish) = &update.finish {
            // 从未展示过预览的过滤项保持静默，避免关闭屏幕输出后仍刷出每个名字。
            if !finish.visible && update.entries.is_empty() && !self.records.contains_key(&group) {
                return;
            }
            let score = finish
                .score
                .map(|value| format!(" {value:.precision$}", precision = update.precision))
                .unwrap_or_default();
            let status = if finish.visible {
                "完成"
            } else {
                "未达日志阈值或未启用屏幕输出"
            };
            log.append(
                &format!("{prefix} {status}{score}"),
                if finish.highlight {
                    LogKind::Highlight
                } else {
                    LogKind::Plain
                },
            );
            if !finish.visible {
                self.remove(group);
                return;
            }
        }
        let record = self.records.entry(group).or_insert_with(|| {
            if self.order.is_empty() {
                self.expanded.insert(group);
                self.selected = Some(group);
            }
            self.order.push_back(group);
            let bytes = size_of::<Record>() + update.label.len() + 128;
            self.bytes += bytes;
            Record {
                label: update.label.clone(),
                kind: update.kind,
                precision: update.precision,
                entries: BTreeMap::new(),
                finish: None,
                top: update.top,
                bytes,
                summary: String::new(),
            }
        });
        for entry in update.entries {
            let display = display_value(&entry, update.precision);
            let bytes = entry.bytes() + display.len() + 64;
            if let Some(old) = record.entries.insert(entry.index, DisplayEntry { data: entry, display }) {
                let old_bytes = old.data.bytes() + old.display.len() + 64;
                record.bytes -= old_bytes;
                self.bytes -= old_bytes;
            }
            record.bytes += bytes;
            self.bytes += bytes;
        }
        if update.finish.is_some() {
            record.finish = update.finish;
        }
        record.update_summary();
        self.dirty = true;
        while self.bytes > MAX_RESULT_BYTES {
            let Some(old) = self.order.front().copied() else { break };
            self.remove(old);
            self.trimmed += 1;
        }
    }

    fn remove(&mut self, group: usize) {
        if let Some(record) = self.records.remove(&group) {
            self.bytes -= record.bytes;
            if self.order.front() == Some(&group) {
                self.order.pop_front();
            } else {
                self.order.retain(|id| *id != group);
            }
            self.expanded.remove(&group);
            if self.selected == Some(group) {
                self.selected = None;
            }
            self.dirty = true;
        }
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (mode, label) in [(ViewMode::Text, "纯文本"), (ViewMode::Cards, "卡片"), (ViewMode::Table, "表格")] {
                if ui.selectable_value(&mut self.mode, mode, label).changed() {
                    self.dirty = true;
                }
            }
            ui.separator();
            ui.checkbox(&mut self.follow, "跟随最新");
            if self.trimmed > 0 {
                ui.weak(format!("已裁剪 {} 条较早记录，完整结果以输出文件为准", self.trimmed));
            }
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            // 仅收紧结果视图，输入控件仍使用全局间距。
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
            ui.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
            ui.spacing_mut().interact_size.y = ROW_HEIGHT;
            self.content_ui(ui);
        });
    }

    fn content_ui(&mut self, ui: &mut egui::Ui) {
        if self.records.is_empty() {
            ui.weak("暂无结果；运行后将在这里显示已完成的明细。");
            return;
        }
        if self.dirty || self.rendered_mode != self.mode {
            self.rows.clear();
            for &group in &self.order {
                self.rows.push((group, None));
                if self.mode == ViewMode::Cards && self.expanded.contains(&group) {
                    self.rows
                        .extend(self.records[&group].detail_order().into_iter().map(|index| (group, Some(index))));
                }
            }
            self.dirty = false;
            self.rendered_mode = self.mode;
        }
        let table = self.mode == ViewMode::Table;
        let scores = self.order.front().is_some_and(|id| self.records[id].kind == ResultKind::Scores);
        if table {
            ui.horizontal(|ui| {
                cell(ui, 180.0, "名字 / 输入序号");
                cell(ui, 130.0, "状态");
                if scores {
                    for label in ["pp", "pd", "qp", "qd", "sum"] {
                        cell(ui, 72.0, label);
                    }
                } else {
                    cell(ui, 100.0, "分数");
                }
            });
            ui.separator();
        }
        let height = if table && self.selected.is_some() {
            ui.available_height() * 0.55
        } else {
            ui.available_height()
        };
        if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|input| input.smooth_scroll_delta.y > 0.0) {
            self.follow = false;
        }
        let mut toggle = None;
        egui::ScrollArea::both()
            .id_salt(if table { "results_table" } else { "results_cards" })
            .auto_shrink([false, table])
            .max_height(height.max(60.0))
            .stick_to_bottom(self.follow)
            .show_rows(ui, ROW_HEIGHT, self.rows.len(), |ui, range| {
                for row in range {
                    let (group, detail) = self.rows[row];
                    let record = &self.records[&group];
                    ui.push_id((group, detail), |ui| {
                        if let Some(index) = detail {
                            let entry = &record.entries[&index];
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                entry_ui(ui, entry);
                            });
                        } else {
                            let state = if record.finish.is_some() {
                                "完成"
                            } else {
                                self.terminal.as_deref().unwrap_or("计算中 · 预览")
                            };
                            let state = if record.finish.is_none() && self.terminal.is_some() {
                                format!("{state} · 结果不完整")
                            } else {
                                state.to_owned()
                            };
                            if table {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            [180.0, ROW_HEIGHT],
                                            egui::Button::selectable(
                                                self.selected == Some(group),
                                                format!("#{} {}", group + 1, record.label),
                                            )
                                            .truncate(),
                                        )
                                        .clicked()
                                    {
                                        self.selected = Some(group);
                                    }
                                    cell(ui, 130.0, &state);
                                    if scores {
                                        for index in 0..5 {
                                            let text = record.entries.get(&index).map_or("—", |entry| &entry.display);
                                            let mut label = egui::RichText::new(text);
                                            if record
                                                .entries
                                                .get(&index)
                                                .is_some_and(|entry| entry.data.kind == EntryKind::Highlight)
                                            {
                                                label = label.color(egui::Color32::from_rgb(210, 40, 40));
                                            }
                                            ui.add_sized([72.0, ROW_HEIGHT], egui::Label::new(label).truncate());
                                        }
                                    } else {
                                        ui.label(summary_text(record));
                                    }
                                });
                            } else {
                                let arrow = if self.expanded.contains(&group) { "▼" } else { "▶" };
                                let top = if record.top.is_some() && record.finish.is_none() {
                                    " · 当前 Top"
                                } else {
                                    ""
                                };
                                let label =
                                    format!("{arrow} #{} {}   {}   {state}{top}", group + 1, record.label, record.summary);
                                let text = if record.finish.as_ref().is_some_and(|f| f.highlight) {
                                    egui::RichText::new(label).color(egui::Color32::from_rgb(210, 40, 40))
                                } else {
                                    egui::RichText::new(label)
                                };
                                if ui
                                    .add_sized(
                                        [ui.available_width().max(300.0), ROW_HEIGHT],
                                        egui::Button::new(text).frame(true).truncate(),
                                    )
                                    .clicked()
                                {
                                    toggle = Some(group);
                                }
                            }
                        }
                    });
                }
            });
        if let Some(group) = toggle {
            if !self.expanded.remove(&group) {
                self.expanded.insert(group);
            }
            self.dirty = true;
        }
        if table && let Some(record) = self.selected.and_then(|id| self.records.get(&id)) {
            ui.separator();
            ui.label(egui::RichText::new(&record.label).strong());
            if record.top.is_some() && record.finish.is_none() {
                ui.weak("当前 Top，全部队友完成后确定最终排名");
            }
            let details = record.detail_order();
            egui::ScrollArea::both()
                .id_salt("selected_result_details")
                .show_rows(ui, ROW_HEIGHT, details.len(), |ui, rows| {
                    for index in rows {
                        entry_ui(ui, &record.entries[&details[index]]);
                    }
                });
        }
    }
}

fn cell(ui: &mut egui::Ui, width: f32, text: &str) { ui.add_sized([width, ROW_HEIGHT], egui::Label::new(text).truncate()); }

fn summary_text(record: &Record) -> egui::RichText {
    let text = egui::RichText::new(&record.summary);
    if record.finish.as_ref().is_some_and(|f| f.highlight) {
        text.color(egui::Color32::from_rgb(210, 40, 40))
    } else {
        text
    }
}

fn entry_ui(ui: &mut egui::Ui, entry: &DisplayEntry) {
    let text = egui::RichText::new(format!("#{} {}   {}", entry.data.index + 1, entry.data.label, entry.display));
    let text = match entry.data.kind {
        EntryKind::Plain => text,
        EntryKind::Highlight => text.color(egui::Color32::from_rgb(210, 40, 40)).strong(),
        EntryKind::SkillBoard => text.color(egui::Color32::from_rgb(45, 120, 220)).strong(),
    };
    ui.add(egui::Label::new(text).extend().selectable(true));
}

fn log_kind(kind: EntryKind) -> LogKind {
    match kind {
        EntryKind::Plain => LogKind::Plain,
        EntryKind::Highlight => LogKind::Highlight,
        EntryKind::SkillBoard => LogKind::SkillBoard,
    }
}

fn display_value(entry: &ResultEntry, precision: usize) -> String {
    if !entry.text.is_empty() {
        entry.text.clone()
    } else {
        entry.value.map(|value| format!("{value:.precision$}")).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_result_lists_only_layout_visible_rows_in_both_render_modes() {
        let mut view = ResultsView::default();
        let mut log = LogBuffer::default();
        for group in 0..10_000 {
            let mut update = ResultUpdate::new(group, "alpha", ResultKind::Pair, 3);
            update.top = Some(2);
            update.entries.push(ResultEntry::number(2, "mate2".into(), 60.0));
            update.entries.push(ResultEntry::number(0, "mate0".into(), 60.0));
            update.entries.push(ResultEntry::number(1, "mate1".into(), 80.0));
            view.apply(update, &mut log);
        }
        assert_eq!(view.records[&9999].detail_order(), vec![1, 0]);
        let ctx = egui::Context::default();
        for mode in [ViewMode::Cards, ViewMode::Table] {
            view.mode = mode;
            view.dirty = true;
            view.selected = Some(9999);
            view.expanded.insert(9999);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0))),
                    ..Default::default()
                },
                |ui| {
                    view.ui(ui);
                },
            );
            assert!(output.shapes.len() < 1000, "应只布局可见结果，而不是遍历绘制全部记录");
            output.textures_delta.clear();
        }
        assert!(view.bytes <= MAX_RESULT_BYTES);
        let mut oversized = ResultUpdate::new(10_001, "大结果", ResultKind::Diy, 0);
        oversized.entries.push(ResultEntry {
            index: 0,
            label: "导出".into(),
            value: None,
            text: "x".repeat(MAX_RESULT_BYTES),
            kind: EntryKind::Plain,
        });
        view.apply(oversized, &mut log);
        assert!(view.bytes <= MAX_RESULT_BYTES);
        assert!(view.trimmed > 0);
    }

    #[test]
    fn duplicate_names_and_out_of_order_details_stay_separate() {
        let mut view = ResultsView::default();
        let mut log = LogBuffer::default();
        for (group, target, value) in [(1, 2, 70.0), (0, 1, 40.0), (1, 0, 60.0)] {
            let mut update = ResultUpdate::new(group, "重名", ResultKind::Rate, 2);
            update.entries.push(ResultEntry::number(target, "相同靶子".into(), value));
            view.apply(update, &mut log);
        }
        assert_eq!(view.order.iter().copied().collect::<Vec<_>>(), vec![1, 0]);
        assert_eq!(view.records[&1].detail_order(), vec![0, 2]);
        assert_eq!(view.records[&0].entries[&1].data.value, Some(40.0));
        let mut rejected = ResultUpdate::new(1, "重名", ResultKind::Rate, 2);
        rejected.finish = Some(ResultFinish {
            score: Some(65.0),
            visible: false,
            highlight: false,
        });
        view.apply(rejected, &mut log);
        assert!(!view.records.contains_key(&1));
        assert!(log.copy_text().contains("预览"));
        assert!(log.copy_text().contains("未达日志阈值"));
        view.finish("已停止");
        assert!(view.records[&0].finish.is_none());
        view.clear();
        assert!(view.records.is_empty());
    }

    #[test]
    fn filtered_result_without_preview_does_not_create_text_output() {
        let mut view = ResultsView::default();
        let mut log = LogBuffer::default();
        let mut update = ResultUpdate::new(0, "仅写文件", ResultKind::Scores, 0);
        update.finish = Some(ResultFinish {
            score: None,
            visible: false,
            highlight: false,
        });
        view.apply(update, &mut log);
        assert!(log.is_empty());
        assert!(view.records.is_empty());
    }
}
