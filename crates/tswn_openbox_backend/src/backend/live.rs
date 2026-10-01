//! GUI 的结构化增量结果与有界收件箱；计算线程不等待界面排空通道。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::ProgressEvent;

const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    Diy,
    Scores,
    Rate,
    Pair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Plain,
    Highlight,
    SkillBoard,
}

/// 索引是输入中的位置，不以可能重复的显示名字作为主键。
#[derive(Debug, Clone)]
pub struct ResultEntry {
    pub index: usize,
    pub label: String,
    pub value: Option<f64>,
    pub text: String,
    pub kind: EntryKind,
}

impl ResultEntry {
    pub fn number(index: usize, label: String, value: f64) -> Self {
        Self {
            index,
            label,
            value: Some(value),
            text: String::new(),
            kind: EntryKind::Plain,
        }
    }

    pub fn bytes(&self) -> usize { size_of::<Self>() + self.label.len() + self.text.len() }
}

#[derive(Debug, Clone)]
pub struct ResultFinish {
    pub score: Option<f64>,
    pub visible: bool,
    pub highlight: bool,
}

#[derive(Debug, Clone)]
pub struct ResultUpdate {
    pub group: usize,
    pub label: String,
    pub kind: ResultKind,
    pub precision: usize,
    pub entries: Vec<ResultEntry>,
    pub finish: Option<ResultFinish>,
    /// pair 的当前 Top 上限；None 表示展示所有已筛选明细。
    pub top: Option<usize>,
    /// 兼容旧版纯文本日志的完整结果块；结构化视图不依赖此字段。
    pub legacy_log: Option<(String, EntryKind)>,
    /// 后端设置后，前端不得用结构化字段生成替代文本。
    pub legacy_log_authoritative: bool,
}

impl ResultUpdate {
    pub fn new(group: usize, label: &str, kind: ResultKind, precision: usize) -> Self {
        Self {
            group,
            label: label.to_owned(),
            kind,
            precision,
            entries: Vec::new(),
            finish: None,
            top: None,
            legacy_log: None,
            legacy_log_authoritative: false,
        }
    }

    pub fn new_with_legacy(group: usize, label: &str, kind: ResultKind, precision: usize) -> Self {
        let mut update = Self::new(group, label, kind, precision);
        update.legacy_log_authoritative = true;
        update
    }

    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.label.len()
            + self.legacy_log.as_ref().map_or(0, |(text, _)| text.len())
            + self.entries.iter().map(ResultEntry::bytes).sum::<usize>()
    }
}

pub type ResultObserver<'a> = Option<&'a (dyn Fn(ResultUpdate) + Sync)>;

#[derive(Debug)]
pub enum LiveEvent {
    Result(ResultUpdate),
    Log(String),
}

impl LiveEvent {
    fn bytes(&self) -> usize {
        match self {
            Self::Result(update) => update.bytes(),
            Self::Log(text) => size_of::<Self>() + text.len(),
        }
    }
}

#[derive(Default)]
pub struct LiveBatch {
    pub events: VecDeque<LiveEvent>,
    pub progress: Option<(usize, usize)>,
    pub done: Option<Result<String, String>>,
    pub dropped: usize,
    bytes: usize,
}

/// 每个任务独享一个收件箱，旧任务无法污染新任务；终态永不被容量裁剪。
#[derive(Clone, Default)]
pub struct LiveFeed(Arc<Mutex<LiveBatch>>);

impl LiveFeed {
    pub fn result(&self, update: ResultUpdate) { self.push(LiveEvent::Result(update)); }

    pub fn progress(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::Progress { done, total } => self.0.lock().unwrap().progress = Some((done, total)),
            ProgressEvent::Done(result) => self.0.lock().unwrap().done = Some(result),
            ProgressEvent::Log(text) | ProgressEvent::HighlightLog(text) | ProgressEvent::SkillBoardLog(text) => {
                self.push(LiveEvent::Log(text));
            }
        }
    }

    fn push(&self, event: LiveEvent) {
        let bytes = event.bytes();
        let mut pending = self.0.lock().unwrap();
        // 超大单条不能突破队列预算；文件输出仍保留完整结果。
        if bytes > MAX_PENDING_BYTES {
            pending.dropped += 1;
            return;
        }
        while pending.bytes + bytes > MAX_PENDING_BYTES {
            if let Some(old) = pending.events.pop_front() {
                pending.bytes -= old.bytes();
                pending.dropped += 1;
            }
        }
        pending.bytes += bytes;
        pending.events.push_back(event);
    }

    /// O(1) 移出队列，格式化和渲染始终在锁外进行。
    pub fn take(&self) -> LiveBatch { std::mem::take(&mut *self.0.lock().unwrap()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_text_counts_toward_queue_budget_without_losing_terminal() {
        let feed = LiveFeed::default();
        let mut huge = ResultUpdate::new_with_legacy(0, "alpha", ResultKind::Diy, 0);
        huge.legacy_log = Some(("x".repeat(MAX_PENDING_BYTES), EntryKind::Plain));
        assert!(huge.bytes() > MAX_PENDING_BYTES);
        feed.result(huge);
        for group in 0..10 {
            let mut update = ResultUpdate::new_with_legacy(group, "alpha", ResultKind::Diy, 0);
            update.legacy_log = Some(("x".repeat(1024 * 1024), EntryKind::Plain));
            feed.result(update);
        }
        feed.progress(ProgressEvent::Done(Ok("完成。".into())));
        let batch = feed.take();
        assert!(batch.bytes <= MAX_PENDING_BYTES);
        assert!(batch.dropped > 1);
        assert!(matches!(batch.events.back(), Some(LiveEvent::Result(update)) if update.group == 9));
        assert!(batch.done.unwrap().is_ok());
    }

    #[test]
    fn slow_consumer_keeps_latest_progress_and_terminal_without_blocking() {
        let feed = LiveFeed::default();
        for done in 0..10_000 {
            feed.progress(ProgressEvent::Progress { done, total: 10_000 });
        }
        for _ in 0..10 {
            feed.push(LiveEvent::Log("x".repeat(1024 * 1024)));
        }
        feed.progress(ProgressEvent::Done(Ok("完成。".into())));
        let batch = feed.take();
        assert!(batch.bytes <= MAX_PENDING_BYTES);
        assert!(batch.dropped > 0);
        assert_eq!(batch.progress, Some((9999, 10_000)));
        assert!(batch.done.unwrap().is_ok());
        assert!(feed.take().events.is_empty());
    }
}
