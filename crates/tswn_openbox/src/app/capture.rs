//! 可选的原生 UI 截图校验驱动，使用本应用真实任务和渲染回传，不操作桌面鼠标。
//!
//! 仅启用 ui_capture 时编译；未指定 capture_dir 时仍正常启动 GUI。capture_dir
//! 指定 PNG 输出目录，light/ds4/diy/align 选择既有验收场景，不改变正式工具的外观。
//! egui 的一次性截图回调只移交自有图像并请求重绘，下一次 UI 更新再保存和切换阶段，
//! 保持一项待回传请求，避免回调直接借用应用状态。
//! 驱动在 60 秒内未收到完整渲染回传时记录失败并关闭窗口；保存错误同样停止任务。
//! 需要可用的原生渲染环境，只验证本窗口内置场景，不是跨平台像素一致性或其他窗口截图。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;

use super::results::{ColumnAlign, ViewMode};
use super::source::TextSource;
use super::state::AccuracyPreset;
use super::{OpenboxApp, Tool, configure_ui_style, install_cjk_fonts};

#[derive(Parser)]
#[command(about = "Openbox 可选界面截图校验；不传参数时正常启动")]
struct CaptureArgs {
    /// 使用内置 pair 样例生成卡片、表格、文本、停止状态四张 PNG，然后退出。
    #[arg(long)]
    capture_dir: Option<PathBuf>,
    /// 使用浅色主题检查对比度；默认使用深色主题。
    #[arg(long)]
    capture_light: bool,
    /// 使用 DS4 工作目录样例，生成主页面、更多设置与详细帮助三张截图。
    #[arg(long)]
    capture_ds4: bool,
    /// 使用 DIY 组合样例校验属性、技能详情及旧版纯文本输出。
    #[arg(long, conflicts_with = "capture_ds4")]
    capture_diy: bool,
    /// 使用 base 样例校验新页面的输入与纯文本输出。
    #[arg(long, conflicts_with_all = ["capture_ds4", "capture_diy"])]
    capture_base: bool,
    /// 指定非纯文本截图的对齐方式，便于实际审图。
    #[arg(long, value_parser = ["left", "center", "right"], default_value = "left")]
    capture_align: String,
}

pub(crate) fn run_if_requested() -> Option<eframe::Result<()>> {
    let args = CaptureArgs::parse();
    let align = match args.capture_align.as_str() {
        "center" => ColumnAlign::Center,
        "right" => ColumnAlign::Right,
        _ => ColumnAlign::Left,
    };
    args.capture_dir.map(|directory| {
        run_capture(
            directory,
            args.capture_light,
            args.capture_ds4,
            args.capture_diy,
            args.capture_base,
            align,
        )
    })
}

struct CaptureApp {
    app: OpenboxApp,
    directory: PathBuf,
    stage: usize,
    pending: bool,
    screenshot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    frame: usize,
    next_frame: usize,
    started: Instant,
    failure: Arc<Mutex<Option<String>>>,
    ds4: bool,
    base: bool,
}

