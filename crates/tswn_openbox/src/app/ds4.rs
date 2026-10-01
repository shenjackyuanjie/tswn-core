//! DS4 工作目录、配置编辑与后台流程入口。

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tswn_ds4::{Config, RunStage};
use tswn_openbox::backend::ProgressEvent;
use tswn_openbox::backend::live::{ResultEntry, ResultFinish, ResultKind, ResultUpdate};

use super::ds4_help as help;
use super::help::{HelpTopic, help_icon};
use super::state::OpenboxApp;
use super::style::Palette;
use super::view::tool_header;

pub struct Ds4State {
    pub root: String,
    pub document: Value,
    loaded_root: Option<PathBuf>,
    pub message: String,
}

impl Default for Ds4State {
    fn default() -> Self {
        Self {
            root: String::new(),
            document: serde_json::from_str(tswn_ds4::DEFAULT_CONFIG_JSON).expect("内置 DS4 配置"),
            loaded_root: None,
            message: String::new(),
        }
    }
}

impl Ds4State {
    fn root_path(&self) -> Result<PathBuf, String> {
        if self.root.trim().is_empty() {
            return Err("请先选择 DS4 工作目录。".into());
        }
        std::path::absolute(self.root.trim()).map_err(|err| err.to_string())
    }

    pub fn load(&mut self) -> Result<(), String> {
        let root = self.root_path()?;
        let path = root.join("config.json");
        let document: Value = match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(raw.trim_start_matches('\u{feff}')).map_err(|err| format!("配置格式错误：{err}"))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                serde_json::from_str(tswn_ds4::DEFAULT_CONFIG_JSON).expect("内置 DS4 配置")
            }
            Err(err) => return Err(format!("读取配置失败：{err}")),
        };
        if !document.is_object() {
            return Err("config.json 必须是 JSON 对象。".into());
        }
        for key in [
            "bc", "fz", "wc", "fs", "pj", "two_fc", "two_wc", "two_rh", "qp", "qd", "pp", "pd", "cqd", "abcp", "three",
        ] {
            if !document[key].is_null() && !document[key].is_object() {
                return Err(format!("配置 {key} 必须是 JSON 对象。"));
            }
        }
        for key in ["ffc", "wfc", "fwc", "wwc", "rwc", "rrh", "prh", "wrh"] {
            if !document["three"][key].is_null() && !document["three"][key].is_object() {
                return Err(format!("配置 three.{key} 必须是 JSON 对象。"));
            }
        }
        // 保留尚未填写队伍名的新配置，以及界面暂未识别的扩展字段。
        self.document = document;
        self.loaded_root = Some(root);
        self.message = if path.is_file() {
            "已读取工作目录配置。"
        } else {
            "新工作目录：已载入默认配置，请填写队伍名。"
        }
        .into();
        Ok(())
    }

    fn save(&mut self) -> Result<(PathBuf, Config), String> {
        let root = self.root_path()?;
        if root.join("config.json").exists() && self.loaded_root.as_ref() != Some(&root) {
            return Err("该目录已有配置，请先点击“读取配置”，避免覆盖原设置。".into());
        }
        let raw = serde_json::to_string_pretty(&self.document).map_err(|err| err.to_string())?;
        let config = Config::from_json(&raw).map_err(|err| err.to_string())?;
        fs::create_dir_all(root.join("input")).map_err(|err| err.to_string())?;
        fs::write(root.join("config.json"), format!("{raw}\n")).map_err(|err| format!("保存配置失败：{err}"))?;
        self.loaded_root = Some(root.clone());
        self.message = "已保存 config.json。".into();
        Ok((root, config))
    }
}

impl OpenboxApp {
    pub(crate) fn ds4_workflow_ui(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::of(ui);
        let requested_help = Cell::new(None);
        colored_section(ui, "从名字到候选组合", palette.info, |ui| {
            ui.horizontal_wrapped(|ui| {
                badge(ui, "1 单人评分", palette.purple);
                badge(ui, "2 二人配对", palette.warning);
                badge(ui, "3 模型预测", palette.info);
                badge(ui, "4 实战复核", palette.success);
                help_icon(ui, HelpTopic::Ds4Workflow, &requested_help);
            });
            ui.label("悬停控件查看提示 · 点击 ⓘ 固定说明").on_hover_text(help::WORKFLOW);
        });
        if let Some(topic) = requested_help.get() {
            self.active_help = Some(topic);
        }
    }

