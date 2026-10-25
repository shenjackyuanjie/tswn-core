use super::*;

#[test]
fn merged_rows_highlight_only_the_changed_items() {
    let ctx = egui::Context::default();
    let mut colored = Vec::new();
    let mut untouched = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let emphasis = Palette::of(ui).emphasis;
        let changed = changed_items_job(ui, "魔 90  抗 96  智 94(+21)  八围 685.7  嘲讽 289").expect("含差额应分段");
        colored = changed
            .sections
            .iter()
            .filter(|section| section.format.color == emphasis)
            .map(|section| changed.text[section.byte_range.start.0..section.byte_range.end.0].to_owned())
            .collect();
        // 胜率、配队的整条高亮不含差额，仍由调用方整体着色。
        untouched.push(changed_items_job(ui, "299.300").is_none());
        untouched.push(changed_items_job(ui, "HP 311 攻 56 防 83").is_none());
    });
    output.textures_delta.clear();
    assert_eq!(colored, ["94(+21)"]);
    assert_eq!(untouched, [true, true]);
}

#[test]
fn skill_rows_highlight_every_changed_skill_on_that_line() {
    let ctx = egui::Context::default();
    let mut colored = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let emphasis = Palette::of(ui).emphasis;
        let changed = changed_items_job(ui, "命轮 20  分身 58  护符 98(+14)  噬魂 3(-2)").expect("含差额应分段");
        colored = changed
            .sections
            .iter()
            .filter(|section| section.format.color == emphasis)
            .map(|section| changed.text[section.byte_range.start.0..section.byte_range.end.0].to_owned())
            .collect();
    });
    output.textures_delta.clear();
    assert_eq!(colored, ["98(+14)", "3(-2)"]);
}

#[test]
fn cells_keep_fixed_geometry_and_align_text_left_center_right() {
    let ctx = egui::Context::default();
    let mut cells = Vec::new();
    let mut expected_height = 0.0;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        expected_height = row_height(ui);
        for align in ColumnAlign::ALL {
            cells.push(cell(ui, 240.0, egui::RichText::new("align-marker"), align, false, None).rect);
        }
    });
    output.textures_delta.clear();
    let text_rects = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "align-marker" => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(text_rects.len(), 3);
    for rect in &cells {
        assert_eq!(rect.size(), egui::vec2(240.0, expected_height));
    }
    assert!((text_rects[0].left() - cells[0].left() - 4.0).abs() < 1.0);
    assert!((text_rects[1].center().x - cells[1].center().x).abs() < 1.0);
    assert!((text_rects[2].right() - cells[2].right() + 4.0).abs() < 1.0);
}

/// 表格与卡片的行高、字体必须与纯文本日志一致：等宽字体、同一套文本样式。
#[test]
fn result_rows_use_the_same_text_metrics_as_the_log_view() {
    let ctx = egui::Context::default();
    let mut metrics = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let height = row_height(ui);
        let cell_rect = cell(
            ui,
            200.0,
            egui::RichText::new("metrics").monospace(),
            ColumnAlign::Left,
            false,
            None,
        )
        .rect;
        let log_height = ui.text_style_height(&egui::TextStyle::Monospace);
        let log_font = egui::TextStyle::Monospace.resolve(ui.style());
        let job_font = changed_items_job(ui, "智 94(+21)").expect("含差额").sections[0].format.font_id.clone();
        metrics.push((height, cell_rect.height(), log_height, log_font, job_font));
    });
    output.textures_delta.clear();
    let (_, cell_height, log_height, log_font, job_font) = metrics.pop().unwrap();
    assert_eq!(cell_height, log_height, "卡片与表格的行高应与日志行高一致");
    assert_eq!(job_font, log_font, "差额高亮应使用等宽字体");
}