impl CaptureApp {
    fn fail(&mut self, ctx: &egui::Context, message: String) {
        *self.failure.lock().unwrap() = Some(message);
        self.app.stop_current_task();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// 本次驱动要生成几张截图：base 页只有一张，DS4 有三张，其余四种状态。
    fn stage_count(&self) -> usize {
        if self.ds4 {
            3
        } else if self.base {
            1
        } else {
            4
        }
    }

    fn save_screenshot(&self, image: &egui::ColorImage) -> Result<(), String> {
        let name = if self.ds4 {
            ["ds4", "ds4-settings", "ds4-help"][self.stage]
        } else if self.base {
            "base"
        } else {
            ["cards", "table", "text", "stopped"][self.stage]
        };
        let pixels = image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect::<Vec<_>>();
        image::save_buffer_with_format(
            self.directory.join(format!("{name}.png")),
            &pixels,
            image.size[0] as u32,
            image.size[1] as u32,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|err| format!("保存 {name} 截图失败：{err}"))
    }
}

impl eframe::App for CaptureApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.frame += 1;
        let ctx = ui.ctx().clone();
        if self.stage >= self.stage_count() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.started.elapsed() > Duration::from_secs(60) {
            self.fail(&ctx, "截图校验超时：60 秒内未完成渲染回传。".into());
            return;
        }
        // 渲染回调只交还图像所有权；保存与阶段推进留在下一次 UI 更新中。
        let screenshot = self.screenshot.lock().unwrap().take();
        if let Some(image) = screenshot.filter(|_| self.pending) {
            if let Err(err) = self.save_screenshot(&image) {
                self.fail(&ctx, err);
                return;
            }
            self.stage += 1;
            self.pending = false;
            // 模式切换后先完整绘制一帧，下一帧才请求该视图的截图。
            self.next_frame = self.frame + 2;
            if self.ds4 {
                self.app.more_settings_open = self.stage == 1;
                self.app.active_help = (self.stage == 2).then_some(super::help::HelpTopic::Ds4Battle);
            } else if !self.base {
                // base 只有一页，下一帧会因 stage 到达上限而退出，不必切视图。
                let page = self.app.tool as usize;
                match self.stage {
                    1 => self.app.views[page].mode = ViewMode::Table,
                    2 => self.app.views[page].mode = ViewMode::Text,
                    3 => {
                        self.app.views[page].mode = ViewMode::Cards;
                        self.app.stop_current_task();
                    }
                    _ => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    }
                }
            }
        }
        self.app.ui(ui, frame);
        let page = self.app.tool as usize;
        if self.app.status == "失败" {
            self.fail(&ctx, format!("截图样例执行失败：{}", self.app.logs[page].copy_text()));
            return;
        }
        if !self.pending
            && self.frame >= self.next_frame
            && self.app.logs[page].len() > 3
            && (self.stage != 3 || !self.app.running)
            && (!self.ds4 || !self.app.running)
            && (!self.base || !self.app.running)
        {
            self.pending = true;
            let screenshot = self.screenshot.clone();
            let repaint = ctx.clone();
            ctx.request_screenshot(move |image| {
                *screenshot.lock().unwrap() = Some(image);
                repaint.request_repaint();
            });
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn run_capture(directory: PathBuf, light: bool, ds4: bool, diy: bool, base: bool, align: ColumnAlign) -> eframe::Result<()> {
    std::fs::create_dir_all(&directory).map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;
    if ds4 {
        let root = directory.join("workspace");
        std::fs::create_dir_all(root.join("input")).map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;
        std::fs::write(
            root.join("input/names.txt"),
            "alpha@teamA\nbeta@teamA\ngamma@teamA\ndelta@teamA\n",
        )
        .map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;
    }
    let failure = Arc::new(Mutex::new(None));
    let app_failure = failure.clone();
    eframe::run_native(
        "Openbox 界面截图校验",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 780.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            install_cjk_fonts(&cc.egui_ctx);
            configure_ui_style(&cc.egui_ctx);
            let mut app = OpenboxApp {
                theme_preference: if light {
                    egui::ThemePreference::Light
                } else {
                    egui::ThemePreference::Dark
                },
                tool: Tool::Pair,
                ..Default::default()
            };
            app.pair.manual_targets = true;
            app.pair.manual_teammates = true;
            app.pair.players = TextSource::inline("mario\nbowser\nluigi\npeach\nyoshi\ntoad\nwario\nwaluigi");
            app.pair.teammates = TextSource::inline("alpha\nbeta\ngamma\ndelta");
            app.pair.targets = TextSource::inline((0..10).map(|i| format!("target{i}")).collect::<Vec<_>>().join("\n"));
            // 截图关注布局而非评分精度，避免等待旧格式完整文本时超过驱动时限。
            app.pair.accuracy = AccuracyPreset::One;
            app.pair.auto_threads = false;
            app.pair.threads = 4;
            app.views[app.tool as usize].card_align = align;
            app.views[app.tool as usize].column_alignments = [align; 7];
            if ds4 {
                app.tool = Tool::Ds4;
                app.views[app.tool as usize].mode = ViewMode::Text;
                app.ds4.root = directory.join("workspace").display().to_string();
                app.ds4.load().map_err(std::io::Error::other)?;
                app.ds4.document["team_name"] = "teamA".into();
                app.ds4.document["thread_number"] = 2.into();
                for key in ["get_3", "openbox_cqp"] {
                    app.ds4.document[key] = 0.into();
                }
                app.ds4.document["abcp"]["enable"] = 0.into();
                for key in ["bc", "fz", "wc", "fs", "pj"] {
                    app.ds4.document[key]["sieve"] = (-10000).into();
                    app.ds4.document[key]["ptt_sieve"] = (-10000).into();
                }
                for key in ["two_fc", "two_wc", "two_rh"] {
                    app.ds4.document[key]["sieve"] = (-10000).into();
                }
                app.start_ds4();
            } else if diy {
                app.tool = Tool::ToDiy;
                app.views[app.tool as usize].follow = false;
                app.to_diy.names = TextSource::inline("1@team+2@team\nmario\nluigi");
                app.start_to_diy();
            } else if base {
                app.tool = Tool::Base;
                // 用参考样例：它同时带「末尾主动」与「末尾座位加成」，能核对两条附注的缩进。
                app.base.name = "霸绩往官脸瞒烤@Hell".to_string();
                app.start_base();
            } else {
                app.start_pair();
            }
            Ok(Box::new(CaptureApp {
                app,
                directory,
                stage: 0,
                pending: false,
                screenshot: Arc::default(),
                frame: 0,
                next_frame: 4,
                started: Instant::now(),
                failure: app_failure,
                ds4,
                base,
            }))
        }),
    )?;
    if let Some(error) = failure.lock().unwrap().take() {
        return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(error))));
    }
    Ok(())
}
