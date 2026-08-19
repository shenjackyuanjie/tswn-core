//! egui rendering for every Openbox tool.

use std::cell::Cell;

use eframe::egui;

use tswn_openbox::backend::PairDetailMode;

use super::help::{HelpTopic, help_icon};
use super::state::{AccuracyPreset, CountMode, OpenboxApp, Tool};
use super::widgets::{
    bench_output_controls, count_mode_controls, optional_file_output_controls, pick_named_output_file, thread_controls,
};

const GROUP_MARGIN: i8 = 8;
const SECTION_MARGIN_X: i8 = 8;
const SECTION_MARGIN_Y: i8 = 6;
const SECTION_GAP: f32 = 5.0;
const LOG_SECTION_GAP: f32 = 6.0;
const NAMER_PF_METRIC_LABEL_WIDTH: f32 = 48.0;
const NAMER_PF_TOGGLE_WIDTH: f32 = 58.0;
const NAMER_PF_VALUE_WIDTH: f32 = 132.0;
const NAMER_PF_HIGHLIGHT_WIDTH: f32 = 116.0;
const NAMER_PF_ACTION_WIDTH: f32 = 112.0;

impl OpenboxApp {
    pub(crate) fn show_diy_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "to-diy", "名字转 DIY / 召唤物 DIY", &mut self.more_settings_open);
        section(ui, "基础选项", |ui| {
            to_diy_basic_controls(ui, self);
        });
        section(ui, "名字", |ui| {
            self.to_diy.names.ui(ui, "名字", "to_diy_names", 16);
        });
        section(ui, "输出", |ui| {
            optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
        });
    }

    pub(crate) fn namer_pf_ui(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        tool_header(ui, "namer-pf", "批量评分并筛选名字", &mut self.more_settings_open);
        section(ui, "精确度", |ui| {
            main_accuracy_controls(
                ui,
                &mut self.namer_pf.count_mode,
                &mut self.namer_pf.accuracy,
                HelpTopic::NamerAccuracy,
                &requested_help,
            );
        });
        section_with_help(ui, "评分项", HelpTopic::NamerMetrics, &requested_help, |ui| {
            namer_pf_metric_controls_clean(ui, self, false);
        });
        section_with_help(ui, "名字", HelpTopic::NamerNames, &requested_help, |ui| {
            self.namer_pf.names.ui(ui, "名字", "namer_pf_names", 14);
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    pub(crate) fn batch_rate_ui(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        tool_header(ui, "cqd/cqp", "计算选手对靶子的平均胜率", &mut self.more_settings_open);
        section(ui, "常用设置", |ui| {
            main_accuracy_controls(
                ui,
                &mut self.batch_rate.count_mode,
                &mut self.batch_rate.accuracy,
                HelpTopic::BatchAccuracy,
                &requested_help,
            );
            target_preset_controls(ui, &mut self.batch_rate.target_presets);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.batch_rate.show_matchups, "每组胜率");
                help_icon(ui, HelpTopic::BatchMatchups, &requested_help);
            });
        });
        section(ui, "输出", |ui| {
            bench_output_controls(
                ui,
                &mut self.batch_rate.output,
                "tswn-openbox-cqd-cqp.txt",
                false,
                false,
                &requested_help,
            );
        });
        section_with_help(ui, "选手列表", HelpTopic::BatchPlayers, &requested_help, |ui| {
            self.batch_rate.players.ui(ui, "选手", "batch_players", 8);
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    pub(crate) fn pair_ui(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        tool_header(ui, "pair", "计算选手与队友组合表现", &mut self.more_settings_open);
        section(ui, "常用设置", |ui| {
            main_accuracy_controls(
                ui,
                &mut self.pair.count_mode,
                &mut self.pair.accuracy,
                HelpTopic::PairAccuracy,
                &requested_help,
            );
            teammate_preset_controls(ui, &mut self.pair.teammate_presets, &requested_help, true);
            pair_detail_controls(ui, self, &requested_help);
        });
        section_with_help(ui, "输出", HelpTopic::PairScore, &requested_help, |ui| {
            bench_output_controls(
                ui,
                &mut self.pair.output,
                "tswn-openbox-pair.txt",
                false,
                false,
                &requested_help,
            );
        });
        section(ui, "选手列表", |ui| {
            self.pair.players.ui(ui, "选手", "pair_players", 6);
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    pub(crate) fn more_settings_window(&mut self, ctx: &egui::Context) {
        if !self.more_settings_open {
            return;
        }

        let mut open = self.more_settings_open;
        egui::Window::new(format!("更多设置 - {}", self.tool.label()))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(640.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Frame::group(ui.style())
                        .inner_margin(egui::Margin::same(GROUP_MARGIN))
                        .show(ui, |ui| {
                            ui.add_enabled_ui(!self.running, |ui| match self.tool {
                                Tool::ToDiy => self.show_diy_more_settings(ui),
                                Tool::NamerPf => self.namer_pf_more_settings(ui),
                                Tool::BatchRate => self.batch_rate_more_settings(ui),
                                Tool::Pair => self.pair_more_settings(ui),
                            });
                        });
                });
            });
        self.more_settings_open = open;
    }

    fn show_diy_more_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "基础选项", |ui| {
            to_diy_basic_controls(ui, self);
            ui.checkbox(&mut self.to_diy.details, "单名详情（仅日志输出）");
        });
        section(ui, "输出", |ui| {
            optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
        });
    }

    fn namer_pf_more_settings(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        section(ui, "计算设置", |ui| {
            count_mode_controls(
                ui,
                &mut self.namer_pf.count_mode,
                &mut self.namer_pf.accuracy,
                &mut self.namer_pf.count,
                &requested_help,
            );
            thread_controls(ui, &mut self.namer_pf.auto_threads, &mut self.namer_pf.threads, &requested_help);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.namer_pf.keep_rq, "不低估短号");
                help_icon(ui, HelpTopic::KeepRq, &requested_help);
            });
            ui.horizontal(|ui| {
                ui.label("保留小数点后 X 位");
                ui.add(egui::DragValue::new(&mut self.namer_pf.precision).range(0..=9).speed(1));
                help_icon(ui, HelpTopic::ScorePrecision, &requested_help);
            });
        });
        section_with_help(ui, "评分项", HelpTopic::NamerMetrics, &requested_help, |ui| {
            namer_pf_metric_controls_clean(ui, self, true);
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    fn batch_rate_more_settings(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        section(ui, "计算设置", |ui| {
            count_mode_controls(
                ui,
                &mut self.batch_rate.count_mode,
                &mut self.batch_rate.accuracy,
                &mut self.batch_rate.count,
                &requested_help,
            );
            thread_controls(
                ui,
                &mut self.batch_rate.auto_threads,
                &mut self.batch_rate.threads,
                &requested_help,
            );
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.batch_rate.keep_rq, "不低估短号");
                ui.checkbox(&mut self.batch_rate.double_plus, "DIYcqp（++分割名字）");
                help_icon(ui, HelpTopic::KeepRq, &requested_help);
                help_icon(ui, HelpTopic::BatchPlayers, &requested_help);
            });
        });
        section_with_help(ui, "靶子", HelpTopic::ManualTargets, &requested_help, |ui| {
            ui.checkbox(&mut self.batch_rate.manual_targets, "使用手动靶子");
            if self.batch_rate.manual_targets {
                ui.checkbox(&mut self.batch_rate.manual_target_double_plus, "DIY靶子（++分割名字）");
                self.batch_rate.targets.ui(ui, "靶子", "batch_targets_more", 8);
            } else {
                target_preset_controls(ui, &mut self.batch_rate.target_presets);
            }
        });
        section(ui, "输出", |ui| {
            highlight_delta_control(ui, &mut self.batch_rate.highlight_delta, &requested_help);
            bench_output_controls(
                ui,
                &mut self.batch_rate.output,
                "tswn-openbox-cqd-cqp.txt",
                true,
                true,
                &requested_help,
            );
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    fn pair_more_settings(&mut self, ui: &mut egui::Ui) {
        let requested_help = Cell::new(None);
        section(ui, "计算设置", |ui| {
            count_mode_controls(
                ui,
                &mut self.pair.count_mode,
                &mut self.pair.accuracy,
                &mut self.pair.count,
                &requested_help,
            );
            thread_controls(ui, &mut self.pair.auto_threads, &mut self.pair.threads, &requested_help);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.pair.keep_rq, "不低估短号");
                help_icon(ui, HelpTopic::KeepRq, &requested_help);
            });
            ui.checkbox(&mut self.pair.player_double_plus, "DIY选手（++分割名字）");
            ui.checkbox(&mut self.pair.teammate_double_plus, "DIY队友（++分割名字）");
            pair_detail_controls(ui, self, &requested_help);
        });

        section_with_help(ui, "靶子", HelpTopic::ManualTargets, &requested_help, |ui| {
            ui.checkbox(&mut self.pair.manual_targets, "使用手动靶子");
            if self.pair.manual_targets {
                self.pair.targets.ui(ui, "靶子", "pair_targets_more", 6);
            } else {
                target_preset_controls(ui, &mut self.pair.target_presets);
            }
        });

        section_with_help(ui, "队友", HelpTopic::PairTeammates, &requested_help, |ui| {
            ui.checkbox(&mut self.pair.manual_teammates, "使用手动队友");
            if self.pair.manual_teammates {
                ui.horizontal(|ui| {
                    ui.label("保留前几");
                    ui.add(egui::DragValue::new(&mut self.pair.head).range(1..=999).speed(1));
                });
                self.pair.teammates.ui(ui, "队友", "pair_teammates_more", 6);
            } else {
                teammate_preset_controls(ui, &mut self.pair.teammate_presets, &requested_help, false);
            }
        });

        section_with_help(ui, "输出", HelpTopic::PairScore, &requested_help, |ui| {
            highlight_delta_control(ui, &mut self.pair.highlight_delta, &requested_help);
            bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt", true, true, &requested_help);
        });
        if requested_help.get().is_some() {
            self.active_help = requested_help.get();
        }
    }

    pub(crate) fn log_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(GROUP_MARGIN))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&self.status);
                    if self.total > 0 {
                        let progress = self.done as f32 / self.total.max(1) as f32;
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .show_percentage()
                                .desired_width(320.0)
                                .desired_height(24.0),
                        );
                        ui.heading(format!("{}/{}", self.done, self.total));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("复制日志").clicked() {
                            ctx.copy_text(self.log.clone());
                        }
                        if ui.button("清空日志").clicked() {
                            self.log.clear();
                            self.log_line_count = 0;
                            self.highlight_lines.clear();
                            self.skill_board_lines.clear();
                        }
                    });
                });
                if self.total > 0 {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("速度: {}", self.rate_text)).size(16.0));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("剩余: {}", self.eta_text)).size(16.0));
                    });
                } else {
                    ui.label(egui::RichText::new("运行结果会显示在这里").weak());
                }
            });
        if !self.skill_board_lines.is_empty() {
            ui.add_space(LOG_SECTION_GAP);
            let skill_board_log = selected_log_lines(&self.log, &self.skill_board_lines);
            let line_count = skill_board_log.lines().count();
            if line_count > 0 {
                egui::CollapsingHeader::new(format!("技能榜 ({line_count})"))
                    .default_open(false)
                    .show(ui, |ui| {
                        let text_height = compact_log_text_height(line_count);
                        readonly_log_view(ui, "skill_board_log", &skill_board_log, text_height);
                    });
            }
        }
        ui.add_space(LOG_SECTION_GAP);
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(GROUP_MARGIN))
            .show(ui, |ui| {
                if self.log.trim().is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(48.0);
                        ui.label(egui::RichText::new("暂无日志").weak().size(18.0));
                        ui.label(egui::RichText::new("选择工具、填好输入，然后点击运行。旧日志会在新任务开始时清空。").weak());
                    });
                } else {
                    let text_height = ui.available_height().max(220.0);
                    egui::ScrollArea::both()
                        .id_salt("main_log")
                        .auto_shrink([false, false])
                        .max_height(text_height)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for (index, line) in self.log.lines().enumerate() {
                                    let mut text = egui::RichText::new(line).monospace();
                                    if self.skill_board_lines.contains(&index) {
                                        text = text.color(egui::Color32::from_rgb(45, 120, 220)).strong();
                                    } else if line.starts_with("  ") {
                                        text = text.color(egui::Color32::GRAY);
                                    } else if self.highlight_lines.contains(&index) {
                                        text = text.color(egui::Color32::from_rgb(210, 40, 40)).strong();
                                    }
                                    ui.add(egui::Label::new(text).extend());
                                }
                            });
                        });
                }
            });
    }
}

