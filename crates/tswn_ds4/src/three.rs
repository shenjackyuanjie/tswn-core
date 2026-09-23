//! DS4 八类三人组的增量配对与评分。

use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;

use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

use crate::abcp::fs_gate;
use crate::config::SingleMode;
use crate::error::{Ds4Error, Ds4Result};
use crate::model::coeffs::{MODEL_BC, MODEL_FS, MODEL_FZ, MODEL_PJ, MODEL_WC};
use crate::model::engine::{NameFeature, PositionTables, gradient, init_pos_tables};

#[derive(Debug, Clone, Copy)]
pub enum ThreeMode {
    Ffc,
    Wfc,
    Fwc,
    Wwc,
    Rwc,
    Rrh,
    Prh,
    Wrh,
}

impl ThreeMode {
    pub const ALL: [Self; 8] = [
        Self::Ffc,
        Self::Wfc,
        Self::Fwc,
        Self::Wwc,
        Self::Rwc,
        Self::Rrh,
        Self::Prh,
        Self::Wrh,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ffc => "FFC",
            Self::Wfc => "WFC",
            Self::Fwc => "FWC",
            Self::Wwc => "WWC",
            Self::Rwc => "RWC",
            Self::Rrh => "RRH",
            Self::Prh => "PRH",
            Self::Wrh => "WRH",
        }
    }

    pub const fn single_mode(self) -> SingleMode {
        match self {
            Self::Ffc | Self::Fwc => SingleMode::Fz,
            Self::Wfc | Self::Wwc | Self::Wrh => SingleMode::Wc,
            Self::Rwc | Self::Rrh => SingleMode::Fs,
            Self::Prh => SingleMode::Pj,
        }
    }

    pub const fn pair_type(self) -> &'static str {
        match self {
            Self::Ffc | Self::Wfc => "FC",
            Self::Fwc | Self::Wwc | Self::Rwc => "WC",
            Self::Rrh | Self::Prh | Self::Wrh => "RH",
        }
    }

    fn member_modes(self) -> [SingleMode; 3] {
        let single = self.single_mode();
        match self.pair_type() {
            "FC" => [single, SingleMode::Fz, SingleMode::Bc],
            "WC" => [single, SingleMode::Wc, SingleMode::Wc],
            _ => [single, SingleMode::Fs, SingleMode::Pj],
        }
    }
}

#[derive(Clone)]
struct Member {
    feature: NameFeature,
    original_base: [u8; 128],
    score_init: f64,
    gradient: [f64; 46],
}

#[derive(Clone)]
struct Pair {
    first: Member,
    second: Member,
}

fn position_tables() -> &'static PositionTables {
    static TABLES: OnceLock<PositionTables> = OnceLock::new();
    TABLES.get_or_init(init_pos_tables)
}

fn model(mode: SingleMode) -> &'static [f64; 1124] {
    match mode {
        SingleMode::Bc => &MODEL_BC,
        SingleMode::Fz => &MODEL_FZ,
        SingleMode::Wc => &MODEL_WC,
        SingleMode::Fs => &MODEL_FS,
        SingleMode::Pj => &MODEL_PJ,
    }
}

fn member(name: &str, score_init: f64, mode: SingleMode) -> Ds4Result<Member> {
    let feature = NameFeature::from_full_name(name)?;
    let original_base = feature.name_base;
    let (pos, pos2) = position_tables();
    let gradient = gradient(&feature.x, model(mode), pos, pos2);
    Ok(Member {
        feature,
        original_base,
        score_init,
        gradient,
    })
}

fn read_singles(path: &Path, mode: SingleMode) -> Ds4Result<Vec<Member>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for line in BufReader::new(fs::File::open(path)?).lines() {
        let line = line?;
        let Some((score, rest)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(score) = score.parse::<f64>() else {
            continue;
        };
        let Some((ptt, name)) = rest.trim_start().split_once(char::is_whitespace) else {
            continue;
        };
        if ptt.parse::<f64>().is_ok() && !name.trim().is_empty() {
            rows.push(member(name.trim(), score, mode)?);
        }
    }
    Ok(rows)
}

