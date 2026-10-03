//! 结果控件与可见行绘制；数据更新、索引及裁剪由父模块维护。

use super::{
    ColumnAlign, DetailLine, Record, ResultRow, ResultsView, TABLE_AUTO_RATIO, TABLE_HEIGHT_AUTO, TABLE_HEIGHT_MAX,
    TABLE_HEIGHT_MIN, ViewMode,
};
use crate::app::help::HelpTopic;
use crate::app::style::Palette;
use tswn_openbox::backend::live::{EntryKind, ResultKind};

const CELL_GAP: f32 = 4.0;
/// 结果区底部留给“复制全部”按钮的高度；由调用方从可用高度中扣除。
pub(crate) const RESULT_FOOTER_HEIGHT: f32 = 34.0;
/// 表格与选中行之间的分隔条高度，用来拖动调整表格高度。
const TABLE_DIVIDER_HEIGHT: f32 = 8.0;
const COLUMN_LABELS: [&str; 7] = ["名字 / 输入序号", "状态", "pp", "pd", "qp", "qd", "sum"];

/// 表格与卡片的文字行度量：与纯文本日志完全一致（等宽字体 + 同样的行距）。
fn row_height(ui: &egui::Ui) -> f32 { ui.text_style_height(&egui::TextStyle::Monospace) }

