use super::*;

#[test]
fn authoritative_text_keeps_diy_blocks_and_skill_board_marks() {
    let mut view = ResultsView::default();
    let mut log = LogBuffer::default();
    for group in 0..2 {
        let mut update = ResultUpdate::new_with_legacy(group, "名字", ResultKind::Diy, 0);
        update.legacy_log = Some((format!("导出{group}\n"), EntryKind::Plain));
        update.entries.push(ResultEntry::number(0, "不能混入日志".into(), 1.0));
        update.finish = Some(ResultFinish {
            score: None,
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    assert_eq!(log.copy_text(), "导出0\n\n=========\n\n导出1\n");
    let mut update = ResultUpdate::new_with_legacy(2, "名字", ResultKind::Scores, 0);
    update.legacy_log = Some(("技能甲 1 名字\n技能乙 2 名字".into(), EntryKind::SkillBoard));
    view.apply(update, &mut log);
    assert_eq!(log.skill_board_line_count(), 2);
    assert_eq!(log.skill_board_line(1).unwrap().display_text(), "技能乙 2 名字");
    assert!(!log.copy_text().contains("预览"));
    assert!(!log.copy_text().contains("完成"));
}

/// 纯文本恢复旧格式后只多了组分隔行；去掉分隔行并压缩空行应与旧 API 逐字节一致。
fn strip_group_separators(text: &str) -> String {
    let mut out = String::new();
    let mut blank_run = 0;
    for line in text.lines().filter(|line| *line != crate::app::log::DIY_GROUP_SEPARATOR) {
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn real_diy_output_is_identical_to_legacy_in_every_view_mode() {
    use std::sync::{Mutex, atomic::AtomicBool};
    use tswn_openbox::backend::{run_to_diy, run_to_diy_observed};

    for details in [false, true] {
        let cancel = AtomicBool::new(false);
        let expected = run_to_diy("1@team+2@team\ntest", false, false, details, None, &cancel).unwrap();
        for mode in [ViewMode::Text, ViewMode::Cards, ViewMode::Table] {
            let state = Mutex::new((
                ResultsView {
                    mode,
                    ..Default::default()
                },
                LogBuffer::default(),
            ));
            run_to_diy_observed(
                "1@team+2@team\ntest",
                false,
                false,
                details,
                None,
                &cancel,
                Some(&|update| {
                    let mut state = state.lock().unwrap();
                    let (view, log) = &mut *state;
                    view.apply(update, log);
                }),
            )
            .unwrap();
            let (_, log) = state.into_inner().unwrap();
            let copied = log.copy_text();
            assert_eq!(strip_group_separators(&copied), expected);
            assert_eq!(
                copied.matches(crate::app::log::DIY_GROUP_SEPARATOR).count(),
                1,
                "两组输入之间应有分隔行，且第一组之前不加"
            );
        }
    }
}

#[test]
fn compact_diy_keeps_card_heading_visible_and_table_has_no_score_column() {
    use std::sync::{Mutex, atomic::AtomicBool};
    let updates = Mutex::new(Vec::new());
    tswn_openbox::backend::run_to_diy_observed(
        "1@team+2@team\nmario\nluigi",
        false,
        false,
        true,
        None,
        &AtomicBool::new(false),
        Some(&|update| updates.lock().unwrap().push(update)),
    )
    .unwrap();
    for mode in [ViewMode::Cards, ViewMode::Table] {
        let mut view = ResultsView {
            mode,
            ..Default::default()
        };
        let mut log = LogBuffer::default();
        for update in updates.lock().unwrap().iter().cloned() {
            view.apply(update, &mut log);
        }
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(690.0, 570.0))),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let text_shapes = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>();
        if mode == ViewMode::Cards {
            let heading = text_shapes
                .iter()
                .find(|text| text.galley.text().starts_with("▼ #1 1@team+2@team"))
                .unwrap();
            assert!(heading.pos.y >= 0.0 && heading.pos.y < 100.0, "默认跟随不应挤掉组合标题");
        } else {
            assert!(!text_shapes.iter().any(|text| text.galley.text() == "分数"));
            let member = text_shapes.iter().find(|text| text.galley.text() == "1@team").unwrap();
            assert!(member.pos.y < 200.0, "少量结果的详情应紧接表格，不留半屏空白");
        }
    }
}

#[test]
fn clearing_results_preserves_layout_and_view_preferences() {
    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        card_align: ColumnAlign::Center,
        ..Default::default()
    };
    view.column_widths[0] = 330.0;
    view.column_alignments[0] = ColumnAlign::Right;
    view.clear();
    assert_eq!(view.mode, ViewMode::Table);
    assert!(!view.follow);
    assert_eq!(view.card_align, ColumnAlign::Center);
    assert_eq!(view.column_widths[0], 330.0);
    assert_eq!(view.column_alignments[0], ColumnAlign::Right);
}

#[test]
fn horizontal_table_scroll_keeps_header_and_data_columns_together() {
    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    view.column_widths[0] = 320.0;
    view.column_alignments = [ColumnAlign::Left; 7];
    let mut log = LogBuffer::default();
    let mut update = ResultUpdate::new_with_legacy(0, "scroll-marker", ResultKind::Rate, 3);
    update.finish = Some(ResultFinish {
        score: Some(80.0),
        visible: true,
        highlight: false,
    });
    view.apply(update, &mut log);
    let ctx = egui::Context::default();
    let mut positions = Vec::new();
    for (time, events) in [
        (0.0, vec![egui::Event::PointerMoved(egui::pos2(100.0, 50.0))]),
        (
            0.1,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(-100.0, 0.0),
                modifiers: egui::Modifiers::NONE,
            }],
        ),
        (0.2, Vec::new()),
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 400.0))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let x = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == label => Some(text.pos.x),
                    _ => None,
                })
                .unwrap()
        };
        positions.push((x("名字 / 输入序号"), x("#1 scroll-marker")));
    }
    let (first_header, first_data) = positions[0];
    let (last_header, last_data) = positions[2];
    assert!(last_header < first_header, "表格应该发生横向滚动");
    assert!((first_header - first_data).abs() < 1.0);
    assert!((last_header - last_data).abs() < 1.0);
}