/// 表格里高分高亮时，名字与分数必须一起标色。
#[test]
fn highlighted_table_rows_color_the_name_as_well_as_the_score() {
    use tswn_openbox::backend::live::ResultUpdate;

    let mut view = ResultsView {
        mode: ViewMode::Table,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    let mut update = ResultUpdate::new_with_legacy(0, "high-score", ResultKind::Rate, 3);
    update.finish = Some(tswn_openbox::backend::live::ResultFinish {
        score: Some(99.0),
        visible: true,
        highlight: true,
    });
    view.apply(update, &mut log);
    let ctx = egui::Context::default();
    let mut emphasis = egui::Color32::PLACEHOLDER;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0))),
            ..Default::default()
        },
        |ui| {
            emphasis = Palette::of(ui).emphasis;
            view.ui(ui);
        },
    );
    output.textures_delta.clear();
    let colors = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => {
                let label = text.galley.job.text.as_str();
                (label == "#1 high-score" || label == "99.000").then(|| {
                    (
                        label.to_owned(),
                        text.galley.job.sections.iter().any(|section| section.format.color == emphasis),
                    )
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(colors.len(), 2, "名字与分数两格都应绘制：{colors:?}");
    assert!(
        colors.iter().all(|(_, colored)| *colored),
        "高分高亮必须同时标色名字与分数：{colors:?}"
    );
}

/// 拖动表格与明细之间的分隔条即可调整表格高度。
#[test]
fn dragging_the_divider_resizes_the_table_area() {
    use tswn_openbox::backend::live::ResultUpdate;

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    let mut update = ResultUpdate::new_with_legacy(0, "resize-marker", ResultKind::Rate, 3);
    update
        .entries
        .push(tswn_openbox::backend::live::ResultEntry::number(0, "target".into(), 70.0));
    update.finish = Some(tswn_openbox::backend::live::ResultFinish {
        score: Some(70.0),
        visible: true,
        highlight: false,
    });
    view.apply(update, &mut log);
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 600.0));
    // 返回（分隔条标题的中心 y, 标题文字）。
    let divider_y = |view: &mut ResultsView, time: f64, events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                // 分隔条左侧就是当前选中行的名字与分数，位置即分隔条所在行。
                egui::epaint::Shape::Text(text) if text.galley.job.text.starts_with("resize-marker") => Some(text),
                _ => None,
            })
            .unwrap_or_else(|| panic!("分隔条应绘制选中行名字"));
        assert_eq!(text.galley.job.text, "resize-marker 70.000", "标题应带上分数");
        (text.pos.y + text.galley.rect.center().y, text.galley.job.text.clone())
    };
    let (start, _) = divider_y(&mut view, 0.0, Vec::new());
    assert_eq!(view.table_height, TABLE_HEIGHT_AUTO, "默认是自动高度");
    // 指针放在标题右侧的横线上：标题文字本身留给选择，横线负责拖动调高。
    let pointer = egui::pos2(screen.width() - 30.0, start);
    divider_y(
        &mut view,
        0.1,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let moved = pointer + egui::vec2(0.0, 80.0);
    divider_y(&mut view, 0.2, vec![egui::Event::PointerMoved(moved)]);
    let (after, _) = divider_y(&mut view, 0.3, Vec::new());
    assert!(
        view.table_height > TABLE_HEIGHT_AUTO,
        "拖动后应保存具体高度：{}",
        view.table_height
    );
    assert!(
        (TABLE_HEIGHT_MIN..=TABLE_HEIGHT_MAX).contains(&view.table_height),
        "高度必须落在允许范围内：{}",
        view.table_height
    );
    assert!(after > start + 40.0, "向下拖动后表格应变高：{start} -> {after}");
}

/// 每张展开的卡片右下角都有“复制全部”，点击复制的就是这张卡片的内容。
#[test]
fn expanded_cards_expose_a_copy_button_for_that_card() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView::default();
    let mut log = crate::app::log::LogBuffer::default();
    for group in 0..2 {
        let label = format!("card{group}");
        let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Diy, 0);
        update.legacy_log = Some((format!("card{group}+ol:{{}}\n明细{group}"), EntryKind::Plain));
        update.finish = Some(ResultFinish {
            score: None,
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 500.0));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| view.ui(ui),
    );
    output.textures_delta.clear();
    let buttons = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "复制全部" => Some(text.pos),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(buttons.len(), 1, "只有展开的卡片带“复制全部”按钮");
    // 第一张卡片默认展开，按钮应贴在该卡片的右下方。
    assert!(buttons[0].x > screen.width() * 0.5, "按钮应位于卡片右下角：{:?}", buttons[0]);

    let pointer = buttons[0] + egui::vec2(8.0, 6.0);
    let mut click = |time: f64, pressed: bool| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
    };
    click(0.1, true);
    let output = click(0.2, false);
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("点击“复制全部”应写入剪贴板"));
    assert!(copied.starts_with("#1 card0"), "复制的是该卡片自己的内容：{copied}");
    assert!(!copied.contains("card1"), "不应混入其它卡片或日志：{copied}");
}

