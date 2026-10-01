//! 仅保存界面偏好；计算状态、日志和输入内容不写入应用存储。

use serde::{Deserialize, Serialize};

use super::results::{ColumnAlign, ResultsView, ViewMode};
use super::{OpenboxApp, Tool};

const UI_SETTINGS_KEY: &str = "tswn_openbox.ui_settings";

/// 单个工具页的表格与卡片排版。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ViewLayout {
    column_widths: [f32; 7],
    column_alignments: [ColumnAlign; 7],
    card_align: ColumnAlign,
}

impl Default for ViewLayout {
    fn default() -> Self {
        let view = ResultsView::default();
        Self {
            column_widths: view.column_widths,
            column_alignments: view.column_alignments,
            card_align: view.card_align,
        }
    }
}

impl ViewLayout {
    fn of(view: &ResultsView) -> Self {
        Self {
            column_widths: view.column_widths,
            column_alignments: view.column_alignments,
            card_align: view.card_align,
        }
    }

    /// 逐列校验宽度，损坏的单列不影响同一页的其它设置。
    fn apply(&self, view: &mut ResultsView) {
        for (index, width) in self.column_widths.into_iter().enumerate() {
            if width.is_finite() && (60.0..=640.0).contains(&width) {
                view.column_widths[index] = width;
            }
        }
        view.column_alignments = self.column_alignments;
        view.card_align = self.card_align;
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct UiSettings {
    theme: egui::ThemePreference,
    tool: Tool,
    result_modes: [ViewMode; 5],
    follow: bool,
    /// 0.4.7 之前的单一布局；仍写入当前页，便于旧版本读取。
    column_widths: [f32; 7],
    column_alignments: [ColumnAlign; 7],
    card_align: ColumnAlign,
    /// 按工具页保存的排版；缺失时用上面的单值铺满所有页。
    view_layouts: Option<[ViewLayout; 5]>,
}

impl Default for UiSettings {
    fn default() -> Self {
        let layout = ViewLayout::default();
        Self {
            theme: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            result_modes: super::results::DEFAULT_VIEW_MODES,
            follow: true,
            column_widths: layout.column_widths,
            column_alignments: layout.column_alignments,
            card_align: layout.card_align,
            view_layouts: None,
        }
    }
}

impl OpenboxApp {
    pub(super) fn from_storage(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut app = Self::default();
        if let Some(settings) = storage.and_then(|storage| eframe::get_value::<UiSettings>(storage, UI_SETTINGS_KEY)) {
            app.theme_preference = settings.theme;
            app.tool = settings.tool;
            let fallback = ViewLayout {
                column_widths: settings.column_widths,
                column_alignments: settings.column_alignments,
                card_align: settings.card_align,
            };
            for (index, view) in app.views.iter_mut().enumerate() {
                view.mode = settings.result_modes[index];
                view.follow = settings.follow;
                let layout = settings.view_layouts.as_ref().map_or_else(|| fallback.clone(), |all| all[index].clone());
                layout.apply(view);
            }
        }
        app
    }

    pub(super) fn save_ui_settings(&self, storage: &mut dyn eframe::Storage) {
        let mut result_modes = super::results::DEFAULT_VIEW_MODES;
        let mut view_layouts = std::array::from_fn(|_| ViewLayout::default());
        for (index, view) in self.views.iter().enumerate() {
            result_modes[index] = view.mode;
            view_layouts[index] = ViewLayout::of(view);
        }
        let current = &view_layouts[self.tool as usize];
        let settings = UiSettings {
            theme: self.theme_preference,
            tool: self.tool,
            result_modes,
            follow: self.views[self.tool as usize].follow,
            column_widths: current.column_widths,
            column_alignments: current.column_alignments,
            card_align: current.card_align,
            view_layouts: Some(view_layouts),
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
            ..Default::default()
        };
        app.views[Tool::ToDiy as usize].mode = ViewMode::Text;
        let pair = &mut app.views[Tool::Pair as usize];
        pair.mode = ViewMode::Table;
        pair.follow = false;
        pair.card_align = ColumnAlign::Center;
        pair.column_widths[0] = 350.0;
        pair.column_alignments[0] = ColumnAlign::Right;
        app.running = true;
        app.append_log("不应持久化的日志");
        let mut storage = MemoryStorage::default();
        app.save_ui_settings(&mut storage);
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.tool, Tool::Pair);
        assert_eq!(restored.theme_preference, egui::ThemePreference::Dark);
        assert_eq!(restored.views[Tool::Pair as usize].mode, ViewMode::Table);
        assert_eq!(restored.views[Tool::ToDiy as usize].mode, ViewMode::Text);
        assert!(!restored.views[Tool::Pair as usize].follow);
        assert_eq!(restored.views[Tool::Pair as usize].card_align, ColumnAlign::Center);
        assert_eq!(restored.views[Tool::Pair as usize].column_widths[0], 350.0);
        assert_eq!(restored.views[Tool::Pair as usize].column_alignments[0], ColumnAlign::Right);
        assert!(!restored.running);
        assert!(restored.logs[Tool::Pair as usize].is_empty());
    }

    #[test]
    fn per_tool_layouts_are_saved_and_restored_independently() {
        let mut app = OpenboxApp::default();
        app.views[Tool::ToDiy as usize].column_widths[0] = 300.0;
        app.views[Tool::NamerPf as usize].column_widths[2] = 120.0;
        app.views[Tool::Ds4 as usize].card_align = ColumnAlign::Right;
        let mut storage = MemoryStorage::default();
        app.save_ui_settings(&mut storage);
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.views[Tool::ToDiy as usize].column_widths[0], 300.0);
        assert_eq!(restored.views[Tool::NamerPf as usize].column_widths[2], 120.0);
        assert_eq!(restored.views[Tool::Ds4 as usize].card_align, ColumnAlign::Right);
        // 未调整的页保持默认，不会被别的页覆盖。
        assert_eq!(
            restored.views[Tool::Pair as usize].column_widths,
            ResultsView::default().column_widths
        );
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
        assert_eq!(
            restored.views[Tool::ToDiy as usize].column_widths,
            ResultsView::default().column_widths
        );
        assert_eq!(restored.views[Tool::ToDiy as usize].mode, ViewMode::Cards);
        assert_eq!(restored.views[Tool::Ds4 as usize].mode, ViewMode::Text);
    }

    #[test]
    fn invalid_widths_do_not_poison_other_preferences() {
        // 旧存储没有按页布局，单值应铺满所有页；非法列宽逐列回退默认。
        let settings = UiSettings {
            column_widths: [-1.0, 300.0, 0.0, 10000.0, 120.0, 140.0, 160.0],
            view_layouts: None,
            ..Default::default()
        };
        let mut storage = MemoryStorage::default();
        eframe::set_value(&mut storage, UI_SETTINGS_KEY, &settings);
        let restored = OpenboxApp::from_storage(Some(&storage));
        let expected = [220.0, 300.0, 82.0, 82.0, 120.0, 140.0, 160.0];
        for view in &restored.views {
            assert_eq!(view.column_widths, expected);
        }
    }
}