    pub(crate) fn ds4_ui(&mut self, ui: &mut egui::Ui) {
        tool_header(ui, "DS4", "批量评分、增量配对与实战筛选", &mut self.more_settings_open);
        let palette = Palette::of(ui);
        let requested_help = Cell::new(None);
        ui.add_enabled_ui(!self.running, |ui| {
            colored_section(ui, "工作目录", palette.info, |ui| {
                ui.horizontal(|ui| {
                    ui.label("配置与历史记录").on_hover_text(help::DIRECTORY);
                    help_icon(ui, HelpTopic::Ds4Storage, &requested_help);
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ds4.root)
                            .desired_width(290.0)
                            .hint_text("选择 Data_Structure4.0 工作目录"),
                    )
                    .on_hover_text(help::DIRECTORY);
                    if ui
                        .button("浏览…")
                        .on_hover_text("选择目录并自动读取配置；无配置时载入默认模板。")
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.ds4.root = path.display().to_string();
                        if let Err(err) = self.ds4.load() {
                            self.ds4.message = err;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("读取配置")
                        .on_hover_text("用该目录 config.json 替换当前未保存设置；无配置时载入默认模板。")
                        .clicked()
                        && let Err(err) = self.ds4.load()
                    {
                        self.ds4.message = err;
                    }
                    if ui
                        .button("保存配置")
                        .on_hover_text("校验设置、创建 input/ 并写入 config.json，保留未识别的扩展字段。")
                        .clicked()
                        && let Err(err) = self.ds4.save()
                    {
                        self.ds4.message = err;
                    }
                    let root = self.ds4.root_path();
                    if ui
                        .add_enabled(root.as_ref().is_ok_and(|path| path.is_dir()), egui::Button::new("打开目录"))
                        .on_hover_text(help::STORAGE)
                        .clicked()
                        && let Ok(root) = root
                        && let Err(err) = open_directory(&root)
                    {
                        self.ds4.message = err;
                    }
                });
                if !self.ds4.message.is_empty() {
                    ui.label(&self.ds4.message);
                }
            });
            colored_section(ui, "基础设置", palette.purple, |ui| {
                let mut team = self.ds4.document["team_name"].as_str().unwrap_or_default().to_owned();
                ui.horizontal(|ui| {
                    ui.label("队伍名").on_hover_text(help::TEAM);
                    if ui
                        .add(egui::TextEdit::singleline(&mut team).hint_text("输入中 @ 后的队伍名"))
                        .on_hover_text(help::TEAM)
                        .changed()
                    {
                        self.ds4.document["team_name"] = team.into();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("线程数").on_hover_text(help::THREADS);
                    integer(ui, &mut self.ds4.document["thread_number"], 1..=1024).on_hover_text(help::THREADS);
                    toggle(ui, &mut self.ds4.document["run_dup"], "历史去重").on_hover_text(help::DEDUP);
                });
            });
            colored_section(ui, "二人配对", palette.warning, |ui| {
                ui.horizontal(|ui| {
                    ui.label("类型与保留阈值").on_hover_text(help::THRESHOLDS);
                    help_icon(ui, HelpTopic::Ds4Thresholds, &requested_help);
                });
                for (key, label) in [
                    ("two_fc", "FC · FZ × BC"),
                    ("two_wc", "WC · WC × WC"),
                    ("two_rh", "RH · FS × PJ"),
                ] {
                    threshold(ui, &mut self.ds4.document[key], label, help::pair(key));
                }
            });
            colored_section(ui, "后续筛选", palette.emphasis, |ui| {
                ui.horizontal(|ui| {
                    threshold(ui, &mut self.ds4.document["abcp"], "ABCP5 二人预测", help::ABCP);
                    help_icon(ui, HelpTopic::Ds4Prediction, &requested_help);
                });
                ui.horizontal(|ui| {
                    toggle(ui, &mut self.ds4.document["get_3"], "生成三人组").on_hover_text(help::THREE);
                    help_icon(ui, HelpTopic::Ds4Three, &requested_help);
                });
                ui.horizontal(|ui| {
                    toggle(ui, &mut self.ds4.document["openbox_cqp"], "Openbox 三轮实战筛选").on_hover_text(help::OPENBOX);
                    help_icon(ui, HelpTopic::Ds4Battle, &requested_help);
                });
                ui.horizontal(|ui| {
                    ui.label("三人 / 实战基础阈值").on_hover_text(help::BASE);
                    integer(ui, &mut self.ds4.document["three"]["pair_abcp_sieve"], 1..=10000).on_hover_text(help::BASE);
                });
            });
        });
        colored_section(ui, "目录与结果", palette.success, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (folder, label) in [
                    ("input", "输入"),
                    ("out", "二人结果"),
                    ("3ren", "三人结果"),
                    ("file", "历史 / 实战结果"),
                ] {
                    let path = self.ds4.root_path().map(|root| root.join(folder));
                    if ui
                        .add_enabled(path.as_ref().is_ok_and(|path| path.is_dir()), egui::Button::new(label))
                        .on_hover_text(if folder == "3ren" {
                            help::THREE_DETAILS
                        } else {
                            help::STORAGE
                        })
                        .clicked()
                        && let Ok(path) = path
                        && let Err(err) = open_directory(&path)
                    {
                        self.ds4.message = err;
                    }
                }
            });
            ui.label("整轮执行 · 完成归档后再关闭").on_hover_text(help::RUN);
        });
        if let Some(topic) = requested_help.get() {
            self.active_help = Some(topic);
        }
    }

    pub(crate) fn ds4_more_settings(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::of(ui);
        let requested_help = Cell::new(None);
        ui.horizontal(|ui| {
            ui.label("悬停参数查看筛选规则").on_hover_text(help::THRESHOLDS);
            help_icon(ui, HelpTopic::Ds4Thresholds, &requested_help);
        });
        colored_section(ui, "单人基础分类", palette.purple, |ui| {
            for key in ["bc", "fz", "wc", "fs", "pj"] {
                ui.horizontal(|ui| {
                    ui.label(key.to_uppercase()).on_hover_text(help::SINGLE);
                    ui.label("评分阈值").on_hover_text(help::SINGLE);
                    integer(ui, &mut self.ds4.document[key]["sieve"], i32::MIN..=i32::MAX).on_hover_text(help::SINGLE);
                    ui.label("潜力阈值").on_hover_text(help::SINGLE);
                    integer(ui, &mut self.ds4.document[key]["ptt_sieve"], i32::MIN..=i32::MAX).on_hover_text(help::SINGLE);
                });
            }
        });
        colored_section(ui, "SP1 单项评分", palette.info, |ui| {
            for key in ["qp", "qd", "pp", "pd", "cqd"] {
                threshold(ui, &mut self.ds4.document[key], &key.to_uppercase(), help::SP1);
                if ["qp", "qd", "pp"].contains(&key) {
                    ui.horizontal(|ui| {
                        ui.label("技能容差").on_hover_text(help::SKILL);
                        integer(ui, &mut self.ds4.document[key]["skill_sieve"], i32::MIN..=i32::MAX).on_hover_text(help::SKILL);
                    });
                }
            }
        });
        colored_section(ui, "三人类型与阈值", palette.warning, |ui| {
            help_icon(ui, HelpTopic::Ds4Three, &requested_help);
            for key in ["ffc", "wfc", "fwc", "wwc", "rwc", "rrh", "prh", "wrh"] {
                threshold(ui, &mut self.ds4.document["three"][key], &key.to_uppercase(), &help::three(key));
            }
        });
        colored_section(ui, "归档设置", palette.success, |ui| {
            toggle(ui, &mut self.ds4.document["copy_pf_to_out"], "单项评分写入 out/").on_hover_text(help::COPY_SCORES);
            toggle(ui, &mut self.ds4.document["copy_to_new"], "本轮分类写入 new/").on_hover_text(help::COPY_NEW);
        });
        if let Some(topic) = requested_help.get() {
            self.active_help = Some(topic);
        }
    }

    pub(crate) fn start_ds4(&mut self) {
        let (root, config) = match self.ds4.save() {
            Ok(value) => value,
            Err(err) => {
                self.fail_before_start(err);
                return;
            }
        };
        self.begin_task();
        self.cancel_token = None;
        self.spawn_worker(move |feed| {
            feed.progress(ProgressEvent::Log(format!("DS4 工作目录：{}", root.display())));
            let result = tswn_ds4::run_with_progress(&root, &config, |stage| {
                feed.progress(ProgressEvent::Progress {
                    done: stage.completed(),
                    total: RunStage::TOTAL,
                });
                let skipped = match stage {
                    RunStage::Three => !config.get_3,
                    RunStage::Abcp => !config.abcp.enabled,
                    RunStage::Openbox => !config.openbox_cqp,
                    _ => false,
                };
                let label = if skipped { "跳过" } else { "阶段" };
                feed.progress(ProgressEvent::Log(format!("[{label}] {}", stage.label())));
            });
            let result = result
                .map(|report| {
                    let counts = [
                        ("新增输入", report.stage1.dedup.remaining),
                        ("FC 二人组", report.pair.fc),
                        ("WC 二人组", report.pair.wc),
                        ("RH 二人组", report.pair.rh),
                        ("三人组", report.three.iter().sum()),
                        ("ABCP5 新结果", report.abcp),
                    ];
                    let mut update = ResultUpdate::new(0, "DS4 本轮统计", ResultKind::Scores, 0);
                    for (index, (label, count)) in counts.into_iter().enumerate() {
                        update.entries.push(ResultEntry::number(index, label.into(), count as f64));
                    }
                    update.finish = Some(ResultFinish {
                        score: None,
                        visible: true,
                        highlight: false,
                    });
                    feed.result(update);
                    format!("DS4 处理完成，结果已保存到 {}。", root.display())
                })
                .map_err(|err| err.to_string());
            feed.progress(ProgressEvent::Done(result));
        });
    }
}

