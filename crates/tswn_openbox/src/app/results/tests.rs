
use super::*;

#[test]
fn updates_to_collapsed_records_do_not_rebuild_rows_and_top_is_stable() {
    let mut view = ResultsView::default();
    let mut log = LogBuffer::default();
    for group in 0..2 {
        view.apply(ResultUpdate::new(group, "名字", ResultKind::Pair, 2), &mut log);
    }
    view.dirty = false;
    let mut update = ResultUpdate::new(1, "名字", ResultKind::Pair, 2);
    update.top = Some(2);
    update.entries = vec![
        ResultEntry::number(2, "丙".into(), 80.0),
        ResultEntry::number(1, "乙".into(), 80.0),
        ResultEntry::number(0, "甲".into(), 90.0),
    ];
    view.records.get_mut(&1).unwrap().top = Some(2);
    view.apply(update, &mut log);
    assert!(!view.dirty, "折叠卡片更新数值无需重建整张行索引");
    let record = view.records.get_mut(&1).unwrap();
    record.refresh_detail_order();
    assert_eq!(record.detail_indexes, vec![0, 1]);
    let allocation = record.detail_indexes.as_ptr();
    record.refresh_detail_order();
    assert_eq!(allocation, record.detail_indexes.as_ptr(), "无变化时应复用排序结果");
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
