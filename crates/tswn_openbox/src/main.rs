#![cfg_attr(windows, windows_subsystem = "windows")]

mod backend;

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use backend::{BatchRateInput, CommonBenchOptions, OutputMode, PairInput, ProgressEvent};
use eframe::egui;

const SARASA_MONO_SC: &[u8] = include_bytes!("SarasaMonoSC-Regular.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    ToDiy,
    NamerPf,
    BatchRate,
    Pair,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::ToDiy => "to-diy",
            Self::NamerPf => "namer-pf",
            Self::BatchRate => "batch-rate",
            Self::Pair => "pair",
        }
    }
}

#[derive(Debug, Clone)]
struct TextSource {
    inline: String,
    from_file: bool,
    file_path: Option<PathBuf>,
    preview: String,
    error: Option<String>,
}

impl TextSource {
    fn inline(text: impl Into<String>) -> Self {
        Self {
            inline: text.into(),
            from_file: false,
            file_path: None,
            preview: String::new(),
            error: None,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, label: &str, id: &'static str, rows: usize) {
        ui.horizontal(|ui| {
            ui.label(label);
            let was_from_file = self.from_file;
            if ui.checkbox(&mut self.from_file, "从文件中读取").changed() {
                if self.from_file && !was_from_file {
                    self.pick_file();
                }
                if !self.from_file {
                    self.error = None;
                }
            }
            if self.from_file {
                if ui.button("选择文件").clicked() {
                    self.pick_file();
                }
            } else if ui.button("清空").clicked() {
                self.inline.clear();
            }
        });

        if self.from_file {
            let path = self
                .file_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "未选择文件".to_string());
            ui.label(path);
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::from_rgb(180, 40, 40), error);
            }
            let mut preview = self.preview.clone();
            egui::ScrollArea::vertical().id_salt(id).max_height(rows as f32 * 22.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut preview)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(rows)
                        .interactive(false),
                );
            });
        } else {
            multiline(ui, id, &mut self.inline, rows);
        }
    }

    fn read_all(&self) -> Result<String, String> {
        if !self.from_file {
            return Ok(self.inline.clone());
        }
        let path = self.file_path.as_ref().ok_or_else(|| "请选择输入文件。".to_string())?;
        read_text_file(path)
    }

    fn pick_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.file_path = Some(path);
            self.refresh_preview();
        }
    }

    fn refresh_preview(&mut self) {
        let Some(path) = &self.file_path else {
            self.preview.clear();
            self.error = None;
            return;
        };
        match read_text_file(path) {
            Ok(content) => {
                let mut preview = content.lines().take(10).collect::<Vec<_>>().join("\n");
                if content.lines().count() > 10 {
                    preview.push_str("\n...");
                }
                self.preview = preview;
                self.error = None;
            }
            Err(err) => {
                self.preview.clear();
                self.error = Some(err);
            }
        }
    }
}

struct OpenboxApp {
    tool: Tool,
    log: String,
    status: String,
    running: bool,
    done: usize,
    total: usize,
    started_at: Option<Instant>,
    rate_text: String,
    eta_text: String,
    rx: Option<Receiver<ProgressEvent>>,

    to_diy_names: TextSource,
    to_diy_old: bool,
    to_diy_details: bool,

    namer_pf_names: TextSource,
    namer_pf_count: usize,
    namer_pf_threads: usize,

    batch_targets: TextSource,
    batch_players: TextSource,
    batch_count: usize,
    batch_threads: usize,
    batch_keep_rq: bool,
    batch_verbose: bool,
    batch_double_plus: bool,
    batch_min_screen: String,
    batch_min_file: String,
    batch_output_file: Option<PathBuf>,
    batch_precision: usize,
    batch_output_mode: OutputMode,

