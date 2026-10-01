//! 结果控件与可见行绘制；数据更新、索引及裁剪由父模块维护。

use super::{ColumnAlign, DisplayEntry, Record, ResultsView, ViewMode};
use crate::app::help::HelpTopic;
use crate::app::style::Palette;
use tswn_openbox::backend::live::{EntryKind, ResultKind};

const ROW_HEIGHT: f32 = 24.0;
const CELL_GAP: f32 = 4.0;
const COLUMN_LABELS: [&str; 7] = ["名字 / 输入序号", "状态", "pp", "pd", "qp", "qd", "sum"];

impl ResultsView {
    pub fn controls(&mut self, ui: &mut egui::Ui, active_help: &mut Option<HelpTopic>, kind: ResultKind) {
        ui.horizontal_wrapped(|ui| {
            for (mode, label) in [(ViewMode::Text, "纯文本"), (ViewMode::Cards, "卡片"), (ViewMode::Table, "表格")] {
                let hint = match mode {
                    ViewMode::Text => "沿用旧版结果格式；完整结果块完成后追加，方便复制。",
                    ViewMode::Cards => "点击卡片标题展开或收起该输入组的明细。",
                    ViewMode::Table => "横向比较多个名字的分数；点击名字查看明细，拖动表头右边界调整列宽。",
                };
                if ui.selectable_value(&mut self.mode, mode, label).on_hover_text(hint).changed() {
                    self.dirty = true;
                }
            }
            ui.separator();
            ui.checkbox(&mut self.follow, "跟随最新")
                .on_hover_text("向上滚动会暂停跟随；重新勾选后继续跟随最新结果。");
            if self.mode != ViewMode::Text {
                ui.menu_button("排版设置", |ui| self.layout_controls(ui, kind));
            }
            if ui.small_button("说明").clicked() {
                *active_help = Some(HelpTopic::LiveResults);
            }
            if self.trimmed > 0 {
                ui.weak(format!("已裁剪 {} 条较早记录，完整结果以输出文件为准", self.trimmed));
            }
        });
        // 纯文本不混入结构化视图的预览图例。
        if self.mode != ViewMode::Text {
            let palette = Palette::of(ui);
            ui.horizontal_wrapped(|ui| {
                for (color, label, hint) in [
                    (palette.info, "预览", "明细已完成，整组仍在计算；#序号对应原输入位置。"),
                    (palette.success, "完成", "该输入组已全部完成，显示最终结果。"),
                    (palette.warning, "未完成", "任务已停止，未算完的组不能作为最终结果。"),
                    (palette.emphasis, "高亮", "达到高亮阈值，或属性相对单独构建发生变化。"),
                ] {
                    ui.label(egui::RichText::new(format!("● {label}")).size(13.0).color(color))
                        .on_hover_text(hint);
                }
            });
        }
    }

