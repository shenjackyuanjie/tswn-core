//! `tswn-cli icon` 子命令实现。
//!
//! 提供三类图标输出：终端预览、PNG Base64 文本和 PNG 文件保存。
//! PNG 相关路径受 `png_render` feature 控制，未启用时只保留可在终端中查看的像素预览。

use std::fs;
use std::path::Path;

use tswn_core::player::icon::icon_from_raw_name;
use tswn_core::player::icon_render::render_icon_vec_from_name;

#[cfg(feature = "png_render")]
use tswn_core::player::icon_render::{render_icon_b64_from_name, render_icon_png};

pub fn print_icons(names: &[String]) {
    for name in names {
        print_icon(name);
    }
}

pub fn print_icon_b64(names: &[String]) -> Result<(), String> {
    #[cfg(feature = "png_render")]
    {
        for name in names {
            let b64 = render_icon_b64_from_name(name);
            if names.len() == 1 {
                println!("{b64}");
            } else {
                println!("{name}: {b64}");
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "png_render"))]
    {
        let _ = names;
        Err(
            "错误: `tswn-cli icon b64` 需要 `png_render` feature\n请使用: cargo run --bin tswn-cli --features png_render -- icon b64 <名字>"
                .to_string(),
        )
    }
}

pub fn save_icons(dir: &Path, names: &[String]) -> Result<(), String> {
    #[cfg(feature = "png_render")]
    {
        fs::create_dir_all(dir).map_err(|err| format!("创建目录失败: {err}"))?;
        for name in names {
            let path = dir.join(format!("{name}.png"));
            let icon = icon_from_raw_name(name);
            let png = render_icon_png(&icon);
            fs::write(&path, &png).map_err(|err| format!("写入 {} 失败: {err}", path.display()))?;
            println!("已保存: {}", path.display());
        }
        Ok(())
    }

    #[cfg(not(feature = "png_render"))]
    {
        let _ = (dir, names);
        Err(
            "错误: `tswn-cli icon save` 需要 `png_render` feature\n请使用: cargo run --bin tswn-cli --features png_render -- icon save <目录> <名字>"
                .to_string(),
        )
    }
}

fn print_icon(name: &str) {
    let icon = icon_from_raw_name(name);
    let [br, bg, bb] = icon.bg_color;

    println!("=== Icon: {name} ===");
    println!("边框样式: {}", icon.border_style);
    println!("形状: {:?}", icon.shapes);
    println!("背景色: #{:02X}{:02X}{:02X} (索引 {})", br, bg, bb, icon.bg_color_idx);

    let pixels = render_icon_vec_from_name(name);
    render_pixels_to_terminal(&pixels);

    for (i, (idx, color)) in icon.fg_color_indices.iter().zip(icon.fg_colors.iter()).enumerate() {
        let [r, g, b] = *color;
        println!(
            "前景色 {i}: \x1b[48;2;{r};{g};{b}m    \x1b[0m #{r:02X}{g:02X}{b:02X} (索引 {idx}, 形状 {})",
            icon.shapes[i]
        );
    }
    println!();
}

fn render_pixels_to_terminal(pixels: &[u8]) {
    let border_line = "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━";
    println!("┌{}┐", border_line);

    for y in 0..16 {
        print!("│");
        for x in 0..16 {
            if let Some((r, g, b)) = get_pixel(pixels, x, y) {
                print!("\x1b[38;2;{r};{g};{b}m██\x1b[0m");
            } else {
                print!("  ");
            }
        }
        println!("│");
    }

    println!("└{}┘", border_line);
}

fn get_pixel(pixels: &[u8], x: usize, y: usize) -> Option<(u8, u8, u8)> {
    if x >= 16 || y >= 16 {
        return None;
    }
    let idx = (y * 16 + x) * 4;
    if idx + 3 >= pixels.len() {
        return None;
    }
    let r = pixels[idx];
    let g = pixels[idx + 1];
    let b = pixels[idx + 2];
    let a = pixels[idx + 3];
    if a == 0 { None } else { Some((r, g, b)) }
}
