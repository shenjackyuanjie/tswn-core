/// Icon generation algorithm - replicates JS/Dart Sgl.createFromName()
///
/// Produces the color selections that would be used to render a 16x16 player icon.
/// Since we don't have canvas/PNG shape data, we output the logical selections:
/// border style, shape indices, background color, and foreground colors (RGB).
use crate::rc4::RC4;

/// 21 predefined colors (sig_colors / $.mf)
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

/// Precomputed color distance matrix.
/// Uses the same formula as Dart/JS: weighted euclidean on R,G,B + luminance diff.
/// Note: the Dart code has a bug where it uses `sig_colors[i][0]` (R channel) for all
/// three luminance terms instead of R,G,B — we replicate that bug faithfully.
fn color_distance(i: usize, j: usize) -> f64 {
    let ci = SIG_COLORS[i];
    let cj = SIG_COLORS[j];
    let dr = (ci[0] as f64 - cj[0] as f64) * 0.3;
    let dg = (ci[1] as f64 - cj[1] as f64) * 0.4;
    let db = (ci[2] as f64 - cj[2] as f64) * 0.25;
    // Bug-compatible: Dart uses sig_colors[i][0] for all three luminance terms
    let dl = (ci[0] as f64 * 0.15 + ci[0] as f64 * 0.25 + ci[0] as f64 * 0.1)
        - (cj[0] as f64 * 0.15 + cj[0] as f64 * 0.25 + cj[0] as f64 * 0.1);
    (dr * dr + dg * dg + db * db + dl * dl).sqrt()
}

/// Lazy-initialized color distance matrix
fn cdif() -> [[f64; C_COUNT]; C_COUNT] {
    let mut dds = [[0.0_f64; C_COUNT]; C_COUNT];
    for i in 1..C_COUNT {
        for j in 0..i {
            let d = color_distance(i, j);
            dds[i][j] = d;
            dds[j][i] = d;
        }
    }
    dds
}

/// Result of icon generation — the logical color selections.
#[derive(Debug, Clone)]
pub struct IconResult {
    pub border_style: usize,
    pub shapes: Vec<usize>,
    pub bg_color_idx: usize,
    pub bg_color: [u8; 3],
    pub fg_color_indices: Vec<usize>,
    pub fg_colors: Vec<[u8; 3]>,
    /// How many entries from the colors array were consumed
    pub colors_consumed: usize,
}

/// Generate icon color selections from a player name.
///
/// Replicates Sgl.createFromName(name):
///   1. RC4 key = [0] + UTF-8(name), 2 rounds
///   2. Map S table: each byte → ((byte ^ 6) * 99 + 218) & 255
///   3. Use mapped table to select shapes and colors
pub fn icon_from_name(name: &str) -> IconResult {
    // Step 1: RC4 with [0] + utf8(name) as key, 2 rounds
    let mut key = Vec::with_capacity(1 + name.len());
    key.push(0u8);
    key.extend_from_slice(name.as_bytes());
    let rc4 = RC4::new(&key, 2);

    // Step 2: Transform S table
    let colors: Vec<u8> = rc4.main_val.iter().map(|&n| (((n ^ 6) as u16 * 99 + 218) & 255) as u8).collect();

    icon_from_colors(&colors)
}

/// Generate icon from the transformed color array (replicates Sgl.create()).
fn icon_from_colors(colors: &[u8]) -> IconResult {
    let cdif = cdif();
    let mut pos = 0;

    // 1. Border style (0..7)
    let border_style = colors[pos] as usize % NUM_BORDERS;
    pos += 1;

    // 2. Shapes (2-4)
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

    // 3. Background color (from first 15 = cCount-6)
    let bg_color_idx = colors[pos] as usize % (C_COUNT - 6);
    pos += 1;

    // 4. Foreground colors
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