fn colored_section(ui: &mut egui::Ui, title: &str, accent: egui::Color32, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(accent.gamma_multiply(0.055))
        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.4)))
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(title).strong().size(15.0).color(accent));
            ui.add_space(5.0);
            content(ui);
        });
    ui.add_space(7.0);
}

fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).strong());
        });
}

fn toggle(ui: &mut egui::Ui, value: &mut Value, label: &str) -> egui::Response {
    let mut enabled = value.as_i64().unwrap_or(0) != 0;
    let response = ui.checkbox(&mut enabled, label);
    if response.changed() {
        *value = Value::from(i32::from(enabled));
    }
    response
}

fn integer(ui: &mut egui::Ui, value: &mut Value, range: std::ops::RangeInclusive<i32>) -> egui::Response {
    let mut number = value.as_i64().and_then(|value| i32::try_from(value).ok()).unwrap_or(0);
    let response = ui.add(egui::DragValue::new(&mut number).range(range));
    if response.changed() {
        *value = number.into();
    }
    response
}

fn threshold(ui: &mut egui::Ui, value: &mut Value, label: &str, help: &str) {
    ui.horizontal(|ui| {
        toggle(ui, &mut value["enable"], label).on_hover_text(help);
        ui.label("阈值").on_hover_text(help);
        integer(ui, &mut value["sieve"], i32::MIN..=i32::MAX).on_hover_text(help);
    });
}

