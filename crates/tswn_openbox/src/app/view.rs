//! egui rendering for every Openbox tool.

use eframe::egui;

use crate::backend::PairDetailMode;

use super::state::{AccuracyPreset, CountMode, OpenboxApp, Tool};
use super::widgets::{
    bench_output_controls, count_mode_controls, optional_file_output_controls, pick_named_output_file, thread_controls,
};

impl OpenboxApp {
    pub(crate) fn to_diy_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "to-diy", "名字转 DIY / 召唤物 DIY", &mut self.more_settings_open);
        section(ui, "基础选项", |ui| {
            to_diy_basic_controls(ui, self);
        });
        section(ui, "名字", |ui| {
            self.to_diy.names.ui(ui, "名字", "to_diy_names", 16);
        });
        section(ui, "输出与运行", |ui| {
            optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
            if run_or_stop_button(ui, self) {
                self.start_to_diy();
            }
        });
    }

    pub(crate) fn namer_pf_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "namer-pf", "批量评分并筛选名字", &mut self.more_settings_open);
        section(ui, "精确度", |ui| {
            main_accuracy_controls(ui, &mut self.namer_pf.count_mode, &mut self.namer_pf.accuracy);
        });
        section(ui, "评分项", |ui| {
            namer_pf_metric_controls(ui, self, false);
        });
        section(ui, "名字", |ui| {
            self.namer_pf.names.ui(ui, "名字", "namer_pf_names", 14);
        });
        section(ui, "运行", |ui| {
            if run_or_stop_button(ui, self) {
                self.start_namer_pf();
            }
        });
    }

    pub(crate) fn batch_rate_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "cqd/cqp", "计算选手对靶子的平均胜率", &mut self.more_settings_open);
        section(ui, "常用设置", |ui| {
            main_accuracy_controls(ui, &mut self.batch_rate.count_mode, &mut self.batch_rate.accuracy);
            target_preset_controls(ui, &mut self.batch_rate.target_presets);
            ui.checkbox(&mut self.batch_rate.show_matchups, "每组胜率");
        });
        section(ui, "输出", |ui| {
            bench_output_controls(ui, &mut self.batch_rate.output, "tswn-openbox-cqd-cqp.txt", false, false);
        });
        section(ui, "选手列表", |ui| {
            self.batch_rate.players.ui(ui, "选手", "batch_players", 8);
        });
        section(ui, "运行", |ui| {
            if run_or_stop_button(ui, self) {
                self.start_batch_rate();
            }
        });
    }

    pub(crate) fn pair_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "pair", "计算选手与队友组合表现", &mut self.more_settings_open);
        section(ui, "常用设置", |ui| {
            main_accuracy_controls(ui, &mut self.pair.count_mode, &mut self.pair.accuracy);
            teammate_preset_controls(ui, &mut self.pair.teammate_presets);
            pair_detail_controls(ui, self);
        });
        section(ui, "输出", |ui| {
            bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt", false, false);
        });
        section(ui, "选手列表", |ui| {
            self.pair.players.ui(ui, "选手", "pair_players", 6);
        });
        section(ui, "运行", |ui| {
            if run_or_stop_button(ui, self) {
                self.start_pair();
            }
        });
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
                    egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                        ui.add_enabled_ui(!self.running, |ui| match self.tool {
                            Tool::ToDiy => self.to_diy_more_settings(ui),
                            Tool::NamerPf => self.namer_pf_more_settings(ui),
                            Tool::BatchRate => self.batch_rate_more_settings(ui),
                            Tool::Pair => self.pair_more_settings(ui),
                        });
                    });
                });
            });
        self.more_settings_open = open;
    }

    fn to_diy_more_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "基础选项", |ui| {
            to_diy_basic_controls(ui, self);
            ui.checkbox(&mut self.to_diy.details, "单名详情（仅日志输出）");
        });
        section(ui, "输出", |ui| {
            optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
        });
    }

    fn namer_pf_more_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "计算设置", |ui| {
            count_mode_controls(
                ui,
                &mut self.namer_pf.count_mode,
                &mut self.namer_pf.accuracy,
                &mut self.namer_pf.count,
            );
            thread_controls(ui, &mut self.namer_pf.auto_threads, &mut self.namer_pf.threads);
            ui.checkbox(&mut self.namer_pf.keep_rq, "不低估短号");
        });
        section(ui, "评分项", |ui| {
            namer_pf_metric_controls(ui, self, true);
        });
    }

    fn batch_rate_more_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "计算设置", |ui| {
            count_mode_controls(
                ui,
                &mut self.batch_rate.count_mode,
                &mut self.batch_rate.accuracy,
                &mut self.batch_rate.count,
            );
            thread_controls(ui, &mut self.batch_rate.auto_threads, &mut self.batch_rate.threads);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.batch_rate.keep_rq, "不低估短号");
                ui.checkbox(&mut self.batch_rate.double_plus, "DIYcqp（++分割名字）");
            });
        });
        section(ui, "靶子", |ui| {
            ui.checkbox(&mut self.batch_rate.manual_targets, "使用手动靶子");
            if self.batch_rate.manual_targets {
                self.batch_rate.targets.ui(ui, "靶子", "batch_targets_more", 8);
            } else {
                target_preset_controls(ui, &mut self.batch_rate.target_presets);
            }
        });
        section(ui, "输出", |ui| {
            highlight_delta_control(ui, &mut self.batch_rate.highlight_delta);
            bench_output_controls(ui, &mut self.batch_rate.output, "tswn-openbox-cqd-cqp.txt", true, true);
        });
    }

    fn pair_more_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "计算设置", |ui| {
            count_mode_controls(ui, &mut self.pair.count_mode, &mut self.pair.accuracy, &mut self.pair.count);
            thread_controls(ui, &mut self.pair.auto_threads, &mut self.pair.threads);
            ui.checkbox(&mut self.pair.keep_rq, "不低估短号");
            pair_detail_controls(ui, self);
        });

        section(ui, "靶子", |ui| {
            ui.checkbox(&mut self.pair.manual_targets, "使用手动靶子");
            if self.pair.manual_targets {
                self.pair.targets.ui(ui, "靶子", "pair_targets_more", 6);
            } else {
                target_preset_controls(ui, &mut self.pair.target_presets);
            }
        });

        section(ui, "队友", |ui| {
            ui.checkbox(&mut self.pair.manual_teammates, "使用手动队友");
            if self.pair.manual_teammates {
                ui.horizontal(|ui| {
                    ui.label("保留前几");
                    ui.add(egui::DragValue::new(&mut self.pair.head).range(1..=999).speed(1));
                });
                self.pair.teammates.ui(ui, "队友", "pair_teammates_more", 6);
            } else {
                teammate_preset_controls(ui, &mut self.pair.teammate_presets);
            }
        });

        section(ui, "输出", |ui| {
            highlight_delta_control(ui, &mut self.pair.highlight_delta);
            bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt", true, true);
        });
    }

    pub(crate) fn log_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
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
                        self.highlight_lines.clear();
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
        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                if self.log.trim().is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(egui::RichText::new("暂无日志").weak().size(18.0));
                        ui.label(egui::RichText::new("选择工具、填好输入，然后点击运行。旧日志会在新任务开始时清空。").weak());
                    });
                } else {
                    ui.vertical(|ui| {
                        for (index, line) in self.log.lines().enumerate() {
                            let mut text = egui::RichText::new(line).monospace();
                            if line.starts_with("  ") {
                                text = text.color(egui::Color32::GRAY);
                            } else if self.highlight_lines.contains(&index) {
                                text = text.color(egui::Color32::from_rgb(210, 40, 40)).strong();
                            }
                            ui.add(egui::Label::new(text).extend());
                        }
                    });
                }
            });
        });
    }
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
    ui.add_space(8.0);
}