/// 内容可滚动且已经滑到底部；用来在滑到底时自动恢复“跟随最新”。
pub(crate) fn scrolled_to_end(content: egui::Vec2, viewport: egui::Rect, offset: egui::Vec2) -> bool {
    let scrollable = content.y > viewport.height() + 1.0;
    scrollable && offset.y >= content.y - viewport.height() - 2.0
}

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
            let mut follow = self.follow;
            if ui
                .checkbox(&mut follow, "跟随最新")
                .on_hover_text("勾选后立即跳到最新并持续跟随；向上滚动会暂停跟随，滑回底部会重新勾选。")
                .changed()
            {
                self.set_follow(follow);
            }
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
        ui.horizontal(|ui| {
            ui.label("表格高度");
            let mut height = self.table_height.max(0.0);
            let mut changed = false;
            ui.add_enabled_ui(self.table_height > TABLE_HEIGHT_AUTO, |ui| {
                changed = ui
                    .add(
                        egui::DragValue::new(&mut height)
                            .range(TABLE_HEIGHT_MIN..=TABLE_HEIGHT_MAX)
                            .speed(1.0)
                            .suffix(" px"),
                    )
                    .on_hover_text("表格区域的高度；也可直接拖动表格与明细之间的分隔条。")
                    .changed();
            });
            if ui
                .selectable_label(self.table_height <= TABLE_HEIGHT_AUTO, "自动")
                .on_hover_text(format!("按可用高度的 {:.0}% 分配表格高度。", TABLE_AUTO_RATIO * 100.0))
                .clicked()
            {
                self.set_table_height(None);
            }
            if changed {
                self.set_table_height(Some(height));
            }
        });
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
            self.set_table_height(None);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            // 行高按等宽字体计算，纵向行距沿用环境值：表格、卡片与纯文本日志逐行对齐。
            let height = row_height(ui);
            ui.spacing_mut().item_spacing = egui::vec2(CELL_GAP, ui.spacing().item_spacing.y);
            ui.spacing_mut().button_padding = egui::vec2(4.0, 1.0);
            ui.spacing_mut().interact_size.y = height;
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
                self.rows.push(ResultRow::Header(group));
                if self.mode == ViewMode::Cards && self.expanded.contains(&group) {
                    let record = self.records.get_mut(&group).unwrap();
                    record.refresh_detail_order();
                    self.rows
                        .extend((0..record.details.len()).map(|index| ResultRow::Detail { group, index }));
                    self.rows.push(ResultRow::CardCopy(group));
                }
            }
            self.dirty = false;
            self.rendered_mode = self.mode;
        }
        let table = self.mode == ViewMode::Table;
        let kind = self.records[self.order.front().unwrap()].kind;
        let scores = kind == ResultKind::Scores;
        let height = row_height(ui);
        let jump_extent = self.results_extent();
        let jump = self.take_follow_jump(jump_extent);
        // 表格高度由分隔条调整：固定后表格区保持该高度，自动模式仍然贴合内容。
        let show_details = table && self.selected.is_some();
        let fixed_table = show_details && self.table_height > TABLE_HEIGHT_AUTO;
        let table_limit = if show_details {
            self.table_area_height(ui.available_height())
        } else {
            ui.available_height()
        };
        if ui.rect_contains_pointer(ui.max_rect()) && ui.input(|input| input.smooth_scroll_delta.y > 0.0) {
            self.follow = false;
        }
        let mut toggle = None;
        let mut select = None;
        // 表头与数据共享横向滚动；垂直方向仍然只布局可见行。
        let scroll = egui::ScrollArea::horizontal()
            .id_salt(if table { "results_table_x" } else { "results_cards_x" })
            .auto_shrink([false, table && !fixed_table])
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
                                egui::RichText::new(column_label(index, kind == ResultKind::Scores)).strong().monospace(),
                                self.column_alignments[index],
                                false,
                                None,
                            );
                            resize_column(ui, index, response.rect, &mut self.column_widths[index]);
                        }
                    });
                    ui.separator();
                }
                let mut area = egui::ScrollArea::vertical()
                    .id_salt(if table { "results_table" } else { "results_cards" })
                    .auto_shrink([false, table && !fixed_table])
                    .max_height(table_limit.max(60.0))
                    .stick_to_bottom(self.follow);
                if let Some(offset) = jump {
                    // 勾选“跟随最新”后当帧就落到最新一行：用上一次的度量算出到底部的偏移。
                    area = area.vertical_scroll_offset(offset);
                }
                area.show_rows(ui, height, self.rows.len(), |ui, range| {
                    for row in range {
                        match self.rows[row] {
                            ResultRow::Detail { group, index } => {
                                let record = &self.records[&group];
                                let line = &record.details[index];
                                ui.push_id((group, index), |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_space(12.0);
                                        let button = copy_button_width(record.kind, index);
                                        detail_ui(ui, line, (width - 12.0 - button).max(60.0), self.card_align);
                                        if button > 0.0
                                            && ui.small_button("复制").on_hover_text("复制该名字的 DIY 导出行。").clicked()
                                        {
                                            ui.ctx().copy_text(line.text.clone());
                                        }
                                    });
                                });
                            }
                            // 每张展开卡片的右下角都有“复制全部”，只复制该卡片自己的内容。
                            ResultRow::CardCopy(group) => {
                                let record = &self.records[&group];
                                ui.push_id((group, "card_copy"), |ui| {
                                    ui.horizontal(|ui| {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui
                                                .small_button("复制全部")
                                                .on_hover_text("复制这张卡片的全部内容：标题与明细，不包含日志的其它结果。")
                                                .clicked()
                                            {
                                                let text = record.copy_text(group, self.terminal.as_deref());
                                                ui.ctx().copy_text(text);
                                            }
                                        });
                                    });
                                });
                            }
                            ResultRow::Header(group) => {
                                let record = &self.records[&group];
                                ui.push_id((group, "header"), |ui| {
                                    let palette = Palette::of(ui);
                                    let state = record.state_text(self.terminal.as_deref());
                                    let state_color = palette.status(if record.finish.is_some() {
                                        "完成"
                                    } else {
                                        self.terminal.as_deref().unwrap_or("运行中")
                                    });
                                    if table {
                                        let highlight = record.finish.as_ref().is_some_and(|finish| finish.highlight);
                                        ui.horizontal(|ui| {
                                            let fill = if self.selected == Some(group) {
                                                ui.visuals().selection.bg_fill
                                            } else if row % 2 == 0 {
                                                ui.visuals().faint_bg_color
                                            } else {
                                                egui::Color32::TRANSPARENT
                                            };
                                            // 高分高亮时名字与分数一起标色，方便一眼定位是哪一行。
                                            let mut name =
                                                egui::RichText::new(format!("#{} {}", group + 1, record.label)).monospace();
                                            if highlight {
                                                name = name.color(palette.emphasis);
                                            }
                                            if cell(ui, self.column_widths[0], name, self.column_alignments[0], true, Some(fill))
                                                .clicked()
                                            {
                                                select = Some(group);
                                            }
                                            cell(
                                                ui,
                                                self.column_widths[1],
                                                egui::RichText::new(&state).color(state_color).monospace(),
                                                self.column_alignments[1],
                                                false,
                                                Some(fill),
                                            );
                                            if scores {
                                                for index in 0..5 {
                                                    let entry = record.entries.get(&index);
                                                    let mut text = egui::RichText::new(entry.map_or("—", |entry| &entry.display))
                                                        .monospace();
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
                                        let mut text = egui::RichText::new(format!(
                                            "{arrow} {}",
                                            record.title_line(group, self.terminal.as_deref())
                                        ))
                                        .monospace();
                                        if record.finish.as_ref().is_some_and(|finish| finish.highlight) {
                                            text = text.color(palette.emphasis);
                                        }
                                        if cell(ui, width, text, self.card_align, true, Some(state_color.gamma_multiply(0.1)))
                                            .on_hover_text("点击展开或收起明细。序号按原输入标记，多个线程的结果可能交错到达。")
                                            .clicked()
                                        {
                                            toggle = Some(group);
                                            select = Some(group);
                                        }
                                    }
                                });
                            }
                        }
                    }
                })
            });
        // 垂直滚动区（`scroll.inner`）才是结果列表本身：跟随判定与跳转度量都以它为准。
        let rows_area = scroll.inner;
        self.set_results_extent(rows_area.content_size.y, rows_area.inner_rect.height());
        if scrolled_to_end(rows_area.content_size, rows_area.inner_rect, rows_area.state.offset) {
            self.follow = true;
        }
        if let Some(group) = toggle {
            if !self.expanded.remove(&group) {
                self.expanded.insert(group);
            }
            self.dirty = true;
        }
        if let Some(group) = select {
            self.selected = Some(group);
        }
        if !table {
            return;
        }
        if let Some(group) = self.selected {
            // 表格与明细之间用可拖动的分隔条调整表格高度；拖动后由自动模式切到固定高度。
            let label = self.records.get(&group).map(|record| record.label.clone()).unwrap_or_default();
            let divider = table_divider(ui, &label);
            if divider.dragged() {
                let current = if self.table_height > TABLE_HEIGHT_AUTO {
                    self.table_height
                } else {
                    table_limit
                };
                self.set_table_height(Some(current + divider.drag_delta().y));
            }
            let Some(record) = self.records.get_mut(&group) else { return };
            record.refresh_detail_order();
            if record.top.is_some() && record.finish.is_none() {
                ui.weak("当前 Top，全部队友完成后确定最终排名");
            }
            let details_height = ui.available_height().max(80.0);
            egui::ScrollArea::both()
                .id_salt("selected_result_details")
                .max_height(details_height)
                .show_rows(ui, height, record.details.len(), |ui, rows| {
                    let width = ui.available_width().max(300.0);
                    for index in rows {
                        let line = &record.details[index];
                        let button = copy_button_width(record.kind, index);
                        ui.horizontal(|ui| {
                            detail_ui(ui, line, (width - button).max(60.0), self.card_align);
                            if button > 0.0 && ui.small_button("复制").on_hover_text("复制该名字的 DIY 导出行。").clicked()
                            {
                                ui.ctx().copy_text(line.text.clone());
                            }
                        });
                    }
                });
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

/// 表格与选中行之间的分隔条：拖动调整表格高度，左侧标出当前选中行。
fn table_divider(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), TABLE_DIVIDER_HEIGHT), egui::Sense::drag());
    let palette = Palette::of(ui);
    let color = if response.hovered() || response.dragged() {
        palette.info
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, ui.visuals().weak_text_color());
    let text_pos = egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y * 0.5);
    let line_start = text_pos.x + galley.size().x + 6.0;
    ui.painter()
        .hline(line_start..=rect.right(), rect.center().y, egui::Stroke::new(1.0, color));
    ui.painter().galley(text_pos, galley, ui.visuals().weak_text_color());
    response
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .on_hover_text("拖动调整表格高度；选中行的明细显示在下方。")
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
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, row_height(ui)), sense);
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
    let text = egui::RichText::new(&record.summary).monospace();
    if record.finish.as_ref().is_some_and(|finish| finish.highlight) {
        text.color(palette.emphasis)
    } else {
        text
    }
}

