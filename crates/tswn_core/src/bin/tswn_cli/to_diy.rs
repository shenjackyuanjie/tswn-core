//! `to-diy` 子命令实现。

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use tswn_core::cli_api;
use tswn_core::namerena::{NamerenaInput, PreparedRoster};

pub fn run(names: &[String], batch: bool, out_file: Option<&Path>, old: bool, minions: bool) {
    let mut out = match open_output(out_file) {
        Ok(out) => out,
        Err(err) => {
            eprintln!("打开输出文件失败: {err}");
            std::process::exit(1);
        }
    };

    if batch {
        for raw in names {
            let _ = writeln!(out, "{}", export_or_exit(raw, old, minions));
        }
    } else if let Some(raw) = names.first() {
        let _ = writeln!(out, "{}", export_or_exit(raw, old, minions));
        if out_file.is_none() && !raw.contains('+') {
            print_player_details(&mut out, raw);
        }
    }
}

fn print_player_details(out: &mut dyn io::Write, raw: &str) {
    let input = match NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]]) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("构建玩家失败: {raw}: {error}");
            std::process::exit(1);
        }
    };
    let roster = match PreparedRoster::build(&input, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    let Some(player) = roster.players.first() else { return };
    let status = player.status;
    let _ = writeln!(out);
    let _ = writeln!(out, "=== 原始信息 ===");
    let _ = writeln!(out, "名字: {}", player.name);
    let _ = writeln!(out, "队伍: {}", player.clan_name);
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
    let _ = writeln!(out, "name_factor: {:.6}", player.name_factor);
}

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

fn export_or_exit(raw: &str, old: bool, minions: bool) -> String {
    match cli_api::to_diy(raw, old, minions) {
        Ok(export) => export,
        Err(err) => {
            eprintln!("导出 DIY 失败: {raw}: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minion_export_is_reparseable() {
        let raw = "mario@team+ol:{\"attrs\":[86,86,86,86,86,86,86,300],\"skills\":{\"sklshadow\":10,\"sklsummon\":10,\"sklzombie\":10}}";
        let exported = cli_api::to_diy(raw, false, true).expect("export should succeed");
        assert!(exported.contains("\"shadow\":{\"attrs\":"));
        assert!(exported.contains("\"summon\":{\"attrs\":"));
        assert!(exported.contains("\"zombie\":{\"attrs\":"));
        assert!(exported.contains("\"phantom:sklpossess\":\"2*"));

        let reparsed = NamerenaInput::from_raw_groups(&[vec![exported]]).expect("exported OL should parse");
        let roster =
            PreparedRoster::build(&reparsed, tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ).expect("exported OL should build");
        let overlay = roster.players[0].overlay.as_ref().expect("exported OL should retain overlay");
        assert!(overlay.shadow.is_some());
        assert!(overlay.summon.is_some());
        assert!(overlay.zombie.is_some());
    }
}
