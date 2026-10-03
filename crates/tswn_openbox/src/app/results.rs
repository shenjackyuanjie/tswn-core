//! 三种视图共用的结果模型；只格式化变化项，布局限于当前可见行。

pub(crate) use view::{RESULT_FOOTER_HEIGHT, scrolled_to_end};

mod view;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tswn_openbox::backend::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultUpdate};

use super::log::{LogBuffer, LogKind};

const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;

/// 表格高度的自动模式：按可用高度的 [`TABLE_AUTO_RATIO`] 分配，拖动分隔条后写入具体像素值。
pub(crate) const TABLE_HEIGHT_AUTO: f32 = 0.0;
pub(crate) const TABLE_HEIGHT_MIN: f32 = 80.0;
pub(crate) const TABLE_HEIGHT_MAX: f32 = 2000.0;
/// 自动模式下表格占结果区高度的比例；与拖动前的默认版面一致。
pub(crate) const TABLE_AUTO_RATIO: f32 = 0.55;
/// 表格下方选中行明细至少保留的高度。
pub(crate) const TABLE_DETAIL_MIN: f32 = 120.0;

/// 与 [`Tool::ALL`] 顺序一致的默认视图：导出与配队看卡片，评分与胜率看表格，DS4 看纯文本。
pub(crate) const DEFAULT_VIEW_MODES: [ViewMode; 5] = [
    ViewMode::Cards,
    ViewMode::Table,
    ViewMode::Table,
    ViewMode::Cards,
    ViewMode::Text,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ViewMode {
    Text,
    #[default]
    Cards,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl ColumnAlign {
    pub(crate) const ALL: [Self; 3] = [Self::Left, Self::Center, Self::Right];

    fn label(self) -> &'static str {
        match self {
            Self::Left => "左",
            Self::Center => "中",
            Self::Right => "右",
        }
    }

    fn egui(self) -> egui::Align {
        match self {
            Self::Left => egui::Align::Min,
            Self::Center => egui::Align::Center,
            Self::Right => egui::Align::Max,
        }
    }
}

struct DisplayEntry {
    data: ResultEntry,
    display: String,
}

/// 卡片与表格里的一行详情；`kind` 决定整体着色，缩进与差额由文本自身决定。
pub(crate) struct DetailLine {
    pub(crate) text: String,
    pub(crate) kind: EntryKind,
}

/// 结果区的一行：结果标题、展开卡片的明细行，或展开卡片右下角的“复制全部”页脚。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultRow {
    Header(usize),
    Detail { group: usize, index: usize },
    CardCopy(usize),
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
    detail_indexes: Vec<usize>,
    /// 导出结果的旧版文本行（导出行与各成员详情）；卡片与表格按行渲染。
    diy_lines: Vec<String>,
    details: Vec<DetailLine>,
    details_dirty: bool,
}

impl Record {
    /// 结果块的状态文字；表格、卡片与复制文本共用。
    fn state_text(&self, terminal: Option<&str>) -> String {
        if self.finish.is_some() {
            "完成".to_owned()
        } else if let Some(terminal) = terminal {
            format!("{terminal} · 结果不完整")
        } else {
            "计算中 · 预览".to_owned()
        }
    }

    /// 结果块的标题行：序号、名字、摘要与状态；卡片显示与复制文本共用。
    fn title_line(&self, group: usize, terminal: Option<&str>) -> String {
        let summary = if self.summary.is_empty() {
            String::new()
        } else {
            format!("   {}", self.summary)
        };
        let top = if self.top.is_some() && self.finish.is_none() {
            " · 当前 Top"
        } else {
            ""
        };
        format!("#{} {}{summary}   {}{top}", group + 1, self.label, self.state_text(terminal))
    }

    /// 结果块的可复制文本：标题行加全部明细行，与卡片和选中行显示的内容一致。
    fn copy_text(&self, group: usize, terminal: Option<&str>) -> String {
        let mut text = self.title_line(group, terminal);
        for line in &self.details {
            text.push('\n');
            text.push_str(&line.text);
        }
        text
    }