fn readonly_log_view(ui: &mut egui::Ui, id: &'static str, text: &str, viewport_height: f32) {
    egui::ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
        .max_height(viewport_height)
        .show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(text).monospace()).selectable(true));
        });
}

fn compact_log_text_height(line_count: usize) -> f32 { (line_count.clamp(4, 20) as f32 * 17.0 + 12.0).min(360.0) }

fn selected_log_lines(log: &str, line_indexes: &std::collections::HashSet<usize>) -> String {
    let mut selected = log
        .lines()
        .enumerate()
        .filter_map(|(index, line)| line_indexes.contains(&index).then_some(line))
        .collect::<Vec<_>>()
        .join("\n");
    if !selected.is_empty() {
        selected.push('\n');
    }
    selected
}

fn tool_header(ui: &mut egui::Ui, title: &str, subtitle: &str, more_settings_open: &mut bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(title);
            ui.label(egui::RichText::new(subtitle).weak());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("更多设置").clicked() {
                *more_settings_open = true;
            }
        });
    });
    ui.add_space(4.0);
}

fn section<R>(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(SECTION_MARGIN_X, SECTION_MARGIN_Y))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().size(15.0));
            ui.separator();
            add_contents(ui)
        })
        .inner;
    ui.add_space(SECTION_GAP);
    inner
}

