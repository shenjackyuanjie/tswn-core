//! DS4 的 QP/QD/PP/PD/CQD 单人评分与技能筛选。

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::OnceLock;

use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::error::{Ds4Error, Ds4Result};
use crate::model::engine::{NameFeature, PositionTables, base_score, init_pos_tables};
use crate::output::AtomicFileWriter;
use crate::sp1_coeffs::{MODEL_CQD0, MODEL_CQD3, MODEL_PD0, MODEL_PP0, MODEL_QD0, MODEL_QP0};

#[derive(Debug, Clone, Copy)]
pub enum Sp1Mode {
    Qp,
    Qd,
    Pp,
    Pd,
    Cqd,
}

impl Sp1Mode {
    pub const ALL: [Self; 5] = [Self::Qp, Self::Qd, Self::Pp, Self::Pd, Self::Cqd];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qp => "qp",
            Self::Qd => "qd",
            Self::Pp => "pp",
            Self::Pd => "pd",
            Self::Cqd => "cqd",
        }
    }

    fn model(self) -> &'static [f64; 1124] {
        match self {
            Self::Qp => &MODEL_QP0,
            Self::Qd => &MODEL_QD0,
            Self::Pp => &MODEL_PP0,
            Self::Pd => &MODEL_PD0,
            Self::Cqd => &MODEL_CQD0,
        }
    }

    fn skill_column(self) -> Option<usize> {
        match self {
            Self::Qp => Some(1),
            Self::Qd => Some(2),
            Self::Pp => Some(0),
            Self::Pd | Self::Cqd => None,
        }
    }
}

pub struct ScoreNow([[i32; 3]; 36]);

impl ScoreNow {
    pub fn load(root: &Path) -> Ds4Result<Self> {
        let source = if root.join("score_now.txt").exists() {
            fs::read_to_string(root.join("score_now.txt"))?
        } else {
            include_str!("../data/score_now.txt").to_owned()
        };
        let numbers = source
            .split_whitespace()
            .map(|part| {
                part.parse::<i32>()
                    .map_err(|_| Ds4Error::parse(format!("score_now.txt 数值无效: {part}")))
            })
            .collect::<Ds4Result<Vec<_>>>()?;
        if numbers.len() != 108 {
            return Err(Ds4Error::parse(format!(
                "score_now.txt 应有 108 个数值，实际为 {}",
                numbers.len()
            )));
        }
        let mut scores = [[0; 3]; 36];
        for (index, score) in scores.iter_mut().enumerate() {
            score.copy_from_slice(&numbers[index * 3..index * 3 + 3]);
        }
        Ok(Self(scores))
    }
}

pub fn score_file(
    input: &Path,
    output: &Path,
    skill_output: Option<(&Path, i32)>,
    mode: Sp1Mode,
    sieve: i32,
    score_now: &ScoreNow,
    threads: usize,
) -> Ds4Result<usize> {
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .map_err(|err| Ds4Error::parse(format!("SP1 线程池创建失败: {err}")))?;
    let mut writer = AtomicFileWriter::new(output)?;
    let column = if skill_output.is_some() {
        Some(mode.skill_column().ok_or_else(|| Ds4Error::parse("该 SP1 模式不支持技能筛选"))?)
    } else {
        None
    };
    let mut skill_writer = skill_output.map(|(path, _)| AtomicFileWriter::new(path)).transpose()?;
    let mut reader = BufReader::new(fs::File::open(input)?);
    let mut line = String::new();
    let mut names = Vec::with_capacity(1024);
    let mut retained = 0;
    loop {
        line.clear();
        let eof = reader.read_line(&mut line)? == 0;
        let name = line.trim();
        if !name.is_empty() {
            names.push(name.to_owned());
        }
        if names.len() == 1024 || eof {
            let rows = pool.install(|| names.par_iter().map(|name| score_one(name, mode)).collect::<Ds4Result<Vec<_>>>())?;
            for (name, (score, skills)) in names.iter().zip(rows) {
                if score >= sieve as f64 {
                    write!(writer.writer(), "{score:.0} {name}\r\n")?;
                    retained += 1;
                }
                if let (Some(column), Some((_, skill_sieve)), Some(skill_writer)) = (column, skill_output, skill_writer.as_mut())
                {
                    for skill in skills {
                        let maximum = score_now.0[skill][column];
                        if score >= (maximum - skill_sieve) as f64 {
                            write!(skill_writer.writer(), "{score:.0} {maximum} {name}\r\n")?;
                        }
                    }
                }
            }
            names.clear();
        }
        if eof {
            break;
        }
    }
    writer.commit()?;
    if let Some(skill_writer) = skill_writer {
        skill_writer.commit()?;
    }
    Ok(retained)
}