fn open_directory(path: &Path) -> Result<(), String> {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("打开目录失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::Tool;

    #[test]
    fn config_loading_preserves_custom_fields_and_refuses_unloaded_overwrite() {
        let root = std::env::temp_dir().join(format!("ds4-ui-config-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let raw = "{\"team_name\":\"队伍\",\"extension\":{\"custom\":42}}";
        fs::write(root.join("config.json"), raw).unwrap();
        let mut state = Ds4State {
            root: root.display().to_string(),
            ..Default::default()
        };
        assert!(state.save().is_err());
        assert_eq!(fs::read_to_string(root.join("config.json")).unwrap(), raw);
        state.load().unwrap();
        state.document["thread_number"] = 4.into();
        let (_, config) = state.save().unwrap();
        assert_eq!(config.threads, 4);
        let saved: Value = serde_json::from_str(&fs::read_to_string(root.join("config.json")).unwrap()).unwrap();
        assert_eq!(saved["extension"]["custom"], 42);
        fs::write(root.join("config.json"), "{\"team_name\":\"队伍\",\"three\":false}").unwrap();
        assert!(state.load().is_err());
        assert_eq!(state.document["extension"]["custom"], 42);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_runs_ds4_and_reports_completed_statistics() {
        let root = std::env::temp_dir().join(format!("ds4-ui-worker-{}", std::process::id()));
        fs::create_dir_all(root.join("input")).unwrap();
        fs::write(root.join("input/names.txt"), "alpha@teamA\nbeta@teamA\n").unwrap();
        let mut app = OpenboxApp {
            tool: super::super::state::Tool::Ds4,
            ..Default::default()
        };
        app.ds4.root = root.display().to_string();
        app.ds4.document = serde_json::json!({"team_name":"teamA", "run_dup":1});
        app.start_ds4();
        assert!(app.running);
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while app.running && std::time::Instant::now() < deadline {
            app.poll_events(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!app.running, "后台任务应完成");
        let log = app.logs[Tool::Ds4 as usize].copy_text();
        assert_eq!(app.status, "完成", "{log}");
        assert_eq!(app.done, RunStage::TOTAL);
        assert!(log.contains("新增输入: 2"));
        assert_eq!(fs::read_to_string(root.join("file/old.txt")).unwrap().lines().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
