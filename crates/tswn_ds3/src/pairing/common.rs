use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::error::{Ds3Error, Ds3Result};
use crate::input::InputRecord;
use crate::model::engine::{NameFeature, init_pos_tables};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct PairResult {
    pub score: i32,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PairScoreMode {
    Wc,
    FcLeftFzRightBc,
    RhLeftFsRightPj,
}

#[derive(Debug, Clone)]
pub struct PairRow {
    pub name: String,
    pub feature: NameFeature,
    pub score_init: f64,
    pub dt: [f64; 46],
}

pub fn run_pair_mode(
    type_same_set: bool,
    left_file: &Path,
    right_file: &Path,
    output_file: &Path,
    sieve: i32,
    mode: PairScoreMode,
    model_left: &[f64; 1124],
    model_right: &[f64; 1124],
) -> Ds3Result<usize> {
    let (pos, pos2) = init_pos_tables();
    let left = load_scored_rows(left_file, model_left, &pos, &pos2)?;
    let right = load_scored_rows(right_file, model_right, &pos, &pos2)?;

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = OpenOptions::new().create(true).append(true).open(output_file)?;
    let mut total = 0usize;

    if type_same_set {
        for i in 0..left.len() {
            for j in (i + 1)..left.len() {
                let score = pair_score(&left[i], &left[j], mode, model_left, model_right, &pos2);
                if score >= sieve {
                    write!(out, "{} {}+{}\r\n", score, left[i].name, left[j].name)?;
                    total += 1;
                }
            }
        }
        // Match C++ two_*.cpp task loop off-by-one behavior for TYPE=0:
        // one extra pair is evaluated at (last_left, tot_1). When a second
        // file has at least one parsed row, tot_1 maps to right[0]; otherwise
        // it reads out-of-bounds memory. We emulate the empty-right case with
        // a deterministic zeroed ghost row.
        if !left.is_empty() {
            let li = left.len() - 1;
            let r0 = right.first().cloned().unwrap_or_else(ghost_row);
            let score = pair_score(&left[li], &r0, mode, model_left, model_right, &pos2);
            if score >= sieve {
                write!(out, "{} {}+{}\r\n", score, left[li].name, r0.name)?;
                total += 1;
            }
        }
    } else {
        if let Some(r) = right.first() {
            for l in &left {
                let score = pair_score(l, r, mode, model_left, model_right, &pos2);
                if score >= sieve {
                    write!(out, "{} {}+{}\r\n", score, l.name, r.name)?;
                    total += 1;
                }
            }
        }
    }

    out.flush()?;
    Ok(total)
}

pub fn run_pair_mode_with_threads(
    type_same_set: bool,
    left_file: &Path,
    right_file: &Path,
    output_file: &Path,
    sieve: i32,
    mode: PairScoreMode,
    model_left: &[f64; 1124],
    model_right: &[f64; 1124],
    threads: usize,
) -> Ds3Result<usize> {
    let (pos, pos2) = init_pos_tables();
    let left = load_scored_rows(left_file, model_left, &pos, &pos2)?;
    let right = load_scored_rows(right_file, model_right, &pos, &pos2)?;
    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let threads = threads.max(1);
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|err| Ds3Error::parse(format!("failed to build pairing thread pool: {err}")))?;
    let mut rows = pool.install(|| {
        if type_same_set {
            (0..left.len())
                .into_par_iter()
                .map(|i| {
                    let mut local = Vec::new();
                    for j in (i + 1)..left.len() {
                        let score = pair_score(&left[i], &left[j], mode, model_left, model_right, &pos2);
                        if score >= sieve {
                            local.push((i, j, format!("{score} {}+{}", left[i].name, left[j].name)));
                        }
                    }
                    local
                })
                .reduce(Vec::new, |mut a, mut b| {
                    a.append(&mut b);
                    a
                })
        } else {
            if let Some(r) = right.first() {
                left.par_iter()
                    .enumerate()
                    .filter_map(|(li, left_row)| {
                        let score = pair_score(left_row, r, mode, model_left, model_right, &pos2);
                        (score >= sieve).then(|| (li, 0usize, format!("{score} {}+{}", left_row.name, r.name)))
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        }
    });
    if type_same_set && !left.is_empty() {
        let li = left.len() - 1;
        let r0 = right.first().cloned().unwrap_or_else(ghost_row);
        let score = pair_score(&left[li], &r0, mode, model_left, model_right, &pos2);
        if score >= sieve {
            rows.push((li, left.len(), format!("{score} {}+{}", left[li].name, r0.name)));
        }
    }

    rows.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut out = OpenOptions::new().create(true).append(true).open(output_file)?;
    for (_, _, row) in &rows {
        write!(out, "{row}\r\n")?;
    }
    out.flush()?;
    Ok(rows.len())
}

fn load_scored_rows(
    path: &Path,
    model: &[f64; 1124],
    pos: &[usize; 46],
    pos2: &[[Option<usize>; 46]; 46],
) -> Ds3Result<Vec<PairRow>> {
    let mut rows = Vec::new();
    if !path.exists() {
        return Ok(rows);
    }
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record = if let Some(parsed) = InputRecord::parse_scored_line(line) {
            parsed
        } else {
            continue;
        };
        let feature = NameFeature::from_full_name(&record.name)?;
        let x = feature.x;
        let score_init = record.raw_score;
        let mut dt = [0.0f64; 46];
        for i in 0..46 {
            dt[i] = model[pos[i]];
            for j in 0..46 {
                if let Some(k) = pos2[i][j] {
                    dt[i] += (1.0 + if i == j { 1.0 } else { 0.0 }) * model[k] * x[j];
                }
            }
        }
        rows.push(PairRow {
            name: pair_output_name_like_cpp(&record.name),
            feature,
            score_init,
            dt,
        });
    }
    Ok(rows)
}

fn pair_score(
    left: &PairRow,
    right: &PairRow,
    mode: PairScoreMode,
    model_left: &[f64; 1124],
    model_right: &[f64; 1124],
    pos2: &[[Option<usize>; 46]; 46],
) -> i32 {
    let mut lx = left.feature.clone();
    let mut rx = right.feature.clone();

    for k in 7..128 {
        if right.feature.name_base[k - 1] == left.feature.name_base[k] {
            lx.name_base[k] = lx.name_base[k].max(right.feature.name_base[k]);
        }
    }
    for k in 7..128 {
        if left.feature.name_base[k - 1] == right.feature.name_base[k] {
            rx.name_base[k] = rx.name_base[k].max(left.feature.name_base[k]);
        }
    }

    let left_new = lx.recompute_x();
    let right_new = rx.recompute_x();

    let left_ok = match mode {
        PairScoreMode::Wc => left_new[29] <= 35.0,
        PairScoreMode::FcLeftFzRightBc => left_new[29] <= 35.0,
        PairScoreMode::RhLeftFsRightPj => {
            left_new[31] * 2.0 + left_new[32] + 0.01 * left_new[31] * left_new[19] + 0.01 * left_new[31] * left_new[36] >= 50.0
        }
    };
    let right_ok = match mode {
        PairScoreMode::Wc => right_new[29] <= 35.0,
        PairScoreMode::FcLeftFzRightBc => right_new[29] >= 25.0,
        PairScoreMode::RhLeftFsRightPj => true,
    };

    let sx = if left_ok {
        incremental_from_dt(left.score_init, &left.feature.x, &left_new, &left.dt, model_left, pos2)
    } else {
        0.0
    };
    let sy = if right_ok {
        incremental_from_dt(right.score_init, &right.feature.x, &right_new, &right.dt, model_right, pos2)
    } else {
        0.0
    };
    (sx + sy) as i32
}

fn incremental_from_dt(
    score_init: f64,
    old_x: &[f64; 46],
    new_x: &[f64; 46],
    dt: &[f64; 46],
    model: &[f64; 1124],
    pos2: &[[Option<usize>; 46]; 46],
) -> f64 {
    let mut changed = Vec::new();
    for i in 0..46 {
        if old_x[i] != new_x[i] {
            changed.push(i);
        }
    }
    let mut score = score_init;
    for t in 0..changed.len() {
        let i = changed[t];
        let di = new_x[i] - old_x[i];
        score += dt[i] * di;
        for &j in &changed[..=t] {
            if let Some(k) = pos2[i][j] {
                score += model[k] * di * (new_x[j] - old_x[j]);
            }
        }
    }
    score
}

fn pair_output_name_like_cpp(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return String::new();
    }

    let mut l = 0i32;
    let mut r = bytes.len() as i32 - 1;

    while l <= r && (bytes[l as usize] == 0 || bytes[l as usize] == b' ') {
        l += 1;
    }
    while l <= r && (bytes[r as usize] == 0 || bytes[r as usize] == b' ') {
        r -= 1;
    }

    while l <= r && (bytes[l as usize] == 0 || bytes[l as usize] == b' ' || bytes[l as usize] == b'+') {
        l += 1;
    }
    while l <= r && (bytes[r as usize] == 0 || bytes[r as usize] == b' ' || bytes[r as usize] == b'+') {
        r -= 1;
    }

    if l > r {
        return String::new();
    }

    // C++ stores NAME_SELF starting at original indices. If l > 0, the
    // output C string starts with '\0', so printf("%s") prints empty.
    if l > 0 {
        return String::new();
    }
    String::from_utf8_lossy(&bytes[..=(r as usize)]).to_string()
}

fn ghost_row() -> PairRow {
    PairRow {
        name: String::new(),
        feature: NameFeature {
            name: String::new(),
            name_base: [0; 128],
            skill: [0; 40],
            val_base: [0; 256],
            x: [0.0; 46],
            last: 0,
            freq14: false,
            freq15: false,
            shadowi: 0.0,
            shadowcfz: 0,
        },
        score_init: 0.0,
        dt: [0.0; 46],
    }
}
