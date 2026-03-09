//! # 图标生成 (icon)
//!
//! 本模块实现玩家图标的生成算法，复现 JS/Dart `Sgl.createFromName()`。
//!
//! ## 功能说明
//!
//! 根据玩家名称生成图标所需的颜色选择：
//! - 边框样式
//! - 形状索引
//! - 背景色
//! - 前景色 (RGB)
//!
//! ## 算法流程
//!
//! 1. **RC4 密钥生成** — 使用 `[0] + UTF-8(name)` 作为密钥，2 轮
//! 2. **S 表映射** — 每个字节 → `((byte ^ 6) * 99 + 218) & 255`
//! 3. **颜色选择** — 使用映射表选择形状和颜色
//!
//! ## 颜色距离矩阵
//!
//! 使用预计算的颜色距离矩阵来确保前景色与背景色有足够的对比度。
//! 矩阵使用 `OnceLock` 实现懒加载，首次调用时计算，之后直接返回缓存。
//!
//! ## 相关模块
//!
//! - [`crate::player::icon_render`] — 图标渲染，将颜色选择转换为 PNG 图像
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::icon::icon_from_name;
//!
//! let result = icon_from_name("mario");
//! println!("边框样式: {}", result.border_style);
//! println!("形状: {:?}", result.shapes);
//! println!("背景色: {:?}", result.bg_color);
//! println!("前景色: {:?}", result.fg_colors);
//! ```

/// 图标生成算法 - 复现 JS/Dart Sgl.createFromName()
///
/// 生成用于渲染 16x16 玩家图标的颜色选择。
/// 由于我们没有 canvas/PNG 形状数据，输出逻辑选择：
/// 边框样式、形状索引、背景色和前景色（RGB）。
use crate::rc4::RC4;
use std::sync::OnceLock;

/// 21 个预定义颜色（sig_colors / $.mf）
pub const SIG_COLORS: [[u8; 3]; 21] = [
    [255, 255, 255], // 0
    [255, 255, 255], // 1
    [0, 0, 0],       // 2
    [0, 180, 0],     // 3
    [0, 255, 0],     // 4
    [255, 0, 0],     // 5
    [255, 192, 0],   // 6
    [255, 255, 0],   // 7
    [0, 224, 128],   // 8
    [255, 0, 128],   // 9
    [255, 108, 0],   // 10
    [0, 108, 255],   // 11
    [0, 192, 255],   // 12
    [0, 255, 255],   // 13
    [128, 120, 255], // 14
    [128, 224, 255], // 15
    [255, 0, 255],   // 16
    [40, 40, 255],   // 17
    [128, 0, 255],   // 18
    [0, 144, 0],     // 19
    [144, 0, 0],     // 20
];

const C_COUNT: usize = SIG_COLORS.len(); // 21
const NUM_SHAPES: usize = 38;
const NUM_BORDERS: usize = 8;

/// 预计算的颜色距离矩阵。
/// 使用与 Dart/JS 相同的公式：R,G,B 加权欧几里得距离 + 亮度差异。
/// 注意：Dart 代码有一个 bug，它对所有三个亮度项都使用 `sig_colors[i][0]`（R 通道）
/// 而不是 R,G,B — 我们忠实地复现了这个 bug。
fn color_distance(i: usize, j: usize) -> f64 {
    let ci = SIG_COLORS[i];
    let cj = SIG_COLORS[j];
    let dr = (ci[0] as f64 - cj[0] as f64) * 0.3;
    let dg = (ci[1] as f64 - cj[1] as f64) * 0.4;
    let db = (ci[2] as f64 - cj[2] as f64) * 0.25;
    // Bug 兼容：Dart 对所有三个亮度项都使用 sig_colors[i][0]
    let dl = (ci[0] as f64 * 0.15 + ci[0] as f64 * 0.25 + ci[0] as f64 * 0.1)
        - (cj[0] as f64 * 0.15 + cj[0] as f64 * 0.25 + cj[0] as f64 * 0.1);
    (dr * dr + dg * dg + db * db + dl * dl).sqrt()
}

