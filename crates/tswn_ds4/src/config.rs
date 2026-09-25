//! 工具全局配置与模式定义。
//!
//! 定义 [`Config`] 结构体（从文件加载的运行时配置）、单字评分模式枚举
//! [`SingleMode`]（Bc/Fz/Wc/Fs/Pj）及字对评分模式枚举 [`PairMode`]（Fc/Wc/Rh）。

#![allow(dead_code)]

use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Ds4Error, Ds4Result};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum SingleMode {
    Bc,
    Fz,
    Wc,
    Fs,
    Pj,
}

impl SingleMode {
    pub const ALL: [Self; 5] = [Self::Bc, Self::Fz, Self::Wc, Self::Fs, Self::Pj];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bc => "bc",
            Self::Fz => "fz",
            Self::Wc => "wc",
            Self::Fs => "fs",
            Self::Pj => "pj",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PairMode {
    Fc,
    Wc,
    Rh,
}

impl PairMode {
    pub const ALL: [Self; 3] = [Self::Fc, Self::Wc, Self::Rh];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fc => "fc",
            Self::Wc => "wc",
            Self::Rh => "rh",
        }
    }
}

impl Display for PairMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SingleThreshold {
    pub score: i32,
    pub potential: i32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PairThreshold {
    pub enabled: bool,
    pub sieve: i32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Sp1Threshold {
    pub enabled: bool,
    pub sieve: i32,
    pub skill_sieve: Option<i32>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ThreeThreshold {
    pub enabled: bool,
    pub sieve: i32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub team_name: Option<String>,
    pub threads: usize,
    pub copy_to_new: bool,
    pub run_dedup: bool,
    pub single_bc: SingleThreshold,
    pub single_fz: SingleThreshold,
    pub single_wc: SingleThreshold,
    pub single_fs: SingleThreshold,
    pub single_pj: SingleThreshold,
    pub pair_fc: PairThreshold,
    pub pair_wc: PairThreshold,
    pub pair_rh: PairThreshold,
    pub sp1: [Sp1Threshold; 5],
    pub copy_pf_to_out: bool,
    pub abcp: PairThreshold,
    pub get_3: bool,
    pub three_pair_abcp_sieve: i32,
    pub three: [ThreeThreshold; 8],
}

impl Config {
    pub fn single(&self, mode: SingleMode) -> SingleThreshold {
        match mode {
            SingleMode::Bc => self.single_bc,
            SingleMode::Fz => self.single_fz,
            SingleMode::Wc => self.single_wc,
            SingleMode::Fs => self.single_fs,
            SingleMode::Pj => self.single_pj,
        }
    }

    pub fn pair(&self, mode: PairMode) -> PairThreshold {
        match mode {
            PairMode::Fc => self.pair_fc,
            PairMode::Wc => self.pair_wc,
            PairMode::Rh => self.pair_rh,
        }
    }

    pub fn load_from_root(root: &Path) -> Ds4Result<Self> {
        let json_path = root.join("config.json");
        let toml_path = root.join("config.toml");
        let txt_path = root.join("config.txt");

        if json_path.exists() {
            return Self::load_from_path(&json_path);
        }

        if toml_path.exists() {
            return Self::load_from_path(&toml_path);
        }
        if txt_path.exists() {
            return Self::load_from_path(&txt_path);
        }

        Err(Ds4Error::MissingConfig {
            checked: vec![json_path, toml_path, txt_path],
        })
    }

    pub fn load_from_path(path: &Path) -> Ds4Result<Self> {
        let data = fs::read_to_string(path)?;
        let data = data.strip_prefix('\u{feff}').unwrap_or(&data);
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return parse_ds4_json(data);
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            return parse_toml_like(data);
        }

        if data.contains('=') || data.contains("[single.") || data.contains("[pair.") {
            return parse_toml_like(data);
        }
        parse_legacy_config(data)
    }
}

fn json_i32(object: &Value, key: &str) -> i32 {
    object
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(0)
}

fn json_enabled(object: &Value, key: &str) -> bool { json_i32(object, key) != 0 }

fn parse_ds4_json(source: &str) -> Ds4Result<Config> {
    let value: Value = serde_json::from_str(source).map_err(|err| Ds4Error::parse(format!("config.json: {err}")))?;
    let team_name = value
        .get("team_name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| Ds4Error::parse("config.json 中 team_name 必须是非空字符串"))?
        .to_owned();
    let single = |key: &str| {
        let item = &value[key];
        SingleThreshold {
            score: json_i32(item, "sieve"),
            potential: json_i32(item, "ptt_sieve"),
        }
    };
    let pair = |key: &str| {
        let item = &value[key];
        PairThreshold {
            enabled: json_enabled(item, "enable"),
            sieve: json_i32(item, "sieve"),
        }
    };
    let mut sp1 = [Sp1Threshold {
        enabled: false,
        sieve: 0,
        skill_sieve: None,
    }; 5];
    for (index, key) in ["qp", "qd", "pp", "pd", "cqd"].iter().enumerate() {
        let item = &value[*key];
        sp1[index] = Sp1Threshold {
            enabled: json_enabled(item, "enable"),
            sieve: json_i32(item, "sieve"),
            skill_sieve: (index < 3).then(|| json_i32(item, "skill_sieve")),
        };
    }
    let three_config = &value["three"];
    let mut three = [ThreeThreshold {
        enabled: false,
        sieve: 0,
    }; 8];
    for (index, key) in ["ffc", "wfc", "fwc", "wwc", "rwc", "rrh", "prh", "wrh"].iter().enumerate() {
        let item = &three_config[*key];
        three[index] = ThreeThreshold {
            enabled: json_enabled(item, "enable"),
            sieve: json_i32(item, "sieve"),
        };
    }
    let get_3 = json_enabled(&value, "get_3");
    let three_pair_abcp_sieve = json_i32(three_config, "pair_abcp_sieve");
    if get_3 && three_pair_abcp_sieve <= 0 {
        return Err(Ds4Error::parse("get_3 启用时 three.pair_abcp_sieve 必须大于 0"));
    }
    Ok(Config {
        team_name: Some(team_name),
        threads: json_i32(&value, "thread_number").max(1) as usize,
        copy_to_new: json_enabled(&value, "copy_to_new"),
        run_dedup: json_enabled(&value, "run_dup"),
        single_bc: single("bc"),
        single_fz: single("fz"),
        single_wc: single("wc"),
        single_fs: single("fs"),
        single_pj: single("pj"),
        pair_fc: pair("two_fc"),
        pair_wc: pair("two_wc"),
        pair_rh: pair("two_rh"),
        sp1,
        copy_pf_to_out: json_enabled(&value, "copy_pf_to_out"),
        abcp: pair("abcp"),
        get_3,
        three_pair_abcp_sieve,
        three,
    })
}

fn parse_legacy_config(source: &str) -> Ds4Result<Config> {
    let numbers = source
        .split_whitespace()
        .map(|part| {
            part.parse::<i32>()
                .map_err(|_| Ds4Error::parse(format!("legacy config number parse failed: {part}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if numbers.len() < 19 {
        return Err(Ds4Error::parse(format!(
            "legacy config requires at least 19 numbers, got {}",
            numbers.len()
        )));
    }

    let mut idx = 0;
    let read = |idx_ref: &mut usize| -> i32 {
        let value = numbers[*idx_ref];
        *idx_ref += 1;
        value
    };

    let threads = read(&mut idx).max(1) as usize;
    let single_bc = SingleThreshold {
        score: read(&mut idx),
        potential: read(&mut idx),
    };
    let single_fz = SingleThreshold {
        score: read(&mut idx),
        potential: read(&mut idx),
    };
    let single_wc = SingleThreshold {
        score: read(&mut idx),
        potential: read(&mut idx),
    };
    let single_fs = SingleThreshold {
        score: read(&mut idx),
        potential: read(&mut idx),
    };
    let single_pj = SingleThreshold {
        score: read(&mut idx),
        potential: read(&mut idx),
    };

    let pair_fc = PairThreshold {
        enabled: read(&mut idx) != 0,
        sieve: read(&mut idx),
    };
    let pair_wc = PairThreshold {
        enabled: read(&mut idx) != 0,
        sieve: read(&mut idx),
    };
    let pair_rh = PairThreshold {
        enabled: read(&mut idx) != 0,
        sieve: read(&mut idx),
    };

    let copy_to_new = read(&mut idx) != 0;
    let run_dedup = read(&mut idx) != 0;

    Ok(Config {
        team_name: None,
        threads,
        copy_to_new,
        run_dedup,
        single_bc,
        single_fz,
        single_wc,
        single_fs,
        single_pj,
        pair_fc,
        pair_wc,
        pair_rh,
        sp1: [Sp1Threshold {
            enabled: false,
            sieve: 0,
            skill_sieve: None,
        }; 5],
        copy_pf_to_out: false,
        abcp: PairThreshold {
            enabled: false,
            sieve: 0,
        },
        get_3: false,
        three_pair_abcp_sieve: 0,
        three: [ThreeThreshold {
            enabled: false,
            sieve: 0,
        }; 8],
    })
}

fn parse_toml_like(source: &str) -> Ds4Result<Config> {
    let mut cfg = Config {
        team_name: None,
        threads: 1,
        copy_to_new: false,
        run_dedup: true,
        single_bc: SingleThreshold { score: 0, potential: 0 },
        single_fz: SingleThreshold { score: 0, potential: 0 },
        single_wc: SingleThreshold { score: 0, potential: 0 },
        single_fs: SingleThreshold { score: 0, potential: 0 },
        single_pj: SingleThreshold { score: 0, potential: 0 },
        pair_fc: PairThreshold {
            enabled: false,
            sieve: 0,
        },
        pair_wc: PairThreshold {
            enabled: false,
            sieve: 0,
        },
        pair_rh: PairThreshold {
            enabled: false,
            sieve: 0,
        },
        sp1: [Sp1Threshold {
            enabled: false,
            sieve: 0,
            skill_sieve: None,
        }; 5],
        copy_pf_to_out: false,
        abcp: PairThreshold {
            enabled: false,
            sieve: 0,
        },
        get_3: false,
        three_pair_abcp_sieve: 0,
        three: [ThreeThreshold {
            enabled: false,
            sieve: 0,
        }; 8],
    };

    let mut section: Option<String> = None;
    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line[1..line.len() - 1].trim().to_ascii_lowercase());
            continue;
        }
        let (key, value) = parse_key_value(line)?;
        let key = key.to_ascii_lowercase();
        let section_name = section.as_deref().unwrap_or("");

        match section_name {
            "" => match key.as_str() {
                "threads" => cfg.threads = parse_i32(value, line)?.max(1) as usize,
                "copy_to_new" => cfg.copy_to_new = parse_bool(value, line)?,
                "run_dedup" => cfg.run_dedup = parse_bool(value, line)?,
                _ => return Err(Ds4Error::parse(format!("unknown root key: {key}"))),
            },
            "single.bc" => apply_single(&mut cfg.single_bc, &key, value, line)?,
            "single.fz" => apply_single(&mut cfg.single_fz, &key, value, line)?,
            "single.wc" => apply_single(&mut cfg.single_wc, &key, value, line)?,
            "single.fs" => apply_single(&mut cfg.single_fs, &key, value, line)?,
            "single.pj" => apply_single(&mut cfg.single_pj, &key, value, line)?,
            "pair.fc" => apply_pair(&mut cfg.pair_fc, &key, value, line)?,
            "pair.wc" => apply_pair(&mut cfg.pair_wc, &key, value, line)?,
            "pair.rh" => apply_pair(&mut cfg.pair_rh, &key, value, line)?,
            _ => return Err(Ds4Error::parse(format!("unknown section: {section_name}"))),
        }
    }

    Ok(cfg)
}

fn parse_key_value(line: &str) -> Ds4Result<(&str, &str)> {
    let pos = line.find('=').ok_or_else(|| Ds4Error::parse(format!("missing '=' in line: {line}")))?;
    let key = line[..pos].trim();
    let value = line[pos + 1..].trim();
    if key.is_empty() || value.is_empty() {
        return Err(Ds4Error::parse(format!("invalid key/value line: {line}")));
    }
    Ok((key, value))
}

fn parse_i32(value: &str, line: &str) -> Ds4Result<i32> {
    value
        .parse::<i32>()
        .map_err(|_| Ds4Error::parse(format!("integer parse failed in line: {line}")))
}

fn parse_bool(value: &str, line: &str) -> Ds4Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(Ds4Error::parse(format!("bool parse failed in line: {line}"))),
    }
}

fn apply_single(target: &mut SingleThreshold, key: &str, value: &str, line: &str) -> Ds4Result<()> {
    match key {
        "score" => target.score = parse_i32(value, line)?,
        "potential" => target.potential = parse_i32(value, line)?,
        _ => return Err(Ds4Error::parse(format!("unknown single key: {key}"))),
    }
    Ok(())
}

fn apply_pair(target: &mut PairThreshold, key: &str, value: &str, line: &str) -> Ds4Result<()> {
    match key {
        "enabled" => target.enabled = parse_bool(value, line)?,
        "sieve" => target.sieve = parse_i32(value, line)?,
        _ => return Err(Ds4Error::parse(format!("unknown pair key: {key}"))),
    }
    Ok(())
}

#[allow(dead_code)]
fn _debug_config_path_list(root: &Path) -> [PathBuf; 2] { [root.join("config.toml"), root.join("config.txt")] }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ds4_json_requires_team_and_three_threshold() {
        assert!(parse_ds4_json(r#"{"thread_number":4}"#).is_err());
        assert!(parse_ds4_json(r#"{"team_name":"a","get_3":1}"#).is_err());
        let config = parse_ds4_json(
            r#"{"team_name":"teamA","thread_number":4,"two_fc":{"enable":1,"sieve":9400},
                "qp":{"enable":1,"sieve":5700,"skill_sieve":300},"get_3":1,
                "three":{"pair_abcp_sieve":4400,"ffc":{"enable":1,"sieve":15000}}}"#,
        )
        .expect("DS4 JSON config");
        assert_eq!(config.team_name.as_deref(), Some("teamA"));
        assert_eq!(config.threads, 4);
        assert_eq!(config.pair_fc.sieve, 9400);
        assert!(config.sp1[0].enabled);
        assert_eq!(config.sp1[0].skill_sieve, Some(300));
        assert!(config.three[0].enabled);
        assert_eq!(config.three_pair_abcp_sieve, 4400);
    }

    #[test]
    fn parse_legacy_ok() {
        let source = "32\n4100 4500\n4200 4600\n4000 4400\n3900 4300\n4100 4500\n1 9500\n1 9300\n1 9250\n1 1\n";
        let cfg = parse_legacy_config(source).expect("legacy config parse should succeed");
        assert_eq!(cfg.threads, 32);
        assert_eq!(
            cfg.single(SingleMode::Bc),
            SingleThreshold {
                score: 4100,
                potential: 4500
            }
        );
        assert!(cfg.pair(PairMode::Fc).enabled);
        assert_eq!(cfg.pair(PairMode::Wc).sieve, 9300);
        assert!(cfg.copy_to_new);
        assert!(cfg.run_dedup);
    }

    #[test]
    fn parse_toml_like_ok() {
        let source = r#"
threads = 64
copy_to_new = true
run_dedup = false

[single.bc]
score = 4100
potential = 4500

[single.fz]
score = 4200
potential = 4600

[single.wc]
score = 4000
potential = 4400

[single.fs]
score = 3900
potential = 4300

[single.pj]
score = 4100
potential = 4500

[pair.fc]
enabled = true
sieve = 9500

[pair.wc]
enabled = false
sieve = 9300

[pair.rh]
enabled = true
sieve = 9250
"#;
        let cfg = parse_toml_like(source).expect("toml-like config parse should succeed");
        assert_eq!(cfg.threads, 64);
        assert!(cfg.copy_to_new);
        assert!(!cfg.run_dedup);
        assert_eq!(cfg.single(SingleMode::Fs).score, 3900);
        assert!(!cfg.pair(PairMode::Wc).enabled);
        assert_eq!(cfg.pair(PairMode::Rh).sieve, 9250);
    }
}
