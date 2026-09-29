//! 后台任务生命周期、收件箱消费与进度统计。

use super::log::LogKind;
use super::state::OpenboxApp;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tswn_openbox::backend::{ProgressEvent, live::LiveFeed};

const MAX_EVENTS_PER_POLL: usize = 256;
const RUNNING_REPAINT_INTERVAL: Duration = Duration::from_millis(100);

impl OpenboxApp {
    pub fn stop_current_task(&mut self) {
        if let Some(token) = &self.cancel_token {
            token.store(true, Ordering::Relaxed);
            self.cancel_requested = true;
            self.status = "停止中".to_string();
        }
    }

    pub(super) fn spawn_worker(&mut self, task: impl FnOnce(&LiveFeed) + Send + 'static) {
        let feed = LiveFeed::default();
        self.live_feed = Some(feed.clone());
        if let Err(err) = std::thread::Builder::new().name("openbox-task".into()).spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task(&feed))).is_err() {
                feed.progress(ProgressEvent::Done(Err("计算线程异常退出。".into())));
            }
        }) {
            self.fail_before_start(format!("启动计算线程失败: {err}"));
        }
    }

    pub fn begin_task(&mut self) {
        self.live_feed = None;
        self.pending_live = Default::default();
        self.running = true;
        self.cancel_requested = false;
        self.cancel_token = Some(Arc::new(AtomicBool::new(false)));
        self.done = 0;
        self.total = 0;
        self.started_at = Some(Instant::now());
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.clear_results();
        self.status = "运行中".to_string();
    }

    pub fn fail_before_start(&mut self, err: String) {
        self.running = false;
        self.cancel_requested = false;
        self.cancel_token = None;
        self.done = 0;
        self.total = 0;
        self.started_at = None;
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.live_feed = None;
        self.pending_live = Default::default();
        self.status = "失败".to_string();
        self.clear_results();
        self.append_log(&err);
    }

    pub fn clear_results(&mut self) {
        self.log.clear();
        self.results.clear();
        // 清空当前已入队内容，保留终态与进度，避免任务完成消息丢失。
        self.pending_live.events.clear();
        if let Some(feed) = &self.live_feed {
            let batch = feed.take();
            if batch.progress.is_some() {
                self.pending_live.progress = batch.progress;
            }
            if batch.done.is_some() {
                self.pending_live.done = batch.done;
            }
        }
    }

    pub fn poll_events(&mut self, ctx: &egui::Context) {
        if self.pending_live.events.is_empty()
            && self.pending_live.done.is_none()
            && self.last_live_poll.elapsed() >= RUNNING_REPAINT_INTERVAL
        {
            if let Some(feed) = &self.live_feed {
                self.pending_live = feed.take();
                self.results.trimmed += self.pending_live.dropped;
            }
            self.last_live_poll = Instant::now();
        }
        if let Some((done, total)) = self.pending_live.progress.take() {
            self.done = done;
            self.total = total;
        }
        let start = Instant::now();
        for _ in 0..MAX_EVENTS_PER_POLL {
            let Some(event) = self.pending_live.events.pop_front() else {
                break;
            };
            match event {
                tswn_openbox::backend::live::LiveEvent::Result(update) => self.results.apply(update, &mut self.log),
                tswn_openbox::backend::live::LiveEvent::Log(text) => self.append_log(&text),
            }
            if start.elapsed() >= Duration::from_millis(4) {
                break;
            }
        }
        if self.pending_live.events.is_empty()
            && let Some(result) = self.pending_live.done.take()
        {
            self.running = false;
            self.cancel_requested = false;
            self.cancel_token = None;
            self.status = match &result {
                Err(_) => "失败",
                Ok(message) if message == "已停止。" => "已停止",
                Ok(_) => "完成",
            }
            .into();
            self.results.finish(&self.status);
            self.update_progress_stats();
            if self.status != "完成" {
                self.eta_text = "--".into();
            }
            match result {
                Ok(text) | Err(text) => self.append_log(&text),
            }
            self.live_feed = None;
        }
        if self.running {
            self.update_progress_stats();
        }
        if !self.pending_live.events.is_empty() {
            ctx.request_repaint();
        } else if self.running {
            ctx.request_repaint_after(RUNNING_REPAINT_INTERVAL);
        }
    }

    pub fn update_progress_stats(&mut self) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let elapsed = started_at.elapsed().as_secs_f64();
        if elapsed <= 0.0 || self.done == 0 {
            self.rate_text = "--".to_string();
            self.eta_text = "--".to_string();
            return;
        }
        let rate = self.done as f64 / elapsed;
        self.rate_text = format!("{rate:.2} 项/s");
        let remaining = self.total.saturating_sub(self.done) as f64;
        self.eta_text = if rate > 0.0 {
            format_duration(remaining / rate)
        } else {
            "--".to_string()
        };
    }

    pub fn append_log(&mut self, text: &str) { self.log.append(text, LogKind::Plain); }
}

fn format_duration(secs: f64) -> String {
    if secs.is_nan() || secs.is_infinite() || secs < 0.0 {
        return "--".to_string();
    }
    let seconds = secs.round() as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m{}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{}m{}s", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
    }
}
