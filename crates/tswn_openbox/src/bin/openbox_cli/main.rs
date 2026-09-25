//! tswn_openbox GUI 的无头 CLI 入口。
//!
//! 与 GUI 共用同一套后端（`tswn_openbox::backend`）与预设
//! （`tswn_openbox::presets`）：同样的解析、执行、输出格式化与配置约定，
//! 因此GUI 面板能跑的工作流可以逐字节复现到脚本里。
//!
//! 输出约定：
//! - 评分/胜率/技能榜等数据行走 stdout（GUI 日志中的数据行），
//!   高亮与技能榜颜色降级为普通行，保证管道友好；
//! - 进度与完成状态走 stderr（tty 时才刷新进度行）；
//! - `to-diy` 是同步后端接口，直接在当前线程执行并打印结果；
//!   namer-pf / cqd / pair 在 worker 线程执行，主线程排空事件通道。
//! - 未实现 GUI 的“停止”按钮：Ctrl+C 直接终止进程。

mod args;
mod input;
mod plan;
mod tools;

use std::io::{IsTerminal, Write as _};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread;
use std::time::Instant;

use tswn_openbox::backend::{self, ProgressEvent};

/// 与 GUI 相同量级的有界事件通道；后端高频进度事件不会无限积压。
const EVENT_CHANNEL_CAPACITY: usize = 4096;
/// 进度行最小刷新间隔（ms）：短 matchup 每秒可完成数万次，不能让刷新反噬吞吐。
const PROGRESS_MIN_INTERVAL_MS: u128 = 40;

fn main() {
    let cli = args::parse().unwrap_or_else(|err| err.exit());
    let job = cli.plan().unwrap_or_else(|err| err.exit());
    let cancel = Arc::new(AtomicBool::new(false));

    // `to-diy` 的后端是同步接口（无进度事件），直接执行。
    if let args::Job::ToDiy(plan) = job {
        let writes_file = plan.output_file.is_some();
        let result = backend::run_to_diy(&plan.raw, plan.old, plan.minions, plan.details, plan.output_file, &cancel);
        match result {
            // 输出到屏幕时返回值就是数据本体（stdout）；写文件时是完成状态（stderr）。
            Ok(message) if writes_file => eprintln!("{message}"),
            Ok(message) => println!("{message}"),
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        return;
    }

    let (tx, rx) = sync_channel(EVENT_CHANNEL_CAPACITY);
    let worker_cancel = Arc::clone(&cancel);
    let worker = thread::spawn(move || tools::run(job, tx, worker_cancel));
    let exit_code = drain_events(rx);
    if worker.join().is_err() {
        // worker panic：重新抛出，让默认 hook 打印 backtrace 并以 101 退出。
        std::process::exit(101);
    }
    let _ = std::io::stdout().flush();
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

/// 排空后端事件通道，返回进程退出码。
///
/// 数据行打印到 stdout，进度与状态打印到 stderr；`Done` 是协议的最后一个事件。
fn drain_events(rx: Receiver<ProgressEvent>) -> i32 {
    let mut exit_code = 0;
    let mut last_draw = Instant::now();
    while let Ok(event) = rx.recv() {
        match event {
            ProgressEvent::Log(line) => println!("{line}"),
            // GUI 的红色“超强名字”行与蓝色技能榜行在 CLI 里降级为普通行，
            // 内容完全一致，只是不带颜色，保证管道与重定向友好。
            ProgressEvent::HighlightLog(line) => println!("{line}"),
            ProgressEvent::SkillBoardLog(line) => println!("{line}"),
            ProgressEvent::Progress { done, total } => draw_progress(done, total, &mut last_draw),
            ProgressEvent::Done(result) => {
                match result {
                    Ok(message) => eprintln!("{message}"),
                    Err(err) => {
                        eprintln!("{err}");
                        exit_code = 1;
                    }
                }
                break;
            }
        }
    }
    exit_code
}

/// tty 时按节流刷新单行进度；重定向到文件时静默。
fn draw_progress(done: usize, total: usize, last_draw: &mut Instant) {
    if !std::io::stderr().is_terminal() {
        return;
    }
    let now = Instant::now();
    if done < total && now.duration_since(*last_draw).as_millis() < PROGRESS_MIN_INTERVAL_MS {
        return;
    }
    *last_draw = now;
    let percent = if total > 0 { done as f64 * 100.0 / total as f64 } else { 0.0 };
    eprint!("\r进度: {done}/{total} ({percent:.1}%)");
    let _ = std::io::stderr().flush();
    if done >= total {
        eprintln!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_draw_is_silent_when_stderr_is_redirected() {
        // 测试环境的 stderr 通常不是 tty：draw_progress 应立即返回、不 panic。
        let mut last = Instant::now();
        draw_progress(1, 10, &mut last);
        draw_progress(10, 10, &mut last);
    }

    #[test]
    fn drain_events_collects_lines_and_done_exit_code() {
        let (tx, rx) = sync_channel(16);
        let sender: std::sync::mpsc::SyncSender<ProgressEvent> = tx;
        sender.send(ProgressEvent::Log("mario pp:1".to_string())).unwrap();
        sender.send(ProgressEvent::SkillBoardLog("冰冻qp 1 mario".to_string())).unwrap();
        sender.send(ProgressEvent::Done(Ok("完成。".to_string()))).unwrap();
        drop(sender);
        assert_eq!(drain_events(rx), 0);

        let (tx, rx) = sync_channel(16);
        tx.send(ProgressEvent::Done(Err("失败".to_string()))).unwrap();
        drop(tx);
        assert_eq!(drain_events(rx), 1);
    }
}
