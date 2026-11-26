//! GUI 应用模块。
//!
//! 定义 `OpenboxApp`（`eframe::App` 实现）的顶层布局，负责安装字体、
//! 组织工具栏与左侧输入面板、中央日志面板，并将各工具 UI 分发给子模块渲染。

/// 任务启动/停止与参数读取动作。
mod actions;
#[cfg(feature = "ui_capture")]
pub mod capture;
/// DS4 工具面板与设置。
mod ds4;
/// DS4 运行说明文本。
mod ds4_help;
/// 帮助窗口渲染。
mod help;
/// 工具图标绘制。
mod icons;
/// 日志面板与容量限制。
mod log;
/// 结果表格与明细视图。
mod results;
/// UI 设置持久化。
mod settings;
/// 输入源选择与预览。
mod source;
/// 应用状态与工具枚举。
mod state;
/// 配色与通用样式助手。
mod style;
/// 后台任务事件轮询。
mod task;
/// 各工具的参数 UI。
mod view;
/// 复用的小控件。
mod widgets;
pub use state::{OpenboxApp, Tool};

/// 内嵌 CJK 等宽字体名。
const SARASA_FONT_NAME: &str = "SarasaMonoSC";
/// 内嵌 Sarasa Mono SC 字体字节。
const SARASA_MONO_SC: &[u8] = include_bytes!("../SarasaMonoSC-Regular.ttf");
/// 面板统一外边距。
const PANEL_MARGIN: i8 = 8;
/// 顶栏上下预留空间。
const TOP_BAR_VERTICAL_SPACE: f32 = 2.0;
/// 运行/停止按钮高度。
const RUN_BUTTON_HEIGHT: f32 = 38.0;
/// 输入面板为底部运行区预留的高度。
const INPUT_FOOTER_RESERVED_HEIGHT: f32 = 56.0;

/// 启动 egui/eframe 桌面应用：配置原生窗口选项并进入 Openbox 主界面。
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("tswn openbox {}", env!("CARGO_PKG_VERSION")))
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([960.0, 620.0]),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "tswn openbox",
        options,
        Box::new(|cc| {
            let app = OpenboxApp::from_storage(cc.storage);
            cc.egui_ctx.set_theme(app.theme_preference);
            install_cjk_fonts(&cc.egui_ctx);
            configure_ui_style(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

impl eframe::App for OpenboxApp {
    /// 每帧布局：轮询任务事件，组织顶栏、左侧输入面板与中央日志，并绘制各类弹窗。
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.set_theme(self.theme_preference);
        self.poll_events(&ctx);

        egui::Panel::top("top_bar").show(ui, |ui| {
            top_bar_ui(ui, self, &ctx);
        });

        egui::Panel::left("inputs")
            .resizable(true)
            .min_size(380.0)
            .default_size(460.0)
            .show(ui, |ui| {
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
                                    Tool::Base => self.base_ui(ui),
                                    Tool::Ds4 => self.ds4_ui(ui),
                                });
                            ui.separator();
                            run_footer_ui(ui, self);
                        });
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::Frame::central_panel(ui.style())
                .inner_margin(egui::Margin::same(PANEL_MARGIN))
                .show(ui, |ui| {
                    self.log_ui(ui, &ctx);
                });
        });

        self.more_settings_window(&ctx);
        self.source_advanced_windows(&ctx);
        help::show_help_window(&ctx, &mut self.active_help);
        show_about_window(&ctx, &mut self.about_open);
    }

    /// 退出时把 UI 设置写入存储。
    fn save(&mut self, storage: &mut dyn eframe::Storage) { self.save_ui_settings(storage); }

    /// 自动保存间隔：5 秒。
    fn auto_save_interval(&self) -> std::time::Duration { std::time::Duration::from_secs(5) }

    /// 允许 eframe 持久化 egui 内存。
    fn persist_egui_memory(&self) -> bool { true }
}