fn read_pairs(path: &Path, modes: [SingleMode; 3]) -> Ds4Result<Vec<Pair>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for line in BufReader::new(fs::File::open(path)?).lines() {
        let line = line?;
        let Some((score, names)) = line.trim_end_matches('\r').split_once(char::is_whitespace) else {
            continue;
        };
        let score = score
            .parse::<f64>()
            .map_err(|_| Ds4Error::parse(format!("三人组二人输入分数无效: {score}")))?;
        let Some((first, second)) = names.trim().split_once('+') else {
            continue;
        };
        let mut first = member(first.trim(), score, modes[1])?;
        let mut second = member(second.trim(), 0.0, modes[2])?;
        for k in 7..128 {
            if second.original_base[k - 1] == first.original_base[k] {
                first.feature.name_base[k] = first.feature.name_base[k].max(second.original_base[k]);
            }
            if first.original_base[k - 1] == second.original_base[k] {
                second.feature.name_base[k] = second.feature.name_base[k].max(first.original_base[k]);
            }
        }
        for (part, mode) in [(&mut first, modes[1]), (&mut second, modes[2])] {
            part.feature.x = part.feature.recompute_x();
            let (pos, pos2) = position_tables();
            part.gradient = gradient(&part.feature.x, model(mode), pos, pos2);
        }
        rows.push(Pair { first, second });
    }
    Ok(rows)
}

fn score_member(base: &Member, new: &[f64; 46], mode: SingleMode, gate: bool) -> f64 {
    if !gate {
        return 0.0;
    }
    let old = &base.feature.x;
    let (_, pos2) = position_tables();
    let mut score = base.score_init;
    let changed_indices = (0..46).filter(|index| old[*index] != new[*index]).collect::<Vec<_>>();
    for (position, index) in changed_indices.iter().enumerate() {
        let delta = new[*index] - old[*index];
        score += base.gradient[*index] * delta;
        for previous in &changed_indices[..=position] {
            if let Some(coefficient) = pos2[*index][*previous] {
                score += model(mode)[coefficient] * delta * (new[*previous] - old[*previous]);
            }
        }
    }
    score
}

fn gate(mode: SingleMode, x: &[f64; 46]) -> bool {
    match mode {
        SingleMode::Bc => x[29] >= 25.0,
        SingleMode::Fz | SingleMode::Wc => x[29] <= 35.0,
        SingleMode::Fs => fs_gate(x),
        SingleMode::Pj => true,
    }
}

fn score_triple(single: &Member, pair: &Pair, modes: [SingleMode; 3], sieve: i32, rh_y_uses_x_gate: bool) -> Option<String> {
    if single.feature.name == pair.first.feature.name || single.feature.name == pair.second.feature.name {
        return None;
    }
    let mut x = single.feature.clone();
    let mut y = pair.first.feature.clone();
    let mut z = pair.second.feature.clone();
    for k in 7..128 {
        if pair.first.original_base[k - 1] == single.original_base[k] {
            x.name_base[k] = single.original_base[k].max(pair.first.original_base[k]);
        }
        if pair.second.original_base[k - 1] == single.original_base[k] {
            x.name_base[k] = single.original_base[k].max(pair.second.original_base[k]);
        }
        if single.original_base[k - 1] == pair.first.original_base[k] {
            y.name_base[k] = pair.first.original_base[k].max(single.original_base[k]);
        }
        if single.original_base[k - 1] == pair.second.original_base[k] {
            z.name_base[k] = pair.second.original_base[k].max(single.original_base[k]);
        }
    }
    let xx = x.recompute_x();
    let yy = y.recompute_x();
    let zz = z.recompute_x();
    let gate_y = if rh_y_uses_x_gate {
        gate(modes[1], &xx)
    } else {
        gate(modes[1], &yy)
    };
    let total = score_member(single, &xx, modes[0], gate(modes[0], &xx))
        + score_member(&pair.first, &yy, modes[1], gate_y)
        + score_member(&pair.second, &zz, modes[2], gate(modes[2], &zz));
    let score = total as i32;
    (score >= sieve).then(|| {
        format!(
            "{score} {}+{}+{}\r\n",
            single.feature.name, pair.first.feature.name, pair.second.feature.name
        )
    })
}

