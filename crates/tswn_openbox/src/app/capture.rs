//! 可选的原生 UI 截图校验驱动。使用真实任务和渲染回传，不操作桌面鼠标或截取其他窗口。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;

use super::results::ViewMode;
use super::source::TextSource;
use super::state::AccuracyPreset;
use super::{OpenboxApp, Tool, configure_ui_style, install_cjk_fonts};

#[derive(Parser)]
#[command(about = "Openbox 可选界面截图校验；不传参数时正常启动")]
struct CaptureArgs {
    /// 使用内置 pair 样例生成卡片、表格、文本、停止状态四张 PNG，然后退出。
    #[arg(long)]
    capture_dir: Option<PathBuf>,
}

pub(crate) fn run_if_requested() -> Option<eframe::Result<()>> { CaptureArgs::parse().capture_dir.map(run_capture) }

struct CaptureApp {
    app: OpenboxApp,
    directory: PathBuf,
    stage: usize,
    pending: bool,
    frame: usize,
    next_frame: usize,
    started: Instant,
    failure: Arc<Mutex<Option<String>>>,
}

impl CaptureApp {
    fn fail(&mut self, ctx: &egui::Context, message: String) {
        *self.failure.lock().unwrap() = Some(message);
        self.app.stop_current_task();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn save_screenshot(&self, image: &egui::ColorImage) -> Result<(), String> {
        let name = ["cards", "table", "text", "stopped"][self.stage];
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
        if self.stage >= 4 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.started.elapsed() > Duration::from_secs(60) {
            self.fail(&ctx, "截图校验超时：60 秒内未完成渲染回传。".into());
            return;
        }
        for event in ctx.input(|input| input.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if !self.pending {
                    continue;
                }
                if let Err(err) = self.save_screenshot(&image) {
                    self.fail(&ctx, err);
                    return;
                }
                self.stage += 1;
                self.pending = false;
                // 模式切换后先完整绘制一帧，下一帧才请求该视图的截图。
                self.next_frame = self.frame + 2;
                match self.stage {
                    1 => self.app.results.mode = ViewMode::Table,
                    2 => self.app.results.mode = ViewMode::Text,
                    3 => {
                        self.app.results.mode = ViewMode::Cards;
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
        if self.app.status == "失败" {
            self.fail(&ctx, format!("截图样例执行失败：{}", self.app.log.copy_text()));
            return;
        }
        if !self.pending && self.frame >= self.next_frame && self.app.log.len() > 3 && (self.stage != 3 || !self.app.running) {
            self.pending = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn run_capture(directory: PathBuf) -> eframe::Result<()> {
    std::fs::create_dir_all(&directory).map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;
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
                theme_preference: egui::ThemePreference::Dark,
                tool: Tool::Pair,
                ..Default::default()
            };
            app.pair.manual_targets = true;
            app.pair.manual_teammates = true;
            app.pair.players = TextSource::inline("mario\nbowser\nluigi\npeach\nyoshi\ntoad\nwario\nwaluigi");
            app.pair.teammates = TextSource::inline("alpha\nbeta\ngamma\ndelta");
            app.pair.targets = TextSource::inline((0..10).map(|i| format!("target{i}")).collect::<Vec<_>>().join("\n"));
            app.pair.accuracy = AccuracyPreset::Hundred;
            app.pair.auto_threads = false;
            app.pair.threads = 4;
            app.start_pair();
            Ok(Box::new(CaptureApp {
                app,
                directory,
                stage: 0,
                pending: false,
                frame: 0,
                next_frame: 4,
                started: Instant::now(),
                failure: app_failure,
            }))
        }),
    )?;
    if let Some(error) = failure.lock().unwrap().take() {
        return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(error))));
    }
    Ok(())
}
