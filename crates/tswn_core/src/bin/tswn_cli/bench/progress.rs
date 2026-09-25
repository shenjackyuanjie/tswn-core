//! 批量评估共用的终端进度条。
//!
//! `batch-rate` 与 `pair` 的外层循环都以“对局”为最小进度单位，但 ETA 估算方式
//! 不同（batch-rate 按已完成 player 的平均耗时，pair 按整体线性外推），这里只
//! 保留两者共享的进度条绘制与结算结构。

use std::io::{self, IsTerminal, Write as _};
use std::time::{Duration, Instant};

use super::common::format_duration;

pub(super) const PROGRESS_BAR_WIDTH: usize = 30;
pub(super) const SLIDING_WINDOW: usize = 5;

/// 批量评估共用的终端进度条（`batch-rate` 与 `pair` 的外层循环都在用）。
///
/// 以“对局”（player × target）为最小进度单位，但 ETA 以“已完成 player 的平均耗时”估算，
/// 这样既能在细粒度上持续动起来，又不会被单个 matchup 的抖动放大。
pub(super) struct BatchProgress {
    enabled: bool,
    total_players: usize,
    completed_players: usize,
    targets_per_player: usize,
    completed_targets_in_current: usize,
    player_durations: Vec<Duration>,
    started_at: Instant,
}

impl BatchProgress {
    pub(super) fn new(total_players: usize, targets_per_player: usize) -> Self {
        Self {
            enabled: io::stderr().is_terminal(),
            total_players,
            completed_players: 0,
            targets_per_player,
            completed_targets_in_current: 0,
            player_durations: Vec::with_capacity(total_players),
            started_at: Instant::now(),
        }
    }

    /// 完成当前选手的一个 target 对局后刷新进度条。
    pub(super) fn tick_target(&mut self) {
        self.completed_targets_in_current += 1;
        self.draw();
    }

    /// 完成一个 player 的全部 matchups，并记录耗时用于 ETA。
    pub(super) fn complete_player(&mut self, duration: Duration) {
        self.completed_players += 1;
        self.completed_targets_in_current = 0;
        self.player_durations.push(duration);
    }

    /// 清除当前进度行，给详细输出腾地方。
    pub(super) fn clear(&self) {
        if !self.enabled {
            return;
        }
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }

    /// 绘制 / 刷新进度条。
    pub(super) fn draw(&self) {
        if !self.enabled {
            return;
        }
        let total_matchups = self.total_players * self.targets_per_player;
        if total_matchups == 0 {
            return;
        }

        let done_matchups = self.completed_players * self.targets_per_player + self.completed_targets_in_current;
        let frac = done_matchups as f64 / total_matchups as f64;
        let filled = (frac * PROGRESS_BAR_WIDTH as f64) as usize;
        let empty = PROGRESS_BAR_WIDTH.saturating_sub(filled);
        let remaining_players = self.total_players - self.completed_players;

        let total_eta = if self.completed_players > 0 {
            let elapsed = self.started_at.elapsed().as_secs_f64();
            let avg_per_player = elapsed / self.completed_players as f64;
            format_duration(avg_per_player * remaining_players as f64)
        } else {
            "--".to_string()
        };

        let sliding_eta = if self.completed_players > 0 {
            let window_start = self.player_durations.len().saturating_sub(SLIDING_WINDOW);
            let window = &self.player_durations[window_start..];
            let window_sum: f64 = window.iter().map(|d| d.as_secs_f64()).sum();
            let avg_per_player = window_sum / window.len() as f64;
            format_duration(avg_per_player * remaining_players as f64)
        } else {
            "--".to_string()
        };

        let bar_filled: String = "█".repeat(filled);
        let bar_empty: String = "░".repeat(empty);
        eprint!(
            "\r进度 [{bar_filled}{bar_empty}] {done}/{total} ({pct:.1}%) | 预计: {total_eta} | 滑动: {sliding_eta}\x1b[K",
            done = done_matchups,
            total = total_matchups,
            pct = frac * 100.0,
        );
        let _ = io::stderr().flush();
    }

    /// 全部完成时打印汇总信息。
    pub(super) fn finish(&self) {
        if !self.enabled {
            return;
        }
        self.clear();
        let elapsed = self.started_at.elapsed();
        eprintln!(
            "完成: {}/{} 组选手, 总用时: {}",
            self.completed_players,
            self.total_players,
            format_duration(elapsed.as_secs_f64()),
        );
    }
}