/// 勾选“跟随最新”后必须立刻跳到最新，而不是等下一条结果。
#[test]
fn checking_follow_jumps_to_the_latest_rows_immediately() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    for group in 0..200 {
        let label = format!("name{group}");
        let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Rate, 0);
        update.finish = Some(ResultFinish {
            score: Some(group as f64),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
    let visible_rows = |view: &mut ResultsView, time: f64| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .filter(|text| text.starts_with('#'))
            .collect::<Vec<_>>()
    };
    let before = visible_rows(&mut view, 0.0);
    assert!(
        before.iter().any(|text| text.starts_with("#1 ")),
        "未跟随时从最早一条开始：{before:?}"
    );
    assert!(!before.iter().any(|text| text.starts_with("#200 ")), "未跟随时看不到最后一条");
    view.set_follow(true);
    let after = visible_rows(&mut view, 0.1);
    assert!(
        after.iter().any(|text| text.starts_with("#200 ")),
        "勾选跟随应立刻跳到最新：{after:?}"
    );
}

/// 滑到最底部时自动恢复“跟随最新”。
#[test]
fn scrolling_to_the_bottom_restores_follow() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    for group in 0..200 {
        let label = format!("name{group}");
        let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Rate, 0);
        update.finish = Some(ResultFinish {
            score: Some(group as f64),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
    let mut first_row = egui::Pos2::ZERO;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| view.ui(ui),
    );
    output.textures_delta.clear();
    for shape in &output.shapes {
        if let egui::epaint::Shape::Text(text) = &shape.shape
            && text.galley.job.text == "#1 name0"
        {
            first_row = text.pos;
        }
    }
    assert!(first_row != egui::Pos2::ZERO, "首行应可见，便于把指针放进表格");
    let pointer = first_row + egui::vec2(20.0, 4.0);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            time: Some(0.1),
            events: vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    source: egui::MouseWheelSource::Unknown,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0.0, -100_000.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ui| view.ui(ui),
    );
    output.textures_delta.clear();
    assert!(view.follow, "滑到最底部后应自动勾选跟随最新");
}