/// 导出结果的第一行是 DIY 导出行，右侧留出复制按钮的宽度。
fn copy_button_width(kind: ResultKind, index: usize) -> f32 { if kind == ResultKind::Diy && index == 0 { 56.0 } else { 0.0 } }

/// 详情行沿用纯文本视图的观感：等宽字体、区块标题加粗、缩进行弱化、差额项单独高亮。
fn detail_ui(ui: &mut egui::Ui, line: &DetailLine, width: f32, align: ColumnAlign) {
    let palette = Palette::of(ui);
    let text: egui::WidgetText = if line.kind == EntryKind::SkillBoard || line.text.starts_with("=== ") {
        egui::RichText::new(&line.text).color(palette.info).strong().monospace().into()
    } else if let Some(job) = changed_items_job(ui, &line.text) {
        // 组队造成的加成只标出带 `(+N)` / `(-N)` 的项。
        job.into()
    } else if line.text.starts_with("  ") {
        egui::RichText::new(&line.text).color(ui.visuals().weak_text_color()).monospace().into()
    } else {
        egui::RichText::new(&line.text).monospace().into()
    };
    cell(ui, width, text, align, false, None).on_hover_text(&line.text);
}

/// 差额项由 `format_delta` 写成 `值(+N)` / `值(-N)`；只给这些项上高亮色，其余保持正文色。
///
/// 找不到差额项时返回 `None`，让调用方沿用整条高亮（胜率、配队等结果仍然整条命中阈值）。
fn changed_items_job(ui: &egui::Ui, text: &str) -> Option<egui::text::LayoutJob> {
    let contains_delta = |token: &str| token.contains("(+") || token.contains("(-");
    if !text.split(' ').any(contains_delta) {
        return None;
    }
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
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
        let mut expected_height = 0.0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            expected_height = row_height(ui);
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
            assert_eq!(rect.size(), egui::vec2(240.0, expected_height));
        }
        assert!((text_rects[0].left() - cells[0].left() - 4.0).abs() < 1.0);
        assert!((text_rects[1].center().x - cells[1].center().x).abs() < 1.0);
        assert!((text_rects[2].right() - cells[2].right() + 4.0).abs() < 1.0);
    }

    /// 表格与卡片的行高、字体必须与纯文本日志一致：等宽字体、同一套文本样式。
    #[test]
    fn result_rows_use_the_same_text_metrics_as_the_log_view() {
        let ctx = egui::Context::default();
        let mut metrics = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let height = row_height(ui);
            let cell_rect = cell(
                ui,
                200.0,
                egui::RichText::new("metrics").monospace(),
                ColumnAlign::Left,
                false,
                None,
            )
            .rect;
            let log_height = ui.text_style_height(&egui::TextStyle::Monospace);
            let log_font = egui::TextStyle::Monospace.resolve(ui.style());
            let job_font = changed_items_job(ui, "智 94(+21)").expect("含差额").sections[0].format.font_id.clone();
            metrics.push((height, cell_rect.height(), log_height, log_font, job_font));
        });
        output.textures_delta.clear();
        let (_, cell_height, log_height, log_font, job_font) = metrics.pop().unwrap();
        assert_eq!(cell_height, log_height, "卡片与表格的行高应与日志行高一致");
        assert_eq!(job_font, log_font, "差额高亮应使用等宽字体");
    }

    /// 表格里高分高亮时，名字与分数必须一起标色。
    #[test]
    fn highlighted_table_rows_color_the_name_as_well_as_the_score() {
        use tswn_openbox::backend::live::ResultUpdate;

        let mut view = ResultsView {
            mode: ViewMode::Table,
            ..Default::default()
        };
        let mut log = crate::app::log::LogBuffer::default();
        let mut update = ResultUpdate::new_with_legacy(0, "high-score", ResultKind::Rate, 3);
        update.finish = Some(tswn_openbox::backend::live::ResultFinish {
            score: Some(99.0),
            visible: true,
            highlight: true,
        });
        view.apply(update, &mut log);
        let ctx = egui::Context::default();
        let mut emphasis = egui::Color32::PLACEHOLDER;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0))),
                ..Default::default()
            },
            |ui| {
                emphasis = Palette::of(ui).emphasis;
                view.ui(ui);
            },
        );
        output.textures_delta.clear();
        let colors = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => {
                    let label = text.galley.job.text.as_str();
                    (label == "#1 high-score" || label == "99.000").then(|| {
                        (
                            label.to_owned(),
                            text.galley.job.sections.iter().any(|section| section.format.color == emphasis),
                        )
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(colors.len(), 2, "名字与分数两格都应绘制：{colors:?}");
        assert!(
            colors.iter().all(|(_, colored)| *colored),
            "高分高亮必须同时标色名字与分数：{colors:?}"
        );
    }

    /// 拖动表格与明细之间的分隔条即可调整表格高度。
    #[test]
    fn dragging_the_divider_resizes_the_table_area() {
        use tswn_openbox::backend::live::ResultUpdate;

        let mut view = ResultsView {
            mode: ViewMode::Table,
            follow: false,
            ..Default::default()
        };
        let mut log = crate::app::log::LogBuffer::default();
        let mut update = ResultUpdate::new_with_legacy(0, "resize-marker", ResultKind::Rate, 3);
        update
            .entries
            .push(tswn_openbox::backend::live::ResultEntry::number(0, "target".into(), 70.0));
        update.finish = Some(tswn_openbox::backend::live::ResultFinish {
            score: Some(70.0),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 600.0));
        let divider_y = |view: &mut ResultsView, time: f64, events: Vec<egui::Event>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| view.ui(ui),
            );
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    // 分隔条左侧就是当前选中行的名字，位置即分隔条所在行。
                    egui::epaint::Shape::Text(text) if text.galley.job.text == "resize-marker" => Some(text.pos.y),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("分隔条应绘制选中行名字"))
        };
        let start = divider_y(&mut view, 0.0, Vec::new());
        assert_eq!(view.table_height, TABLE_HEIGHT_AUTO, "默认是自动高度");
        let pointer = egui::pos2(120.0, start + TABLE_DIVIDER_HEIGHT * 0.5);
        divider_y(
            &mut view,
            0.1,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let moved = pointer + egui::vec2(0.0, 80.0);
        divider_y(&mut view, 0.2, vec![egui::Event::PointerMoved(moved)]);
        let after = divider_y(&mut view, 0.3, Vec::new());
        assert!(
            view.table_height > TABLE_HEIGHT_AUTO,
            "拖动后应保存具体高度：{}",
            view.table_height
        );
        assert!(
            (TABLE_HEIGHT_MIN..=TABLE_HEIGHT_MAX).contains(&view.table_height),
            "高度必须落在允许范围内：{}",
            view.table_height
        );
        assert!(after > start + 40.0, "向下拖动后表格应变高：{start} -> {after}");
    }

    /// 每张展开的卡片右下角都有“复制全部”，点击复制的就是这张卡片的内容。
    #[test]
    fn expanded_cards_expose_a_copy_button_for_that_card() {
        use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

        let mut view = ResultsView::default();
        let mut log = crate::app::log::LogBuffer::default();
        for group in 0..2 {
            let label = format!("card{group}");
            let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Diy, 0);
            update.legacy_log = Some((format!("card{group}+ol:{{}}\n明细{group}"), EntryKind::Plain));
            update.finish = Some(ResultFinish {
                score: None,
                visible: true,
                highlight: false,
            });
            view.apply(update, &mut log);
        }
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 500.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let buttons = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == "复制全部" => Some(text.pos),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(buttons.len(), 1, "只有展开的卡片带“复制全部”按钮");
        // 第一张卡片默认展开，按钮应贴在该卡片的右下方。
        assert!(buttons[0].x > screen.width() * 0.5, "按钮应位于卡片右下角：{:?}", buttons[0]);

        let pointer = buttons[0] + egui::vec2(8.0, 6.0);
        let mut click = |time: f64, pressed: bool| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(time),
                    events: vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| view.ui(ui),
            );
            output.textures_delta.clear();
            output
        };
        click(0.1, true);
        let output = click(0.2, false);
        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("点击“复制全部”应写入剪贴板"));
        assert!(copied.starts_with("#1 card0"), "复制的是该卡片自己的内容：{copied}");
        assert!(!copied.contains("card1"), "不应混入其它卡片或日志：{copied}");
    }

    /// 勾选“跟随最新”后必须立刻跳到最新，而不是等下一条结果。
    #[test]
    fn checking_follow_jumps_to_the_latest_rows_immediately() {
        use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

        let mut view = ResultsView {
            mode: ViewMode::Table,
            follow: false,
            ..Default::default()
        };
        let mut log = crate::app::log::LogBuffer::default();
        for group in 0..200 {
            let label = format!("name{group}");
            let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Rate, 0);
            update.finish = Some(ResultFinish {
                score: Some(group as f64),
                visible: true,
                highlight: false,
            });
            view.apply(update, &mut log);
        }
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
        let visible_rows = |view: &mut ResultsView, time: f64| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| view.ui(ui),
            );
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .filter(|text| text.starts_with('#'))
                .collect::<Vec<_>>()
        };
        let before = visible_rows(&mut view, 0.0);
        assert!(
            before.iter().any(|text| text.starts_with("#1 ")),
            "未跟随时从最早一条开始：{before:?}"
        );
        assert!(!before.iter().any(|text| text.starts_with("#200 ")), "未跟随时看不到最后一条");
        view.set_follow(true);
        let after = visible_rows(&mut view, 0.1);
        assert!(
            after.iter().any(|text| text.starts_with("#200 ")),
            "勾选跟随应立刻跳到最新：{after:?}"
        );
    }

    /// 滑到最底部时自动恢复“跟随最新”。
    #[test]
    fn scrolling_to_the_bottom_restores_follow() {
        use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

        let mut view = ResultsView {
            mode: ViewMode::Table,
            follow: false,
            ..Default::default()
        };
        let mut log = crate::app::log::LogBuffer::default();
        for group in 0..200 {
            let label = format!("name{group}");
            let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Rate, 0);
            update.finish = Some(ResultFinish {
                score: Some(group as f64),
                visible: true,
                highlight: false,
            });
            view.apply(update, &mut log);
        }
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
        let mut first_row = egui::Pos2::ZERO;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && text.galley.job.text == "#1 name0"
            {
                first_row = text.pos;
            }
        }
        assert!(first_row != egui::Pos2::ZERO, "首行应可见，便于把指针放进表格");
        let pointer = first_row + egui::vec2(20.0, 4.0);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(0.1),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0.0, -100_000.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        assert!(view.follow, "滑到最底部后应自动勾选跟随最新");
    }

    #[test]
    fn header_separator_drag_changes_width_within_bounds() {
        let ctx = egui::Context::default();
        let mut width = 220.0;
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(220.0, 20.0));
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
