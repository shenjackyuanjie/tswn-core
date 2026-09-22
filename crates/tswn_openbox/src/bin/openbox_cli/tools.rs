//! 执行计划到后端输入结构的映射与调用。
//!
//! 这一层只做“翻译”与转发：字段名与默认值全部对齐
//! `tswn_openbox/src/app/actions.rs` 里 GUI 的构造方式，保证两条入口
//! 喂给后端的是同一种输入。

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::SyncSender;

use tswn_openbox::backend::{
    self, BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfSkillBoardOptions, PairInput, ProgressEvent,
};

use super::args::{CqdPlan, Job, NamerPfPlan, PairPlan};
use super::plan::SkillBoardPlan;

/// 在 worker 线程执行事件驱动型工具（namer-pf / cqd / pair）。
pub(super) fn run(job: Job, tx: SyncSender<ProgressEvent>, cancel: Arc<AtomicBool>) {
    let send = |event| {
        let _ = tx.send(event);
    };
    match job {
        Job::NamerPf(plan) => backend::run_namer_pf(namer_pf_input(plan, cancel), send),
        Job::Cqd(plan) => backend::run_batch_rate(cqd_input(plan, cancel), send),
        Job::Pair(plan) => backend::run_pair(pair_input(plan, cancel), send),
        // to-diy 是同步接口，由 main 直接执行，不会走到这里。
        Job::ToDiy(_) => send(ProgressEvent::Done(Err("to-diy 不应经此路径执行".to_string()))),
    }
}

fn namer_pf_input(plan: NamerPfPlan, cancel: Arc<AtomicBool>) -> NamerPfInput {
    NamerPfInput {
        raw: plan.raw,
        count: plan.count,
        threads: plan.threads,
        keep_rq: plan.keep_rq,
        precision: plan.precision,
        metrics: plan.metrics,
        skill_board: skill_board_options(plan.skill_board, plan.no_screen),
        cancel,
    }
}

/// GUI 的语义：勾选屏幕输出时上屏；CLI 的 `--no-screen` 统一关闭。
fn skill_board_options(plan: SkillBoardPlan, no_screen: bool) -> NamerPfSkillBoardOptions {
    NamerPfSkillBoardOptions {
        screen: plan.enabled && !no_screen,
        output_file: plan.output_file,
        config: plan.config,
    }
}

fn cqd_input(plan: CqdPlan, cancel: Arc<AtomicBool>) -> BatchRateInput {
    BatchRateInput {
        target_text: plan.target_text,
        target_factor_enabled: plan.target_factor_enabled,
        target_double_plus: plan.target_double_plus,
        player_text: plan.player_text,
        player_double_plus: plan.player_double_plus,
        show_matchups: plan.show_matchups,
        // 高亮是 GUI 的可视化语义，CLI 不映射（数据行本身一致）。
        highlight_delta: None,
        output_mode: plan.output.mode,
        output_file: plan.output.out_file,
        options: CommonBenchOptions {
            count: plan.count,
            threads: plan.threads,
            keep_rq: plan.keep_rq,
            verbose: false,
            min_screen: plan.min_screen,
            min_file: plan.min_file,
            wr_precision: plan.wr_precision,
        },
        cancel,
    }
}

fn pair_input(plan: PairPlan, cancel: Arc<AtomicBool>) -> PairInput {
    PairInput {
        target_text: plan.target_text,
        target_factor_enabled: plan.target_factor_enabled,
        player_text: plan.player_text,
        player_double_plus: plan.player_double_plus,
        teammate_text: plan.teammate_text,
        teammate_double_plus: plan.teammate_double_plus,
        teammate_factor_enabled: plan.teammate_factor_enabled,
        head: plan.head,
        detail_mode: plan.detail,
        detail_min: plan.detail_min,
        highlight_delta: None,
        output_mode: plan.output.mode,
        output_file: plan.output.out_file,
        options: CommonBenchOptions {
            count: plan.count,
            threads: plan.threads,
            keep_rq: plan.keep_rq,
            verbose: false,
            min_screen: plan.min_screen,
            min_file: plan.min_file,
            wr_precision: plan.wr_precision,
        },
        cancel,
    }
}
