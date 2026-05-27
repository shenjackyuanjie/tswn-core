//! GUI 应用模块。
//!
//! 定义 `OpenboxApp`（`eframe::App` 实现）的顶层布局，负责安装字体、
//! 组织工具栏与左侧输入面板、中央日志面板，并将各工具 UI 分发给子模块渲染。

mod actions;
mod source;
mod state;
mod view;
mod widgets;

use eframe::egui;

pub(crate) use state::{OpenboxApp, Tool};

const SARASA_MONO_SC: &[u8] = include_bytes!("SarasaMonoSC-Regular.ttf");

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tswn openbox")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "tswn openbox",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::<OpenboxApp>::default())
        }),
    )
}

impl eframe::App for OpenboxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_events(&ctx);

        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("tswn openbox");
                ui.separator();
                for tool in Tool::ALL {
                    if ui.selectable_label(self.tool == tool, tool.label()).clicked() && !self.running {
                        self.tool = tool;
                    }
                }
            });
        });

        egui::Panel::left("inputs")
            .resizable(true)
            .min_size(360.0)
            .default_size(430.0)
            .show_inside(ui, |ui| {
                ui.add_enabled_ui(!self.running, |ui| match self.tool {
                    Tool::ToDiy => self.to_diy_ui(ui),
                    Tool::NamerPf => self.namer_pf_ui(ui),
                    Tool::BatchRate => self.batch_rate_ui(ui),
                    Tool::Pair => self.pair_ui(ui),
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.log_ui(ui, &ctx);
        });
    }
}

fn install_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "SarasaMonoSC".to_string(),
        std::sync::Arc::new(egui::FontData::from_static(SARASA_MONO_SC)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "SarasaMonoSC".to_string());
    }
    ctx.set_fonts(fonts);
}