fn section_with_help<R>(
    ui: &mut egui::Ui,
    title: &str,
    topic: HelpTopic,
    requested_help: &Cell<Option<HelpTopic>>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner = egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(SECTION_MARGIN_X, SECTION_MARGIN_Y))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).strong().size(15.0));
                help_icon(ui, topic, requested_help);
            });
            ui.separator();
            add_contents(ui)
        })
        .inner;
    ui.add_space(SECTION_GAP);
    inner
}

fn to_diy_basic_controls(ui: &mut egui::Ui, app: &mut OpenboxApp) {
    ui.horizontal(|ui| {
        if ui.checkbox(&mut app.to_diy.old, "旧 +diy").changed() && app.to_diy.old {
            app.to_diy.minions = false;
        }
        if ui.checkbox(&mut app.to_diy.minions, "召唤物diy").changed() && app.to_diy.minions {
            app.to_diy.old = false;
        }
    });
}

fn highlight_delta_control(ui: &mut egui::Ui, value: &mut String, requested_help: &Cell<Option<HelpTopic>>) {
    ui.horizontal(|ui| {
        ui.label("高亮超强名字");
        ui.add(egui::TextEdit::singleline(value).desired_width(72.0));
        help_icon(ui, HelpTopic::Highlight, requested_help);
    });
}

