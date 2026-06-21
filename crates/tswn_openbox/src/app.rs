//! GUI 应用模块。
//!
//! 定义 `OpenboxApp`（`eframe::App` 实现）的顶层布局，负责安装字体、
//! 组织工具栏与左侧输入面板、中央日志面板，并将各工具 UI 分发给子模块渲染。

mod actions;
mod source;
mod state;
mod target_presets;
mod view;
mod widgets;

use eframe::egui;

pub use state::{OpenboxApp, Tool};

const SARASA_FONT_NAME: &str = "SarasaMonoSC";
const SARASA_MONO_SC: &[u8] = include_bytes!("SarasaMonoSC-Regular.ttf");
const PANEL_MARGIN: i8 = 8;
const TOP_BAR_VERTICAL_SPACE: f32 = 2.0;
const RUN_BUTTON_HEIGHT: f32 = 38.0;
const INPUT_FOOTER_RESERVED_HEIGHT: f32 = 56.0;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("tswn openbox")
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "tswn openbox",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::ThemePreference::System);
            install_cjk_fonts(&cc.egui_ctx);
            configure_ui_style(&cc.egui_ctx);
            Ok(Box::<OpenboxApp>::default())
        }),
    )
}

impl eframe::App for OpenboxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.set_theme(self.theme_preference);
        self.poll_events(&ctx);

        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            top_bar_ui(ui, self, &ctx);
        });

        egui::Panel::left("inputs")
            .resizable(true)
            .min_size(380.0)
            .default_size(460.0)
            .show_inside(ui, |ui| {
                egui::Frame::side_top_panel(ui.style())
                    .inner_margin(egui::Margin::same(PANEL_MARGIN))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let scroll_height = (ui.available_height() - INPUT_FOOTER_RESERVED_HEIGHT).max(160.0);
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .max_height(scroll_height)
                                .show(ui, |ui| match self.tool {
                                    Tool::ToDiy => self.show_diy_ui(ui),
                                    Tool::NamerPf => self.namer_pf_ui(ui),
                                    Tool::BatchRate => self.batch_rate_ui(ui),
                                    Tool::Pair => self.pair_ui(ui),
                                });
                            ui.separator();
                            run_footer_ui(ui, self);
                        });
                    });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::Frame::central_panel(ui.style())
                .inner_margin(egui::Margin::same(PANEL_MARGIN))
                .show(ui, |ui| {
                    self.log_ui(ui, &ctx);
                });
        });

        self.more_settings_window(&ctx);
    }
}

fn run_footer_ui(ui: &mut egui::Ui, app: &mut OpenboxApp) {
    ui.add_space(2.0);
    if app.running {
        let label = if app.cancel_requested { "停止中..." } else { "停止" };
        let button = egui::Button::new(egui::RichText::new(label).size(17.0))
            .min_size(egui::vec2(ui.available_width(), RUN_BUTTON_HEIGHT));
        if ui.add_enabled(!app.cancel_requested, button).clicked() {
            app.stop_current_task();
        }
    } else {
        let button = egui::Button::new(egui::RichText::new("运行").size(17.0))
            .min_size(egui::vec2(ui.available_width(), RUN_BUTTON_HEIGHT));
        if ui.add(button).clicked() {
            match app.tool {
                Tool::ToDiy => app.start_to_diy(),
                Tool::NamerPf => app.start_namer_pf(),
                Tool::BatchRate => app.start_batch_rate(),
                Tool::Pair => app.start_pair(),
            }
        }
    }
}

fn top_bar_ui(ui: &mut egui::Ui, app: &mut OpenboxApp, ctx: &egui::Context) {
    ui.add_space(TOP_BAR_VERTICAL_SPACE);
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("tswn openbox").size(23.0));
        ui.label(egui::RichText::new("本地工具箱").weak());
        ui.separator();

        for tool in Tool::ALL {
            let selected = app.tool == tool;
            let label = egui::RichText::new(tool.label()).size(16.0);
            if ui.selectable_label(selected, label).clicked() && !app.running {
                app.tool = tool;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme_switcher(ui, &mut app.theme_preference) {
                ctx.set_theme(app.theme_preference);
            }
            ui.separator();
            status_pill(ui, app);
        });
    });
    ui.add_space(TOP_BAR_VERTICAL_SPACE);
}

