//! BC 单字评分入口。
//!
//! 使用 `MODEL_BC` 对输入文件逐行评分，并按基础分/潜力分阈值过滤后写出。
//! 本文件只绑定模型常量与评分模式，具体特征和文件处理逻辑位于 `model::common`。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::MODEL_BC;
use super::common::{score_file, score_file_with_threads};
use super::engine::{ScoreContext, ScoreMode};

pub fn score_file_bc(input_file: &Path, output_file: &Path, score_sieve: i32, potential_sieve: i32) -> Ds3Result<usize> {
    let ctx = ScoreContext::new();
    score_file(
        input_file,
        output_file,
        ScoreMode::Bc,
        score_sieve,
        potential_sieve,
        &MODEL_BC,
        &ctx,
    )
}

pub fn score_file_bc_with_threads(
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
        ScoreMode::Bc,
        score_sieve,
        potential_sieve,
        &MODEL_BC,
        &ctx,
        threads,
    )
}
