//! 各工具的 egui 界面渲染。

use eframe::egui;

use super::state::{AccuracyPreset, CountMode, OpenboxApp, Tool};
use super::widgets::{
    bench_output_controls, count_mode_controls, optional_file_output_controls, pick_named_output_file, thread_controls,
};

impl OpenboxApp {
    pub(crate) fn to_diy_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "to-diy", &mut self.more_settings_open);
        to_diy_basic_controls(ui, self);
        self.to_diy.names.ui(ui, "名字", "to_diy_names", 16);
        optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
        if run_or_stop_button(ui, self) {
            self.start_to_diy();
        }
    }

    pub(crate) fn namer_pf_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "namer-pf", &mut self.more_settings_open);
        main_accuracy_controls(ui, &mut self.namer_pf.count_mode, &mut self.namer_pf.accuracy);
        namer_pf_metric_controls(ui, self);
        self.namer_pf.names.ui(ui, "名字", "namer_pf_names", 14);
        if run_or_stop_button(ui, self) {
            self.start_namer_pf();
        }
    }

    pub(crate) fn batch_rate_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "cqd/cqp", &mut self.more_settings_open);
        main_accuracy_controls(ui, &mut self.batch_rate.count_mode, &mut self.batch_rate.accuracy);
        bench_output_controls(ui, &mut self.batch_rate.output, "tswn-openbox-cqd-cqp.txt", false, false);
        ui.separator();
        self.batch_rate.targets.ui(ui, "靶子列表", "batch_targets", 8);
        ui.separator();
        self.batch_rate.players.ui(ui, "选手列表", "batch_players", 8);
        if run_or_stop_button(ui, self) {
            self.start_batch_rate();
        }
    }

    pub(crate) fn pair_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "pair", &mut self.more_settings_open);
        main_accuracy_controls(ui, &mut self.pair.count_mode, &mut self.pair.accuracy);
        ui.horizontal(|ui| {
            ui.label("head");
            ui.add(egui::DragValue::new(&mut self.pair.head).range(1..=999).speed(1));
        });
        bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt", false, false);
        ui.separator();
        self.pair.targets.ui(ui, "靶子列表", "pair_targets", 6);
        ui.separator();
        self.pair.players.ui(ui, "选手列表", "pair_players", 6);
        ui.separator();
        self.pair.teammates.ui(ui, "队友列表", "pair_teammates", 6);
        if run_or_stop_button(ui, self) {
            self.start_pair();
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
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.running, |ui| match self.tool {
                    Tool::ToDiy => self.to_diy_more_settings(ui),
                    Tool::NamerPf => self.namer_pf_more_settings(ui),
                    Tool::BatchRate => self.batch_rate_more_settings(ui),
                    Tool::Pair => self.pair_more_settings(ui),
                });
            });
        self.more_settings_open = open;
    }

    fn to_diy_more_settings(&mut self, ui: &mut egui::Ui) {
        to_diy_basic_controls(ui, self);
        ui.checkbox(&mut self.to_diy.details, "单名详情（仅日志输出）");
        optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
    }

    fn namer_pf_more_settings(&mut self, ui: &mut egui::Ui) {
        count_mode_controls(
            ui,
            &mut self.namer_pf.count_mode,
            &mut self.namer_pf.accuracy,
            &mut self.namer_pf.count,
        );
        thread_controls(ui, &mut self.namer_pf.auto_threads, &mut self.namer_pf.threads);
        ui.checkbox(&mut self.namer_pf.keep_rq, "keep rq");
        namer_pf_metric_controls(ui, self);
    }

    fn batch_rate_more_settings(&mut self, ui: &mut egui::Ui) {
        count_mode_controls(
            ui,
            &mut self.batch_rate.count_mode,
            &mut self.batch_rate.accuracy,
            &mut self.batch_rate.count,
        );
        thread_controls(ui, &mut self.batch_rate.auto_threads, &mut self.batch_rate.threads);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.batch_rate.keep_rq, "keep rq");
            ui.checkbox(&mut self.batch_rate.double_plus, "DIYcqp（++分割名字）");
        });
        bench_output_controls(ui, &mut self.batch_rate.output, "tswn-openbox-cqd-cqp.txt", true, true);
    }

    fn pair_more_settings(&mut self, ui: &mut egui::Ui) {
        count_mode_controls(ui, &mut self.pair.count_mode, &mut self.pair.accuracy, &mut self.pair.count);
        thread_controls(ui, &mut self.pair.auto_threads, &mut self.pair.threads);
        ui.horizontal(|ui| {
            ui.label("head");
            ui.add(egui::DragValue::new(&mut self.pair.head).range(1..=999).speed(1));
            ui.checkbox(&mut self.pair.keep_rq, "keep rq");
        });
        bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt", true, true);
    }

    pub(crate) fn log_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.vertical(|ui| {
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
                    }
                });
            });
            if self.total > 0 {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("速度: {}", self.rate_text)).size(16.0));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("剩余: {}", self.eta_text)).size(16.0));
                });
            }
        });
        ui.separator();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut self.log)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(30),
            );
        });
    }
}

fn tool_header(ui: &mut egui::Ui, title: &str, more_settings_open: &mut bool) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("更多设置").clicked() {
                *more_settings_open = true;
            }
        });
    });
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
    if app.running {
        let label = if app.cancel_requested { "停止中..." } else { "停止" };
        if ui.add_enabled(!app.cancel_requested, egui::Button::new(label)).clicked() {
            app.stop_current_task();
        }
        false
    } else {
        ui.button("运行").clicked()
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

fn namer_pf_metric_controls(ui: &mut egui::Ui, app: &mut OpenboxApp) {
    let all_selected = app.namer_pf.metrics.iter().all(|metric| metric.screen && metric.file_output.enabled);
    let mut select_all = all_selected;
    if ui.checkbox(&mut select_all, "全选").changed() {
        for metric in &mut app.namer_pf.metrics {
            metric.screen = select_all;
            metric.file_output.enabled = select_all;
        }
    }
    egui::Grid::new("namer_pf_metrics")
        .num_columns(6)
        .striped(true)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label("");
            ui.label("屏幕");
            ui.label("屏幕阈值");
            ui.label("输出文件");
            ui.label("文件阈值");
            ui.label("路径");
            ui.end_row();

            for metric in &mut app.namer_pf.metrics {
                let label = metric.metric.label();
                ui.label(label);
                ui.checkbox(&mut metric.screen, "");
                ui.add(egui::TextEdit::singleline(&mut metric.min_screen).desired_width(72.0));
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