/// 全局颜色距离矩阵，首次使用时初始化。
static CDIF: OnceLock<[[f64; C_COUNT]; C_COUNT]> = OnceLock::new();

/// 获取颜色距离矩阵（首次调用时懒初始化）。
#[allow(clippy::needless_range_loop)]
fn cdif() -> &'static [[f64; C_COUNT]; C_COUNT] {
    CDIF.get_or_init(|| {
        let mut dds = [[0.0_f64; C_COUNT]; C_COUNT];
        for i in 1..C_COUNT {
            for j in 0..i {
                let d = color_distance(i, j);
                dds[i][j] = d;
                dds[j][i] = d;
            }
        }
        dds
    })
}

/// 图标生成结果 — 逻辑颜色选择。
#[derive(Debug, Clone)]
pub struct IconResult {
    pub border_style: usize,
    pub shapes: Vec<usize>,
    pub bg_color_idx: usize,
    pub bg_color: [u8; 3],
    pub fg_color_indices: Vec<usize>,
    pub fg_colors: Vec<[u8; 3]>,
    /// 从颜色数组中消耗了多少条目
    pub colors_consumed: usize,
}

/// 从玩家名称生成图标颜色选择。
///
/// 复现 Sgl.createFromName(name):
///   1. RC4 密钥 = \[0] + UTF-8(name)，2 轮
///   2. 映射 S 表：每个字节 → ((byte ^ 6) * 99 + 218) & 255
///   3. 使用映射表选择形状和颜色
pub fn icon_from_name(name: &str) -> IconResult {
    // 步骤 1：使用 [0] + utf8(name) 作为密钥的 RC4，2 轮
    let mut key = Vec::with_capacity(1 + name.len());
    key.push(0u8);
    key.extend_from_slice(name.as_bytes());
    let rc4 = RC4::new(&key, 2);

    // 步骤 2：转换 S 表
    let colors: Vec<u8> = rc4.main_val.iter().map(|&n| (((n ^ 6) as u16 * 99 + 218) & 255) as u8).collect();

    icon_from_colors(&colors)
}

