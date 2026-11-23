//! PJ 单字评分入口。
//!
//! 使用 `MODEL_PJ` 对输入文件逐行评分，并按基础分/潜力分阈值过滤后写出。
//! 单线程与多线程版本共享同一套特征提取和潜力分估算规则。

use std::path::Path;

use crate::error::Ds4Result;

use super::coeffs::MODEL_PJ;
use super::common::{score_file, score_file_with_threads};
use super::engine::{ScoreContext, ScoreMode};

/// 以 pj 模式串行评分单个输入文件并原子写出结果，返回写入行数。
pub fn score_file_pj(input_file: &Path, output_file: &Path, score_sieve: i32, potential_sieve: i32) -> Ds4Result<usize> {
    let ctx = ScoreContext::new();
    score_file(
        input_file,
        output_file,
        ScoreMode::Pj,
        score_sieve,
        potential_sieve,
        &MODEL_PJ,
        &ctx,
    )
}

/// pj 模式的并行版本：按 threads 并发评分输入文件，返回写入行数。
pub fn score_file_pj_with_threads(
    input_file: &Path,
    output_file: &Path,
    score_sieve: i32,
    potential_sieve: i32,
    threads: usize,
) -> Ds4Result<usize> {
    let ctx = ScoreContext::new();
    score_file_with_threads(
        input_file,
        output_file,
        ScoreMode::Pj,
        score_sieve,
        potential_sieve,
        &MODEL_PJ,
        &ctx,
        threads,
    )
}
