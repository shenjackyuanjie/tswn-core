use eframe::egui;

use super::state::OpenboxApp;
use super::widgets::{bench_controls, bench_output_controls, optional_file_output_controls};

impl OpenboxApp {
    pub(crate) fn to_diy_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("to-diy");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.to_diy.old, "旧 +diy");
            ui.checkbox(&mut self.to_diy.details, "单名详情（仅日志输出）");
        });
        self.to_diy.names.ui(ui, "名字", "to_diy_names", 16);
        optional_file_output_controls(ui, &mut self.to_diy.output, "tswn-openbox-diy.txt");
        if ui.button("运行").clicked() {
            self.start_to_diy();
        }
    }

    pub(crate) fn namer_pf_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("namer-pf");
        bench_controls(ui, &mut self.namer_pf.count, &mut self.namer_pf.threads);
        self.namer_pf.names.ui(ui, "名字", "namer_pf_names", 14);
        optional_file_output_controls(ui, &mut self.namer_pf.output, "tswn-openbox-namer-pf.txt");
        if ui.button("运行").clicked() {
            self.start_namer_pf();
        }
    }

    pub(crate) fn batch_rate_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("batch-rate");
        bench_controls(ui, &mut self.batch_rate.count, &mut self.batch_rate.threads);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.batch_rate.keep_rq, "keep rq");
            ui.checkbox(&mut self.batch_rate.verbose, "详细");
            ui.checkbox(&mut self.batch_rate.perf, "perf");
            ui.checkbox(&mut self.batch_rate.double_plus, "++ 分组");
        });
        bench_output_controls(ui, &mut self.batch_rate.output, "tswn-openbox-batch-rate.txt");
        ui.separator();
        self.batch_rate.targets.ui(ui, "靶子列表", "batch_targets", 8);
        ui.separator();
        self.batch_rate.players.ui(ui, "选手列表", "batch_players", 8);
        if ui.button("运行").clicked() {
            self.start_batch_rate();
        }
    }

    pub(crate) fn pair_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("pair");
        bench_controls(ui, &mut self.pair.count, &mut self.pair.threads);
        ui.horizontal(|ui| {
            ui.label("head");
            ui.add(egui::DragValue::new(&mut self.pair.head).range(1..=999).speed(1));
            ui.checkbox(&mut self.pair.keep_rq, "keep rq");
            ui.checkbox(&mut self.pair.verbose, "详细");
            ui.checkbox(&mut self.pair.perf, "perf");
        });
        bench_output_controls(ui, &mut self.pair.output, "tswn-openbox-pair.txt");
        ui.separator();
        self.pair.targets.ui(ui, "靶子列表", "pair_targets", 6);
        ui.separator();
        self.pair.players.ui(ui, "选手列表", "pair_players", 6);
        ui.separator();
        self.pair.teammates.ui(ui, "队友列表", "pair_teammates", 6);
        if ui.button("运行").clicked() {
            self.start_pair();
        }
    }

    pub(crate) fn log_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(&self.status);
            if self.total > 0 {
                let progress = self.done as f32 / self.total.max(1) as f32;
                ui.add(egui::ProgressBar::new(progress).show_percentage().desired_width(180.0));
                ui.label(format!("{}/{}", self.done, self.total));
                ui.separator();
                ui.label(format!("速度: {}", self.rate_text));
                ui.label(format!("剩余: {}", self.eta_text));
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
