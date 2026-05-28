//! Preset loading from `setting/settings.toml`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const SETTING_DIR_NAME: &str = "setting";
const SETTINGS_FILE_NAME: &str = "settings.toml";
const DEFAULT_SETTINGS_TOML: &str = include_str!("../../assets/settings.toml");
const DEFAULT_SETTING_FILES: &[(&str, &str)] = &[
    ("targets/target1.txt", include_str!("../../assets/targets/target1.txt")),
    ("targets/target2.txt", include_str!("../../assets/targets/target2.txt")),
    ("targets/target3.txt", include_str!("../../assets/targets/target3.txt")),
    (
        "teammates/teammate_fz.txt",
        include_str!("../../assets/teammates/teammate_fz.txt"),
    ),
    (
        "teammates/teammate_bc.txt",
        include_str!("../../assets/teammates/teammate_bc.txt"),
    ),
    (
        "teammates/teammate_wc.txt",
        include_str!("../../assets/teammates/teammate_wc.txt"),
    ),
    (
        "teammates/teammate_pj.txt",
        include_str!("../../assets/teammates/teammate_pj.txt"),
    ),
    (
        "teammates/teammate_fs.txt",
        include_str!("../../assets/teammates/teammate_fs.txt"),
    ),
];

#[derive(Debug, Clone)]
pub struct TargetPreset {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TargetPresetState {
    pub items: Vec<TargetPreset>,
    pub selected_id: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TeammatePreset {
    pub head: usize,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TeammatePresetState {
    pub items: Vec<TeammatePreset>,
    pub selected_index: Option<usize>,
    pub error: Option<String>,
}

impl TargetPresetState {
    pub fn load() -> Self { Self::load_with_preferred_id(None) }

    pub fn load_with_preferred_id(preferred_id: Option<u64>) -> Self {
        match load_setting_file().map(load_target_presets) {
            Ok(items) => {
                let selected_id = preferred_id
                    .filter(|id| items.iter().any(|item| item.id == *id))
                    .or_else(|| items.first().map(|item| item.id));
                Self {
                    items,
                    selected_id,
                    error: None,
                }
            }
            Err(error) => Self {
                items: Vec::new(),
                selected_id: None,
                error: Some(error),
            },
        }
    }

    pub fn reload(&mut self) {
        let next = Self::load();
        let previous_id = self.selected_id;
        self.items = next.items;
        self.error = next.error;
        if previous_id.is_some_and(|id| self.items.iter().any(|item| item.id == id)) {
            self.selected_id = previous_id;
        } else {
            self.selected_id = self.items.first().map(|item| item.id);
        }
    }

    pub fn selected(&self) -> Option<&TargetPreset> {
        let selected_id = self.selected_id?;
        self.items.iter().find(|item| item.id == selected_id)
    }
}

impl TeammatePresetState {
    pub fn load() -> Self {
        match load_setting_file().map(load_teammate_presets) {
            Ok(items) => Self {
                selected_index: (!items.is_empty()).then_some(0),
                items,
                error: None,
            },
            Err(error) => Self {
                items: Vec::new(),
                selected_index: None,
                error: Some(error),
            },
        }
    }

    pub fn reload(&mut self) {
        let next = Self::load();
        let previous_index = self.selected_index;
        self.items = next.items;
        self.error = next.error;
        self.selected_index = previous_index
            .filter(|index| *index < self.items.len())
            .or_else(|| (!self.items.is_empty()).then_some(0));
    }

    pub fn selected(&self) -> Option<&TeammatePreset> {
        let selected_index = self.selected_index?;
        self.items.get(selected_index)
    }
}

#[derive(Debug)]
struct LoadedSettingFile {
    setting_dir: PathBuf,
    config: OpenboxSettingFile,
}

#[derive(Debug, Deserialize)]
struct OpenboxSettingFile {
    #[serde(default)]
    targets: Vec<TargetPresetEntry>,
    #[serde(default, alias = "teammates")]
    teammate: Vec<TeammatePresetEntry>,
}

#[derive(Debug, Deserialize)]
struct TargetPresetEntry {
    id: u64,
    name: String,
    file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct TeammatePresetEntry {
    head: usize,
    name: String,
    file: PathBuf,
}

pub fn load_selected_target_text(state: &TargetPresetState) -> Result<String, String> {
    let preset = state.selected().ok_or_else(|| "请先选择靶子预设。".to_string())?;
    fs::read_to_string(&preset.path)
        .map(|content| content.trim_start_matches('\u{feff}').to_string())
        .map_err(|err| format!("读取靶子预设失败: {}: {err}", preset.path.display()))
}

pub fn load_selected_teammate_text(state: &TeammatePresetState) -> Result<String, String> {
    let preset = state.selected().ok_or_else(|| "请先选择队友预设。".to_string())?;
    fs::read_to_string(&preset.path)
        .map(|content| content.trim_start_matches('\u{feff}').to_string())
        .map_err(|err| format!("读取队友预设失败: {}: {err}", preset.path.display()))
}

fn load_target_presets(loaded: LoadedSettingFile) -> Vec<TargetPreset> {
    let mut items = Vec::with_capacity(loaded.config.targets.len());
    for entry in loaded.config.targets {
        if entry.name.trim().is_empty() {
            continue;
        }
        items.push(TargetPreset {
            id: entry.id,
            name: entry.name,
            path: normalize_relative_path(&loaded.setting_dir, &entry.file),
        });
    }
    items
}

fn load_teammate_presets(loaded: LoadedSettingFile) -> Vec<TeammatePreset> {
    let mut items = Vec::with_capacity(loaded.config.teammate.len());
    for entry in loaded.config.teammate {
        if entry.name.trim().is_empty() {
            continue;
        }
        items.push(TeammatePreset {
            head: entry.head.max(1),
            name: entry.name,
            path: normalize_relative_path(&loaded.setting_dir, &entry.file),
        });
    }
    items
}

fn load_setting_file() -> Result<LoadedSettingFile, String> {
    let setting_dir = current_dir()?.join(SETTING_DIR_NAME);
    let config_path = setting_dir.join(SETTINGS_FILE_NAME);
    let raw = read_or_create_setting_file(&setting_dir, &config_path)?;
    let config = toml::from_str(&raw).map_err(|err| format!("解析设置配置失败: {}: {err}", config_path.display()))?;
    Ok(LoadedSettingFile { setting_dir, config })
}

fn read_or_create_setting_file(setting_dir: &Path, config_path: &Path) -> Result<String, String> {
    match fs::read_to_string(config_path) {
        Ok(raw) => Ok(raw),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            write_default_setting_tree(setting_dir, config_path)?;
            fs::read_to_string(config_path).map_err(|err| format!("读取设置配置失败: {}: {err}", config_path.display()))
        }
        Err(err) => Err(format!("读取设置配置失败: {}: {err}", config_path.display())),
    }
}

fn write_default_setting_tree(setting_dir: &Path, config_path: &Path) -> Result<(), String> {
    fs::create_dir_all(setting_dir).map_err(|err| format!("创建设置目录失败: {}: {err}", setting_dir.display()))?;
    for (relative_path, content) in DEFAULT_SETTING_FILES {
        let path = setting_dir.join(relative_path);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("创建设置目录失败: {}: {err}", parent.display()))?;
        }
        fs::write(&path, content).map_err(|err| format!("写入默认预设失败: {}: {err}", path.display()))?;
    }
    fs::write(config_path, DEFAULT_SETTINGS_TOML).map_err(|err| format!("写入默认设置配置失败: {}: {err}", config_path.display()))
}

fn current_dir() -> Result<PathBuf, String> { std::env::current_dir().map_err(|err| format!("读取当前目录失败: {err}")) }

fn normalize_relative_path(base: &Path, file: &Path) -> PathBuf {
    if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    }
}
