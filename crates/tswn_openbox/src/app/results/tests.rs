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
            let last_row = text_shapes.iter().find(|text| text.galley.text() == "#3 luigi").unwrap();
            let export = text_shapes
                .iter()
                .find(|text| text.galley.text().starts_with("1@team+ol:"))
                .expect("详情首行应是导出行");
            assert!(
                export.pos.y > last_row.pos.y && export.pos.y - last_row.pos.y < 120.0,
                "少量结果的详情应紧接表格，不留半屏空白"
            );
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
    view.set_table_height(Some(260.0));
    view.clear();
    assert_eq!(view.mode, ViewMode::Table);
    assert!(!view.follow);
    assert_eq!(view.card_align, ColumnAlign::Center);
    assert_eq!(view.column_widths[0], 330.0);
    assert_eq!(view.column_alignments[0], ColumnAlign::Right);
    assert_eq!(view.table_height, 260.0, "清空结果不应重置表格高度");
    assert!(view.take_follow_jump(None).is_none(), "清空结果不应残留跳转请求");
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
    let frame = |view: &mut ResultsView, time: f32, events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 400.0))),
                time: Some(f64::from(time)),
                events,
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let position = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == label => Some((text.pos.x, text.pos.y)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{label} 应可见"))
        };
        // 用状态列做标记：滚动前后它都在可视区内（首列与分数列会分别被裁掉或尚未进入视口）。
        (position("状态"), position("完成"))
    };
    let mut positions = vec![frame(&mut view, 0.0, Vec::new())];
    let (header_x, header_y) = positions[0].0;
    let pointer = egui::pos2(header_x + 40.0, header_y + 3.0);
    positions.push(frame(
        &mut view,
        0.1,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(-100.0, 0.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    ));
    // 滚轮增量由 egui 分摊到后续帧，多跑几帧等滚动稳定。
    for step in 0..3u32 {
        positions.push(frame(&mut view, 0.2 + 0.1 * step as f32, Vec::new()));
    }
    let ((first_header, _), (first_data, _)) = positions[0];
    let ((last_header, _), (last_data, _)) = *positions.last().unwrap();
    assert!(last_header < first_header, "表格应该发生横向滚动");
    assert!(
        (last_header - last_data - (first_header - first_data)).abs() < 1.0,
        "表头与数据列必须保持同偏移"
    );
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

/// 勾选“跟随最新”必须立刻跳一次；取消勾选只停止跟随。
#[test]
fn follow_checkbox_requests_one_immediate_jump() {
    let mut view = ResultsView {
        follow: false,
        ..Default::default()
    };
    assert!(view.take_follow_jump(None).is_none(), "初始没有待处理的跳转");
    view.set_follow(true);
    assert!(view.follow);
    assert_eq!(
        view.take_follow_jump(Some((500.0, 120.0))),
        Some(380.0),
        "用上一次的内容高度与视口高度算出到底部的偏移"
    );
    assert!(view.take_follow_jump(Some((500.0, 120.0))).is_none(), "跳转请求只消费一次");
    view.set_follow(true);
    assert!(view.take_follow_jump(None).is_none(), "已经是跟随时不重复跳转");
    view.set_follow(false);
    view.set_follow(true);
    assert_eq!(
        view.take_follow_jump(None),
        Some(0.0),
        "还没有内容度量时先停在当前位置，由 stick_to_bottom 接管"
    );
    view.set_follow(false);
    view.set_follow(true);
    assert_eq!(
        view.take_follow_jump(Some((80.0, 120.0))),
        Some(0.0),
        "内容比视口短时不需要滚动"
    );
    view.set_follow(false);
    assert!(view.take_follow_jump(None).is_none(), "取消勾选不应触发跳转");
}

/// 只有“内容可滚动且已经在底部”才自动恢复跟随，避免内容不足时反复勾选。
#[test]
fn auto_follow_only_triggers_at_the_bottom_of_scrollable_content() {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 100.0));
    assert!(
        !scrolled_to_end(egui::vec2(200.0, 90.0), viewport, egui::vec2(0.0, 0.0)),
        "内容没溢出"
    );
    assert!(
        !scrolled_to_end(egui::vec2(200.0, 400.0), viewport, egui::vec2(0.0, 120.0)),
        "还没到底"
    );
    assert!(
        scrolled_to_end(egui::vec2(200.0, 400.0), viewport, egui::vec2(0.0, 300.0)),
        "到底应恢复跟随"
    );
}

/// 表格高度：默认按可用高度自动分配，拖动后固定，且明细区始终保留一段高度。
#[test]
fn table_height_switches_between_auto_and_dragged_value() {
    let mut view = ResultsView::default();
    assert_eq!(view.table_height, TABLE_HEIGHT_AUTO);
    let auto = view.table_area_height(400.0);
    assert!((auto - 400.0 * TABLE_AUTO_RATIO).abs() < 0.5, "自动模式按比例分配：{auto}");
    view.set_table_height(Some(300.0));
    assert_eq!(view.table_height, 300.0, "保存的是拖动后的高度");
    assert!((view.table_area_height(900.0) - 300.0).abs() < 0.5, "空间足够时按保存值");
    assert!(
        (view.table_area_height(400.0) - (400.0 - TABLE_DETAIL_MIN)).abs() < 0.5,
        "可用高度不足时压缩表格，给明细区留空间"
    );
    assert!(
        view.table_area_height(200.0) <= 200.0 - TABLE_DETAIL_MIN + 0.5,
        "表格过高时必须给明细区留出空间"
    );
    view.set_table_height(Some(f32::NAN));
    assert_eq!(view.table_height, TABLE_HEIGHT_AUTO, "非法高度回退自动");
    view.set_table_height(Some(10.0));
    assert!(view.table_height >= TABLE_HEIGHT_MIN, "过小的高度被夹到下限");
    view.set_table_height(None);
    assert_eq!(view.table_height, TABLE_HEIGHT_AUTO);
}

/// 复制内容是该卡片/该行自己的文本：标题行加明细，不含日志里的其它结果。
#[test]
fn copy_text_uses_the_record_content_instead_of_the_log() {
    let mut view = ResultsView {
        mode: ViewMode::Table,
        ..Default::default()
    };
    let mut log = LogBuffer::default();
    let mut first = ResultUpdate::new_with_legacy(0, "1@team", ResultKind::Diy, 0);
    first.legacy_log = Some(("1@team+ol:{}\n\n=== 原始信息 ===\n1@team".into(), EntryKind::Plain));
    first.finish = Some(ResultFinish {
        score: None,
        visible: true,
        highlight: false,
    });
    view.apply(first, &mut log);
    let mut second = ResultUpdate::new(1, "player1", ResultKind::Pair, 3);
    second.top = Some(1);
    second.entries.push(ResultEntry::number(0, "mate1".into(), 60.0));
    view.apply(second, &mut log);

    assert_eq!(
        view.copy_record_text(0).as_deref(),
        Some("#1 1@team   完成\n1@team+ol:{}\n\n=== 原始信息 ===\n1@team")
    );
    view.selected = Some(1);
    let text = view.copy_selected_text().expect("选中行存在");
    assert!(text.starts_with("#2 player1"), "{text}");
    assert!(text.contains("mate1   60.000"), "{text}");
    assert!(!text.contains("1@team"), "只复制选中行，不混入其它结果：{text}");
    view.selected = None;
    assert!(view.copy_selected_text().is_none(), "没有选中项时不复制");
}
