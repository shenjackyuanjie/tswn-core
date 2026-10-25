//! 仅持久化界面偏好，计算状态、日志和输入内容不写入应用存储。
//!
//! 恢复时先构造应用默认状态，再按工具页应用有效布局；缺失或损坏设置回退默认值，
//! 数值逐字段校验，不因单列损坏丢弃其他偏好。默认应用构造仍会触发预设加载及
//! 缺失资源写出，这与 eframe 的偏好存储不是同一份文件或同一生命周期。

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
    /// 表格区域高度；0 表示按可用高度自动分配，拖动分隔条后保存具体像素值。
    table_height: f32,
}

impl Default for ViewLayout {
    fn default() -> Self {
        let view = ResultsView::default();
        Self {
            column_widths: view.column_widths,
            column_alignments: view.column_alignments,
            card_align: view.card_align,
            table_height: view.table_height,
        }
    }
}

impl ViewLayout {
    fn of(view: &ResultsView) -> Self {
        Self {
            column_widths: view.column_widths,
            column_alignments: view.column_alignments,
            card_align: view.card_align,
            table_height: view.table_height,
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
        // 0 表示自动；其它值必须落在拖动允许的范围内，非法时回退自动。
        view.set_table_height((self.table_height >= super::results::TABLE_HEIGHT_MIN).then_some(self.table_height));
    }
}

/// 旧版本的按页列表是 `[to-diy, namer-pf, cqd/cqp, pair, DS4]`，没有 base 页。
/// 读到比当前少一项的列表时，在 base 的位置补一项默认值，
/// 否则 DS4 的视图与排版会被挪到新加的这一页上。
fn insert_base_page<T: Clone>(items: &[T], default: T) -> Vec<T> {
    let mut out = items.to_vec();
    if out.len() + 1 == Tool::ALL.len() {
        out.insert(Tool::Base as usize, default);
    }
    out
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct UiSettings {
    theme: egui::ThemePreference,
    tool: Tool,
    /// 与 [`Tool::ALL`] 等长的视图选择；长度取 `Vec` 是为了旧版本存的 5 项仍能读，
    /// 少的那一页按默认值补齐，不会因为多一个工具页就把整份偏好判为损坏。
    result_modes: Vec<ViewMode>,
    follow: bool,
    /// 0.5.0 之前的单一布局；仍写入当前页，便于旧版本读取。
    column_widths: [f32; 7],
    column_alignments: [ColumnAlign; 7],
    card_align: ColumnAlign,
    /// 按工具页保存的排版；缺失时用上面的单值铺满所有页。
    view_layouts: Option<Vec<ViewLayout>>,
}

impl Default for UiSettings {
    fn default() -> Self {
        let layout = ViewLayout::default();
        Self {
            theme: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            result_modes: super::results::DEFAULT_VIEW_MODES.to_vec(),
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
                ..ViewLayout::default()
            };
            let result_modes = insert_base_page(&settings.result_modes, super::results::DEFAULT_VIEW_MODES[Tool::Base as usize]);
            let view_layouts = settings.view_layouts.as_ref().map(|all| insert_base_page(all, ViewLayout::default()));
            for (index, view) in app.views.iter_mut().enumerate() {
                view.mode = result_modes.get(index).copied().unwrap_or(super::results::DEFAULT_VIEW_MODES[index]);
                view.follow = settings.follow;
                let layout = view_layouts
                    .as_ref()
                    .and_then(|all| all.get(index))
                    .cloned()
                    .unwrap_or_else(|| fallback.clone());
                layout.apply(view);
            }
        }
        app
    }

    pub(super) fn save_ui_settings(&self, storage: &mut dyn eframe::Storage) {
        let mut result_modes = Vec::with_capacity(self.views.len());
        let mut view_layouts = Vec::with_capacity(self.views.len());
        for view in &self.views {
            result_modes.push(view.mode);
            view_layouts.push(ViewLayout::of(view));
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
        pair.set_table_height(Some(280.0));
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
        assert_eq!(restored.views[Tool::Pair as usize].table_height, 280.0, "表格高度按页保存");
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
            // 旧存储没有表格高度字段，回退自动分配。
            assert_eq!(view.table_height, super::super::results::TABLE_HEIGHT_AUTO);
        }
    }

    #[test]
    fn broken_table_height_falls_back_to_auto_without_touching_other_layout() {
        let mut layouts = vec![ViewLayout::default(); Tool::ALL.len()];
        layouts[Tool::Pair as usize].column_widths[0] = 260.0;
        layouts[Tool::Pair as usize].table_height = 5.0;
        let settings = UiSettings {
            view_layouts: Some(layouts),
            ..Default::default()
        };
        let mut storage = MemoryStorage::default();
        eframe::set_value(&mut storage, UI_SETTINGS_KEY, &settings);
        let restored = OpenboxApp::from_storage(Some(&storage));
        let pair = &restored.views[Tool::Pair as usize];
        assert_eq!(pair.table_height, super::super::results::TABLE_HEIGHT_AUTO);
        assert_eq!(pair.column_widths[0], 260.0, "非法高度不应影响同一页的列宽");
    }

    #[test]
    fn storing_five_pages_still_restores_every_page_after_base_was_added() {
        // 0.5.1 及更早存的是 5 项（第 5 项是 DS4）：新增 base 页后仍要能读出主题与既有页面，
        // 老的第 5 项继续归 DS4，base 按默认补齐。
        let mut layouts = vec![ViewLayout::default(); 5];
        layouts[Tool::ToDiy as usize].column_widths[0] = 300.0;
        layouts[4].card_align = ColumnAlign::Right;
        let settings = UiSettings {
            theme: egui::ThemePreference::Light,
            tool: Tool::Pair,
            result_modes: vec![
                ViewMode::Cards,
                ViewMode::Table,
                ViewMode::Table,
                ViewMode::Text,
                ViewMode::Table,
            ],
            column_widths: [300.0, 132.0, 82.0, 82.0, 82.0, 82.0, 100.0],
            view_layouts: Some(layouts),
            ..Default::default()
        };
        let mut storage = MemoryStorage::default();
        eframe::set_value(&mut storage, UI_SETTINGS_KEY, &settings);
        let restored = OpenboxApp::from_storage(Some(&storage));
        assert_eq!(restored.theme_preference, egui::ThemePreference::Light);
        assert_eq!(restored.tool, Tool::Pair);
        assert_eq!(restored.views[Tool::ToDiy as usize].mode, ViewMode::Cards);
        assert_eq!(restored.views[Tool::Pair as usize].mode, ViewMode::Text);
        assert_eq!(restored.views[Tool::ToDiy as usize].column_widths[0], 300.0);
        assert_eq!(
            restored.views[Tool::Ds4 as usize].mode,
            ViewMode::Table,
            "老的第 5 项仍属于 DS4"
        );
        assert_eq!(restored.views[Tool::Ds4 as usize].card_align, ColumnAlign::Right);
        // 新增的 base 页按默认纯文本、默认排版显示，不继承 DS4 的设置。
        assert_eq!(restored.views[Tool::Base as usize].mode, ViewMode::Text);
        assert_eq!(
            restored.views[Tool::Base as usize].column_widths,
            ResultsView::default().column_widths
        );
    }
}
