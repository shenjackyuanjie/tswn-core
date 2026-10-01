//! GUI 日志的有界存储与逐行标记。

use std::collections::VecDeque;

const MAX_LOG_BYTES: usize = 4 * 1024 * 1024;
/// 纯文本里不同输入组之间的分隔行。
pub(crate) const DIY_GROUP_SEPARATOR: &str = "=========";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogKind {
    Plain,
    Highlight,
    SkillBoard,
}

pub(crate) struct LogLine {
    text: String,
    pub(crate) kind: LogKind,
}

impl LogLine {
    pub(crate) fn display_text(&self) -> &str { self.text.strip_suffix('\r').unwrap_or(&self.text) }
}

/// 折行后的一段渲染行：所属日志行（绝对下标）与该段在行内的字节区间。
#[derive(Clone, Copy)]
pub(crate) struct WrappedRow {
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) struct LogBuffer {
    lines: VecDeque<LogLine>,
    bytes: usize,
    non_blank_lines: usize,
    skill_board_indices: VecDeque<usize>,
    max_bytes: usize,
    discarded: usize,
    /// 折行缓存；宽度未变时只切分新增行，行高因此保持单行高度。
    wrapped: VecDeque<WrappedRow>,
    wrapped_lines: usize,
    wrapped_width: f32,
    wrapped_ascii: f32,
}

impl Default for LogBuffer {
    fn default() -> Self { Self::with_limit(MAX_LOG_BYTES) }
}

impl LogBuffer {
    fn with_limit(max_bytes: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            non_blank_lines: 0,
            skill_board_indices: VecDeque::new(),
            max_bytes,
            discarded: 0,
            wrapped: VecDeque::new(),
            wrapped_lines: 0,
            wrapped_width: 0.0,
            wrapped_ascii: 0.0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.bytes = 0;
        self.non_blank_lines = 0;
        self.skill_board_indices.clear();
        self.discarded = 0;
        self.wrapped.clear();
        self.wrapped_lines = 0;
    }

    pub(crate) fn is_empty(&self) -> bool { self.non_blank_lines == 0 }

    pub(crate) fn len(&self) -> usize { self.lines.len() }

    pub(crate) fn get(&self, index: usize) -> Option<&LogLine> { self.lines.get(index) }

    pub(crate) fn skill_board_line_count(&self) -> usize { self.skill_board_indices.len() }

    pub(crate) fn skill_board_line(&self, index: usize) -> Option<&LogLine> {
        self.get(self.skill_board_indices.get(index)?.checked_sub(self.discarded)?)
    }

    pub(crate) fn discarded_lines(&self) -> usize { self.discarded }

    /// 按可用宽度把日志切成渲染行，返回总段数；宽度未变时只切分新增内容。
    ///
    /// 每段宽度不超过 `width`，因此渲染时仍可用固定行高做可见行虚拟化。
    pub(crate) fn wrap_rows(&mut self, width: f32, ascii_width: f32) -> usize {
        if width <= 0.0 || ascii_width <= 0.0 {
            return self.lines.len();
        }
        if (self.wrapped_width - width).abs() > 0.5 || (self.wrapped_ascii - ascii_width).abs() > 0.01 {
            self.wrapped.clear();
            self.wrapped_lines = 0;
            self.wrapped_width = width;
            self.wrapped_ascii = ascii_width;
        }
        let max_units = (width / ascii_width).floor().max(8.0);
        while self.wrapped_lines < self.lines.len() {
            let index = self.wrapped_lines;
            let line = index + self.discarded;
            let text = self.lines[index].display_text();
            let mut start = 0;
            let mut units = 0.0_f32;
            for (offset, ch) in text.char_indices() {
                // 等宽字体下 ASCII 占一个字符宽、全角字符占两个；据此估算折行点。
                let char_units = if ch.is_ascii() { 1.0 } else { 2.0 };
                if units + char_units > max_units && offset > start {
                    self.wrapped.push_back(WrappedRow {
                        line,
                        start,
                        end: offset,
                    });
                    start = offset;
                    units = 0.0;
                }
                units += char_units;
            }
            self.wrapped.push_back(WrappedRow {
                line,
                start,
                end: text.len(),
            });
            self.wrapped_lines += 1;
        }
        self.wrapped.len()
    }

    /// 第 `index` 段所属的日志行与行内字节区间；已裁剪的段返回 `None`。
    pub(crate) fn wrapped_row(&self, index: usize) -> Option<(usize, usize, usize)> {
        let row = self.wrapped.get(index)?;
        Some((row.line.checked_sub(self.discarded)?, row.start, row.end))
    }

    pub(crate) fn append(&mut self, text: &str, kind: LogKind) {
        let trimmed = text.trim_end_matches('\n');
        if trimmed.is_empty() {
            return;
        }
        // 多行结果之间保留原有的空行；标记跟随内容行一起被裁剪。
        if trimmed.contains('\n') && self.lines.back().is_some_and(|last| !last.text.is_empty()) {
            self.push_line("", LogKind::Plain);
        }
        for (index, line) in trimmed.split('\n').enumerate() {
            self.push_line(line, if index == 0 { kind } else { LogKind::Plain });
        }
    }

    /// 追加一组结果；已有内容时先插入分隔行，便于区分不同输入组。
    pub(crate) fn append_diy_block(&mut self, text: &str, kind: LogKind) {
        if text.trim().is_empty() {
            return;
        }
        if self.lines.back().is_some_and(|last| !last.text.is_empty()) {
            self.push_line("", LogKind::Plain);
            self.push_line(DIY_GROUP_SEPARATOR, LogKind::Plain);
            self.push_line("", LogKind::Plain);
        }
        self.append(text, kind);
    }

