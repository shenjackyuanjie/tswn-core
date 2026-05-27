//! RH 字对配对入口。
//!
//! 使用 FS 模型作为左侧、PJ 模型作为右侧，调用通用配对流程筛选高分组合。
//! 该层只绑定模型组合和配对模式，输出格式由 `pairing::common` 保持一致。

use std::path::Path;

use crate::error::Ds3Result;

use super::coeffs::{MODEL_FS, MODEL_PJ};
use super::common::{PairScoreMode, run_pair_mode, run_pair_mode_with_threads};

pub fn run_rh(type_same_set: bool, left_file: &Path, right_file: &Path, output_file: &Path, sieve: i32) -> Ds3Result<usize> {
    run_pair_mode(
        type_same_set,
        left_file,
        right_file,
        output_file,
        sieve,
        PairScoreMode::RhLeftFsRightPj,
        &MODEL_FS,
        &MODEL_PJ,
    )
}

pub fn run_rh_with_threads(
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
        PairScoreMode::RhLeftFsRightPj,
        &MODEL_FS,
        &MODEL_PJ,
        threads,
    )
}
