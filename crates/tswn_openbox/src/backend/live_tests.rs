//! 验证实际后端的提前发布、并发归属与旧输出兼容性。

use std::cell::RefCell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::*;

fn options(threads: usize) -> CommonBenchOptions {
    CommonBenchOptions {
        count: 24,
        threads: Some(threads),
        keep_rq: true,
        verbose: false,
        min_screen: None,
        min_file: None,
        wr_precision: 9,
    }
}

fn batch(threads: usize) -> BatchRateInput {
    BatchRateInput {
        target_text: "gamma@green\ndelta@yellow".into(),
        player_text: "alpha@red\nalpha@red".into(),
        target_factor_enabled: false,
        target_double_plus: false,
        player_double_plus: false,
        show_matchups: true,
        highlight_delta: None,
        output_mode: OutputMode::Log,
        output_file: None,
        options: options(threads),
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

fn pair(threads: usize) -> PairInput {
    PairInput {
        target_text: "gamma@green\ndelta@yellow".into(),
        target_factor_enabled: false,
        player_text: "alpha@red\nalpha@red".into(),
        player_double_plus: false,
        teammate_text: "mate@blue\nmate2@blue\nmate@blue".into(),
        teammate_double_plus: false,
        teammate_factor_enabled: false,
        head: 2,
        detail_mode: PairDetailMode::Every,
        detail_min: None,
        highlight_delta: None,
        output_mode: OutputMode::Log,
        output_file: None,
        options: options(threads),
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn batch_detail_is_published_before_matrix_finishes() {
    let input = batch(1);
    let cancel = input.cancel.clone();
    let updates = Mutex::new(Vec::new());
    run_batch_rate_observed(
        input,
        |_| {},
        Some(&|update| {
            updates.lock().unwrap().push(update);
            cancel.store(true, Ordering::Relaxed);
        }),
    );
    let updates = updates.into_inner().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].group, 0);
    assert_eq!(updates[0].entries[0].index, 0);
    assert!(updates[0].finish.is_none());
}

#[test]
fn pair_detail_is_published_before_window_and_player_finish() {
    let input = pair(1);
    let cancel = input.cancel.clone();
    let updates = Mutex::new(Vec::new());
    run_pair_observed(
        input,
        |_| {},
        Some(&|update| {
            updates.lock().unwrap().push(update);
            cancel.store(true, Ordering::Relaxed);
        }),
    );
    let updates = updates.into_inner().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].entries[0].index, 0);
    assert!(updates[0].finish.is_none());
}

#[test]
fn diy_publishes_completed_line_before_reading_next_line() {
    let cancel = AtomicBool::new(false);
    let updates = Mutex::new(Vec::new());
    let result = run_to_diy_observed(
        "alpha\nbeta",
        false,
        false,
        true,
        None,
        &cancel,
        Some(&|update| {
            updates.lock().unwrap().push(update);
            cancel.store(true, Ordering::Relaxed);
        }),
    )
    .unwrap();
    assert_eq!(result, "已停止。");
    let updates = updates.into_inner().unwrap();
    assert_eq!(updates.len(), 1);
    assert!(updates[0].finish.is_some());
    assert!(updates[0].entries.len() > 8);
}

