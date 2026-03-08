//! Icon rendering — generates a 16×16 RGBA PNG image for a player icon.
//!
//! Replicates the JS `Sgls.ts()` / `Sgls.o4()` / `Sgls.tu()` pipeline:
//!   1. Fill inner 14×14 area with the background colour.
//!   2. Source-over composite each foreground shape.
//!   3. Draw the dark border overlay.
//!   4. Apply the border opacity mask (directly replaces alpha channel).
//!   5. Encode to PNG.

use std::sync::LazyLock;

use crate::player::icon::{icon_from_name, IconResult};

// ── Sprite sheet ──────────────────────────────────────────────────────────────

/// The 128×128 sprite sheet, compiled into the binary.
static SPRITE_BYTES: &[u8] = include_bytes!("icon_sprites.png");

struct SpriteData {
    /// 38 foreground shapes, each stored as 256 alpha values (16×16, row-major).
    shapes: Box<[[u8; 256]; 38]>,
    /// 8 border dark-overlay alpha maps (16×16).
    borders_dark: [[u8; 256]; 8],
    /// 8 border opacity masks (16×16) — directly replaces α after all drawing.
    borders_opacity: [[u8; 256]; 8],
}

static SPRITES: LazyLock<SpriteData> = LazyLock::new(load_sprites);

fn load_sprites() -> SpriteData {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(SPRITE_BYTES));
    // The sprite sheet uses indexed colour. Expand palette+alpha so every pixel is RGBA.
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().expect("icon_sprites.png is embedded and must be valid");
    let mut pixels_buf = vec![0u8; reader.output_buffer_size().expect("icon_sprites.png size must be known")];
    reader.next_frame(&mut pixels_buf).expect("icon_sprites.png frame decode failed");
    let pixels: &[u8] = &pixels_buf;
    // Image is 128×128 RGBA → row stride = 128 * 4 = 512 bytes.

    let mut shapes = Box::new([[0u8; 256]; 38]);
    let mut borders_dark = [[0u8; 256]; 8];
    let mut borders_opacity = [[0u8; 256]; 8];

    // Shapes: q=0..37, cell offsets match the JS tv() function:
    //   base = (q % 8) * 64  +  (q / 8) * 8192
    //        = col_in_sheet * (16 * 4)  +  row_in_sheet * (16 * 128 * 4)
    // Alpha = R channel if R > G, else 0.
    for q in 0usize..38 {
        let base = (q % 8) * 64 + (q / 8) * 8192;
        for n in 0usize..16 {
            for l in 0usize..16 {
                let k = base + l * 4 + n * 512;
                let r = pixels[k];
                let g = pixels[k + 1];
                shapes[q][n * 16 + l] = if r > g { r } else { 0 };
            }
        }
    }

    // Borders: q=0..7, starting at byte offset 57344 (= row 112 of a 128px-wide image).
    //   borders_dark alpha  = R if R > G, else 0
    //   borders_opacity     = 255-G if G > B, else 255
    for q in 0usize..8 {
        let base = q * 64 + 57344;
        for n in 0usize..16 {
            for l in 0usize..16 {
                let k = base + l * 4 + n * 512;
                let r = pixels[k];
                let g = pixels[k + 1];
                let b = pixels[k + 2];
                borders_dark[q][n * 16 + l] = if r > g { r } else { 0 };
                borders_opacity[q][n * 16 + l] = if g > b { 255 - g } else { 255 };
            }
        }
    }

    SpriteData { shapes, borders_dark, borders_opacity }
}

// ── Compositing ───────────────────────────────────────────────────────────────

/// HTML Canvas `source-over` blend: composite a solid-colour pixel onto `dst`.
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
    let ch = |s: u8, d: u8| -> u8 {
        ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a).round() as u8
    };
    dst[0] = ch(src_rgb[0], dst[0]);
    dst[1] = ch(src_rgb[1], dst[1]);
    dst[2] = ch(src_rgb[2], dst[2]);
    dst[3] = (out_a * 255.0).round() as u8;
}

/// Draw an alpha-mapped shape with a solid colour onto the 256-pixel canvas
/// (replicates the JS `Sgls.o4()` logic, including `source-over` blend).
fn draw_shape(canvas: &mut [[u8; 4]; 256], alpha_map: &[u8; 256], fg: [u8; 3]) {
    for i in 0..256 {
        blend_over(&mut canvas[i], fg, alpha_map[i]);
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Render an `IconResult` to a 16×16 RGBA PNG (returned as raw bytes).
pub fn render_icon_png(icon: &IconResult) -> Vec<u8> {
    let sprites = &*SPRITES;
    let mut canvas = [[0u8; 4]; 256]; // 16×16, starts fully transparent

    // 1. Fill inner 14×14 region with the background colour (JS: fillRect(1,1,14,14)).
    let bg = icon.bg_color;
    for y in 1usize..=14 {
        for x in 1usize..=14 {
            canvas[y * 16 + x] = [bg[0], bg[1], bg[2], 255];
        }
    }

    // 2. Source-over composite each foreground shape.
    for (i, &shape_idx) in icon.shapes.iter().enumerate() {
        draw_shape(&mut canvas, &sprites.shapes[shape_idx], icon.fg_colors[i]);
    }

    // 3. Draw the dark border overlay (colour [64, 64, 64]).
    draw_shape(&mut canvas, &sprites.borders_dark[icon.border_style], [64, 64, 64]);

    // 4. Apply the border opacity mask — directly replaces alpha (JS: pixels[p*4+3] = r[p]).
    let border_opacity = &sprites.borders_opacity[icon.border_style];
    for p in 0..256 {
        canvas[p][3] = border_opacity[p];
    }

    // 5. Encode as 16×16 RGBA PNG.
    let flat: Vec<u8> = canvas.iter().flat_map(|&px| px).collect();
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

/// Render an icon to a base64 data URL (`data:image/png;base64,...`),
/// matching the output of the JS `canvas.toDataURL("image/png")`.
pub fn render_icon_b64(icon: &IconResult) -> String {
    let png = render_icon_png(icon);
    format!("data:image/png;base64,{}", base64_encode(&png))
}

/// Convenience wrapper: render by player name → PNG bytes.
pub fn render_icon_png_from_name(name: &str) -> Vec<u8> {
    render_icon_png(&icon_from_name(name))
}

/// Convenience wrapper: render by player name → base64 data URL.
pub fn render_icon_b64_from_name(name: &str) -> String {
    render_icon_b64(&icon_from_name(name))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Standard RFC 4648 base64 encoder — avoids an extra crate dependency.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
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