    pair_targets: TextSource,
    pair_players: TextSource,
    pair_teammates: TextSource,
    pair_head: usize,
    pair_count: usize,
    pair_threads: usize,
    pair_keep_rq: bool,
    pair_verbose: bool,
    pair_min_screen: String,
    pair_min_file: String,
    pair_output_file: Option<PathBuf>,
    pair_precision: usize,
    pair_output_mode: OutputMode,
}

impl Default for OpenboxApp {
    fn default() -> Self {
        Self {
            tool: Tool::ToDiy,
            log: String::new(),
            status: "就绪".to_string(),
            running: false,
            done: 0,
            total: 0,
            started_at: None,
            rate_text: "--".to_string(),
            eta_text: "--".to_string(),
            rx: None,
            to_diy_names: TextSource::inline("mario@team+fire"),
            to_diy_old: false,
            to_diy_details: true,
            namer_pf_names: TextSource::inline("mario\nluigi+peach"),
            namer_pf_count: 1000,
            namer_pf_threads: 0,
            batch_targets: TextSource::inline("luigi\npeach"),
            batch_players: TextSource::inline("mario\nbowser"),
            batch_count: 1000,
            batch_threads: 0,
            batch_keep_rq: false,
            batch_verbose: false,
            batch_double_plus: false,
            batch_min_screen: String::new(),
            batch_min_file: String::new(),
            batch_output_file: None,
            batch_precision: 3,
            batch_output_mode: OutputMode::Log,
            pair_targets: TextSource::inline("luigi\npeach"),
            pair_players: TextSource::inline("mario\nbowser"),
            pair_teammates: TextSource::inline("yoshi\ntoad"),
            pair_head: 3,
            pair_count: 1000,
            pair_threads: 0,
            pair_keep_rq: false,
            pair_verbose: false,
            pair_min_screen: String::new(),
            pair_min_file: String::new(),
            pair_output_file: None,
            pair_precision: 3,
            pair_output_mode: OutputMode::Log,
        }
    }
}

