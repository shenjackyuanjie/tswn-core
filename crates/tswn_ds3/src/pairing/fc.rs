//! FC 字对配对入口。
//!
//! 使用 FZ 模型作为左侧、BC 模型作为右侧，调用通用配对流程筛选高分组合。
//! 同时提供单线程和指定线程数两个版本。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::{MODEL_BC, MODEL_FZ};
use super::common::{PairScoreMode, run_pair_mode, run_pair_mode_with_threads};

pub fn run_fc(type_same_set: bool, left_file: &Path, right_file: &Path, output_file: &Path, sieve: i32) -> Ds3Result<usize> {
    run_pair_mode(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::FcLeftFzRightBc,
        &MODEL_FZ,
        &MODEL_BC,
    )
}

pub fn run_fc_with_threads(
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
        PairScoreMode::FcLeftFzRightBc,
        &MODEL_FZ,
        &MODEL_BC,
        threads,
    )
}