    fn layout_controls(&mut self, ui: &mut egui::Ui, kind: ResultKind) {
        if self.mode == ViewMode::Cards {
            ui.horizontal(|ui| {
                ui.label("卡片对齐");
                alignment_controls(ui, &mut self.card_align);
            });
            if ui.button("恢复默认排版").clicked() {
                self.card_align = ColumnAlign::Left;
            }
            return;
        }
        ui.label("列宽 / 对齐（也可拖动表头右边界）");
        egui::Grid::new("result_column_settings").show(ui, |ui| {
            for index in 0..column_count(kind) {
                ui.label(column_label(index, kind == ResultKind::Scores));
                ui.add(
                    egui::DragValue::new(&mut self.column_widths[index])
                        .range(60.0..=640.0)
                        .speed(1.0)
                        .suffix(" px"),
                );
                ui.horizontal(|ui| alignment_controls(ui, &mut self.column_alignments[index]));
                ui.end_row();
            }
        });
        if ui.button("恢复默认排版").clicked() {
            let defaults = Self::default();
            self.column_widths = defaults.column_widths;
            self.column_alignments = defaults.column_alignments;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(CELL_GAP, 2.0);
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
        let kind = self.records[self.order.front().unwrap()].kind;
        let scores = kind == ResultKind::Scores;
        let height = if table && self.selected.is_some() {
            ui.available_height() * 0.55
        } else {
            ui.available_height()
        };
        if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|input| input.smooth_scroll_delta.y > 0.0) {
            self.follow = false;
        }
        let mut toggle = None;
        // 表头与数据共享横向滚动；垂直方向仍然只布局可见行。
        egui::ScrollArea::horizontal()
            .id_salt(if table { "results_table_x" } else { "results_cards_x" })
            .auto_shrink([false, table])
            .show(ui, |ui| {
                let width = if table {
                    let count = column_count(kind);
                    self.column_widths[..count].iter().sum::<f32>() + CELL_GAP * (count - 1) as f32
                } else {
                    ui.available_width().max(300.0)
                };
                ui.set_min_width(width);
                if table {
                    ui.horizontal(|ui| {
                        for index in 0..column_count(kind) {
                            let response = cell(
                                ui,
                                self.column_widths[index],
                                egui::RichText::new(column_label(index, kind == ResultKind::Scores)).strong(),
                                self.column_alignments[index],
                                false,
                                None,
                            );
                            resize_column(ui, index, response.rect, &mut self.column_widths[index]);
                        }
                    });
                    ui.separator();
                }
                egui::ScrollArea::vertical()
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
                                    ui.horizontal(|ui| {
                                        ui.add_space(12.0);
                                        entry_ui(ui, &record.entries[&index], (width - 12.0).max(60.0), self.card_align);
                                    });
                                    return;
                                }
                                let palette = Palette::of(ui);
                                let state = if record.finish.is_some() {
                                    "完成".to_owned()
                                } else if let Some(terminal) = &self.terminal {
                                    format!("{terminal} · 结果不完整")
                                } else {
                                    "计算中 · 预览".to_owned()
                                };
                                let state_color = palette.status(if record.finish.is_some() {
                                    "完成"
                                } else {
                                    self.terminal.as_deref().unwrap_or("运行中")
                                });
                                if table {
                                    ui.horizontal(|ui| {
                                        let fill = if self.selected == Some(group) {
                                            ui.visuals().selection.bg_fill
                                        } else if row % 2 == 0 {
                                            ui.visuals().faint_bg_color
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        };
                                        if cell(
                                            ui,
                                            self.column_widths[0],
                                            egui::RichText::new(format!("#{} {}", group + 1, record.label)),
                                            self.column_alignments[0],
                                            true,
                                            Some(fill),
                                        )
                                        .clicked()
                                        {
                                            self.selected = Some(group);
                                        }
                                        cell(
                                            ui,
                                            self.column_widths[1],
                                            egui::RichText::new(&state).color(state_color),
                                            self.column_alignments[1],
                                            false,
                                            Some(fill),
                                        );
                                        if scores {
                                            for index in 0..5 {
                                                let entry = record.entries.get(&index);
                                                let mut text = egui::RichText::new(entry.map_or("—", |entry| &entry.display));
                                                if entry.is_some_and(|entry| entry.data.kind == EntryKind::Highlight) {
                                                    text = text.color(palette.emphasis);
                                                }
                                                cell(
                                                    ui,
                                                    self.column_widths[index + 2],
                                                    text,
                                                    self.column_alignments[index + 2],
                                                    false,
                                                    Some(fill),
                                                );
                                            }
                                        } else if kind != ResultKind::Diy {
                                            cell(
                                                ui,
                                                self.column_widths[2],
                                                summary_text(record, palette),
                                                self.column_alignments[2],
                                                false,
                                                Some(fill),
                                            );
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
                                    let text = if record.finish.as_ref().is_some_and(|finish| finish.highlight) {
                                        egui::RichText::new(label).color(palette.emphasis)
                                    } else {
                                        egui::RichText::new(label)
                                    };
                                    if cell(ui, width, text, self.card_align, true, Some(state_color.gamma_multiply(0.1)))
                                        .on_hover_text("点击展开或收起明细。序号按原输入标记，多个线程的结果可能交错到达。")
                                        .clicked()
                                    {
                                        toggle = Some(group);
                                    }
                                }
                            });
                        }
                    });
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
            egui::ScrollArea::both().id_salt("selected_result_details").show_rows(
                ui,
                ROW_HEIGHT,
                record.detail_indexes.len(),
                |ui, rows| {
                    let width = ui.available_width().max(300.0);
                    for index in rows {
                        entry_ui(ui, &record.entries[&record.detail_indexes[index]], width, self.card_align);
                    }
                },
            );
        }
    }
}

