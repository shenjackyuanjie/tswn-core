//! FZ 单字评分入口。
//!
//! 使用 `MODEL_FZ` 对输入文件逐行评分，并按基础分/潜力分阈值过滤后写出。
//! 作为模式适配层，它只负责传入正确的模型系数和 `ScoreMode::Fz`。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::MODEL_FZ;
use super::common::{score_file, score_file_with_threads};
use super::engine::{ScoreContext, ScoreMode};

pub fn score_file_fz(input_file: &Path, output_file: &Path, score_sieve: i32, potential_sieve: i32) -> Ds3Result<usize> {
    let ctx = ScoreContext::new();
    score_file(
        input_file,
        output_file,
        ScoreMode::Fz,
        score_sieve,
        potential_sieve,
        &MODEL_FZ,
        &ctx,
    )
}

pub fn score_file_fz_with_threads(
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
        ScoreMode::Fz,
        score_sieve,
        potential_sieve,
        &MODEL_FZ,
        &ctx,
        threads,
    )
}