fn target_preset_controls(ui: &mut egui::Ui, state: &mut super::target_presets::TargetPresetState) {
    ui.horizontal(|ui| {
        ui.label("靶子");
        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(state.selected().map(|item| item.name.clone()).unwrap_or_else(|| "未配置靶子".to_string()))
            .show_ui(ui, |ui| {
                for item in &state.items {
                    ui.selectable_value(&mut state.selected_id, Some(item.id), &item.name);
                }
            });
        if ui.button("刷新").clicked() {
            state.reload();
        }
    });
    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
    }
}

fn teammate_preset_controls(
    ui: &mut egui::Ui,
    state: &mut super::target_presets::TeammatePresetState,
    requested_help: &Cell<Option<HelpTopic>>,
    show_help: bool,
) {
    ui.horizontal(|ui| {
        ui.label("队友");
        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(
                state
                    .selected()
                    .map(|item| format!("{}（保留前{}）", item.name, item.head))
                    .unwrap_or_else(|| "未配置队友".to_string()),
            )
            .show_ui(ui, |ui| {
                for (index, item) in state.items.iter().enumerate() {
                    ui.selectable_value(
                        &mut state.selected_index,
                        Some(index),
                        format!("{}（保留前{}）", item.name, item.head),
                    );
                }
            });
        if ui.button("刷新").clicked() {
            state.reload();
        }
        if show_help {
            help_icon(ui, HelpTopic::PairTeammates, requested_help);
        }
    });
    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
    }
}