fn status_pill(ui: &mut egui::Ui, app: &OpenboxApp) {
    let color = if app.running {
        egui::Color32::from_rgb(35, 130, 220)
    } else if app.status == "失败" {
        egui::Color32::from_rgb(190, 50, 50)
    } else if app.status == "完成" {
        egui::Color32::from_rgb(40, 150, 90)
    } else {
        ui.visuals().weak_text_color()
    };
    ui.label(egui::RichText::new(&app.status).color(color).strong());
}

fn theme_switcher(ui: &mut egui::Ui, theme_preference: &mut egui::ThemePreference) -> bool {
    let mut changed = false;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
        ui.spacing_mut().button_padding = egui::vec2(3.0, 1.0);
        ui.horizontal(|ui| {
            changed |= theme_button(ui, theme_preference, egui::ThemePreference::Light, "亮", "浅色模式");
            changed |= theme_button(ui, theme_preference, egui::ThemePreference::Dark, "暗", "深色模式");
            changed |= theme_button(ui, theme_preference, egui::ThemePreference::System, "自", "跟随系统主题");
        })
        .response
        .on_hover_text("主题");
    });
    changed
}

fn theme_button(
    ui: &mut egui::Ui,
    theme_preference: &mut egui::ThemePreference,
    value: egui::ThemePreference,
    icon: &str,
    tooltip: &str,
) -> bool {
    let selected = *theme_preference == value;
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if selected {
        visuals.selection.stroke
    } else {
        egui::Stroke::new(1.0_f32, visuals.widgets.inactive.bg_stroke.color)
    };
    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        visuals.strong_text_color()
    };
    let button = egui::Button::new(egui::RichText::new(icon).size(13.0).strong().color(text_color))
        .fill(fill)
        .stroke(stroke)
        .min_size(egui::vec2(22.0, 22.0));
    let response = ui.add(button).on_hover_text(tooltip);
    if response.clicked() {
        *theme_preference = value;
        true
    } else {
        false
    }
}

fn install_cjk_fonts(ctx: &egui::Context) { ctx.set_fonts(openbox_font_definitions()); }

fn openbox_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        SARASA_FONT_NAME.to_string(),
        std::sync::Arc::new(egui::FontData::from_static(SARASA_MONO_SC)),
    );
    let emoji_fonts = install_system_emoji_fonts(&mut fonts);
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let entries = fonts.families.entry(family).or_default();
        entries.retain(|name| name != SARASA_FONT_NAME && !emoji_fonts.iter().any(|emoji| emoji == name));
        entries.insert(0, SARASA_FONT_NAME.to_string());
        for (offset, emoji_font) in emoji_fonts.iter().enumerate() {
            entries.insert(offset + 1, emoji_font.clone());
        }
    }
    fonts
}

fn install_system_emoji_fonts(fonts: &mut egui::FontDefinitions) -> Vec<String> {
    let candidates = [
        (
            "SegoeUIEmoji",
            &[r"C:\Windows\Fonts\seguiemj.ttf", r"C:\Windows\Fonts\Segoe UI Emoji.ttf"][..],
        ),
        ("AppleColorEmoji", &["/System/Library/Fonts/Apple Color Emoji.ttc"][..]),
        (
            "NotoColorEmoji",
            &[
                "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf",
                "/usr/share/fonts/noto/NotoColorEmoji.ttf",
            ][..],
        ),
    ];
    let mut installed = Vec::new();
    for (name, paths) in candidates {
        if install_first_existing_font(fonts, name, paths) {
            installed.push(name.to_string());
        }
    }
    installed
}

fn install_first_existing_font(fonts: &mut egui::FontDefinitions, name: &str, paths: &[&str]) -> bool {
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            return true;
        }
    }
    false
}

fn configure_ui_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 4.0);
        style.spacing.window_margin = egui::Margin::same(8);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.interact_size = egui::vec2(30.0, 26.0);
        style.spacing.combo_width = 160.0;
        style.spacing.text_edit_width = 160.0;
        style.spacing.slider_width = 160.0;
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(22.0, egui::FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::new(15.0, egui::FontFamily::Proportional));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::new(15.0, egui::FontFamily::Proportional));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::new(14.0, egui::FontFamily::Monospace));
    });
}
