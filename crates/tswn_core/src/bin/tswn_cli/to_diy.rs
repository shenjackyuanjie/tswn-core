//! `to-diy` 子命令实现。

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::Path;

use tswn_core::cli_api;
use tswn_core::namerena::{NamerenaInput, PreparedRoster};

pub fn run(names: &[String], batch: bool, out_file: Option<&Path>, old: bool, minions: bool, details: bool) {
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
        if details && out_file.is_none() && !raw.contains('+') {
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
    // 技能对象要从旧版 `+diy` 形式里抽：导出结果形如
    // `name@team+diy[atk,def,spd,agi,mag,res,wis,maxhp]{...技能 JSON...}`。
    let diy = cli_api::to_diy(raw, true, false).unwrap_or_default();
    let _ = writeln!(out, "技能: {}", extract_diy_skill_object(&diy).unwrap_or("{}"));
    let _ = writeln!(out, "name_factor: {:.6}", player.name_factor);
}

/// 从 `+diy` 导出文本中抽出技能 JSON 对象（对齐 openbox `extract_diy_skill_object`）。
///
/// 需要按花括号深度与字符串转义扫描，直接截取固定区间会被技能值里的
/// `+` / 引号破坏。
fn extract_diy_skill_object(diy: &str) -> Option<&str> {
    let attrs_start = diy.find("+diy[")? + "+diy[".len();
    let attrs_end = attrs_start + diy[attrs_start..].find(']')?;
    let object_start = attrs_end + 1;
    if !diy[object_start..].starts_with('{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in diy[object_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = object_start + offset + ch.len_utf8();
                    return Some(&diy[object_start..end]);
                }
            }
            _ => {}
        }
    }
    None
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
    fn diy_skill_object_extracts_balanced_json() {
        let diy = r#"mario+diy[72,39,69,76,67,66,0,84]{"sklfire":5,"sklheal":"40+30"}"#;
        assert_eq!(extract_diy_skill_object(diy), Some(r#"{"sklfire":5,"sklheal":"40+30"}"#));
    }

    #[test]
    fn diy_skill_object_handles_nested_and_escaped_braces() {
        let diy = r#"mario+diy[1,2,3,4,5,6,7,8]{"a":"}{\"x\":1}","b":[1,2]}"#;
        assert_eq!(extract_diy_skill_object(diy), Some(r#"{"a":"}{\"x\":1}","b":[1,2]}"#));
    }

    #[test]
    fn diy_skill_object_missing_object_returns_none() {
        assert_eq!(extract_diy_skill_object("mario+diy[1,2,3,4,5,6,7,8]"), None);
        assert_eq!(extract_diy_skill_object("mario"), None);
    }

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