impl eframe::App for OpenboxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_events(&ctx);
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("tswn openbox");
                ui.separator();
                for tool in [Tool::ToDiy, Tool::NamerPf, Tool::BatchRate, Tool::Pair] {
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

impl OpenboxApp {
    fn to_diy_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("to-diy");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.to_diy_old, "旧 +diy");
            ui.checkbox(&mut self.to_diy_details, "单名详情");
        });
        self.to_diy_names.ui(ui, "名字", "to_diy_names", 16);
        if ui.button("运行").clicked() {
            self.start_to_diy();
        }
    }

    fn namer_pf_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("namer-pf");
        bench_controls(ui, &mut self.namer_pf_count, &mut self.namer_pf_threads);
        self.namer_pf_names.ui(ui, "名字", "namer_pf_names", 14);
        if ui.button("运行").clicked() {
            self.start_namer_pf();
        }
    }

    fn batch_rate_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("batch-rate");
        bench_controls(ui, &mut self.batch_count, &mut self.batch_threads);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.batch_keep_rq, "keep rq");
            ui.checkbox(&mut self.batch_verbose, "详细");
            ui.checkbox(&mut self.batch_double_plus, "++ 分组");
        });
        output_controls(
            ui,
            &mut self.batch_output_mode,
            &mut self.batch_min_screen,
            &mut self.batch_min_file,
            &mut self.batch_precision,
            &mut self.batch_output_file,
        );
        ui.separator();
        self.batch_targets.ui(ui, "靶子列表", "batch_targets", 8);
        ui.separator();
        self.batch_players.ui(ui, "选手列表", "batch_players", 8);
        if ui.button("运行").clicked() {
            self.start_batch_rate();
        }
    }

    fn pair_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("pair");
        bench_controls(ui, &mut self.pair_count, &mut self.pair_threads);
        ui.horizontal(|ui| {
            ui.label("head");
            ui.add(egui::DragValue::new(&mut self.pair_head).range(1..=999).speed(1));
            ui.checkbox(&mut self.pair_keep_rq, "keep rq");
            ui.checkbox(&mut self.pair_verbose, "详细");
        });
        output_controls(
            ui,
            &mut self.pair_output_mode,
            &mut self.pair_min_screen,
            &mut self.pair_min_file,
            &mut self.pair_precision,
            &mut self.pair_output_file,
        );
        ui.separator();
        self.pair_targets.ui(ui, "靶子列表", "pair_targets", 6);
        ui.separator();
        self.pair_players.ui(ui, "选手列表", "pair_players", 6);
        ui.separator();
        self.pair_teammates.ui(ui, "队友列表", "pair_teammates", 6);
        if ui.button("运行").clicked() {
            self.start_pair();
        }
    }

    fn log_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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

    fn start_to_diy(&mut self) {
        let raw = match self.to_diy_names.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let old = self.to_diy_old;
        let details = self.to_diy_details;
        std::thread::spawn(move || {
            let result = backend::run_to_diy(&raw, old, details);
            let _ = tx.send(ProgressEvent::Done(result));
        });
    }

    fn start_namer_pf(&mut self) {
        let raw = match self.namer_pf_names.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let count = self.namer_pf_count.max(1);
        let threads = non_zero(self.namer_pf_threads);
        std::thread::spawn(move || {
            backend::run_namer_pf(&raw, count, threads, |event| {
                let _ = tx.send(event);
            });
        });
    }

    fn start_batch_rate(&mut self) {
        let target_text = match self.batch_targets.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let player_text = match self.batch_players.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let Some(output_file) = self.batch_output_file.clone() else {
            self.fail_before_start("请先选择输出文件。".to_string());
            return;
        };
        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let input = BatchRateInput {
            target_text,
            player_text,
            player_double_plus: self.batch_double_plus,
            output_mode: self.batch_output_mode,
            output_file,
            options: CommonBenchOptions {
                count: self.batch_count.max(1),
                threads: non_zero(self.batch_threads),
                keep_rq: self.batch_keep_rq,
                verbose: self.batch_verbose,
                min_screen: parse_optional_f64(&self.batch_min_screen),
                min_file: parse_optional_f64(&self.batch_min_file),
                wr_precision: self.batch_precision.min(9),
            },
        };
        std::thread::spawn(move || {
            backend::run_batch_rate(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    fn start_pair(&mut self) {
        let target_text = match self.pair_targets.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let player_text = match self.pair_players.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let teammate_text = match self.pair_teammates.read_all() {
            Ok(raw) => raw,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        let Some(output_file) = self.pair_output_file.clone() else {
            self.fail_before_start("请先选择输出文件。".to_string());
            return;
        };
        self.begin_task();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let input = PairInput {
            target_text,
            player_text,
            teammate_text,
            head: self.pair_head.max(1),
            output_mode: self.pair_output_mode,
            output_file,
            options: CommonBenchOptions {
                count: self.pair_count.max(1),
                threads: non_zero(self.pair_threads),
                keep_rq: self.pair_keep_rq,
                verbose: self.pair_verbose,
                min_screen: parse_optional_f64(&self.pair_min_screen),
                min_file: parse_optional_f64(&self.pair_min_file),
                wr_precision: self.pair_precision.min(9),
            },
        };
        std::thread::spawn(move || {
            backend::run_pair(input, |event| {
                let _ = tx.send(event);
            });
        });
    }

    fn begin_task(&mut self) {
        self.running = true;
        self.done = 0;
        self.total = 0;
        self.started_at = Some(Instant::now());
        self.rate_text = "--".to_string();
        self.eta_text = "--".to_string();
        self.log.clear();
        self.status = "运行中".to_string();
    }

    fn fail_before_start(&mut self, err: String) {
        self.status = "失败".to_string();
        self.log = err;
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.rx.take() {
            let mut keep_rx = true;
            while let Ok(event) = rx.try_recv() {
                match event {
                    ProgressEvent::Log(line) => {
                        self.log.push_str(&line);
                        if !self.log.ends_with('\n') {
                            self.log.push('\n');
                        }
                    }
                    ProgressEvent::Progress { done, total } => {
                        self.done = done;
                        self.total = total;
                        self.status = "运行中".to_string();
                        self.update_progress_stats();
                    }
                    ProgressEvent::Done(result) => {
                        self.running = false;
                        self.status = match &result {
                            Ok(_) => "完成".to_string(),
                            Err(_) => "失败".to_string(),
                        };
                        match result {
                            Ok(output) => {
                                if !output.is_empty() {
                                    self.log.push_str(&output);
                                    self.log.push('\n');
                                }
                            }
                            Err(err) => {
                                self.log.push_str(&err);
                                self.log.push('\n');
                            }
                        }
                        self.update_progress_stats();
                        keep_rx = false;
                    }
                }
                ctx.request_repaint();
            }
            if keep_rx {
                self.rx = Some(rx);
            }
        }
    }

    fn update_progress_stats(&mut self) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let elapsed = started_at.elapsed().as_secs_f64();
        if elapsed <= 0.0 || self.done == 0 {
            self.rate_text = "--".to_string();
            self.eta_text = "--".to_string();
            return;
        }
        let rate = self.done as f64 / elapsed;
        self.rate_text = format!("{rate:.2} 项/s");
        let remaining = self.total.saturating_sub(self.done) as f64;
        self.eta_text = if rate > 0.0 {
            format_duration(remaining / rate)
        } else {
            "--".to_string()
        };
    }
}

fn bench_controls(ui: &mut egui::Ui, count: &mut usize, threads: &mut usize) {
    ui.horizontal(|ui| {
        ui.label("场数");
        ui.add(egui::DragValue::new(count).range(1..=10_000_000).speed(100));
        ui.label("线程");
        ui.add(egui::DragValue::new(threads).range(0..=256).speed(1));
    });
}

fn output_controls(
    ui: &mut egui::Ui,
    mode: &mut OutputMode,
    min_screen: &mut String,
    min_file: &mut String,
    precision: &mut usize,
    output_file: &mut Option<PathBuf>,
) {
    ui.horizontal(|ui| {
        ui.label("文件格式");
        ui.radio_value(mode, OutputMode::Log, "分数 名字");
        ui.radio_value(mode, OutputMode::Jsonl, "JSONL (--log)");
        ui.radio_value(mode, OutputMode::Pure, "名字 (--pure)");
    });
    ui.horizontal(|ui| {
        ui.label("日志阈值");
        ui.add(egui::TextEdit::singleline(min_screen).desired_width(72.0));
        ui.label("文件阈值");
        ui.add(egui::TextEdit::singleline(min_file).desired_width(72.0));
        ui.label("小数");
        ui.add(egui::DragValue::new(precision).range(0..=9).speed(1));
    });
    ui.horizontal(|ui| {
        if ui.button("选择输出文件").clicked()
            && let Some(path) = rfd::FileDialog::new().set_file_name("tswn-openbox-output.txt").save_file()
        {
            *output_file = Some(path);
        }
        let label = output_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未选择输出文件".to_string());
        ui.label(label);
    });
}

fn multiline(ui: &mut egui::Ui, id: &'static str, text: &mut String, rows: usize) {
    egui::ScrollArea::vertical().id_salt(id).max_height(rows as f32 * 22.0).show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(text)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(rows),
        );
    });
}

fn read_text_file(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|content| content.trim_start_matches('\u{feff}').to_string())
        .map_err(|err| format!("读取文件失败: {}: {err}", path.display()))
}

fn non_zero(value: usize) -> Option<usize> { if value == 0 { None } else { Some(value) } }

fn parse_optional_f64(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() { None } else { trimmed.parse().ok() }
}

fn format_duration(secs: f64) -> String {
    if secs.is_nan() || secs.is_infinite() || secs < 0.0 {
        return "--".to_string();
    }
    let seconds = secs.round() as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m{}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{}m{}s", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
    }
}

fn main() -> eframe::Result<()> {
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
