use std::fs::File;
use std::io::{Cursor, Write};

fn main() {
    let sprite_bytes = include_bytes!("../src/player/icon_sprites.png");

    let mut decoder = png::Decoder::new(Cursor::new(sprite_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().expect("icon_sprites.png is embedded and must be valid");
    let mut pixels_buf = vec![0u8; reader.output_buffer_size().expect("icon_sprites.png size must be known")];
    reader.next_frame(&mut pixels_buf).expect("icon_sprites.png frame decode failed");
    let pixels = &pixels_buf;

    let mut out = File::create("src/player/sprite_data.rs").expect("create file");

    writeln!(out, "// Auto-generated sprite data - DO NOT EDIT").unwrap();
    writeln!(out, "// Generated from icon_sprites.png").unwrap();
    writeln!(out).unwrap();

    // shapes: 38 个前景形状
    writeln!(out, "/// 38 个前景形状 alpha 映射 (每个 16x16 = 256 字节)").unwrap();
    writeln!(out, "pub const SHAPES: [[u8; 256]; 38] = [").unwrap();
    for q in 0usize..38 {
        let base = (q % 8) * 64 + (q / 8) * 8192;
        write!(out, "    [").unwrap();
        for n in 0usize..16 {
            if n > 0 {
                write!(out, ",").unwrap();
            }
            writeln!(out).unwrap();
            write!(out, "        ").unwrap();
            for l in 0usize..16 {
                if l > 0 {
                    write!(out, ", ").unwrap();
                }
                let k = base + l * 4 + n * 512;
                let r = pixels[k];
                let g = pixels[k + 1];
                let alpha = if r > g { r } else { 0 };
                write!(out, "{alpha:3}").unwrap();
            }
        }
        writeln!(out, ",").unwrap();
        write!(out, "    ]").unwrap();
        if q < 37 {
            writeln!(out, ",").unwrap();
        } else {
            writeln!(out).unwrap();
        }
    }
    writeln!(out, "];").unwrap();
    writeln!(out).unwrap();

    // borders_dark: 8 个边框深色覆盖层
    writeln!(out, "/// 8 个边框深色覆盖层 alpha 映射 (每个 16x16 = 256 字节)").unwrap();
    writeln!(out, "pub const BORDERS_DARK: [[u8; 256]; 8] = [").unwrap();
    for q in 0usize..8 {
        let base = q * 64 + 57344;
        write!(out, "    [").unwrap();
        for n in 0usize..16 {
            if n > 0 {
                write!(out, ",").unwrap();
            }
            writeln!(out).unwrap();
            write!(out, "        ").unwrap();
            for l in 0usize..16 {
                if l > 0 {
                    write!(out, ", ").unwrap();
                }
                let k = base + l * 4 + n * 512;
                let r = pixels[k];
                let g = pixels[k + 1];
                let alpha = if r > g { r } else { 0 };
                write!(out, "{alpha:3}").unwrap();
            }
        }
        writeln!(out, ",").unwrap();
        write!(out, "    ]").unwrap();
        if q < 7 {
            writeln!(out, ",").unwrap();
        } else {
            writeln!(out).unwrap();
        }
    }
    writeln!(out, "];").unwrap();
    writeln!(out).unwrap();

    // borders_opacity: 8 个边框不透明度掩码
    writeln!(out, "/// 8 个边框不透明度掩码 (每个 16x16 = 256 字节)").unwrap();
    writeln!(out, "pub const BORDERS_OPACITY: [[u8; 256]; 8] = [").unwrap();
    for q in 0usize..8 {
        let base = q * 64 + 57344;
        write!(out, "    [").unwrap();
        for n in 0usize..16 {
            if n > 0 {
                write!(out, ",").unwrap();
            }
            writeln!(out).unwrap();
            write!(out, "        ").unwrap();
            for l in 0usize..16 {
                if l > 0 {
                    write!(out, ", ").unwrap();
                }
                let k = base + l * 4 + n * 512;
                let g = pixels[k + 1];
                let b = pixels[k + 2];
                let alpha = if g > b { 255 - g } else { 255 };
                write!(out, "{alpha:3}").unwrap();
            }
        }
        writeln!(out, ",").unwrap();
        write!(out, "    ]").unwrap();
        if q < 7 {
            writeln!(out, ",").unwrap();
        } else {
            writeln!(out).unwrap();
        }
    }
    writeln!(out, "];").unwrap();

    println!("Generated src/player/sprite_data.rs");
}