/// 输入面板底部运行/停止按钮：运行中显示停止，空闲时按当前工具启动任务。
fn run_footer_ui(ui: &mut egui::Ui, app: &mut OpenboxApp) {
    ui.add_space(2.0);
    let palette = style::Palette::of(ui);
    if app.running {
        if app.tool == Tool::Ds4 {
            ui.add_enabled(
                false,
                egui::Button::new("DS4 处理中…").min_size(egui::vec2(ui.available_width(), RUN_BUTTON_HEIGHT)),
            )
            .on_disabled_hover_text("DS4 按整轮执行，等待历史归档完成后即可再次运行。");
            return;
        }
        let label = if app.cancel_requested { "停止中..." } else { "停止" };
        let button = egui::Button::new(egui::RichText::new(label).size(17.0).color(palette.warning))
            .fill(palette.warning.gamma_multiply(0.12))
            .min_size(egui::vec2(ui.available_width(), RUN_BUTTON_HEIGHT));
        if ui
            .add_enabled(!app.cancel_requested, button)
            .on_hover_text(format!(
                "停止 {} 页正在运行的任务，已完成的明细会保留；未完成的组合不会作为完整结果显示。",
                app.task_tool.label()
            ))
            .clicked()
        {
            app.stop_current_task();
        }
    } else {
        let accent = palette.tool(app.tool.label());
        let button = egui::Button::new(egui::RichText::new("运行").size(17.0).color(accent).strong())
            .fill(accent.gamma_multiply(0.12))
            .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.5)))
            .min_size(egui::vec2(ui.available_width(), RUN_BUTTON_HEIGHT));
        if ui
            .add(button)
            .on_hover_text(if app.tool == Tool::Ds4 {
                ds4_help::RUN
            } else {
                "使用当前输入和设置开始计算。新任务会清空上一次的日志与结果。"
            })
            .clicked()
        {
            match app.tool {
                Tool::ToDiy => app.start_to_diy(),
                Tool::NamerPf => app.start_namer_pf(),
                Tool::BatchRate => app.start_batch_rate(),
                Tool::Pair => app.start_pair(),
                Tool::Base => app.start_base(),
                Tool::Ds4 => app.start_ds4(),
            }
            // 任务刚启动就主动要一帧：否则要等下一次输入事件才会去排空收件箱，
            // base 这类瞬间完成的任务会一直停在“运行中”。
            ui.ctx().request_repaint();
        }
    }
}

/// 顶栏：标题、工具切换、主题切换、关于与状态徽标。
fn top_bar_ui(ui: &mut egui::Ui, app: &mut OpenboxApp, ctx: &egui::Context) {
    ui.add_space(TOP_BAR_VERTICAL_SPACE);
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new("tswn openbox").size(23.0));
        ui.label(egui::RichText::new(format!("v{} · core {}", env!("CARGO_PKG_VERSION"), tswn_core::version())).weak());
        ui.label(egui::RichText::new("本地工具箱").weak());
        ui.separator();

        for tool in Tool::ALL {
            let selected = app.tool == tool;
            // 任务页在运行中用蓝色标出；输出已按页独立，可以随时切走查看其他页。
            let running_here = app.running && app.task_tool == tool;
            let mut label = egui::RichText::new(tool.label()).size(16.0);
            if running_here {
                label = label.color(style::Palette::of(ui).info).strong();
            }
            let response = ui.selectable_label(selected, label);
            let response = if running_here {
                response.on_hover_text("该页任务正在运行；完成后状态与结果会继续留在这一页。")
            } else {
                response
            };
            if response.clicked() && !selected {
                app.tool = tool;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme_switcher(ui, &mut app.theme_preference) {
                ctx.set_theme(app.theme_preference);
            }
            if ui.button("关于").clicked() {
                app.about_open = true;
            }
            ui.separator();
            status_pill(ui, app);
        });
    });
    ui.add_space(TOP_BAR_VERTICAL_SPACE);
}

/// 按状态着色的文本徽标。
fn status_pill(ui: &mut egui::Ui, app: &OpenboxApp) {
    let color = style::Palette::of(ui).status(&app.status);
    ui.label(egui::RichText::new(&app.status).color(color).strong());
}

/// 关于弹窗：版本信息与项目主页。
fn show_about_window(ctx: &egui::Context, open: &mut bool) {
    let mut is_open = *open;
    egui::Window::new("关于")
        .id(egui::Id::unique("openbox_about"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("tswn openbox").size(22.0));
                ui.label(egui::RichText::new("本地工具箱").weak());
            });
            ui.separator();
            ui.label(egui::RichText::new("版本").strong());
            version_row(ui, "tswn_openbox", env!("CARGO_PKG_VERSION"));
            version_row(ui, "tswn_core", tswn_core::version());
            ui.separator();
            ui.label(egui::RichText::new("项目主页").strong());
            ui.hyperlink("https://github.com/shenjackyuanjie/tswn-core");
        });
    *open = is_open;
}

/// 关于窗口中的一行名称/版本。
fn version_row(ui: &mut egui::Ui, name: &str, version: &str) {
    ui.horizontal(|ui| {
        ui.label(name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(version);
        });
    });
}

/// 亮/暗/跟随系统三选一主题切换，返回是否发生变化。
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

/// 单个主题按钮，选中时高亮；点击后更新偏好并返回 true。
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

/// 安装内嵌 CJK 与系统 Emoji 字体到上下文。
fn install_cjk_fonts(ctx: &egui::Context) { ctx.set_fonts(openbox_font_definitions()); }

/// 构造字体定义：内嵌字体置首，系统 Emoji 依次追加。
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

/// 按平台候选路径安装系统 Emoji 字体，返回成功安装的字体名。
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

/// 安装候选路径中第一个可读的字体文件。
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

/// 统一设置间距、交互尺寸与文字样式。
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
