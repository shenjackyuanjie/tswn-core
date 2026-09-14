//! 从 `setting/settings.toml` 加载预设。

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;

const SETTING_DIR_NAME: &str = "setting";
const SETTINGS_FILE_NAME: &str = "settings.toml";
const SCORE_NOW_FILE_NAME: &str = "score_now.toml";
const DEFAULT_SETTINGS_TOML: &str = include_str!("../../assets/settings.toml");
const DEFAULT_SCORE_NOW_TOML: &str = include_str!("../../assets/score_now.toml");
const DEFAULT_SETTING_FILES: &[(&str, &str)] = &[
    (
        "teammates/teammate_fz.toml",
        include_str!("../../assets/teammates/teammate_fz.toml"),
    ),
    (
        "teammates/teammate_bc.toml",
        include_str!("../../assets/teammates/teammate_bc.toml"),
    ),
    (
        "teammates/teammate_wc.toml",
        include_str!("../../assets/teammates/teammate_wc.toml"),
    ),
    (
        "teammates/teammate_pj.toml",
        include_str!("../../assets/teammates/teammate_pj.toml"),
    ),
    (
        "teammates/teammate_fs.toml",
        include_str!("../../assets/teammates/teammate_fs.toml"),
    ),
    (SCORE_NOW_FILE_NAME, DEFAULT_SCORE_NOW_TOML),
    ("targets/target1.txt", include_str!("../../assets/targets/target1.txt")),
    ("targets/target2.txt", include_str!("../../assets/targets/target2.txt")),
    ("targets/target3.txt", include_str!("../../assets/targets/target3.txt")),
    ("targets/newTarget1.toml", include_str!("../../assets/targets/newTarget1.toml")),
    ("targets/newTarget2.toml", include_str!("../../assets/targets/newTarget2.toml")),
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
    pub diy: bool,
    pub factor_enabled: bool,
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
    pub factor_enabled: bool,
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
    #[serde(default)]
    diy: bool,
    #[serde(default)]
    factor_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct TeammatePresetEntry {
    head: usize,
    name: String,
    file: PathBuf,
    #[serde(default)]
    factor_enabled: bool,
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
            diy: entry.diy,
            factor_enabled: entry.factor_enabled,
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
            factor_enabled: entry.factor_enabled,
        });
    }
    items
}

fn load_setting_file() -> Result<LoadedSettingFile, String> {
    let setting_dir = current_dir()?.join(SETTING_DIR_NAME);
    let config_path = setting_dir.join(SETTINGS_FILE_NAME);
    let raw = read_or_create_setting_file(&setting_dir, &config_path)?;
    let config = toml::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|err| format!("解析设置配置失败: {}: {err}", config_path.display()))?;
    Ok(LoadedSettingFile { setting_dir, config })
}

fn read_or_create_setting_file(setting_dir: &Path, config_path: &Path) -> Result<String, String> {
    match fs::read_to_string(config_path) {
        Ok(raw) => {
            ensure_default_setting_files(setting_dir)?;
            Ok(raw)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            write_default_setting_tree(setting_dir, config_path)?;
            fs::read_to_string(config_path).map_err(|err| format!("读取设置配置失败: {}: {err}", config_path.display()))
        }
        Err(err) => Err(format!("读取设置配置失败: {}: {err}", config_path.display())),
    }
}

// 只补缺失资源；升级或重新加载不能覆盖用户修改过的配置和靶子。
fn write_default_setting_tree(setting_dir: &Path, config_path: &Path) -> Result<(), String> {
    ensure_default_setting_files(setting_dir)?;
    write_missing_file(config_path, DEFAULT_SETTINGS_TOML)
}

fn ensure_default_setting_files(setting_dir: &Path) -> Result<(), String> {
    for (relative_path, content) in DEFAULT_SETTING_FILES {
        write_missing_file(&setting_dir.join(relative_path), content)?;
    }
    Ok(())
}