fn pair_detail_controls(ui: &mut egui::Ui, app: &mut OpenboxApp, requested_help: &Cell<Option<HelpTopic>>) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::None, "不显示cqp");
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::Every, "每组cqp");
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::Top, "有效cqp");
        help_icon(ui, HelpTopic::PairDetails, requested_help);
    });
    if app.pair.detail_mode == PairDetailMode::Every {
        ui.horizontal(|ui| {
            ui.label("cqp阈值");
            ui.add(egui::TextEdit::singleline(&mut app.pair.detail_min).desired_width(72.0));
        });
    }
}

fn main_accuracy_controls(
    ui: &mut egui::Ui,
    mode: &mut CountMode,
    accuracy: &mut AccuracyPreset,
    topic: HelpTopic,
    requested_help: &Cell<Option<HelpTopic>>,
) {
    ui.horizontal(|ui| {
        ui.label("精确度");
        for preset in AccuracyPreset::ALL {
            if ui.radio_value(accuracy, preset, preset.label()).changed() {
                *mode = CountMode::Accuracy;
            }
        }
        help_icon(ui, topic, requested_help);
    });
}

fn namer_pf_metric_controls_clean(ui: &mut egui::Ui, app: &mut OpenboxApp, show_highlight: bool) {
    let all_selected = app.namer_pf.metrics.iter().all(|metric| metric.screen && metric.file_output.enabled)
        && app.namer_pf.skill_board.screen
        && app.namer_pf.skill_board.file_output.enabled;
    let mut select_all = all_selected;
    if ui.checkbox(&mut select_all, "全选").changed() {
        for metric in &mut app.namer_pf.metrics {
            metric.screen = select_all;
            metric.file_output.enabled = select_all;
        }
        app.namer_pf.skill_board.screen = select_all;
        app.namer_pf.skill_board.file_output.enabled = select_all;
    }
    namer_pf_metric_header(ui, show_highlight);

    for metric in &mut app.namer_pf.metrics {
        let label = metric.metric.label();
        ui.horizontal(|ui| {
            namer_pf_label_cell(ui, label);
            namer_pf_checkbox_cell(ui, &mut metric.screen);
            namer_pf_text_cell(ui, &mut metric.min_screen);
            if show_highlight {
                namer_pf_text_cell_width(ui, &mut metric.highlight_delta, NAMER_PF_HIGHLIGHT_WIDTH);
            }
            namer_pf_checkbox_cell(ui, &mut metric.file_output.enabled);
            namer_pf_text_cell(ui, &mut metric.min_file);
            namer_pf_action_cell(ui, |ui| {
                if ui.button("选择").clicked()
                    && let Some(path) = pick_named_output_file(&format!("tswn-openbox-namer-pf-{label}.txt"))
                {
                    metric.file_output.enabled = true;
                    metric.file_output.path = Some(path);
                }
                if metric.file_output.path.is_some() && ui.button("清空").clicked() {
                    metric.file_output.path = None;
                }
            });
        });

        let path_label = metric
            .file_output
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未选择输出文件".to_string());
        namer_pf_path_row(ui, path_label);
    }

    ui.horizontal(|ui| {
        namer_pf_label_cell(ui, "技能榜");
        namer_pf_checkbox_cell(ui, &mut app.namer_pf.skill_board.screen);
        namer_pf_fixed_label_cell(ui, "来自 score_now.toml", NAMER_PF_VALUE_WIDTH);
        if show_highlight {
            namer_pf_fixed_label_cell(ui, "", NAMER_PF_HIGHLIGHT_WIDTH);
        }
        namer_pf_checkbox_cell(ui, &mut app.namer_pf.skill_board.file_output.enabled);
        namer_pf_fixed_label_cell(ui, "来自 score_now.toml", NAMER_PF_VALUE_WIDTH);
        namer_pf_action_cell(ui, |ui| {
            if ui.button("选择").clicked()
                && let Some(path) = pick_named_output_file("tswn-openbox-namer-pf-skill-board.txt")
            {
                app.namer_pf.skill_board.file_output.enabled = true;
                app.namer_pf.skill_board.file_output.path = Some(path);
            }
            if app.namer_pf.skill_board.file_output.path.is_some() && ui.button("清空").clicked() {
                app.namer_pf.skill_board.file_output.path = None;
            }
        });
    });
    let path_label = app
        .namer_pf
        .skill_board
        .file_output
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "未选择输出文件".to_string());
    namer_pf_path_row(ui, path_label);
}

