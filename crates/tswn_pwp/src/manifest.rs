//! 校准报告到完整编码契约的命令适配；词表和数值规则仍由核心 encoder 负责。
//!
//! 只接受有标签 train 且选中样本非空的报告，绑定 BASELINE_64 并计算 contract_digest。
//! 先构造 FeatureEncoder 验证完整词表/数值槽，再创建父目录并覆盖目标 JSON。
//! 不重新拟合字段，也不将诊断切分的报告静默转成训练契约。
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
use tswn_core::encoder::{
    capacity::BASELINE_64,
    encode::FeatureEncoder,
    manifest::{CalibrationReport, EncoderManifest},
};

use crate::storage;

#[derive(Debug, Clone, clap::Args)]
pub struct BuildManifestArgs {
    /// calibrate 输出的有标签 train 校准报告。
    #[arg(long)]
    pub calibration: PathBuf,
    /// 完整 EncoderManifest JSON 的输出位置。
    #[arg(long)]
    pub out: PathBuf,
}

pub fn run(args: &BuildManifestArgs) -> Result<()> {
    let report: CalibrationReport = storage::read_json(&args.calibration).context("读取校准报告")?;
    ensure!(
        report.split == "train" && report.require_label,
        "训练契约只接受有标签 train 校准报告"
    );
    ensure!(report.samples_selected > 0, "校准报告没有选中样本");
    let mut manifest =
        EncoderManifest::from_calibration(&report, &BASELINE_64).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    manifest.contract_digest = Some(manifest.digest());
    // 自洽性检查不能替代完整数值槽和默认词表门禁；失败时不发布半成品。
    FeatureEncoder::new(manifest.clone()).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    if let Some(parent) = args.out.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    storage::write_json(&args.out, &manifest)?;
    println!("编码契约已生成：{}，sha256={}", args.out.display(), manifest.digest());
    Ok(())
}
