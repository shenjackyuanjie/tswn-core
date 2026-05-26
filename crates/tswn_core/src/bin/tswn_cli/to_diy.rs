//! `to-diy` 子命令：将名字转换为 DIY/OL overlay 格式。

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use tswn_core::engine::storage::Storage;
use tswn_core::player::Player;

pub fn run(names: &[String], batch: bool, out_file: Option<&Path>, old: bool) {
    if batch {
        run_batch(names, out_file, old);
    } else if let Some(raw) = names.first() {
        run_single(raw, out_file, old);
    }
}

fn run_single(raw: &str, out_file: Option<&Path>, old: bool) {
    let storage = Storage::new_arc();
    let mut player = build_player_or_exit(raw, storage);
    player.build();

    let mut out = match open_output(out_file) {
        Ok(out) => out,
        Err(err) => {
            eprintln!("打开输出文件失败: {err}");
            std::process::exit(1);
        }
    };

    let export = export_line(&player, old);
    let _ = writeln!(out, "{export}");

    if out_file.is_none() {
        let _ = writeln!(out);
        let _ = writeln!(out, "=== 原始信息 ===");
        let _ = writeln!(out, "名字: {}", player.id_name());
        let _ = writeln!(out, "队伍: {}", player.clan_name());
        let status = player.get_status();
        let _ = writeln!(
            out,
            "八围 (计算后): atk={} def={} spd={} agi={} mag={} res={} wis={} maxhp={}",
            status.attack,
            status.defense,
            status.speed,
            status.agility,
            status.magic,
            status.resistance,
            status.wisdom,
            status.max_hp,
        );
        let _ = writeln!(out, "name_factor: {:.6}", player.get_name_factor());
    }
}

fn run_batch(names: &[String], out_file: Option<&Path>, old: bool) {
    let storage = Storage::new_arc();
    let mut out = match open_output(out_file) {
        Ok(out) => out,
        Err(err) => {
            eprintln!("打开输出文件失败: {err}");
            std::process::exit(1);
        }
    };

    for raw in names {
        let mut player = build_player_or_exit(raw, storage.clone());
        player.build();
        let _ = writeln!(out, "{}", export_line(&player, old));
    }
}

fn export_line(player: &Player, old: bool) -> String { if old { player.to_diy_compact() } else { player.to_ol_json() } }

fn open_output(path: Option<&Path>) -> io::Result<Box<dyn io::Write>> {
    match path {
        Some(path) => open_file(path).map(|file| Box::new(file) as Box<dyn io::Write>),
        None => Ok(Box::new(io::stdout())),
    }
}

fn open_file(path: &Path) -> io::Result<File> {
    if path.exists() && path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径不能是目录: {}", path.display()),
        ));
    }
    if path.exists() {
        OpenOptions::new().write(true).truncate(true).open(path)
    } else {
        OpenOptions::new().write(true).create_new(true).open(path)
    }
}

fn build_player_or_exit(raw: &str, storage: std::sync::Arc<Storage>) -> Player {
    match Player::new_from_namerena_raw(raw.to_string(), storage) {
        Ok(player) => player,
        Err(err) => {
            eprintln!("构建玩家失败: {raw}: {err}");
            std::process::exit(1);
        }
    }
}