fn score_one(name: &str, mode: Sp1Mode) -> Ds4Result<(f64, Vec<usize>)> {
    let feature = NameFeature::from_full_name(name)?;
    let (x, skills) = sp1_features(&feature);
    static TABLES: OnceLock<PositionTables> = OnceLock::new();
    let (pos, pos2) = TABLES.get_or_init(init_pos_tables);
    let mut score = base_score(&x, mode.model(), pos, pos2).max(0.0);
    if matches!(mode, Sp1Mode::Cqd) {
        if x[32] >= 30.0 {
            score = score.max(base_score(&x, &MODEL_CQD3, pos, pos2));
        }
        score *= 100.0;
    }
    Ok((score, skills))
}

fn sp1_features(feature: &NameFeature) -> ([f64; 46], Vec<usize>) {
    let mut x = feature.x;
    x[8..].fill(0.0);
    let mut freq = [0u8; 16];
    for (index, value) in freq.iter_mut().enumerate() {
        let start = 64 + index * 4;
        let minimum = feature.name_base[start..start + 4].iter().copied().min().unwrap_or(0);
        *value = if minimum > 10 && feature.skill[index] < 35 {
            minimum - 10
        } else {
            0
        };
    }
    if feature.last >= 0 {
        let index = feature.last as usize;
        freq[index] = freq[index].wrapping_mul(2);
    }
    if freq[14] != 0 && feature.last != 14 {
        freq[14] = freq[14].wrapping_add(feature.name_base[60].min(feature.name_base[61]).min(freq[14]));
    }
    if freq[15] != 0 && feature.last != 15 {
        freq[15] = freq[15].wrapping_add(feature.name_base[62].min(feature.name_base[63]).min(freq[15]));
    }
    let maximum = freq.iter().copied().max().unwrap_or(0);
    let skills = if maximum < 30 {
        vec![0]
    } else {
        freq.iter()
            .enumerate()
            .filter(|(_, value)| **value == maximum)
            .map(|(index, _)| feature.skill[index] as usize + 1)
            .collect()
    };

    let mut zd = 1.0;
    let mut kill = 1.0;
    for (index, frequency) in freq.iter().enumerate() {
        let skill = feature.skill[index] as usize;
        let amount = *frequency as f64;
        if skill < 25 {
            let coefficient = match skill {
                9 | 16 => 0.3,
                18 => 0.35,
                19 | 23 => 0.6,
                20 | 22 => 0.7,
                _ => 1.0,
            };
            x[skill + 8] = zd * amount;
            zd *= 1.0 - amount * coefficient / 128.0;
        } else if skill == 31 || skill == 32 {
            x[skill + 8] = kill * amount;
            kill *= 1.0 - amount / 128.0;
        } else if skill + 8 < x.len() {
            x[skill + 8] = amount;
        }
    }
    if x[37] <= 70.0 {
        x[37] = x[37] * x[37] / 70.0;
    } else {
        x[37] = x[37] * 2.0 - 70.0;
    }
    x[43] = if x[32] > 0.0 {
        feature.shadowi_sp1 * x[32] / 100.0
    } else {
        0.0
    };
    if x[42] > 0.0 {
        x[44] = 1.0;
    }
    if x[37] > 0.0 {
        x[45] = x[26];
        x[26] = 0.0;
    }
    (x, skills)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sp1_scores_match_ds4_cpp_sample() {
        let cases = [
            (Sp1Mode::Qp, [2238.0, 0.0, 1371.0]),
            (Sp1Mode::Qd, [0.0, 0.0, 0.0]),
            (Sp1Mode::Pp, [4162.0, 3090.0, 3073.0]),
            (Sp1Mode::Pd, [4982.0, 2983.0, 3234.0]),
            (Sp1Mode::Cqd, [0.0, 0.0, 0.0]),
        ];
        for (mode, expected) in cases {
            for (name, score) in ["alpha@teamA", "beta@teamA", "gamma@teamA"].into_iter().zip(expected) {
                let (actual, _) = score_one(name, mode).expect("SP1 score");
                assert_eq!(format!("{actual:.0}"), format!("{score:.0}"), "{name} {mode:?}");
            }
        }
    }
}
