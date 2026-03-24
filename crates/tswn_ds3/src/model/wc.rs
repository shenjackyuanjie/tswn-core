use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::MODEL_WC;
use super::common::{score_file, score_file_with_threads};
use super::engine::{ScoreContext, ScoreMode};

pub fn score_file_wc(input_file: &Path, output_file: &Path, score_sieve: i32, potential_sieve: i32) -> Ds3Result<usize> {
    let ctx = ScoreContext::new();
    score_file(
        input_file,
        output_file,
        ScoreMode::Wc,
        score_sieve,
        potential_sieve,
        &MODEL_WC,
        &ctx,
    )
}

pub fn score_file_wc_with_threads(
    input_file: &Path,
    output_file: &Path,
    score_sieve: i32,
    potential_sieve: i32,
    threads: usize,
) -> Ds3Result<usize> {
    let ctx = ScoreContext::new();
    score_file_with_threads(
        input_file,
        output_file,
        ScoreMode::Wc,
        score_sieve,
        potential_sieve,
        &MODEL_WC,
        &ctx,
        threads,
    )
}
