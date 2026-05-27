//! WC 字对配对入口。
//!
//! 左右两侧都使用 WC 模型，适用于同模式字对组合筛选。
//! 具体候选读取、评分与多线程输出排序由 `pairing::common` 实现。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::MODEL_WC;
use super::common::{PairScoreMode, run_pair_mode, run_pair_mode_with_threads};

pub fn run_wc(type_same_set: bool, left_file: &Path, right_file: &Path, output_file: &Path, sieve: i32) -> Ds3Result<usize> {
    run_pair_mode(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::Wc,
        &MODEL_WC,
        &MODEL_WC,
    )
}

pub fn run_wc_with_threads(
    type_same_set: bool,
    left_file: &Path,
    right_file: &Path,
    output_file: &Path,
    sieve: i32,
    threads: usize,
) -> Ds3Result<usize> {
    run_pair_mode_with_threads(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::Wc,
        &MODEL_WC,
        &MODEL_WC,
        threads,
    )
}
