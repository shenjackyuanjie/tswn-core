//! observed 日志逐块对照旧 API，包含详情布局、筛选和高亮。

use super::*;
use crate::backend::live::{EntryKind, ResultUpdate};

fn legacy_event(event: ProgressEvent) -> Option<(String, EntryKind)> {
    match event {
        ProgressEvent::Log(text) if text != "未选择输出文件，本次只输出到日志。" => {
            Some((text, EntryKind::Plain))
        }
        ProgressEvent::HighlightLog(text) => Some((text, EntryKind::Highlight)),
        ProgressEvent::SkillBoardLog(text) => Some((text, EntryKind::SkillBoard)),
        ProgressEvent::Done(result) => {
            assert!(result.is_ok(), "{result:?}");
            None
        }
        _ => None,
    }
}

/// 并发允许各组完成顺序不同，但每个旧格式结果块和样式必须完全一致。
fn assert_legacy_logs(expected: Vec<(String, EntryKind)>, updates: &[ResultUpdate]) {
    assert!(updates.iter().all(|update| update.legacy_log_authoritative));
    let mut actual = updates.iter().filter_map(|update| update.legacy_log.clone()).collect::<Vec<_>>();
    let mut expected = expected;
    actual.sort_by(|a, b| a.0.cmp(&b.0));
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(actual, expected);
}

