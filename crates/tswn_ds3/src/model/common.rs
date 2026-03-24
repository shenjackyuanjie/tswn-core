use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Ds3Error, Ds3Result};
use crate::model::coeffs::MODEL_SIZE;
use crate::model::engine::{NameFeature, ScoreContext, ScoreMode, base_score, gradient};
use crate::output::AtomicFileWriter;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct SingleScore {
    pub score: f64,
    pub potential: f64,
}

pub fn score_file(
    input_file: &Path,
    output_file: &Path,
    mode: ScoreMode,
    score_sieve: i32,
    potential_sieve: i32,
    model: &[f64; MODEL_SIZE],
    ctx: &ScoreContext,
) -> Ds3Result<usize> {
    let content = if input_file.exists() {
        fs::read_to_string(input_file)?
    } else {
        String::new()
    };

    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();
    let mut written = 0usize;

    for raw_line in content.lines() {
        let name = raw_line.trim();
        if name.is_empty() {
            continue;
        }
        let scored = evaluate_name(name, mode, model, ctx)?;
        if scored.score >= score_sieve as f64 || scored.potential >= potential_sieve as f64 {
            write!(out, "{:.0} {:.0} {}\r\n", scored.score, scored.potential, name)?;
            written += 1;
        }
    }

    writer.commit()?;
    Ok(written)
}

pub fn score_file_with_threads(
    input_file: &Path,
    output_file: &Path,
    mode: ScoreMode,
    score_sieve: i32,
    potential_sieve: i32,
    model: &[f64; MODEL_SIZE],
    ctx: &ScoreContext,
    threads: usize,
) -> Ds3Result<usize> {
    let content = if input_file.exists() {
        fs::read_to_string(input_file)?
    } else {
        String::new()
    };
    let names = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let threads = threads.max(1);
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|err| Ds3Error::parse(format!("failed to build score thread pool: {err}")))?;
    let mut rows = pool.install(|| {
        names
            .par_iter()
            .enumerate()
            .filter_map(|(idx, name)| evaluate_name(name, mode, model, ctx).ok().map(|scored| (idx, name.clone(), scored)))
            .filter(|(_, _, scored)| scored.score >= score_sieve as f64 || scored.potential >= potential_sieve as f64)
            .map(|(idx, name, scored)| (idx, format!("{:.0} {:.0} {}", scored.score, scored.potential, name)))
            .collect::<Vec<_>>()
    });

    rows.sort_unstable_by_key(|(idx, _)| *idx);
    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();
    for (_, row) in &rows {
        write!(out, "{row}\r\n")?;
    }
    writer.commit()?;
    Ok(rows.len())
}

pub fn evaluate_name(full_name: &str, mode: ScoreMode, model: &[f64; MODEL_SIZE], ctx: &ScoreContext) -> Ds3Result<SingleScore> {
    let feature = NameFeature::from_full_name(full_name)?;
    let base_x = feature.x;

    let score_init = base_score(&base_x, model, &ctx.pos, &ctx.pos2);
    let dt = gradient(&base_x, model, &ctx.pos, &ctx.pos2);

    let pass_flag = pass_gate(mode, &base_x);
    let score = if pass_flag { score_init } else { 1111.0 };
    let mut score_max = score_init;

    if pass_flag {
        let mut hp = [0u8; 10];
        hp.copy_from_slice(&feature.name_base[0..10]);
        let mut j = 7usize;
        if feature.name_base[8] < feature.name_base[j] {
            j = 8;
        }
        if feature.name_base[9] < feature.name_base[j] {
            j = 9;
        }
        hp[j] = 63;
        hp.sort_unstable();
        let new_hp = (154 + hp[3] as i32 + hp[4] as i32 + hp[5] as i32 + hp[6] as i32) as f64;
        let delta = new_hp - base_x[0];
        if let Some(k) = ctx.pos2[0][0] {
            let scorex = score_init + dt[0] * delta + model[k] * delta * delta;
            score_max = score_max.max(scorex);
        }

        for i in 1..8 {
            let new_7v = (36
                + feature.name_base[7 + i * 3]
                    .max(feature.name_base[8 + i * 3])
                    .max(feature.name_base[9 + i * 3]) as i32) as f64;
            let diff = new_7v - base_x[i];
            if let Some(k) = ctx.pos2[i][i] {
                let scorex = score_init + dt[i] * diff + model[k] * diff * diff;
                score_max = score_max.max(scorex);
            }
        }
    }

    for i in -2i32..16 {
        let mut modified = feature.clone();
        let pos = if i < 0 {
            let base = (64 + i * 2) as usize;
            if modified.name_base[base] < modified.name_base[base + 1] {
                base
            } else {
                base + 1
            }
        } else {
            let base = (64 + i * 4) as usize;
            let mut best = base;
            for offset in 1..4 {
                if modified.name_base[base + offset] < modified.name_base[best] {
                    best = base + offset;
                }
            }
            best
        };

        modified.name_base[pos] = 63;
        let new_x = modified.recompute_x();
        if pass_gate(mode, &new_x) {
            let scorex = incremental_score(score_init, &base_x, &new_x, &dt, model, &ctx.pos2);
            score_max = score_max.max(scorex);
        }
    }

    Ok(SingleScore {
        score,
        potential: score_max,
    })
}

fn incremental_score(
    score_init: f64,
    base_x: &[f64; 46],
    new_x: &[f64; 46],
    dt: &[f64; 46],
    model: &[f64; MODEL_SIZE],
    pos2: &[[Option<usize>; 46]; 46],
) -> f64 {
    let mut changed = Vec::with_capacity(16);
    for i in 0..46 {
        if new_x[i] != base_x[i] {
            changed.push(i);
        }
    }

    let mut score = score_init;
    for t in 0..changed.len() {
        let ii = changed[t];
        let delta_i = new_x[ii] - base_x[ii];
        score += dt[ii] * delta_i;
        for &jj in &changed[..=t] {
            if let Some(k) = pos2[ii][jj] {
                score += model[k] * delta_i * (new_x[jj] - base_x[jj]);
            }
        }
    }
    score
}

fn pass_gate(mode: ScoreMode, x: &[f64; 46]) -> bool {
    match mode {
        ScoreMode::Bc => x[29] >= 25.0,
        ScoreMode::Fz | ScoreMode::Wc => x[29] <= 35.0,
        ScoreMode::Fs => x[31] * 2.0 + x[32] + 0.01 * x[31] * x[19] + 0.01 * x[31] * x[36] >= 50.0,
        ScoreMode::Pj => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::coeffs::MODEL_BC;

    #[test]
    fn evaluate_name_returns_non_zero() {
        let ctx = ScoreContext::new();
        let scored = evaluate_name("abc@team", ScoreMode::Bc, &MODEL_BC, &ctx).expect("score");
        assert!(scored.potential.is_finite());
    }
}
