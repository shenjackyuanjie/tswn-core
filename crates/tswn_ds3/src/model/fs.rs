//! FS 单字评分入口。
//!
//! 使用 `MODEL_FS` 对输入文件逐行评分，并按基础分/潜力分阈值过滤后写出。
//! 本文件保留单线程和指定线程数两种接口，供 CLI pipeline 按配置选择。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::MODEL_FS;
use super::common::{score_file, score_file_with_threads};
use super::engine::{ScoreContext, ScoreMode};

pub fn score_file_fs(input_file: &Path, output_file: &Path, score_sieve: i32, potential_sieve: i32) -> Ds3Result<usize> {
    let ctx = ScoreContext::new();
    score_file(
        input_file,
        output_file,
        ScoreMode::Fs,
        score_sieve,
        potential_sieve,
        &MODEL_FS,
        &ctx,
    )
}

pub fn score_file_fs_with_threads(
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
        ScoreMode::Fs,
        score_sieve,
        potential_sieve,
        &MODEL_FS,
        &ctx,
        threads,
    )
}