#[test]
fn diy_observed_legacy_preserves_details_and_export_options() {
    let raw = "1@team+2@team\ntest";
    for old in [false, true] {
        for minions in [false, true] {
            for details in [false, true] {
                let cancel = AtomicBool::new(false);
                if old && minions {
                    let expected = run_to_diy(raw, old, minions, details, None, &cancel).unwrap_err();
                    let actual = run_to_diy_observed(raw, old, minions, details, None, &cancel, Some(&|_| {})).unwrap_err();
                    assert_eq!(actual, expected);
                    continue;
                }
                let expected = run_to_diy(raw, old, minions, details, None, &cancel).unwrap();
                let updates = Mutex::new(Vec::new());
                let result = run_to_diy_observed(
                    raw,
                    old,
                    minions,
                    details,
                    None,
                    &cancel,
                    Some(&|update| updates.lock().unwrap().push(update)),
                )
                .unwrap();
                assert_eq!(result, "完成。");
                let updates = updates.into_inner().unwrap();
                assert_eq!(updates.len(), 2);
                assert!(updates.iter().all(|update| update.legacy_log_authoritative));
                let blocks = updates
                    .iter()
                    .map(|update| {
                        let (text, kind) = update.legacy_log.as_ref().unwrap();
                        assert_eq!(*kind, EntryKind::Plain);
                        assert!(update.finish.as_ref().unwrap().visible);
                        text.as_str()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(blocks.join("\n"), expected);
                if details {
                    assert!(blocks[0].contains("\n\n=== 原始信息 ===\n1@team\nHP "));
                    assert!(blocks[0].contains("\n\n=== 原始信息 ===\n2@team\nHP "));
                    assert!(
                        updates.iter().all(|update| update.entries.is_empty()),
                        "详情不再重复进入结构化条目"
                    );
                } else {
                    assert!(updates.iter().all(|update| update.entries.is_empty()));
                }
            }
        }
    }
}

#[test]
fn rate_observed_legacy_matches_thresholds_details_and_mirror_order() {
    for factored in [false, true] {
        for show_matchups in [false, true] {
            for min_screen in [None, Some(0.0), Some(101.0)] {
                let mut input = batch(4);
                input.target_factor_enabled = factored;
                input.target_text = if factored {
                    "[[targets]]\nfactor=2\nplayers=[\"gamma@green\"]\n[[targets]]\nfactor=0.5\nplayers=[\"alpha@red\"]\n[[targets]]\nfactor=1\nplayers=[\"delta@yellow\"]".into()
                } else {
                    "gamma@green\nalpha@red\ndelta@yellow".into()
                };
                input.show_matchups = show_matchups;
                input.options.min_screen = min_screen;
                input.highlight_delta = Some(0.0);
                let expected = RefCell::new(Vec::new());
                run_batch_rate(input.clone(), |event| {
                    if let Some(log) = legacy_event(event) {
                        expected.borrow_mut().push(log);
                    }
                });
                let updates = Mutex::new(Vec::new());
                run_batch_rate_observed(
                    input,
                    |event| {
                        assert!(legacy_event(event).is_none(), "observed 不应重复发布结果日志");
                    },
                    Some(&|update| updates.lock().unwrap().push(update)),
                );
                let updates = updates.into_inner().unwrap();
                assert_eq!(updates.iter().filter(|update| update.finish.is_some()).count(), 2);
                if !show_matchups {
                    assert!(updates.iter().all(|update| update.entries.is_empty()));
                }
                assert_legacy_logs(expected.into_inner(), &updates);
            }
        }
    }
}

#[test]
fn rate_and_pair_legacy_keep_zero_score_at_threshold_boundary() {
    for min_screen in [Some(0.0), Some(f64::EPSILON)] {
        let mut rate = batch(1);
        rate.player_text = "alpha@red".into();
        rate.target_text = "alpha@red".into();
        rate.options.min_screen = min_screen;
        rate.highlight_delta = Some(0.0);
        let expected = RefCell::new(Vec::new());
        run_batch_rate(rate.clone(), |event| {
            if let Some(log) = legacy_event(event) {
                expected.borrow_mut().push(log);
            }
        });
        let updates = Mutex::new(Vec::new());
        run_batch_rate_observed(rate, |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
        let updates = updates.into_inner().unwrap();
        assert_eq!(updates.last().unwrap().finish.as_ref().unwrap().score, Some(0.0));
        assert_legacy_logs(expected.into_inner(), &updates);

        let mut pair = pair(1);
        pair.player_text = "alpha@red".into();
        pair.teammate_text = "alpha@red".into();
        pair.options.min_screen = min_screen;
        pair.highlight_delta = Some(0.0);
        let expected = RefCell::new(Vec::new());
        run_pair(pair.clone(), |event| {
            if let Some(log) = legacy_event(event) {
                expected.borrow_mut().push(log);
            }
        });
        let updates = Mutex::new(Vec::new());
        run_pair_observed(pair, |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
        let updates = updates.into_inner().unwrap();
        assert_eq!(updates.last().unwrap().finish.as_ref().unwrap().score, Some(0.0));
        assert_legacy_logs(expected.into_inner(), &updates);
    }
}

#[test]
fn pair_observed_legacy_matches_top_every_and_thresholds() {
    for detail_mode in [PairDetailMode::None, PairDetailMode::Top, PairDetailMode::Every] {
        for detail_min in [None, Some(0.0), Some(101.0)] {
            for min_screen in [None, Some(0.0), Some(201.0)] {
                let mut input = pair(4);
                input.detail_mode = detail_mode;
                input.detail_min = detail_min;
                input.options.min_screen = min_screen;
                input.highlight_delta = Some(0.0);
                let expected = RefCell::new(Vec::new());
                run_pair(input.clone(), |event| {
                    if let Some(log) = legacy_event(event) {
                        expected.borrow_mut().push(log);
                    }
                });
                let updates = Mutex::new(Vec::new());
                run_pair_observed(
                    input,
                    |event| {
                        assert!(legacy_event(event).is_none(), "observed 不应重复发布结果日志");
                    },
                    Some(&|update| updates.lock().unwrap().push(update)),
                );
                let updates = updates.into_inner().unwrap();
                assert_eq!(updates.iter().filter(|update| update.finish.is_some()).count(), 2);
                assert_legacy_logs(expected.into_inner(), &updates);
            }
        }
    }
}

#[test]
fn scores_observed_legacy_matches_metrics_thresholds_and_skill_board() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("openbox-legacy-skill-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let config = root.join("score.toml");
    for skill_threshold in [0, u64::MAX] {
        // 显式配置避免依赖工作目录；覆盖有榜单及被阈值全部过滤的场景。
        let mut text = String::new();
        for id in 0..35 {
            text.push_str(&format!(
                "[{}]\npp={skill_threshold}\npd={skill_threshold}\nqp={skill_threshold}\nqd={skill_threshold}\n",
                tswn_core::namerena::skill_name_for_export(id)
            ));
        }
        std::fs::write(&config, text).unwrap();
        for skill_screen in [false, true] {
            for min_screen in [None, Some(0.0), Some(f64::MAX)] {
                let input = NamerPfInput {
                    raw: "alpha@red\nalpha@red".into(),
                    count: 1,
                    threads: Some(4),
                    keep_rq: true,
                    precision: 3,
                    metrics: NamerPfMetric::ALL
                        .into_iter()
                        .map(|metric| NamerPfMetricOptions {
                            metric,
                            screen: true,
                            min_screen,
                            highlight_delta: Some(0.0),
                            output_file: None,
                            min_file: None,
                        })
                        .collect(),
                    skill_board: NamerPfSkillBoardOptions {
                        screen: skill_screen,
                        output_file: None,
                        config: Some(config.clone()),
                    },
                    cancel: Arc::new(AtomicBool::new(false)),
                };
                let expected = RefCell::new(Vec::new());
                run_namer_pf(input.clone(), |event| {
                    if let Some(log) = legacy_event(event) {
                        expected.borrow_mut().push(log);
                    }
                });
                let updates = Mutex::new(Vec::new());
                run_namer_pf_observed(
                    input,
                    |event| {
                        assert!(legacy_event(event).is_none(), "observed 不应重复发布结果日志");
                    },
                    Some(&|update| updates.lock().unwrap().push(update)),
                );
                let updates = updates.into_inner().unwrap();
                // 技能榜允许同一结果块包含多行；展开后逐行检查旧格式与样式。
                let expanded = updates
                    .iter()
                    .flat_map(|update| {
                        update.legacy_log.as_ref().into_iter().flat_map(move |(text, kind)| {
                            text.lines().map(move |line| {
                                let mut line_update =
                                    ResultUpdate::new_with_legacy(update.group, &update.label, update.kind, update.precision);
                                line_update.legacy_log = Some((line.to_owned(), *kind));
                                line_update
                            })
                        })
                    })
                    .collect::<Vec<_>>();
                assert!(updates.iter().all(|update| update.legacy_log_authoritative));
                assert_eq!(updates.iter().filter(|update| update.finish.is_some()).count(), 2);
                if skill_threshold == 0 && skill_screen {
                    assert!(
                        expanded
                            .iter()
                            .any(|update| update.legacy_log.as_ref().unwrap().1 == EntryKind::SkillBoard)
                    );
                }
                assert_legacy_logs(expected.into_inner(), &expanded);
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