fn section<R>(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().size(16.0));
            ui.separator();
            add_contents(ui)
        })
        .inner;
    ui.add_space(8.0);
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

fn run_or_stop_button(ui: &mut egui::Ui, app: &mut OpenboxApp) -> bool {
    ui.add_space(4.0);
    if app.running {
        let label = if app.cancel_requested { "停止中..." } else { "停止" };
        let button = egui::Button::new(egui::RichText::new(label).size(18.0)).min_size(egui::vec2(ui.available_width(), 44.0));
        if ui.add_enabled(!app.cancel_requested, button).clicked() {
            app.stop_current_task();
        }
        false
    } else {
        ui.add(egui::Button::new(egui::RichText::new("运行").size(18.0)).min_size(egui::vec2(ui.available_width(), 44.0)))
            .clicked()
    }
}

fn highlight_delta_control(ui: &mut egui::Ui, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label("高亮超强名字");
        ui.add(egui::TextEdit::singleline(value).desired_width(72.0));
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

fn teammate_preset_controls(ui: &mut egui::Ui, state: &mut super::target_presets::TeammatePresetState) {
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
    });
    if let Some(error) = &state.error {
        ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
    }
}

fn pair_detail_controls(ui: &mut egui::Ui, app: &mut OpenboxApp) {
    ui.horizontal(|ui| {
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::None, "不显示cqp");
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::Every, "每组cqp");
        ui.radio_value(&mut app.pair.detail_mode, PairDetailMode::Top, "有效cqp");
    });
    if app.pair.detail_mode == PairDetailMode::Every {
        ui.horizontal(|ui| {
            ui.label("cqp阈值");
            ui.add(egui::TextEdit::singleline(&mut app.pair.detail_min).desired_width(72.0));
        });
    }
}

