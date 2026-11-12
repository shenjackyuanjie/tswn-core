//! 玩家徽章（名字图标）的渲染与纹理缓存。
//!
//! 徽章像素由 [`tswn_core::namerena::icon_render::render_icon_vec_from_name`] 生成，
//! 与 `tswn-cli icon`、WASM show 页面同一套名字推导；`name@team` 由 core 归一化为
//! 队伍名，因此同队玩家共享徽章。组合标签按 `+` 拆成成员各自渲染，并跳过 `ol:`
//! overlay 与 `diy[` 段，与后端展示标签的清理规则一致。
//!
//! 纹理按名字常驻缓存：只在首次遇到某个名字时上传，之后各帧只取句柄。清空结果
//! 不回收纹理，避免同一批名字重复上传造成帧时间抖动。缓存只由 UI 线程访问。

use std::collections::HashMap;

/// 徽章原生边长（像素）；图标是 16×16 RGBA，最外圈是 1 像素透明边框。
const ICON_SIDE: usize = 16;
/// 徽章显示边长上限；行高更矮时按行高收缩，避免裁切或撑高虚拟列表行。
pub(super) const ICON_SIZE_MAX: f32 = 16.0;
/// 徽章组与右侧文字的间距。
pub(super) const ICON_TEXT_GAP: f32 = 4.0;

/// 按名字缓存徽章纹理；纹理句柄由 egui 的纹理管理器持有，克隆只复制句柄。
#[derive(Default)]
pub(super) struct IconCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl IconCache {
    /// 取单个名字的徽章纹理；空名返回 `None`，调用方退化为纯文本。
    pub(super) fn texture(&mut self, ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.clone());
        }
        let pixels = tswn_core::namerena::icon_render::render_icon_vec_from_name(name);
        let image = egui::ColorImage::from_rgba_unmultiplied([ICON_SIDE, ICON_SIDE], &pixels);
        // 徽章是像素画：最近邻放大保留硬边，插值会糊掉轮廓。
        let handle = ctx.load_texture(format!("openbox_icon:{name}"), image, egui::TextureOptions::NEAREST);
        self.textures.insert(name.to_owned(), handle.clone());
        Some(handle)
    }

    /// 拆分组合标签并取各成员的徽章；没有可用成员时返回空向量。
    pub(super) fn textures_for_label(&mut self, ctx: &egui::Context, label: &str) -> Vec<egui::TextureHandle> {
        label
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty() && !part.starts_with("ol:") && !part.starts_with("diy["))
            .filter_map(|part| self.texture(ctx, part))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_labels_render_one_badge_per_member_and_skip_overlays() {
        let ctx = egui::Context::default();
        let mut cache = IconCache::default();
        let mut handles = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            handles = cache.textures_for_label(ui.ctx(), "mario@team+bowser+ol:fire+diy[1]");
        });
        output.textures_delta.clear();
        assert_eq!(handles.len(), 2, "组合按 + 拆成两个成员，overlay 与 diy 段不渲染徽章");
    }

    #[test]
    fn the_same_name_reuses_one_texture() {
        let ctx = egui::Context::default();
        let mut cache = IconCache::default();
        let mut ids = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let first = cache.texture(ui.ctx(), "mario").expect("名字应能渲染徽章");
            let second = cache.texture(ui.ctx(), " mario ").expect("同名应能渲染徽章");
            ids.push((first.id(), second.id()));
        });
        output.textures_delta.clear();
        assert_eq!(ids[0].0, ids[0].1, "同名复用同一纹理，不重复上传");
    }

    #[test]
    fn empty_labels_have_no_badge() {
        let ctx = egui::Context::default();
        let mut cache = IconCache::default();
        let mut handles = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            handles = cache.textures_for_label(ui.ctx(), "  +ol:x+ ");
        });
        output.textures_delta.clear();
        assert!(handles.is_empty(), "没有可用成员时不应产生徽章");
    }
}