/// 表格调小后：分隔条与明细紧贴表格、明细区变高、可视明细行变多。
///
/// 回归用例：`auto_shrink = false` 会把多余空间留在滚动区内部，若横向滚动区不设高度上限，
/// 表格会吃掉整个结果区，明细被挤到可视区之外，中间还留出一大段空白。
#[test]
fn shrinking_the_table_keeps_details_attached_and_gives_them_more_room() {
    use tswn_openbox::backend::live::{ResultEntry, ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    for group in 0..2 {
        let label = format!("row{group}");
        let mut update = ResultUpdate::new_with_legacy(group, &label, ResultKind::Pair, 3);
        for index in 0..30 {
            update.entries.push(ResultEntry::number(index, format!("mate{index}"), 60.0));
        }
        update.finish = Some(ResultFinish {
            score: Some(240.0),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    view.selected = Some(0);
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 600.0));
    // 返回（分隔条 y, 首条明细 y, 可见明细行数）。
    let measure = |view: &mut ResultsView, table_height: f32, time: f64| {
        view.set_table_height(Some(table_height));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        let mut divider = None;
        let mut detail_ys = Vec::new();
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape {
                // 分隔条左侧画的就是当前选中行的名字与分数，明细行则形如 `mate0   60.000`。
                if text.galley.job.text.starts_with("row0") {
                    divider = Some(text.pos.y);
                } else if text.galley.job.text.starts_with("mate") {
                    detail_ys.push(text.pos.y);
                }
            }
        }
        let divider = divider.unwrap_or_else(|| panic!("分隔条应绘制选中行名字"));
        assert!(!detail_ys.is_empty(), "明细区应显示选中行的队友明细");
        (divider, detail_ys[0], detail_ys.len())
    };
    let (tall_divider, tall_first, tall_rows) = measure(&mut view, 460.0, 0.0);
    let (short_divider, short_first, short_rows) = measure(&mut view, 160.0, 0.1);
    assert!(
        short_divider < tall_divider,
        "表格调小后分隔条应上移：{tall_divider} -> {short_divider}"
    );
    assert!(
        (tall_first - tall_divider).abs() < 40.0 && (short_first - short_divider).abs() < 40.0,
        "明细应紧贴分隔条，中间不留空白：{tall_divider}/{tall_first} 与 {short_divider}/{short_first}"
    );
    assert!(
        (tall_divider - short_divider) > 200.0,
        "分隔条位移应跟随表格高度：{tall_divider} -> {short_divider}"
    );
    assert!(
        short_rows > tall_rows,
        "表格调小后明细区应变高、显示更多行：{tall_rows} -> {short_rows}"
    );
}

#[test]
fn header_separator_drag_changes_width_within_bounds() {
    let ctx = egui::Context::default();
    let mut width = 220.0;
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(220.0, 20.0));
    for (time, events) in [
        (0.0, Vec::new()),
        (
            0.1,
            vec![
                egui::Event::PointerMoved(egui::pos2(239.0, 30.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(239.0, 30.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ),
        (0.2, vec![egui::Event::PointerMoved(egui::pos2(280.0, 30.0))]),
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| resize_column(ui, 0, rect, &mut width),
        );
        output.textures_delta.clear();
    }
    assert!(width > 220.0);
    assert!((60.0..=640.0).contains(&width));
}

/// 完成的组不再显示“完成”：表格那一列换成「复制」按钮，点了把名字写进剪贴板。
#[test]
fn finished_table_rows_replace_the_state_text_with_a_copy_name_button() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    for (group, label) in [(0usize, "alpha@team"), (1, "beta@team")] {
        let mut update = ResultUpdate::new_with_legacy(group, label, ResultKind::Rate, 3);
        update.finish = Some(ResultFinish {
            score: Some(70.0 + group as f64),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
    let drawn_texts = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some((text.galley.job.text.clone(), text.pos)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| view.ui(ui),
    );
    output.textures_delta.clear();
    let drawn = drawn_texts(&output);
    assert!(
        !drawn.iter().any(|(text, _)| text == "完成"),
        "完成的组不再显示“完成”：{drawn:?}"
    );
    // 表头这一列叫“复制名字”，行内按钮用短标签“复制”。
    assert!(drawn.iter().any(|(text, _)| text == "复制名字"));
    let copies = drawn.iter().filter(|(text, _)| text == "复制").map(|(_, pos)| *pos).collect::<Vec<_>>();
    assert_eq!(copies.len(), 2, "每个完成的行都有一个复制按钮：{drawn:?}");

    let pointer = copies[0] + egui::vec2(8.0, 4.0);
    let mut click = |time: f64, pressed: bool| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
    };
    click(0.1, true);
    let output = click(0.2, false);
    let copied = output.platform_output.commands.iter().find_map(|command| match command {
        egui::OutputCommand::CopyText(text) => Some(text.clone()),
        _ => None,
    });
    assert_eq!(copied.as_deref(), Some("alpha@team"), "复制按钮写进剪贴板的是名字本身");
}

/// 卡片同样不再把“完成”写进标题：完成时右侧换成「复制名字」按钮。
#[test]
fn finished_cards_show_a_copy_name_button_instead_of_the_state_text() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView::default();
    let mut log = crate::app::log::LogBuffer::default();
    let mut update = ResultUpdate::new_with_legacy(0, "card0", ResultKind::Rate, 3);
    update.finish = Some(ResultFinish {
        score: Some(80.0),
        visible: true,
        highlight: false,
    });
    view.apply(update, &mut log);
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
    let drawn_texts = |output: &egui::FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some((text.galley.job.text.clone(), text.pos)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| view.ui(ui),
    );
    output.textures_delta.clear();
    let drawn = drawn_texts(&output);
    assert!(
        drawn.iter().any(|(text, _)| text == "▼ #1 card0   80.000"),
        "完成状态不再写进标题，标题只有序号、名字与分数：{drawn:?}"
    );
    assert!(!drawn.iter().any(|(text, _)| text == "完成"), "卡片不再显示“完成”：{drawn:?}");
    let Some((_, position)) = drawn.iter().find(|(text, _)| text == "复制名字") else {
        panic!("完成的卡片应提供「复制名字」按钮：{drawn:?}");
    };
    let pointer = *position + egui::vec2(8.0, 4.0);
    let mut click = |time: f64, pressed: bool| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
    };
    click(0.1, true);
    let output = click(0.2, false);
    let copied = output.platform_output.commands.iter().find_map(|command| match command {
        egui::OutputCommand::CopyText(text) => Some(text.clone()),
        _ => None,
    });
    assert_eq!(copied.as_deref(), Some("card0"), "复制的是名字，不含明细");
}

/// 还在算或已停止的组没有“完成”可言：那些行仍然显示状态文字，不换成复制按钮。
#[test]
fn unfinished_rows_keep_the_state_text_instead_of_the_copy_button() {
    use tswn_openbox::backend::live::ResultUpdate;

    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
    let drawn_texts = |view: &mut ResultsView| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // 表格第二列与卡片标题右侧都要保留状态文字，而不是提前露出复制按钮。
    for mode in [ViewMode::Cards, ViewMode::Table] {
        let mut view = ResultsView {
            mode,
            follow: false,
            ..Default::default()
        };
        let mut log = crate::app::log::LogBuffer::default();
        let mut update = ResultUpdate::new_with_legacy(0, "pending", ResultKind::Rate, 3);
        update.finish = None;
        view.apply(update, &mut log);
        let running = drawn_texts(&mut view);
        assert!(
            running.iter().any(|text| text == "计算中 · 预览"),
            "{mode:?} 里计算中的组仍显示状态：{running:?}"
        );
        // 表格里的“复制名字”是列名，行内按钮叫“复制”；卡片右侧的按钮叫“复制名字”。
        let row_buttons: &[&str] = if mode == ViewMode::Table {
            &["复制"]
        } else {
            &["复制", "复制名字"]
        };
        for button in row_buttons {
            assert!(
                !running.iter().any(|text| text == button),
                "{mode:?} 未完成时没有复制名字按钮：{running:?}"
            );
        }
        view.finish("已停止");
        let stopped = drawn_texts(&mut view);
        assert!(
            stopped.iter().any(|text| text == "已停止 · 结果不完整"),
            "{mode:?} 停止后保留“结果不完整”标记：{stopped:?}"
        );
    }
}

/// 在名字格子上拖动可以选中文字，选中后 Ctrl+C 会把选中的部分写进剪贴板。
#[test]
fn dragging_over_a_name_selects_text_that_can_be_copied() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    let mut update = ResultUpdate::new_with_legacy(0, "alpha", ResultKind::Rate, 3);
    update.finish = Some(ResultFinish {
        score: Some(70.0),
        visible: true,
        highlight: false,
    });
    view.apply(update, &mut log);
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
    let mut run = |time: f64, events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
    };
    let output = run(0.0, Vec::new());
    let text_rect = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "#1 alpha" => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            _ => None,
        })
        .expect("名字应可见");
    let y = text_rect.center().y;
    let start = egui::pos2(text_rect.left() + 1.0, y);
    let end = egui::pos2(text_rect.right() - 1.0, y);
    let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    run(0.1, vec![egui::Event::PointerMoved(start), button(start, true)]);
    run(0.2, vec![egui::Event::PointerMoved(end)]);
    run(0.3, vec![egui::Event::PointerMoved(end), button(end, false)]);
    let output = run(0.4, vec![egui::Event::Copy]);
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("复制快捷键应复制选中的文字");
    assert!(copied.contains("alpha"), "选中的应是名字格子里的文字，实际复制：{copied:?}");
}