fn column_count(kind: ResultKind) -> usize {
    match kind {
        ResultKind::Diy => 2,
        ResultKind::Scores => 7,
        ResultKind::Rate | ResultKind::Pair => 3,
    }
}

fn column_label(index: usize, scores: bool) -> &'static str {
    if !scores && index == 2 {
        "分数"
    } else {
        COLUMN_LABELS[index]
    }
}

fn alignment_controls(ui: &mut egui::Ui, alignment: &mut ColumnAlign) {
    for align in ColumnAlign::ALL {
        ui.selectable_value(alignment, align, align.label());
    }
}

fn resize_column(ui: &mut egui::Ui, index: usize, rect: egui::Rect, width: &mut f32) {
    let handle = egui::Rect::from_min_max(
        egui::pos2(rect.right() - 4.0, rect.top()),
        egui::pos2(rect.right() + 2.0, rect.bottom()),
    );
    let response = ui
        .interact(handle, ui.id().with(("column_resize", index)), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if response.dragged() {
        *width = (*width + ui.input(|input| input.pointer.delta().x)).clamp(60.0, 640.0);
    }
    ui.painter()
        .vline(rect.right(), rect.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
}

/// 固定尺寸且不换行，避免长导出行或多行字段破坏虚拟列表行高。
fn cell(
    ui: &mut egui::Ui,
    width: f32,
    text: impl Into<egui::WidgetText>,
    align: ColumnAlign,
    clickable: bool,
    fill: Option<egui::Color32>,
) -> egui::Response {
    let sense = if clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), sense);
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 2.0, fill);
    }
    let label = egui::Label::new(text.into()).truncate().halign(align.egui()).selectable(!clickable);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
            .layout(egui::Layout::top_down(align.egui()).with_main_align(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child.add(label);
    if clickable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

fn summary_text(record: &Record, palette: Palette) -> egui::RichText {
    let text = egui::RichText::new(&record.summary);
    if record.finish.as_ref().is_some_and(|finish| finish.highlight) {
        text.color(palette.emphasis)
    } else {
        text
    }
}

fn entry_ui(ui: &mut egui::Ui, entry: &DisplayEntry, width: f32, align: ColumnAlign) {
    let palette = Palette::of(ui);
    let text = if entry.data.label.is_empty() {
        entry.display.replace(['\n', '\r'], " ")
    } else if entry.display.is_empty() {
        entry.data.label.clone()
    } else {
        format!("{}   {}", entry.data.label, entry.display.replace(['\n', '\r'], " "))
    };
    let text: egui::WidgetText = match entry.data.kind {
        EntryKind::Plain if entry.display.is_empty() => egui::RichText::new(text).strong().into(),
        EntryKind::Plain => egui::RichText::new(text).into(),
        // 结构化详情把一名成员的属性、技能合并成一行，只有带差额的项才是被加成的部分。
        EntryKind::Highlight => match changed_items_job(ui, &text) {
            Some(job) => job.into(),
            None => egui::RichText::new(text).color(palette.emphasis).strong().into(),
        },
        EntryKind::SkillBoard => egui::RichText::new(text).color(palette.info).strong().into(),
    };
    cell(ui, width, text, align, false, None).on_hover_text(&entry.display);
}

/// 差额项由 `format_delta` 写成 `值(+N)` / `值(-N)`；只给这些项上高亮色，其余保持正文色。
///
/// 找不到差额项时返回 `None`，让调用方沿用整条高亮（胜率、配队等结果仍然整条命中阈值）。
fn changed_items_job(ui: &egui::Ui, text: &str) -> Option<egui::text::LayoutJob> {
    let contains_delta = |token: &str| token.contains("(+") || token.contains("(-");
    if !text.split(' ').any(contains_delta) {
        return None;
    }
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let emphasis = egui::text::TextFormat {
        font_id: font_id.clone(),
        color: Palette::of(ui).emphasis,
        ..Default::default()
    };
    let plain = egui::text::TextFormat {
        font_id,
        color: ui.visuals().text_color(),
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    for (index, token) in text.split(' ').enumerate() {
        if index > 0 {
            job.append(" ", 0.0, plain.clone());
        }
        let format = if contains_delta(token) {
            emphasis.clone()
        } else {
            plain.clone()
        };
        job.append(token, 0.0, format);
    }
    Some(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_rows_highlight_only_the_changed_items() {
        let ctx = egui::Context::default();
        let mut colored = Vec::new();
        let mut untouched = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let emphasis = Palette::of(ui).emphasis;
            let changed = changed_items_job(ui, "魔 90  抗 96  智 94(+21)  八围 685.7  嘲讽 289").expect("含差额应分段");
            colored = changed
                .sections
                .iter()
                .filter(|section| section.format.color == emphasis)
                .map(|section| changed.text[section.byte_range.start.0..section.byte_range.end.0].to_owned())
                .collect();
            // 胜率、配队的整条高亮不含差额，仍由调用方整体着色。
            untouched.push(changed_items_job(ui, "299.300").is_none());
            untouched.push(changed_items_job(ui, "HP 311 攻 56 防 83").is_none());
        });
        output.textures_delta.clear();
        assert_eq!(colored, ["94(+21)"]);
        assert_eq!(untouched, [true, true]);
    }

    #[test]
    fn skill_rows_highlight_every_changed_skill_on_that_line() {
        let ctx = egui::Context::default();
        let mut colored = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let emphasis = Palette::of(ui).emphasis;
            let changed = changed_items_job(ui, "命轮 20  分身 58  护符 98(+14)  噬魂 3(-2)").expect("含差额应分段");
            colored = changed
                .sections
                .iter()
                .filter(|section| section.format.color == emphasis)
                .map(|section| changed.text[section.byte_range.start.0..section.byte_range.end.0].to_owned())
                .collect();
        });
        output.textures_delta.clear();
        assert_eq!(colored, ["98(+14)", "3(-2)"]);
    }

    #[test]
    fn cells_keep_fixed_geometry_and_align_text_left_center_right() {
        let ctx = egui::Context::default();
        let mut cells = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for align in ColumnAlign::ALL {
                cells.push(cell(ui, 240.0, egui::RichText::new("align-marker"), align, false, None).rect);
            }
        });
        output.textures_delta.clear();
        let text_rects = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == "align-marker" => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(text_rects.len(), 3);
        for rect in &cells {
            assert_eq!(rect.size(), egui::vec2(240.0, ROW_HEIGHT));
        }
        assert!((text_rects[0].left() - cells[0].left() - 4.0).abs() < 1.0);
        assert!((text_rects[1].center().x - cells[1].center().x).abs() < 1.0);
        assert!((text_rects[2].right() - cells[2].right() + 4.0).abs() < 1.0);
    }

    #[test]
    fn header_separator_drag_changes_width_within_bounds() {
        let ctx = egui::Context::default();
        let mut width = 220.0;
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(220.0, ROW_HEIGHT));
        for (time, events) in [
            (0.0, Vec::new()),
            (
                0.1,
                vec![
                    egui::Event::PointerMoved(egui::pos2(239.0, 30.0)),
                    egui::Event::PointerButton {
                        pos: egui::pos2(239.0, 30.0),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ),
            (0.2, vec![egui::Event::PointerMoved(egui::pos2(280.0, 30.0))]),
        ] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| resize_column(ui, 0, rect, &mut width),
            );
            output.textures_delta.clear();
        }
        assert!(width > 220.0);
        assert!((60.0..=640.0).contains(&width));
    }
}