    pub(crate) fn copy_text(&self) -> String {
        let mut text = String::with_capacity(self.bytes);
        for line in &self.lines {
            text.push_str(&line.text);
            text.push('\n');
        }
        text
    }

    fn push_line(&mut self, text: &str, kind: LogKind) {
        self.bytes += text.len() + 1;
        self.non_blank_lines += usize::from(!text.trim().is_empty());
        if kind == LogKind::SkillBoard {
            self.skill_board_indices.push_back(self.discarded + self.lines.len());
        }
        self.lines.push_back(LogLine {
            text: text.to_owned(),
            kind,
        });
        // 单条超长日志仍可完整查看；后续内容到来时再淘汰它。
        while self.bytes > self.max_bytes && self.lines.len() > 1 {
            let removed = self.lines.pop_front().unwrap();
            self.discarded += 1;
            self.wrapped_lines = self.wrapped_lines.saturating_sub(1);
            self.bytes -= removed.text.len() + 1;
            self.non_blank_lines -= usize::from(!removed.text.trim().is_empty());
            if removed.kind == LogKind::SkillBoard {
                self.skill_board_indices.pop_front();
            }
            // 段按行号递增排列，被淘汰行的段都在队首。
            while self.wrapped.front().is_some_and(|row| row.line < self.discarded) {
                self.wrapped.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_log_keeps_spacing_and_line_kinds() {
        let mut log = LogBuffer::default();
        log.append("开头", LogKind::Plain);
        log.append("高亮\n明细", LogKind::Highlight);
        log.append("技能\n说明", LogKind::SkillBoard);

        assert_eq!(log.copy_text(), "开头\n\n高亮\n明细\n\n技能\n说明\n");
        assert_eq!(log.get(2).unwrap().kind, LogKind::Highlight);
        assert_eq!(log.get(5).unwrap().kind, LogKind::SkillBoard);
        assert_eq!(log.skill_board_line(0).unwrap().display_text(), "技能");
        assert_eq!(log.skill_board_line_count(), 1);
    }

    #[test]
    fn bounded_log_evicts_lines_and_their_marks_together() {
        let mut log = LogBuffer::with_limit(14);
        log.append("甲甲", LogKind::SkillBoard);
        log.append("高亮", LogKind::Highlight);
        log.append("技能", LogKind::SkillBoard);

        assert_eq!(log.copy_text(), "高亮\n技能\n");
        assert_eq!(log.get(0).unwrap().kind, LogKind::Highlight);
        assert_eq!(log.get(1).unwrap().kind, LogKind::SkillBoard);
        assert_eq!(log.skill_board_line_count(), 1);
        assert_eq!(log.skill_board_line(0).unwrap().display_text(), "技能");

        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.copy_text(), "");
        assert_eq!(log.skill_board_line_count(), 0);
    }

    #[test]
    fn oversized_single_line_stays_visible_until_new_content_arrives() {
        let mut log = LogBuffer::with_limit(4);
        log.append("123456", LogKind::Plain);
        assert_eq!(log.copy_text(), "123456\n");
        log.append("新", LogKind::Plain);
        assert_eq!(log.copy_text(), "新\n");
    }

    #[test]
    fn diy_blocks_are_separated_by_an_equals_line() {
        let mut log = LogBuffer::default();
        log.append_diy_block("甲\n", LogKind::Plain);
        log.append_diy_block("乙\n", LogKind::Plain);
        log.append_diy_block("   ", LogKind::Plain);

        assert_eq!(log.copy_text(), "甲\n\n=========\n\n乙\n");
        assert_eq!(log.get(2).unwrap().display_text(), DIY_GROUP_SEPARATOR);
    }

    #[test]
    fn wrap_rows_split_long_lines_into_visible_segments() {
        let mut log = LogBuffer::default();
        log.append("abcdefghijklmnopqrst", LogKind::Plain);
        // 每段最多 8 个单位宽：ASCII 记 1、全角记 2。
        assert_eq!(log.wrap_rows(8.0, 1.0), 3);
        let segments = (0..3)
            .map(|index| {
                let (line, start, end) = log.wrapped_row(index).unwrap();
                assert_eq!(line, 0);
                log.get(line).unwrap().display_text()[start..end].to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(segments, ["abcdefgh", "ijklmnop", "qrst"]);

        let mut wide = LogBuffer::default();
        wide.append("中文中文中文", LogKind::Plain);
        assert_eq!(wide.wrap_rows(8.0, 1.0), 2);
        let (_, start, end) = wide.wrapped_row(0).unwrap();
        assert_eq!(&wide.get(0).unwrap().display_text()[start..end], "中文中文");
    }

    #[test]
    fn wrapped_segments_follow_evicted_lines() {
        let mut log = LogBuffer::with_limit(10);
        log.append("aaaaaaaaa", LogKind::Plain);
        log.append("bbbbbbbbb", LogKind::Plain);
        assert_eq!(log.discarded_lines(), 1);
        let total = log.wrap_rows(8.0, 1.0);
        assert!(total > 0);
        // 首段应属于仍保留的那一行，切分不会引用已淘汰内容。
        let (line, start, end) = log.wrapped_row(0).unwrap();
        assert_eq!(&log.get(line).unwrap().display_text()[start..end], "bbbbbbbb");
    }

    #[test]
    fn copied_log_keeps_crlf_while_display_uses_clean_lines() {
        let mut log = LogBuffer::default();
        log.append("技能\r\n说明\r\n", LogKind::SkillBoard);

        assert_eq!(log.copy_text(), "技能\r\n说明\r\n");
        assert_eq!(log.get(0).unwrap().display_text(), "技能");
        assert_eq!(log.skill_board_line(0).unwrap().display_text(), "技能");
    }
}