    fn refresh_detail_order(&mut self) {
        if !self.details_dirty {
            return;
        }
        let mut indexes: Vec<_> = self.entries.keys().copied().collect();
        if self.kind == ResultKind::Pair {
            let compare = |a: &usize, b: &usize| {
                self.entries[b]
                    .data
                    .value
                    .unwrap_or(0.0)
                    .total_cmp(&self.entries[a].data.value.unwrap_or(0.0))
                    .then(a.cmp(b))
            };
            // Top 模式只排序会显示的部分，避免大队友表全量排序。
            if let Some(top) = self.top.filter(|top| *top < indexes.len()) {
                indexes.select_nth_unstable_by(top, compare);
                indexes.truncate(top);
            }
            indexes.sort_unstable_by(compare);
        }
        if let Some(top) = self.top {
            indexes.truncate(top);
        }
        self.detail_indexes = indexes;
        // 导出结果按旧版文本块逐行显示，属性与技能的排版因此与纯文本完全一致。
        self.details = if self.kind == ResultKind::Diy {
            self.diy_lines
                .iter()
                .map(|line| DetailLine {
                    text: line.clone(),
                    kind: EntryKind::Plain,
                })
                .collect()
        } else {
            self.detail_indexes
                .iter()
                .map(|&index| {
                    let entry = &self.entries[&index];
                    DetailLine {
                        text: detail_line_text(entry),
                        kind: entry.data.kind,
                    }
                })
                .collect()
        };
        self.details_dirty = false;
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
    pub column_widths: [f32; 7],
    pub column_alignments: [ColumnAlign; 7],
    pub card_align: ColumnAlign,
    /// 表格区域高度；[`TABLE_HEIGHT_AUTO`] 表示按可用高度自动分配。
    pub table_height: f32,
    records: HashMap<usize, Record>,
    order: VecDeque<usize>,
    expanded: HashSet<usize>,
    rows: Vec<ResultRow>,
    selected: Option<usize>,
    dirty: bool,
    rendered_mode: ViewMode,
    bytes: usize,
    pub trimmed: usize,
    terminal: Option<String>,
    /// 勾选“跟随最新”时的一次性跳转请求；结果区与日志共用同一个跟随开关。
    follow_jump: bool,
    /// 上一次渲染时结果区与日志滚动区的（内容高度, 视口高度），用于把跳转换算成精确偏移。
    results_extent: Option<(f32, f32)>,
    log_extent: Option<(f32, f32)>,
}

impl Default for ResultsView {
    fn default() -> Self {
        Self {
            mode: ViewMode::Cards,
            follow: true,
            column_widths: [220.0, 132.0, 82.0, 82.0, 82.0, 82.0, 100.0],
            column_alignments: [
                ColumnAlign::Left,
                ColumnAlign::Left,
                ColumnAlign::Right,
                ColumnAlign::Right,
                ColumnAlign::Right,
                ColumnAlign::Right,
                ColumnAlign::Right,
            ],
            card_align: ColumnAlign::Left,
            table_height: TABLE_HEIGHT_AUTO,
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
            follow_jump: false,
            results_extent: None,
            log_extent: None,
        }
    }
}

impl ResultsView {
    /// 按指定视图模式创建空结果视图；初始状态与设置恢复共用。
    pub(crate) fn with_mode(mode: ViewMode) -> Self { Self { mode, ..Self::default() } }

    pub fn clear(&mut self) {
        *self = Self {
            mode: self.mode,
            follow: self.follow,
            column_widths: self.column_widths,
            column_alignments: self.column_alignments,
            card_align: self.card_align,
            table_height: self.table_height,
            ..Self::default()
        };
    }

    /// 勾选“跟随最新”时立即跳到最新；取消勾选只停止跟随，不改变当前位置。
    pub(crate) fn set_follow(&mut self, follow: bool) {
        if follow && !self.follow {
            self.follow_jump = true;
        }
        self.follow = follow;
    }

    /// 取出一次性的“跳到最新”请求，换算成精确的底部偏移。
    ///
    /// 用上一次渲染的内容高度与视口高度算出到底部的偏移，勾选后当帧就直接落在最新一行；
    /// 还没有度量时返回 0，由 `stick_to_bottom` 在后续帧接管。
    pub(crate) fn take_follow_jump(&mut self, extent: Option<(f32, f32)>) -> Option<f32> {
        if !std::mem::take(&mut self.follow_jump) {
            return None;
        }
        Some(extent.map_or(0.0, |(content, viewport)| (content - viewport).max(0.0)))
    }

    /// 记录结果区滚动区本次的内容高度与视口高度。
    pub(crate) fn set_results_extent(&mut self, content: f32, viewport: f32) { self.results_extent = Some((content, viewport)); }