#[test]
fn live_scores_match_legacy_and_are_independent_of_threads_and_duplicate_labels() {
    for threads in [1, 4] {
        let updates = Mutex::new(Vec::new());
        let old = RefCell::new(Vec::new());
        run_batch_rate(batch(threads), |event| old.borrow_mut().push(event));
        run_batch_rate_observed(batch(threads), |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
        let updates = updates.into_inner().unwrap();
        for group in 0..2 {
            let entries = updates
                .iter()
                .filter(|u| u.group == group)
                .flat_map(|u| u.entries.iter())
                .collect::<Vec<_>>();
            assert_eq!(entries.len(), 2);
            let finish = updates
                .iter()
                .find(|u| u.group == group && u.finish.is_some())
                .unwrap()
                .finish
                .as_ref()
                .unwrap();
            let line = format!("{:.9} alpha@red", finish.score.unwrap());
            assert!(
                old.borrow()
                    .iter()
                    .any(|event| matches!(event, ProgressEvent::Log(text) if text.starts_with(&line)))
            );
        }
        let updates = Mutex::new(Vec::new());
        let old = RefCell::new(Vec::new());
        run_pair(pair(threads), |event| old.borrow_mut().push(event));
        run_pair_observed(pair(threads), |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
        let updates = updates.into_inner().unwrap();
        for group in 0..2 {
            let mut entries = updates
                .iter()
                .filter(|u| u.group == group)
                .flat_map(|u| &u.entries)
                .map(|e| e.index)
                .collect::<Vec<_>>();
            entries.sort();
            assert_eq!(entries, vec![0, 1, 2]);
            let finish = updates
                .iter()
                .find(|u| u.group == group && u.finish.is_some())
                .unwrap()
                .finish
                .as_ref()
                .unwrap();
            let line = format!("{:.9} alpha@red", finish.score.unwrap());
            assert!(
                old.borrow()
                    .iter()
                    .any(|event| matches!(event, ProgressEvent::Log(text) if text.starts_with(&line)))
            );
        }
    }
}

#[test]
fn observed_file_bytes_match_legacy_for_all_formats() {
    let root = std::env::temp_dir().join(format!("openbox-live-compat-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for mode in [OutputMode::Log, OutputMode::Jsonl, OutputMode::Pure] {
        let before = root.join("before.txt");
        let after = root.join("after.txt");
        let mut input = batch(4);
        input.output_mode = mode;
        input.output_file = Some(before.clone());
        run_batch_rate(input.clone(), |_| {});
        input.output_file = Some(after.clone());
        run_batch_rate_observed(input, |_| {}, Some(&|_| {}));
        assert_eq!(std::fs::read(&before).unwrap(), std::fs::read(&after).unwrap());
        let mut input = pair(4);
        input.output_mode = mode;
        input.output_file = Some(before.clone());
        run_pair(input.clone(), |_| {});
        input.output_file = Some(after.clone());
        run_pair_observed(input, |_| {}, Some(&|_| {}));
        assert_eq!(std::fs::read(&before).unwrap(), std::fs::read(&after).unwrap());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn namer_metrics_stream_before_final_result_and_keep_old_text() {
    let metrics = NamerPfMetric::ALL
        .into_iter()
        .map(|metric| NamerPfMetricOptions {
            metric,
            screen: true,
            min_screen: None,
            highlight_delta: None,
            output_file: None,
            min_file: None,
        })
        .collect();
    let input = NamerPfInput {
        raw: "alpha@red\nalpha@red".into(),
        count: 1,
        threads: Some(4),
        keep_rq: true,
        precision: 3,
        metrics,
        skill_board: NamerPfSkillBoardOptions {
            screen: false,
            output_file: None,
            config: None,
        },
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let old = RefCell::new(Vec::new());
    run_namer_pf(input.clone(), |event| old.borrow_mut().push(event));
    let updates = Mutex::new(Vec::new());
    run_namer_pf_observed(input, |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
    let updates = updates.into_inner().unwrap();
    for group in 0..2 {
        let group_updates = updates.iter().filter(|u| u.group == group).collect::<Vec<_>>();
        assert_eq!(group_updates.len(), 6);
        assert!(group_updates[..5].iter().all(|u| u.finish.is_none()));
        assert!(group_updates[5].finish.is_some());
        for update in &group_updates[..5] {
            let entry = &update.entries[0];
            let expected = format!("alpha@red {}:{:.3}", entry.label, entry.value.unwrap());
            assert!(
                old.borrow()
                    .iter()
                    .any(|event| matches!(event, ProgressEvent::Log(text) if text == &expected))
            );
        }
    }
}