/// 名字格子的文字可以选中复制，点击它依然能选中该行（下方明细据此刷新）。
#[test]
fn clicking_a_name_still_selects_that_row() {
    use tswn_openbox::backend::live::{ResultFinish, ResultUpdate};

    let mut view = ResultsView {
        mode: ViewMode::Table,
        follow: false,
        ..Default::default()
    };
    let mut log = crate::app::log::LogBuffer::default();
    for (group, label) in [(0usize, "alpha"), (1, "beta")] {
        let mut update = ResultUpdate::new_with_legacy(group, label, ResultKind::Rate, 3);
        update.finish = Some(ResultFinish {
            score: Some(70.0),
            visible: true,
            highlight: false,
        });
        view.apply(update, &mut log);
    }
    assert_eq!(view.selected, Some(0), "首条默认选中");
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 400.0));
    let mut run = |time: f64, events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| view.ui(ui),
        );
        output.textures_delta.clear();
        output
    };
    let output = run(0.0, Vec::new());
    let target = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "#2 beta" => Some(text.pos),
            _ => None,
        })
        .expect("第二行名字应可见");
    let pointer = target + egui::vec2(20.0, 6.0);
    run(
        0.1,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    run(
        0.2,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(view.selected, Some(1), "点击可选中复制的名字仍然能选中该行");
}
