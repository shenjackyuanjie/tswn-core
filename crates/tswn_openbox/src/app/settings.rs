//! 仅保存界面偏好；计算状态、日志和输入内容不写入应用存储。

use serde::{Deserialize, Serialize};

use super::results::{ColumnAlign, ResultsView, ViewMode};
use super::{OpenboxApp, Tool};

const UI_SETTINGS_KEY: &str = "tswn_openbox.ui_settings";

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct UiSettings {
    theme: egui::ThemePreference,
    tool: Tool,
    result_modes: [ViewMode; 5],
    follow: bool,
    column_widths: [f32; 7],
    column_alignments: [ColumnAlign; 7],
    card_align: ColumnAlign,
}

impl Default for UiSettings {
    fn default() -> Self {
        let view = ResultsView::default();
        Self {
            theme: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            result_modes: [
                ViewMode::Cards,
                ViewMode::Table,
                ViewMode::Table,
                ViewMode::Cards,
                ViewMode::Text,
            ],
            follow: true,
            column_widths: view.column_widths,
            column_alignments: view.column_alignments,
            card_align: view.card_align,
        }
    }
}

impl OpenboxApp {
    pub(super) fn from_storage(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut app = Self::default();
        if let Some(settings) = storage.and_then(|storage| eframe::get_value::<UiSettings>(storage, UI_SETTINGS_KEY)) {
            app.theme_preference = settings.theme;
            app.tool = settings.tool;
            app.result_modes = settings.result_modes;
            app.results.mode = settings.result_modes[settings.tool as usize];
            app.results.follow = settings.follow;
            // 损坏的单列不影响其余设置，非法值恢复默认而不是进入布局计算。
            for (index, width) in settings.column_widths.into_iter().enumerate() {
                if width.is_finite() && (60.0..=640.0).contains(&width) {
                    app.results.column_widths[index] = width;
                }
            }
            app.results.column_alignments = settings.column_alignments;
            app.results.card_align = settings.card_align;
        }
        app
    }

    pub(super) fn save_ui_settings(&self, storage: &mut dyn eframe::Storage) {
        let mut result_modes = self.result_modes;
        result_modes[self.tool as usize] = self.results.mode;
        let settings = UiSettings {
            theme: self.theme_preference,
            tool: self.tool,
            result_modes,
            follow: self.results.follow,
            column_widths: self.results.column_widths,
            column_alignments: self.results.column_alignments,
            card_align: self.results.card_align,
        };
        eframe::set_value(storage, UI_SETTINGS_KEY, &settings);
    }
}

#[cfg(test)]
mod tests {
    use eframe::Storage;
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> { self.0.get(key).cloned() }
        fn set_string(&mut self, key: &str, value: String) { self.0.insert(key.to_owned(), value); }
        fn remove_string(&mut self, key: &str) { self.0.remove(key); }
        fn flush(&mut self) {}
    }

    #[test]
    fn ui_preferences_round_trip_without_persisting_running_state() {
        let mut app = OpenboxApp {
            tool: Tool::Pair,
            theme_preference: egui::ThemePreference::Dark,
            result_modes: [
                ViewMode::Text,
                ViewMode::Cards,
                ViewMode::Table,
                ViewMode::Cards,
                ViewMode::Text,
            ],
            ..Default::default()
        };
        app.results.mode = ViewMode::Table;
        app.results.follow = false;
        app.results.card_align = ColumnAlign::Center;
        app.results.column_widths[0] = 350.0;
        app.results.column_alignments[0] = ColumnAlign::Right;
        app.running = true;
        app.append_log("不应持久化的日志");
        let mut storage = MemoryStorage::default();
        app.save_ui_settings(&mut storage);
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.tool, Tool::Pair);
        assert_eq!(restored.theme_preference, egui::ThemePreference::Dark);
        assert_eq!(restored.results.mode, ViewMode::Table);
        assert_eq!(restored.result_modes[0], ViewMode::Text);
        assert_eq!(restored.result_modes[3], ViewMode::Table);
        assert!(!restored.results.follow);
        assert_eq!(restored.results.card_align, ColumnAlign::Center);
        assert_eq!(restored.results.column_widths[0], 350.0);
        assert_eq!(restored.results.column_alignments[0], ColumnAlign::Right);
        assert!(!restored.running);
        assert!(restored.log.is_empty());
    }

    #[test]
    fn absent_or_broken_storage_and_missing_fields_use_defaults() {
        assert_eq!(OpenboxApp::from_storage(None).tool, Tool::ToDiy);
        let mut storage = MemoryStorage::default();
        storage.set_string(UI_SETTINGS_KEY, "不是有效设置".into());
        assert_eq!(OpenboxApp::from_storage(Some(&storage)).tool, Tool::ToDiy);
        storage.set_string(UI_SETTINGS_KEY, "(tool:Pair)".into());
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.tool, Tool::Pair);
        assert_eq!(restored.results.column_widths, ResultsView::default().column_widths);
        assert_eq!(restored.results.mode, ViewMode::Cards);
    }

    #[test]
    fn invalid_widths_do_not_poison_other_preferences() {
        let settings = UiSettings {
            column_widths: [-1.0, 300.0, 0.0, 10000.0, 120.0, 140.0, 160.0],
            ..Default::default()
        };
        let mut storage = MemoryStorage::default();
        eframe::set_value(&mut storage, UI_SETTINGS_KEY, &settings);
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.results.column_widths, [220.0, 300.0, 82.0, 82.0, 120.0, 140.0, 160.0]);
    }
}
