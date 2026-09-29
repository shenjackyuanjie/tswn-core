//! 结果控件与可见行绘制；数据更新、索引及裁剪由父模块维护。

use super::{DisplayEntry, Record, ResultsView, ViewMode};
use tswn_openbox::backend::live::{EntryKind, ResultKind};

const ROW_HEIGHT: f32 = 20.0;

impl ResultsView {
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
                    let record = self.records.get_mut(&group).unwrap();
                    record.refresh_detail_order();
                    self.rows.extend(record.detail_indexes.iter().map(|&index| (group, Some(index))));
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
        if table && let Some(record) = self.selected.and_then(|id| self.records.get_mut(&id)) {
            record.refresh_detail_order();
            ui.separator();
            ui.label(egui::RichText::new(&record.label).strong());
            if record.top.is_some() && record.finish.is_none() {
                ui.weak("当前 Top，全部队友完成后确定最终排名");
            }
            let details = &record.detail_indexes;
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
