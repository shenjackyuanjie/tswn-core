//! # 图标渲染 (icon_render)
//!
//! 本模块实现玩家图标的渲染，将 [`IconResult`] 转换为 16×16 RGBA 像素数据或 PNG 图像。
//!
//! ## 功能说明
//!
//! - **像素渲染** — 将图标颜色选择渲染为 RGBA 像素数据 (1024 字节)
//! - **PNG 渲染** — 将图标颜色选择渲染为 PNG 字节流 [需要 `png_render` feature]
//! - **Base64 编码** — 将 PNG 编码为 data URL 格式 [需要 `png_render` feature]
//! - **便捷接口** — 直接从玩家名称渲染图标
//!
//! ## Feature Flag
//!
//! PNG 输出功能需要 `png_render` feature：
//!
//! ```toml
//! [dependencies]
//! tswn-core = { version = "0.1.6", features = ["png_render"] }
//! ```
//!
//! 或者在运行时启用：
//!
//! ```bash
//! cargo run --bin namerena_cli --features png_render -- --icon mario
//! ```
//!
//! **注意**: `render_icon_vec` 函数不需要 `png_render` feature，可以直接使用。
//!
//! ## 渲染流程
//!
//! 复现 JS `Sgls.ts()` / `Sgls.o4()` / `Sgls.tu()` 流程：
//!
//! 1. **填充背景** — 用背景色填充内部 14×14 区域
//! 2. **合成前景** — 使用 source-over 混合模式合成每个前景形状
//! 3. **绘制边框** — 绘制深色边框覆盖层
//! 4. **应用掩码** — 应用边框不透明度掩码（直接替换 alpha 通道）
//! 5. **输出数据** — 返回 RGBA 像素数据或编码为 PNG
//!
//! ## 精灵表
//!
//! 精灵数据在编译时硬编码，包含：
//! - 38 个前景形状（每个 16×16 alpha 映射）
//! - 8 个边框深色覆盖层
//! - 8 个边框不透明度掩码
//!
//! ## 相关模块
//!
//! - [`crate::player::icon`] — 图标生成，根据玩家名称生成颜色选择
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::icon_render::render_icon_vec_from_name;
//!
//! let rgba_pixels = render_icon_vec_from_name("mario");
//! // rgba_pixels 是 16×16 RGBA 像素数据 (1024 字节)
//! ```
//!
//! ## 性能说明
//!
//! - 精灵数据直接编译到二进制中，无需运行时解析
//! - Base64 编码器为标准 RFC 4648 实现，避免额外依赖
//! - 混合算法复现 HTML Canvas `source-over` 混合模式

use crate::player::icon::{IconResult, icon_from_name};

#[path = "sprite_data.rs"]
mod sprite_data;
use sprite_data::{BORDERS_DARK, BORDERS_OPACITY, SHAPES};

// ── 合成 ───────────────────────────────────────────────────────────────

/// HTML Canvas `source-over` 混合: 将纯色像素合成到 `dst` 上。
#[inline]
fn blend_over(dst: &mut [u8; 4], src_rgb: [u8; 3], src_a: u8) {
    if src_a == 0 {
        return;
    }
    if src_a == 255 {
        *dst = [src_rgb[0], src_rgb[1], src_rgb[2], 255];
        return;
    }
    let sa = src_a as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    let ch = |s: u8, d: u8| -> u8 { ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a).round() as u8 };
    dst[0] = ch(src_rgb[0], dst[0]);
    dst[1] = ch(src_rgb[1], dst[1]);
    dst[2] = ch(src_rgb[2], dst[2]);
    dst[3] = (out_a * 255.0).round() as u8;
}

/// 使用纯色在 256 像素画布上绘制 alpha 映射形状
/// (复现 JS `Sgls.o4()` 逻辑, 包括 `source-over` 混合)。
fn draw_shape(canvas: &mut [[u8; 4]; 256], alpha_map: &[u8; 256], fg: [u8; 3]) {
    for i in 0..256 {
        blend_over(&mut canvas[i], fg, alpha_map[i]);
    }
}

// ── 公共 API ────────────────────────────────────────────────────────────────

/// 将 `IconResult` 渲染为 16×16 RGBA 像素数据 (1024 字节, 行主序)。
pub fn render_icon_vec(icon: &IconResult) -> Vec<u8> {
    let mut canvas = [[0u8; 4]; 256]; // 16×16, 初始完全透明

    // 1. 用背景色填充内部 14×14 区域 (JS: fillRect(1,1,14,14))。
    let bg = icon.bg_color;
    for y in 1usize..=14 {
        for x in 1usize..=14 {
            canvas[y * 16 + x] = [bg[0], bg[1], bg[2], 255];
        }
    }

    // 2. 使用 source-over 混合模式合成每个前景形状。
    for (i, &shape_idx) in icon.shapes.iter().enumerate() {
        draw_shape(&mut canvas, &SHAPES[shape_idx], icon.fg_colors[i]);
    }

    // 3. 绘制深色边框覆盖层 (颜色 [64, 64, 64])。
    draw_shape(&mut canvas, &BORDERS_DARK[icon.border_style], [64, 64, 64]);

    // 4. 应用边框不透明度掩码 — 直接替换 alpha (JS: pixels[p*4+3] = r[p])。
    //    如果 alpha 变为 0, 也将 RGB 清零 (匹配 JS canvas 预乘 alpha 行为)。
    let border_opacity = &BORDERS_OPACITY[icon.border_style];
    for p in 0..256 {
        canvas[p][3] = border_opacity[p];
        if canvas[p][3] == 0 {
            canvas[p][0] = 0;
            canvas[p][1] = 0;
            canvas[p][2] = 0;
        }
    }

    // 5. 展平为 1024 字节的 RGBA 数据 (16×16×4)。
    canvas.iter().flat_map(|&px| px).collect()
}

/// 将 `IconResult` 渲染为 16×16 RGBA PNG (以原始字节返回)。
#[cfg(feature = "png_render")]
pub fn render_icon_png(icon: &IconResult) -> Vec<u8> {
    let flat = render_icon_vec(icon);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 16, 16);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(&flat))
        .expect("PNG encoding should never fail for a valid image");
    out
}

/// 将图标渲染为 base64 data URL (`data:image/png;base64,...`),
/// 匹配 JS `canvas.toDataURL("image/png")` 的输出。
#[cfg(feature = "png_render")]
pub fn render_icon_b64(icon: &IconResult) -> String {
    let png = render_icon_png(icon);
    format!("data:image/png;base64,{}", base64_encode(&png))
}

/// 便捷包装器: 通过玩家名称渲染 → PNG 字节。
#[cfg(feature = "png_render")]
pub fn render_icon_png_from_name(name: &str) -> Vec<u8> { render_icon_png(&icon_from_name(name)) }

/// 便捷包装器: 通过玩家名称渲染 → base64 data URL。
#[cfg(feature = "png_render")]
pub fn render_icon_b64_from_name(name: &str) -> String { render_icon_b64(&icon_from_name(name)) }

/// 便捷包装器: 通过玩家名称渲染 → RGBA 像素数据 (1024 字节)。
pub fn render_icon_vec_from_name(name: &str) -> Vec<u8> { render_icon_vec(&icon_from_name(name)) }

// ── 内部辅助函数 ──────────────────────────────────────────────────────────

/// 标准 RFC 4648 base64 编码器 — 避免额外的 crate 依赖。
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) & 63] as char);
        out.push(TABLE[(n >> 12) & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod test;