#[test]
fn large_result_lists_only_layout_visible_rows_in_both_render_modes() {
    let mut view = ResultsView::default();
    let mut log = LogBuffer::default();
    for group in 0..10_000 {
        let mut update = ResultUpdate::new(group, "alpha", ResultKind::Pair, 3);
        update.top = Some(2);
        update.entries.push(ResultEntry::number(2, "mate2".into(), 60.0));
        update.entries.push(ResultEntry::number(0, "mate0".into(), 60.0));
        update.entries.push(ResultEntry::number(1, "mate1".into(), 80.0));
        view.apply(update, &mut log);
    }
    view.records.get_mut(&9999).unwrap().refresh_detail_order();
    assert_eq!(view.records[&9999].detail_indexes, vec![1, 0]);
    let ctx = egui::Context::default();
    for mode in [ViewMode::Cards, ViewMode::Table] {
        view.mode = mode;
        view.dirty = true;
        view.selected = Some(9999);
        view.expanded.insert(9999);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0))),
                ..Default::default()
            },
            |ui| {
                view.ui(ui);
            },
        );
        assert!(output.shapes.len() < 1000, "应只布局可见结果，而不是遍历绘制全部记录");
        output.textures_delta.clear();
    }
    assert!(view.bytes <= MAX_RESULT_BYTES);
    let mut oversized = ResultUpdate::new(10_001, "大结果", ResultKind::Diy, 0);
    oversized.entries.push(ResultEntry {
        index: 0,
        label: "导出".into(),
        value: None,
        text: "x".repeat(MAX_RESULT_BYTES),
        kind: EntryKind::Plain,
    });
    view.apply(oversized, &mut log);
    assert!(view.bytes <= MAX_RESULT_BYTES);
    assert!(view.trimmed > 0);
}

#[test]
fn duplicate_names_and_out_of_order_details_stay_separate() {
    let mut view = ResultsView::default();
    let mut log = LogBuffer::default();
    for (group, target, value) in [(1, 2, 70.0), (0, 1, 40.0), (1, 0, 60.0)] {
        let mut update = ResultUpdate::new(group, "重名", ResultKind::Rate, 2);
        update.entries.push(ResultEntry::number(target, "相同靶子".into(), value));
        view.apply(update, &mut log);
    }
    assert_eq!(view.order.iter().copied().collect::<Vec<_>>(), vec![1, 0]);
    view.records.get_mut(&1).unwrap().refresh_detail_order();
    assert_eq!(view.records[&1].detail_indexes, vec![0, 2]);
    assert_eq!(view.records[&0].entries[&1].data.value, Some(40.0));
    let mut rejected = ResultUpdate::new(1, "重名", ResultKind::Rate, 2);
    rejected.finish = Some(ResultFinish {
        score: Some(65.0),
        visible: false,
        highlight: false,
    });
    view.apply(rejected, &mut log);
    assert!(!view.records.contains_key(&1));
    assert!(log.copy_text().contains("预览"));
    assert!(log.copy_text().contains("未达日志阈值"));
    view.finish("已停止");
    assert!(view.records[&0].finish.is_none());
    view.clear();
    assert!(view.records.is_empty());
}

#[test]
fn filtered_result_without_preview_does_not_create_text_output() {
    let mut view = ResultsView::default();
    let mut log = LogBuffer::default();
    let mut update = ResultUpdate::new(0, "仅写文件", ResultKind::Scores, 0);
    update.finish = Some(ResultFinish {
        score: None,
        visible: false,
        highlight: false,
    });
    view.apply(update, &mut log);
    assert!(log.is_empty());
    assert!(view.records.is_empty());
}
