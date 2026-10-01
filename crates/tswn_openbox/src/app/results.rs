//! 三种视图共用的结果模型；只格式化变化项，布局限于当前可见行。

mod view;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tswn_openbox::backend::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultUpdate};

use super::log::{LogBuffer, LogKind};

const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;

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
    details_dirty: bool,
}

impl Record {
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
        *self = Self {
            mode: self.mode,
            follow: self.follow,
            column_widths: self.column_widths,
            column_alignments: self.column_alignments,
            card_align: self.card_align,
            ..Self::default()
        };
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
