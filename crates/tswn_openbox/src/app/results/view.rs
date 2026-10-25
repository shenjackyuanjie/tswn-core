//! 结果控件与可见行绘制，数据更新、索引及裁剪由父模块维护。
//!
//! 输入是 UI 线程持有的 ResultsView 和 egui Ui；控件只修改展开、选择、跟随与
//! 排版偏好，不执行对局。卡片、表格与日志共享文字度量，滚动边界以有效范围内
//! 的偏移判断；被缓存裁剪的数据不会在本模块重新计算。

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
/// 表格与选中行之间的分隔条高度，用来拖动调整表格高度；按行高留出可选中复制的标题。
const TABLE_DIVIDER_HEIGHT: f32 = 8.0;
/// 卡片标题右侧「复制名字」按钮占用的宽度。
const COPY_NAME_WIDTH: f32 = 88.0;
/// 卡片标题右侧显示未完成状态时占用的宽度，够放“已停止 · 结果不完整”。
const CARD_STATE_WIDTH: f32 = 168.0;
const COLUMN_LABELS: [&str; 7] = ["名字 / 输入序号", "复制名字", "pp", "pd", "qp", "qd", "sum"];

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
                // “完成”不再显示在行里（那一格换成「复制名字」），图例也跟着去掉。
                for (color, label, hint) in [
                    (palette.info, "预览", "明细已完成，整组仍在计算；#序号对应原输入位置。"),
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
        //
        // 横向滚动区的高度必须钉在表格区高度上：`auto_shrink = false` 会把多余空间留在
        // 滚动区*内部*，不设上限时表格区会吃掉整个结果区，把分隔条与明细挤到可视区之外。
        let scroll = egui::ScrollArea::horizontal()
            .id_salt(if table { "results_table_x" } else { "results_cards_x" })
            .auto_shrink([false, table && !fixed_table])
            .max_height(if table { table_limit.max(60.0) } else { f32::INFINITY })
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
                                                let text = record.copy_text(group);
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
                                    let finished = record.finish.is_some();
                                    let state_color = palette.status(if finished {
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
                                            // 完成的组不再显示“完成”，这一格换成复制名字的按钮；
                                            // 还在计算或已停止的组仍把状态写在这里。
                                            if finished {
                                                if cell_button(
                                                    ui,
                                                    self.column_widths[1],
                                                    "复制",
                                                    self.column_alignments[1],
                                                    Some(fill),
                                                )
                                                .on_hover_text("复制这个名字，便于再次粘贴到输入框。")
                                                .clicked()
                                                {
                                                    ui.ctx().copy_text(record.label.clone());
                                                }
                                            } else {
                                                cell(
                                                    ui,
                                                    self.column_widths[1],
                                                    egui::RichText::new(record.state_text(self.terminal.as_deref()))
                                                        .color(state_color)
                                                        .monospace(),
                                                    self.column_alignments[1],
                                                    false,
                                                    Some(fill),
                                                );
                                            }
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
                                        let mut text =
                                            egui::RichText::new(format!("{arrow} {}", record.title_text(group))).monospace();
                                        if record.finish.as_ref().is_some_and(|finish| finish.highlight) {
                                            text = text.color(palette.emphasis);
                                        }
                                        ui.horizontal(|ui| {
                                            // 右侧固定留出位置：完成时是「复制名字」按钮，未完成时是状态文字。
                                            let fill = state_color.gamma_multiply(0.1);
                                            let slot = if finished { COPY_NAME_WIDTH } else { CARD_STATE_WIDTH };
                                            let title_width = (width - slot - CELL_GAP).max(60.0);
                                            let response = cell(ui, title_width, text, self.card_align, true, Some(fill))
                                                .on_hover_text(
                                                    "点击展开或收起明细。序号按原输入标记，多个线程的结果可能交错到达。",
                                                );
                                            if response.clicked() {
                                                toggle = Some(group);
                                                select = Some(group);
                                            }
                                            if finished {
                                                if cell_button(ui, COPY_NAME_WIDTH, "复制名字", ColumnAlign::Right, Some(fill))
                                                    .on_hover_text("复制这个名字，不包含明细与日志里的其它结果。")
                                                    .clicked()
                                                {
                                                    ui.ctx().copy_text(record.label.clone());
                                                }
                                            } else {
                                                cell(
                                                    ui,
                                                    CARD_STATE_WIDTH,
                                                    egui::RichText::new(record.state_text(self.terminal.as_deref()))
                                                        .color(state_color),
                                                    ColumnAlign::Right,
                                                    false,
                                                    Some(fill),
                                                );
                                            }
                                        });
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
        if scrolled_to_end(rows_area.content_size, rows_area.inner_rect, rows_area.state.clamped_offset()) {
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
            let title = self.records.get(&group).map(|record| record.divider_text()).unwrap_or_default();
            let divider = table_divider(ui, &title);
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
                // 提示文字也允许框选：结果区里不该有选不中的文字。
                ui.add(egui::Label::new(egui::RichText::new("当前 Top，全部队友完成后确定最终排名").weak()).selectable(true));
            }
            // 明细区占满表格下方的全部剩余高度：表格调小时明细随之变高。
            let details_height = ui.available_height().max(80.0);
            egui::ScrollArea::both()
                .id_salt("selected_result_details")
                .auto_shrink([false, false])
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
        .interact(handle, ui.scope_id().with(("column_resize", index)), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if response.dragged() {
        *width = (*width + ui.input(|input| input.pointer.delta().x)).clamp(60.0, 640.0);
    }
    ui.painter()
        .vline(rect.right(), rect.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
}

/// 表格与选中行之间的分隔条：拖动调整表格高度，左侧标出当前选中行的名字与分数。
fn table_divider(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let height = row_height(ui).max(TABLE_DIVIDER_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::drag());
    let palette = Palette::of(ui);
    let color = if response.hovered() || response.dragged() {
        palette.info
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    // 标题用可选中复制的 Label 绘制，右侧剩下的横线区域仍然负责拖动调高。
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    let label = child.add(egui::Label::new(egui::RichText::new(text).monospace().weak()).truncate().selectable(true));
    let line_start = label.rect.right() + 6.0;
    ui.painter()
        .hline(line_start..=rect.right(), rect.center().y, egui::Stroke::new(1.0, color));
    let response = if label.contains_pointer() {
        // 指针在标题文字上时留给文字选择，不抢成拖动调高。
        response
    } else {
        response.on_hover_cursor(egui::CursorIcon::ResizeVertical)
    };
    response.on_hover_text("拖动调整表格高度；选中行的明细显示在下方。")
}

/// 固定尺寸且不换行，避免长导出行或多行字段破坏虚拟列表行高。
///
/// 文字始终可选中复制：可点击的格子把外层点击与文字标签的响应合并后返回，
/// 因此“点名字选中该行”和“拖选文字”两件事可以同时成立。
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
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
            .layout(egui::Layout::top_down(align.egui()).with_main_align(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    let label = child.add(egui::Label::new(text.into()).truncate().halign(align.egui()).selectable(true));
    let response = response.union(label);
    if clickable {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// 格子里的按钮：占据与 [`cell`] 相同的尺寸，用来替代原来那列“完成”状态文字。
fn cell_button(ui: &mut egui::Ui, width: f32, text: &str, align: ColumnAlign, fill: Option<egui::Color32>) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, row_height(ui)), egui::Sense::hover());
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 2.0, fill);
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(4.0, 0.0)))
            .layout(egui::Layout::top_down(align.egui()).with_main_align(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    child.add(egui::Button::new(egui::RichText::new(text).monospace()).small())
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
#[path = "view_tests.rs"]
mod tests;
