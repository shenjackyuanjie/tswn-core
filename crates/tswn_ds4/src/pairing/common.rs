//! 字对配对的通用执行逻辑。
//!
//! 负责读取已评分的左右候选、预计算单字特征与梯度，并按模式遍历同集合或左右集合配对。
//! 为了贴近 C++ 原版，部分边界行为会保留原实现的特殊循环语义。

use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use crate::error::{Ds4Error, Ds4Result};
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
) -> Ds4Result<usize> {
    run_pair_mode_with_threads(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        mode,
        model_left,
        model_right,
        1,
    )
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
) -> Ds4Result<usize> {
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
        .map_err(|err| Ds4Error::parse(format!("failed to build pairing thread pool: {err}")))?;
    let mut out = BufWriter::new(OpenOptions::new().create(true).append(true).open(output_file)?);
    let mut total = 0;
    for (index, left_row) in left.iter().enumerate() {
        let candidates = if type_same_set { &left[index + 1..] } else { &right[..] };
        for tile in candidates.chunks(4096) {
            let rows = pool.install(|| {
                tile.par_iter()
                    .map(|right_row| {
                        if left_row.name == right_row.name {
                            return None;
                        }
                        let score = pair_score(left_row, right_row, mode, model_left, model_right, &pos2);
                        (score >= sieve).then(|| format!("{score} {}+{}", left_row.name, right_row.name))
                    })
                    .collect::<Vec<_>>()
            });
            for row in rows.into_iter().flatten() {
                write!(out, "{row}\r\n")?;
                total += 1;
            }
        }
    }
    out.flush()?;
    Ok(total)
}

fn load_scored_rows(
    path: &Path,
    model: &[f64; 1124],
    pos: &[usize; 46],
    pos2: &[[Option<usize>; 46]; 46],
) -> Ds4Result<Vec<PairRow>> {
    let mut rows = Vec::new();
    if !path.exists() {
        return Ok(rows);
    }
    for line in BufReader::new(fs::File::open(path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record = if let Some(parsed) = InputRecord::parse_scored_line(&line) {
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

    // C++ 从原始下标开始存储 NAME_SELF。若 l > 0，输出 C 字符串以 '\0' 开头，因此 printf("%s") 打印为空。
    if l > 0 {
        return String::new();
    }
    String::from_utf8_lossy(&bytes[..=(r as usize)]).to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::pairing::coeffs::{MODEL_BC, MODEL_FZ};

    #[test]
    fn cross_set_pairs_visit_every_right_row_and_skip_identical_names() {
        let root = std::env::temp_dir().join(format!("tswn-ds4-pair-cross-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create temp dir");
        let left = root.join("left.txt");
        let right = root.join("right.txt");
        let output = root.join("output.txt");
        fs::write(&left, "5000 5000 a@team\n").expect("write left");
        fs::write(&right, "5000 5000 a@team\n5000 5000 b@team\n5000 5000 c@team\n").expect("write right");

        let count = run_pair_mode_with_threads(
            false,
            &left,
            &right,
            &output,
            -100_000,
            PairScoreMode::FcLeftFzRightBc,
            &MODEL_FZ,
            &MODEL_BC,
            2,
        )
        .expect("pair candidates");
        let rows = fs::read_to_string(&output).expect("read output");
        assert_eq!(count, 2);
        assert!(rows.contains("a@team+b@team"));
        assert!(rows.contains("a@team+c@team"));
        assert!(!rows.contains("a@team+a@team"));
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