fn write_missing_file(path: &Path, content: &str) -> Result<(), String> {
    // 常见的只读发布目录中，已有资源不需要写权限。
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建设置目录失败: {}: {err}", parent.display()))?;
    }
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file
            .write_all(content.as_bytes())
            .map_err(|err| format!("写入默认预设失败: {}: {err}", path.display())),
        // 两个预设面板同时初始化时，也不能截断另一个调用者刚写入的文件。
        Err(err) if err.kind() == ErrorKind::AlreadyExists && path.is_file() => Ok(()),
        Err(err) => Err(format!("创建默认预设失败: {}: {err}", path.display())),
    }
}

#[cfg(test)]
fn write_default_score_now_file(setting_dir: &Path) -> Result<(), String> {
    write_missing_file(&setting_dir.join(SCORE_NOW_FILE_NAME), DEFAULT_SCORE_NOW_TOML)
}

fn current_dir() -> Result<PathBuf, String> { std::env::current_dir().map_err(|err| format!("读取当前目录失败: {err}")) }

fn normalize_relative_path(base: &Path, file: &Path) -> PathBuf {
    if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DEFAULT_SCORE_NOW_TOML, SCORE_NOW_FILE_NAME, write_default_score_now_file};

    #[test]
    fn writes_default_score_now_file_when_missing() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tswn_openbox_score_now_test_{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");

        write_default_score_now_file(&dir).expect("write default score_now.toml");

        let path = dir.join(SCORE_NOW_FILE_NAME);
        let content = fs::read_to_string(&path).expect("read generated score_now.toml");
        assert_eq!(content, DEFAULT_SCORE_NOW_TOML);

        fs::remove_dir_all(&dir).expect("remove temp dir");
    }
}

#[cfg(test)]
mod asset_regressions {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tswn-presets-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn clean_install_extracts_every_referenced_asset() {
        let temp = TempDir::new();
        let setting = temp.0.join("setting");
        let raw = read_or_create_setting_file(&setting, &setting.join(SETTINGS_FILE_NAME)).unwrap();
        let config: OpenboxSettingFile = toml::from_str(&raw).unwrap();
        for entry in config.targets {
            assert!(setting.join(entry.file).is_file());
        }
        let mut toml_count = 0;
        for entry in config.teammate {
            let content = fs::read_to_string(setting.join(&entry.file)).unwrap();
            if entry.factor_enabled {
                let value: toml::Value = toml::from_str(&content).unwrap();
                assert!(!value["targets"].as_array().unwrap().is_empty());
                toml_count += 1;
            }
        }
        assert_eq!(toml_count, 5);
    }

    #[test]
    fn existing_settings_repairs_missing_toml_without_overwriting_custom_files() {
        let temp = TempDir::new();
        let config_path = temp.0.join(SETTINGS_FILE_NAME);
        let custom_settings = format!("# 用户自定义配置\n{DEFAULT_SETTINGS_TOML}");
        fs::write(&config_path, &custom_settings).unwrap();
        let custom_path = temp.0.join("teammates/teammate_fz.toml");
        fs::create_dir_all(custom_path.parent().unwrap()).unwrap();
        let custom = "[[targets]]\nfactor=3\nplayers=[\"custom\"]\n";
        fs::write(&custom_path, custom).unwrap();
        assert_eq!(read_or_create_setting_file(&temp.0, &config_path).unwrap(), custom_settings);
        assert_eq!(fs::read_to_string(custom_path).unwrap(), custom);
        for (relative, _) in DEFAULT_SETTING_FILES {
            assert!(temp.0.join(relative).is_file());
        }
        fs::remove_file(temp.0.join("teammates/teammate_bc.toml")).unwrap();
        read_or_create_setting_file(&temp.0, &config_path).unwrap();
        assert!(temp.0.join("teammates/teammate_bc.toml").is_file());
        assert_eq!(fs::read_to_string(config_path).unwrap(), custom_settings);
    }

    #[test]
    fn conflicting_directory_is_reported_instead_of_silently_skipped() {
        let temp = TempDir::new();
        let path = temp.0.join("occupied.toml");
        fs::create_dir(&path).unwrap();
        assert!(write_missing_file(&path, "value=1").is_err());
    }
}
