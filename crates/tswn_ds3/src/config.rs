//! 工具全局配置与模式定义。
//!
//! 定义 [`Config`] 结构体（从文件加载的运行时配置）、单字评分模式枚举
//! [`SingleMode`]（Bc/Fz/Wc/Fs/Pj）及字对评分模式枚举 [`PairMode`]（Fc/Wc/Rh）。

#![allow(dead_code)]

use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Ds3Error, Ds3Result};

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
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

    pub fn load_from_root(root: &Path) -> Ds3Result<Self> {
        let toml_path = root.join("config.toml");
        let txt_path = root.join("config.txt");

        if toml_path.exists() {
            return Self::load_from_path(&toml_path);
        }
        if txt_path.exists() {
            return Self::load_from_path(&txt_path);
        }

        Err(Ds3Error::MissingConfig {
            checked: vec![toml_path, txt_path],
        })
    }

    pub fn load_from_path(path: &Path) -> Ds3Result<Self> {
        let data = fs::read_to_string(path)?;
        let data = data.strip_prefix('\u{feff}').unwrap_or(&data);
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

fn parse_legacy_config(source: &str) -> Ds3Result<Config> {
    let numbers = source
        .split_whitespace()
        .map(|part| {
            part.parse::<i32>()
                .map_err(|_| Ds3Error::parse(format!("legacy config number parse failed: {part}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if numbers.len() < 19 {
        return Err(Ds3Error::parse(format!(
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
    })
}

fn parse_toml_like(source: &str) -> Ds3Result<Config> {
    let mut cfg = Config {
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
                _ => return Err(Ds3Error::parse(format!("unknown root key: {key}"))),
            },
            "single.bc" => apply_single(&mut cfg.single_bc, &key, value, line)?,
            "single.fz" => apply_single(&mut cfg.single_fz, &key, value, line)?,
            "single.wc" => apply_single(&mut cfg.single_wc, &key, value, line)?,
            "single.fs" => apply_single(&mut cfg.single_fs, &key, value, line)?,
            "single.pj" => apply_single(&mut cfg.single_pj, &key, value, line)?,
            "pair.fc" => apply_pair(&mut cfg.pair_fc, &key, value, line)?,
            "pair.wc" => apply_pair(&mut cfg.pair_wc, &key, value, line)?,
            "pair.rh" => apply_pair(&mut cfg.pair_rh, &key, value, line)?,
            _ => return Err(Ds3Error::parse(format!("unknown section: {section_name}"))),
        }
    }

    Ok(cfg)
}

fn parse_key_value(line: &str) -> Ds3Result<(&str, &str)> {
    let pos = line.find('=').ok_or_else(|| Ds3Error::parse(format!("missing '=' in line: {line}")))?;
    let key = line[..pos].trim();
    let value = line[pos + 1..].trim();
    if key.is_empty() || value.is_empty() {
        return Err(Ds3Error::parse(format!("invalid key/value line: {line}")));
    }
    Ok((key, value))
}

fn parse_i32(value: &str, line: &str) -> Ds3Result<i32> {
    value
        .parse::<i32>()
        .map_err(|_| Ds3Error::parse(format!("integer parse failed in line: {line}")))
}

fn parse_bool(value: &str, line: &str) -> Ds3Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(Ds3Error::parse(format!("bool parse failed in line: {line}"))),
    }
}

fn apply_single(target: &mut SingleThreshold, key: &str, value: &str, line: &str) -> Ds3Result<()> {
    match key {
        "score" => target.score = parse_i32(value, line)?,
        "potential" => target.potential = parse_i32(value, line)?,
        _ => return Err(Ds3Error::parse(format!("unknown single key: {key}"))),
    }
    Ok(())
}

fn apply_pair(target: &mut PairThreshold, key: &str, value: &str, line: &str) -> Ds3Result<()> {
    match key {
        "enabled" => target.enabled = parse_bool(value, line)?,
        "sieve" => target.sieve = parse_i32(value, line)?,
        _ => return Err(Ds3Error::parse(format!("unknown pair key: {key}"))),
    }
    Ok(())
}

#[allow(dead_code)]
fn _debug_config_path_list(root: &Path) -> [PathBuf; 2] { [root.join("config.toml"), root.join("config.txt")] }

#[cfg(test)]
mod tests {
    use super::*;

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