fn namer_pf_metric_header(ui: &mut egui::Ui, show_highlight: bool) {
    ui.horizontal(|ui| {
        namer_pf_fixed_label_cell(ui, "", NAMER_PF_METRIC_LABEL_WIDTH);
        namer_pf_fixed_label_cell(ui, "屏幕", NAMER_PF_TOGGLE_WIDTH);
        namer_pf_fixed_label_cell(ui, "屏幕阈值", NAMER_PF_VALUE_WIDTH);
        if show_highlight {
            namer_pf_fixed_label_cell(ui, "高亮超强名字", NAMER_PF_HIGHLIGHT_WIDTH);
        }
        namer_pf_fixed_label_cell(ui, "输出文件", NAMER_PF_TOGGLE_WIDTH);
        namer_pf_fixed_label_cell(ui, "文件阈值", NAMER_PF_VALUE_WIDTH);
        namer_pf_fixed_label_cell(ui, "操作", NAMER_PF_ACTION_WIDTH);
    });
}

fn namer_pf_label_cell(ui: &mut egui::Ui, text: &str) { namer_pf_fixed_label_cell(ui, text, NAMER_PF_METRIC_LABEL_WIDTH); }

fn namer_pf_fixed_label_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.add_sized([width, ui.spacing().interact_size.y], egui::Label::new(text));
}

fn namer_pf_checkbox_cell(ui: &mut egui::Ui, value: &mut bool) {
    ui.add_sized(
        [NAMER_PF_TOGGLE_WIDTH, ui.spacing().interact_size.y],
        egui::Checkbox::without_text(value),
    );
}

fn namer_pf_text_cell(ui: &mut egui::Ui, text: &mut String) { namer_pf_text_cell_width(ui, text, NAMER_PF_VALUE_WIDTH); }

fn namer_pf_text_cell_width(ui: &mut egui::Ui, text: &mut String, width: f32) {
    ui.add_sized([width, ui.spacing().interact_size.y], egui::TextEdit::singleline(text));
}

fn namer_pf_action_cell(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(NAMER_PF_ACTION_WIDTH, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        add_contents,
    );
}

fn namer_pf_path_row(ui: &mut egui::Ui, text: String) {
    ui.scope(|ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(text).wrap());
    });
}
