//! GUI 日志的有界存储与逐行标记。

use std::collections::VecDeque;

const MAX_LOG_BYTES: usize = 4 * 1024 * 1024;

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

pub(crate) struct LogBuffer {
    lines: VecDeque<LogLine>,
    bytes: usize,
    non_blank_lines: usize,
    skill_board_lines: usize,
    max_bytes: usize,
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
            skill_board_lines: 0,
            max_bytes,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.bytes = 0;
        self.non_blank_lines = 0;
        self.skill_board_lines = 0;
    }

    pub(crate) fn is_empty(&self) -> bool { self.non_blank_lines == 0 }

    pub(crate) fn len(&self) -> usize { self.lines.len() }

    pub(crate) fn get(&self, index: usize) -> Option<&LogLine> { self.lines.get(index) }

    pub(crate) fn skill_board_line_count(&self) -> usize { self.skill_board_lines }

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

    pub(crate) fn copy_text(&self) -> String {
        let mut text = String::with_capacity(self.bytes);
        for line in &self.lines {
            text.push_str(&line.text);
            text.push('\n');
        }
        text
    }

    pub(crate) fn skill_board_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            if line.kind == LogKind::SkillBoard {
                text.push_str(line.display_text());
                text.push('\n');
            }
        }
        text
    }

    fn push_line(&mut self, text: &str, kind: LogKind) {
        self.bytes += text.len() + 1;
        self.non_blank_lines += usize::from(!text.trim().is_empty());
        self.skill_board_lines += usize::from(kind == LogKind::SkillBoard);
        self.lines.push_back(LogLine {
            text: text.to_owned(),
            kind,
        });
        // 单条超长日志仍可完整查看；后续内容到来时再淘汰它。
        while self.bytes > self.max_bytes && self.lines.len() > 1 {
            let removed = self.lines.pop_front().unwrap();
            self.bytes -= removed.text.len() + 1;
            self.non_blank_lines -= usize::from(!removed.text.trim().is_empty());
            self.skill_board_lines -= usize::from(removed.kind == LogKind::SkillBoard);
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
        assert_eq!(log.skill_board_text(), "技能\n");
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
        assert_eq!(log.skill_board_text(), "技能\n");

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
    fn copied_log_keeps_crlf_while_display_uses_clean_lines() {
        let mut log = LogBuffer::default();
        log.append("技能\r\n说明\r\n", LogKind::SkillBoard);

        assert_eq!(log.copy_text(), "技能\r\n说明\r\n");
        assert_eq!(log.get(0).unwrap().display_text(), "技能");
        assert_eq!(log.skill_board_text(), "技能\n");
    }
}