fn main_accuracy_controls(ui: &mut egui::Ui, mode: &mut CountMode, accuracy: &mut AccuracyPreset) {
    ui.horizontal(|ui| {
        ui.label("精确度");
        for preset in AccuracyPreset::ALL {
            if ui.radio_value(accuracy, preset, preset.label()).changed() {
                *mode = CountMode::Accuracy;
            }
        }
    });
}

fn namer_pf_metric_controls(ui: &mut egui::Ui, app: &mut OpenboxApp, show_highlight: bool) {
    let all_selected = app.namer_pf.metrics.iter().all(|metric| metric.screen && metric.file_output.enabled);
    let mut select_all = all_selected;
    if ui.checkbox(&mut select_all, "全选").changed() {
        for metric in &mut app.namer_pf.metrics {
            metric.screen = select_all;
            metric.file_output.enabled = select_all;
        }
    }
    egui::Grid::new(if show_highlight {
        "namer_pf_metrics_more"
    } else {
        "namer_pf_metrics"
    })
    .num_columns(if show_highlight { 7 } else { 6 })
    .striped(true)
    .spacing([10.0, 6.0])
    .show(ui, |ui| {
        ui.label("");
        ui.label("屏幕");
        ui.label("屏幕阈值");
        if show_highlight {
            ui.label("高亮超强名字");
        }
        ui.label("输出文件");
        ui.label("文件阈值");
        ui.label("路径");
        ui.end_row();

        for metric in &mut app.namer_pf.metrics {
            let label = metric.metric.label();
            ui.label(label);
            ui.checkbox(&mut metric.screen, "");
            ui.add(egui::TextEdit::singleline(&mut metric.min_screen).desired_width(72.0));
            if show_highlight {
                ui.add(egui::TextEdit::singleline(&mut metric.highlight_delta).desired_width(72.0));
            }
            ui.checkbox(&mut metric.file_output.enabled, "");
            ui.add(egui::TextEdit::singleline(&mut metric.min_file).desired_width(72.0));
            ui.horizontal(|ui| {
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
            ui.end_row();

            ui.label("");
            ui.label("");
            ui.label("");
            if show_highlight {
                ui.label("");
            }
            ui.label("");
            ui.label("");
            let path_label = metric
                .file_output
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "未选择输出文件".to_string());
            ui.label(path_label);
            ui.end_row();
        }
    });
}