pub fn run_incremental(root: &Path, mode: ThreeMode, sieve: i32, threads: usize) -> Ds4Result<usize> {
    let single = mode.single_mode().as_str();
    let pair = mode.pair_type();
    let new_single = root.join("tmp").join(format!("new_{single}.txt"));
    let old_single = root.join("file").join(format!("old_{single}.txt"));
    let new_pair = root.join("tmp").join(format!("new_three_{pair}.txt"));
    let old_pair = root.join("file").join(format!("old_three_{pair}.txt"));
    let output = root.join("3ren").join(format!("{}.txt", mode.as_str()));
    let modes = mode.member_modes();
    let mut writer = BufWriter::new(OpenOptions::new().create(true).append(true).open(output)?);
    let mut count = 0;
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
        .map_err(|err| Ds4Error::parse(format!("三人组线程池创建失败: {err}")))?;
    for (single_path, pair_path) in [(&new_single, &new_pair), (&new_single, &old_pair), (&old_single, &new_pair)] {
        let singles = read_singles(single_path, modes[0])?;
        let pairs = read_pairs(pair_path, modes)?;
        let rh_y_uses_x_gate = mode.pair_type() == "RH";
        for single in &singles {
            for tile in pairs.chunks(2048) {
                let rows = pool.install(|| {
                    tile.par_iter()
                        .map(|pair| score_triple(single, pair, modes, sieve, rh_y_uses_x_gate))
                        .collect::<Vec<_>>()
                });
                for row in rows.into_iter().flatten() {
                    writer.write_all(row.as_bytes())?;
                    count += 1;
                }
            }
        }
    }
    writer.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_modes_match_ds4_cpp_sample() {
        let root = std::env::temp_dir().join(format!("tswn-ds4-three-{}", std::process::id()));
        for directory in ["tmp", "file", "3ren"] {
            fs::create_dir_all(root.join(directory)).expect("create directory");
        }
        let singles = "5000 5000 alpha@teamA\n5100 5100 delta@teamA\n";
        let pairs = "9000 alphaa@teamA+alphab@teamA\n9100 beta@teamA+gamma@teamA\n";
        for kind in ["fz", "wc", "fs", "pj"] {
            fs::write(root.join("tmp").join(format!("new_{kind}.txt")), singles).expect("write singles");
        }
        for kind in ["FC", "WC", "RH"] {
            fs::write(root.join("tmp").join(format!("new_three_{kind}.txt")), pairs).expect("write pairs");
        }
        let expected = [
            [14000, 14100, 14199, 14200],
            [14000, 14100, 14642, 14200],
            [14000, 14100, 14624, 14317],
            [14000, 14100, 15067, 14317],
            [9000, 9100, 9493, 9217],
            [0, 0, 117, 32],
            [5000, 5000, 5472, 5132],
            [5000, 5000, 5691, 5132],
        ];
        for (mode, scores) in ThreeMode::ALL.into_iter().zip(expected) {
            assert_eq!(run_incremental(&root, mode, -100_000, 2).expect("run three"), 4);
            let output = fs::read_to_string(root.join("3ren").join(format!("{}.txt", mode.as_str()))).expect("read output");
            let actual = output
                .lines()
                .map(|line| line.split_whitespace().next().unwrap().parse::<i32>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, scores, "{}", mode.as_str());
        }
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