/// 从转换后的颜色数组生成图标（复现 Sgl.create()）。
fn icon_from_colors(colors: &[u8]) -> IconResult {
    let cdif = cdif();
    let mut pos = 0;

    // 1. 边框样式 (0..7)
    let border_style = colors[pos] as usize % NUM_BORDERS;
    pos += 1;

    // 2. 形状 (2-4)
    let mut shapes = Vec::new();
    let shape1 = colors[pos] as usize % NUM_SHAPES;
    shapes.push(shape1);
    pos += 1;

    let mut shape2 = colors[pos] as usize % NUM_SHAPES;
    pos += 1;
    if shape2 == shapes[0] {
        shape2 = colors[pos] as usize % NUM_SHAPES;
        pos += 1;
    }
    shapes.push(shape2);

    if (colors[pos] as usize) < 4 {
        pos += 1;
        shapes.push(colors[pos] as usize % NUM_SHAPES);
        pos += 1;
        if (colors[pos] as usize) < 64 {
            pos += 1;
            shapes.push(colors[pos] as usize % NUM_SHAPES);
        }
        pos += 1;
    } else {
        pos += 1;
    }

    // 3. 背景颜色（从前 15 个 = cCount-6）
    let bg_color_idx = colors[pos] as usize % (C_COUNT - 6);
    pos += 1;

    // 4. 前景颜色
    let mut used_colors: Vec<usize> = Vec::new();
    let mut fg_color_indices = Vec::new();
    let mut fg_colors = Vec::new();

    let valid_color = |c: usize, used: &[usize]| -> bool {
        if !used.is_empty() && c == bg_color_idx && shapes[0] != shapes[1] {
            return true;
        }
        if cdif[c][bg_color_idx] < 90.0 {
            return false;
        }
        for &n in used {
            if n == c {
                return true;
            }
        }
        for &n in used {
            if cdif[c][n] < 90.0 {
                return false;
            }
        }
        true
    };

    for _ in 0..shapes.len() {
        let mut c = colors[pos] as usize % C_COUNT;
        pos += 1;
        while !valid_color(c, &used_colors) {
            c = colors[pos] as usize % C_COUNT;
            pos += 1;
        }
        used_colors.push(c);
        fg_color_indices.push(c);
        fg_colors.push(SIG_COLORS[c]);
    }

    IconResult {
        border_style,
        shapes,
        bg_color_idx,
        bg_color: SIG_COLORS[bg_color_idx],
        fg_color_indices,
        fg_colors,
        colors_consumed: pos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_mario() {
        let r = icon_from_name("mario");
        assert_eq!(r.border_style, 1);
        assert_eq!(r.shapes, vec![22, 5]);
        assert_eq!(r.bg_color_idx, 3);
        assert_eq!(r.bg_color, [0, 180, 0]);
        assert_eq!(r.fg_color_indices, vec![7, 3]);
        assert_eq!(r.fg_colors, vec![[255, 255, 0], [0, 180, 0]]);
        assert_eq!(r.colors_consumed, 7);
    }

    #[test]
    fn test_icon_aaa() {
        let r = icon_from_name("aaa");
        assert_eq!(r.border_style, 5);
        assert_eq!(r.shapes, vec![12, 11]);
        assert_eq!(r.bg_color_idx, 13);
        assert_eq!(r.bg_color, [0, 255, 255]);
        assert_eq!(r.fg_color_indices, vec![20, 6]);
        assert_eq!(r.fg_colors, vec![[144, 0, 0], [255, 192, 0]]);
        assert_eq!(r.colors_consumed, 9);
    }

    #[test]
    fn test_icon_test() {
        let r = icon_from_name("test");
        assert_eq!(r.border_style, 4);
        assert_eq!(r.shapes, vec![8, 22]);
        assert_eq!(r.bg_color_idx, 4);
        assert_eq!(r.bg_color, [0, 255, 0]);
        assert_eq!(r.fg_color_indices, vec![15, 6]);
        assert_eq!(r.fg_colors, vec![[128, 224, 255], [255, 192, 0]]);
        assert_eq!(r.colors_consumed, 8);
    }

    #[test]
    fn test_icon_fengshen() {
        let r = icon_from_name("Fengshen");
        assert_eq!(r.border_style, 1);
        assert_eq!(r.shapes, vec![2, 7]);
        assert_eq!(r.bg_color_idx, 2);
        assert_eq!(r.bg_color, [0, 0, 0]);
        assert_eq!(r.fg_color_indices, vec![16, 0]);
        assert_eq!(r.fg_colors, vec![[255, 0, 255], [255, 255, 255]]);
        assert_eq!(r.colors_consumed, 7);
    }

    #[test]
    fn test_icon_chinese_name() {
        let r = icon_from_name("渊HG");
        assert_eq!(r.border_style, 4);
        assert_eq!(r.shapes, vec![6, 19]);
        assert_eq!(r.bg_color_idx, 13);
        assert_eq!(r.bg_color, [0, 255, 255]);
        assert_eq!(r.fg_color_indices, vec![7, 18]);
        assert_eq!(r.fg_colors, vec![[255, 255, 0], [128, 0, 255]]);
        assert_eq!(r.colors_consumed, 16);
    }

    #[test]
    fn test_icon_special_chars() {
        let r = icon_from_name("syVS:et");
        assert_eq!(r.border_style, 4);
        assert_eq!(r.shapes, vec![31, 34]);
        assert_eq!(r.bg_color_idx, 14);
        assert_eq!(r.bg_color, [128, 120, 255]);
        assert_eq!(r.fg_color_indices, vec![10, 13]);
        assert_eq!(r.fg_colors, vec![[255, 108, 0], [0, 255, 255]]);
        assert_eq!(r.colors_consumed, 8);
    }
}
