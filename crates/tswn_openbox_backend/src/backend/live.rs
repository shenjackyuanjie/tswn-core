//! GUI 的结构化增量结果与有界收件箱；计算线程不等待界面排空通道。
//!
//! 每次任务创建新的 LiveFeed，clone 共享同一个短临界区互斥队列；这不是无锁通道。
//! 数据按估算的事件字节数限制为 8 MiB，超限丢弃旧事件，超大单条直接丢弃；
//! 估算不包含所有容器容量和分配器开销，因此不是进程内存硬上限。进度与终态
//! 在队列外各保留最新值，消费者应按 dropped 提示展示不完整，并以输出文件为准。

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

/// 一条自有结果明细，index 是输入位置，不以可能重复的显示名字作为主键。
/// value 为 None 时使用文本展示；跨线程移交整个对象即可，不需要保留生产者。
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
/// 同一输入组的一批增量明细；group 为组索引，entries 由各自 index 标识。
/// finish 缺失表示仍在计算，不能把当前预览当成完整排名；legacy_log 可保留旧文本。
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

/// 仅在任务调用期间借用的同步观察者，可从计算 worker 并发调用，故要求 Sync。
/// 回调接收自有更新，应尽快转发到 UI 收件箱；本层不会替观察者捕获 panic。
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
/// take 一次移出的独立批次；拥有事件队列、最新进度/终态与本批丢弃数。
/// take 后 feed 重新累积新数据，消费者可在不持锁的情况下格式化和绘制本批。
pub struct LiveBatch {
    pub events: VecDeque<LiveEvent>,
    pub progress: Option<(usize, usize)>,
    pub done: Option<Result<String, String>>,
    pub dropped: usize,
    bytes: usize,
}

/// 一个任务的共享收件箱；默认构造新队列，clone 只共享该队列，不应复用于下一个任务。
///
/// 容量裁剪只影响普通事件，最新终态不被裁剪；持锁期间若发生 panic 导致锁中毒，
/// 后续操作会 panic，调用方不应把它当成可以恢复的业务错误。
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

    /// 用空批次替换并移出全部待处理数据；不逐条克隆，返回后格式化/渲染在锁外进行。
    /// 终态和丢弃计数也被移出，不会在下一次 take 重复返回。
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