    /// 记录日志滚动区本次的内容高度与视口高度。
    pub(crate) fn set_log_extent(&mut self, content: f32, viewport: f32) { self.log_extent = Some((content, viewport)); }

    /// 上一次渲染时结果区滚动区的度量。
    pub(crate) fn results_extent(&self) -> Option<(f32, f32)> { self.results_extent }

    /// 上一次渲染时日志滚动区的度量。
    pub(crate) fn log_extent(&self) -> Option<(f32, f32)> { self.log_extent }

    /// 表格区域高度；自动模式按可用高度取比例，拖动后按保存的像素值。
    pub(crate) fn table_area_height(&self, available: f32) -> f32 {
        let limit = (available - TABLE_DETAIL_MIN).max(TABLE_HEIGHT_MIN);
        if self.table_height > TABLE_HEIGHT_AUTO {
            self.table_height.clamp(TABLE_HEIGHT_MIN, limit)
        } else {
            (available * TABLE_AUTO_RATIO).clamp(TABLE_HEIGHT_MIN, limit)
        }
    }

    /// 记录拖动后的表格高度；`None` 恢复自动分配。
    pub(crate) fn set_table_height(&mut self, height: Option<f32>) {
        self.table_height = match height {
            Some(height) if height.is_finite() => height.clamp(TABLE_HEIGHT_MIN, TABLE_HEIGHT_MAX),
            _ => TABLE_HEIGHT_AUTO,
        };
    }

    /// 当前选中卡片或选中行的可复制文本；没有选中项时返回 `None`。
    pub(crate) fn copy_selected_text(&mut self) -> Option<String> {
        let group = self.selected?;
        self.copy_record_text(group)
    }

    /// 是否有选中的卡片或行；底部“复制全部”按钮据此决定是否可用。
    pub(crate) fn has_selection(&self) -> bool { self.selected.is_some() }

    /// 指定结果块的可复制文本：标题行加明细行；明细顺序在此处刷新。
    pub(crate) fn copy_record_text(&mut self, group: usize) -> Option<String> {
        let terminal = self.terminal.as_deref();
        let record = self.records.get_mut(&group)?;
        record.refresh_detail_order();
        Some(record.copy_text(group, terminal))
    }

    pub fn finish(&mut self, status: &str) { self.terminal = Some(status.to_owned()); }

    pub fn apply(&mut self, update: ResultUpdate, log: &mut LogBuffer) {
        let group = update.group;
        let prefix = format!("[#{} {}]", group + 1, update.label);
        if update.legacy_log_authoritative {
            if let Some((text, kind)) = update.legacy_log.as_ref() {
                if *kind == EntryKind::SkillBoard {
                    for line in text.lines() {
                        log.append(line, LogKind::SkillBoard);
                    }
                } else if update.kind == ResultKind::Diy {
                    log.append_diy_block(text, log_kind(*kind));
                } else {
                    log.append(text, log_kind(*kind));
                }
            }
        } else {
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
        }
        if let Some(finish) = &update.finish {
            // 从未展示过预览的过滤项保持静默，避免关闭屏幕输出后仍刷出每个名字。
            if !finish.visible && update.entries.is_empty() && !self.records.contains_key(&group) {
                return;
            }
            if !update.legacy_log_authoritative {
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
            }
            if !finish.visible {
                self.remove(group);
                return;
            }
        }
        let record = self.records.entry(group).or_insert_with(|| {
            self.dirty = true;
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
                detail_indexes: Vec::new(),
                diy_lines: Vec::new(),
                details: Vec::new(),
                details_dirty: true,
            }
        });
        for entry in update.entries {
            record.details_dirty = true;
            if self.mode == ViewMode::Cards && self.expanded.contains(&group) {
                self.dirty = true;
            }
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
        if update.kind == ResultKind::Diy
            && let Some((text, _)) = &update.legacy_log
        {
            record.diy_lines = text.lines().map(str::to_owned).collect();
            record.details_dirty = true;
        }
        if update.finish.is_some() {
            record.finish = update.finish;
        }
        record.update_summary();
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
}

/// 结构化结果的一行详情文本；标签或数值为空时不留多余空白。
fn detail_line_text(entry: &DisplayEntry) -> String {
    let display = entry.display.replace(['\n', '\r'], " ");
    match (entry.data.label.is_empty(), display.is_empty()) {
        (true, _) => display,
        (false, true) => entry.data.label.clone(),
        (false, false) => format!("{}   {display}", entry.data.label),
    }
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
mod tests;
