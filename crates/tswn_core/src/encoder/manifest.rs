//! `encoder-manifest.json`：编码契约的唯一载体。
//!
//! 规格第 5 节要求训练、原生推理与 WASM 绑定加载**同一份** manifest；Python 不拟合或覆盖常数。
//! 本模块定义 manifest 结构与两级门禁：
//!
//! 1. [`EncoderManifest::validate`]：manifest **自洽性**（版本身份、容量不变量、词表稠密性、
//!    拟合常数与证据一致），可离线对未发布工件运行。
//! 2. `FeatureEncoder::new`：manifest 与**当前实现**的交叉核对（字段表齐全、词表与默认注册表
//!    一致、变换类型相同、支持域匹配）。任何一端过期都拒绝，不自动采用“最新版本”。
//!
//! manifest 由 `tswn-pwp calibrate` 的输出（校准证据与 `N_f` 常数）加编码契约组装而成；
//! 组装入口是 [`EncoderManifest::from_calibration`]。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::encoder::capacity::EncoderProfile;
use crate::encoder::error::EncodeError;
use crate::encoder::numeric::TransformKind;
use crate::encoder::vocab::{Vocabulary, boss_kind_vocabulary, player_kind_vocabulary};
use crate::runtime::model_state::MODEL_STATE_SCHEMA_VERSION;

/// manifest schema 名。
pub const ENCODER_MANIFEST_SCHEMA: &str = "tswn-core/encoder-manifest";
/// manifest schema 版本。
pub const ENCODER_MANIFEST_VERSION: u32 = 1;
/// encoder 语义版本；编码语义变化时 bump，manifest 声明值必须与实现相等。
pub const ENCODER_SEMANTIC_VERSION: &str = "encoder-v1";
/// 张量布局版本（dtype/shape/槽位含义）。
pub const TENSOR_LAYOUT_VERSION: u32 = 1;
/// 数值策略版本（变换公式与裁剪规则）。
pub const NUMERIC_POLICY_VERSION: u32 = 1;
/// `tswn-pwp calibrate` 输出报告的 schema 名。
pub const CALIBRATION_SCHEMA: &str = "tswn-pwp/encoder-calibration";
/// 校准报告 schema 版本。
pub const CALIBRATION_SCHEMA_VERSION: u32 = 1;

/// 第 8 节已启用的载荷 kind（按 kind 词表顺序）；Boss 专属本轮不支持。
pub const SUPPORTED_PAYLOAD_KINDS: [&str; 11] = [
    "none",
    "fire_mag_half_steps",
    "ice",
    "shield_value",
    "curse",
    "poison",
    "haste",
    "berserk",
    "charm",
    "slow",
    "iron",
];

/// 本轮不映射的 Boss 专属/感染载荷 kind。
pub const UNSUPPORTED_PAYLOAD_KINDS: [&str; 5] = ["covid_boss", "covid_infection", "saitama_boss", "lazy_boss", "lazy_infection"];

/// manifest 声明的容量档位；名称用 `String` 以便序列化，
/// 不变量与 [`BASELINE_64`] 相同（规格第 4 节容量表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSpec {
    pub name: String,
    pub e_max: usize,
    pub t_max: usize,
    pub r_max: usize,
    pub h_max: usize,
    pub l_max: usize,
    pub s_max: usize,
    pub q_max: usize,
    pub v_max: usize,
    pub x_max: usize,
}

impl ProfileSpec {
    /// 自洽性检查；容量推导见规格第 4 节。全部用 checked 算术，畸形 manifest 不得回绕。
    pub fn validate(&self) -> Result<(), EncodeError> {
        let mismatch = |detail: String| EncodeError::ManifestMismatch {
            path: "profile".to_owned(),
            detail,
        };
        if self.name.is_empty() {
            return Err(mismatch("profile 名称为空".to_owned()));
        }
        let dims = [
            ("e_max", self.e_max),
            ("t_max", self.t_max),
            ("r_max", self.r_max),
            ("h_max", self.h_max),
            ("l_max", self.l_max),
            ("s_max", self.s_max),
            ("q_max", self.q_max),
            ("v_max", self.v_max),
            ("x_max", self.x_max),
        ];
        for (name, value) in dims {
            if value == 0 {
                return Err(mismatch(format!("{name} 必须为正")));
            }
        }
        // Q 覆盖默认注册表 7 个实体槽 × E_max + 3 个全局模板槽。
        let slot_bound = self
            .e_max
            .checked_mul(7)
            .and_then(|value| value.checked_add(3))
            .ok_or_else(|| mismatch("e_max 溢出".to_owned()))?;
        if self.q_max < slot_bound {
            return Err(mismatch(format!("q_max={} < 7*e_max+3={}", self.q_max, slot_bound)));
        }
        // S 至少覆盖每实体一条状态的规划预留。
        if self.s_max < self.e_max {
            return Err(mismatch(format!("s_max={} < e_max={}", self.s_max, self.e_max)));
        }
        // V 的规划下界：五类 lane list。
        let lane_bound = self.l_max.checked_mul(5).ok_or_else(|| mismatch("l_max 溢出".to_owned()))?;
        if self.v_max < lane_bound {
            return Err(mismatch(format!("v_max={} < 5*l_max={}", self.v_max, lane_bound)));
        }
        // X 的上界式：V + 15H + 9E + 3Q + 3S + 2，按每槽 3 条计费。
        // raw 已包含在各项内，不能再重复加 10H+Q；溢出直接拒绝，不让 usize::MAX 蒙混通过。
        let mut bound = self.v_max;
        for (value, factor) in [(self.h_max, 15), (self.e_max, 9), (self.q_max, 3), (self.s_max, 3)] {
            bound = value
                .checked_mul(factor)
                .and_then(|term| bound.checked_add(term))
                .ok_or_else(|| mismatch("X 容量上界式溢出".to_owned()))?;
        }
        let bound = bound.checked_add(2).ok_or_else(|| mismatch("X 容量上界式溢出".to_owned()))?;
        if self.x_max < bound {
            return Err(mismatch(format!("x_max={} < 上界式 {}", self.x_max, bound)));
        }
        Ok(())
    }
}

impl From<&EncoderProfile> for ProfileSpec {
    fn from(profile: &EncoderProfile) -> Self {
        Self {
            name: profile.name.to_owned(),
            e_max: profile.e_max,
            t_max: profile.t_max,
            r_max: profile.r_max,
            h_max: profile.h_max,
            l_max: profile.l_max,
            s_max: profile.s_max,
            q_max: profile.q_max,
            v_max: profile.v_max,
            x_max: profile.x_max,
        }
    }
}

/// `N_f` 的拟合常数与 train 证据；非有限值不得进入（校准阶段已剔除）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FittedConstants {
    pub s_f: f64,
    pub c_f: f64,
    /// `s_f`/`c_f` 的权威 f64 bit 表示；读取时校验与十进制字段一致，
    /// 避免 JSON 重写工具改变数值身份（外部评审 4.2）。
    pub s_f_bits: u64,
    pub c_f_bits: u64,
    /// train 观测样本数；0 表示没有观测，不允许发布。
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p99: f64,
    pub abs_p50: f64,
    pub abs_p99: f64,
}

impl FittedConstants {
    /// 自校验：常数有限、下界为 1、`c_f ≥ s_f`（分位数单调）、bit 表示一致。
    pub fn validate(&self, path: &str) -> Result<(), EncodeError> {
        let mismatch = |detail: String| EncodeError::ManifestMismatch {
            path: path.to_owned(),
            detail,
        };
        if self.count == 0 {
            return Err(mismatch("没有 train 观测值".to_owned()));
        }
        for (name, value) in [("s_f", self.s_f), ("c_f", self.c_f)] {
            if !value.is_finite() || value < 1.0 {
                return Err(mismatch(format!("{name}={value} 必须有限且 ≥ 1")));
            }
        }
        if self.c_f < self.s_f {
            return Err(mismatch(format!("c_f={} < s_f={}", self.c_f, self.s_f)));
        }
        if self.s_f.to_bits() != self.s_f_bits || self.c_f.to_bits() != self.c_f_bits {
            return Err(mismatch("s_f/c_f 与 bit 表示不一致".to_owned()));
        }
        for (name, value) in [
            ("min", self.min),
            ("max", self.max),
            ("p50", self.p50),
            ("p99", self.p99),
            ("abs_p50", self.abs_p50),
            ("abs_p99", self.abs_p99),
        ] {
            if !value.is_finite() {
                return Err(mismatch(format!("统计量 {name} 非有限")));
            }
        }
        if self.min > self.p50 || self.p50 > self.p99 || self.p99 > self.max {
            return Err(mismatch("分位数与 min/max 不自洽".to_owned()));
        }
        if self.abs_p50 > self.abs_p99 {
            return Err(mismatch("绝对分位数不自洽".to_owned()));
        }
        Ok(())
    }
}

/// 单个数值槽的归一化声明；`transform` 唯一确定变换类别，
/// `fitted` 仅在 `TransformKind::Fitted` 时存在（外部评审 4.2：不能只给 `s_f/c_f`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizationField {
    pub transform: TransformKind,
    pub fitted: Option<FittedConstants>,
}

impl NormalizationField {
    /// 自校验；`path` 是第 5 节的完整字段路径。
    pub fn validate(&self, path: &str) -> Result<(), EncodeError> {
        match (&self.transform, &self.fitted) {
            (TransformKind::Fitted, Some(constants)) => constants.validate(path),
            (TransformKind::FixedCount { divisor }, None) => {
                if !divisor.is_finite() || *divisor <= 0.0 {
                    return Err(EncodeError::ManifestMismatch {
                        path: path.to_owned(),
                        detail: format!("固定计数尺度 {divisor} 必须为正有限值"),
                    });
                }
                Ok(())
            }
            (TransformKind::Structural | TransformKind::Passthrough, None) => Ok(()),
            (TransformKind::Fitted, None) => Err(EncodeError::ManifestMismatch {
                path: path.to_owned(),
                detail: "声明为 fitted 但缺少常数".to_owned(),
            }),
            (_, Some(_)) => Err(EncodeError::ManifestMismatch {
                path: path.to_owned(),
                detail: "非 fitted 变换不得携带拟合常数".to_owned(),
            }),
        }
    }
}

/// 校准证据：证明常数由哪些 train 行拟合而来。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationEvidence {
    pub schema: String,
    pub schema_version: u32,
    pub split: String,
    pub require_label: bool,
    pub samples_seen: usize,
    pub samples_selected: usize,
    pub input_sha256: String,
    pub executable_sha256: String,
    pub selected_rows_digest: String,
    pub calibrator: String,
}

/// 支持域声明；Boss 专属载荷本轮明确不支持（规格第 9 节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportDomain {
    /// 已启用的载荷 kind（第 8 节词表子集）。
    pub payload_kinds: Vec<String>,
    /// 明确不支持的载荷 kind。
    pub unsupported_payload_kinds: Vec<String>,
}

impl SupportDomain {
    /// 当前实现的支持域。
    pub fn current() -> Self {
        Self {
            payload_kinds: SUPPORTED_PAYLOAD_KINDS.iter().map(|kind| (*kind).to_owned()).collect(),
            unsupported_payload_kinds: UNSUPPORTED_PAYLOAD_KINDS.iter().map(|kind| (*kind).to_owned()).collect(),
        }
    }

    /// 自校验：两类 kind 不重叠且非空。
    pub fn validate(&self) -> Result<(), EncodeError> {
        let mismatch = |detail: String| EncodeError::ManifestMismatch {
            path: "support".to_owned(),
            detail,
        };
        if self.payload_kinds.is_empty() {
            return Err(mismatch("没有启用任何载荷 kind".to_owned()));
        }
        for kind in &self.unsupported_payload_kinds {
            if self.payload_kinds.contains(kind) {
                return Err(mismatch(format!("{kind} 同时出现在启用与不支持列表")));
            }
        }
        Ok(())
    }
}

/// 编码契约 manifest。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncoderManifest {
    pub schema: String,
    pub schema_version: u32,
    pub state_schema_version: u32,
    pub encoder_version: String,
    pub tensor_layout_version: u32,
    pub numeric_policy_version: u32,
    pub profile: ProfileSpec,
    /// 分类词表；键是分类域名（如 `runtime.kind`、`template.identity.boss_kind`）。
    pub vocabularies: BTreeMap<String, Vocabulary>,
    /// 数值槽声明；键是第 5 节的完整字段路径。
    pub normalization: BTreeMap<String, NormalizationField>,
    pub calibration: CalibrationEvidence,
    pub support: SupportDomain,
    /// 契约摘要；由 [`EncoderManifest::digest`] 计算后写入，摘录自身不参与计算。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_digest: Option<String>,
}

impl EncoderManifest {
    /// manifest 自洽性门禁；不核对实现细节（那由 `FeatureEncoder::new` 负责）。
    pub fn validate(&self) -> Result<(), EncodeError> {
        if self.schema != ENCODER_MANIFEST_SCHEMA || self.schema_version != ENCODER_MANIFEST_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "manifest.schema".to_owned(),
                expected: format!("{ENCODER_MANIFEST_SCHEMA} v{ENCODER_MANIFEST_VERSION}"),
                actual: format!("{} v{}", self.schema, self.schema_version),
            });
        }
        if self.state_schema_version != MODEL_STATE_SCHEMA_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "manifest.state_schema_version".to_owned(),
                expected: MODEL_STATE_SCHEMA_VERSION.to_string(),
                actual: self.state_schema_version.to_string(),
            });
        }
        if self.encoder_version != ENCODER_SEMANTIC_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "manifest.encoder_version".to_owned(),
                expected: ENCODER_SEMANTIC_VERSION.to_owned(),
                actual: self.encoder_version.clone(),
            });
        }
        for (name, expected, actual) in [
            ("tensor_layout_version", TENSOR_LAYOUT_VERSION, self.tensor_layout_version),
            ("numeric_policy_version", NUMERIC_POLICY_VERSION, self.numeric_policy_version),
        ] {
            if actual != expected {
                return Err(EncodeError::SchemaMismatch {
                    path: format!("manifest.{name}"),
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                });
            }
        }
        self.profile.validate()?;
        for (name, vocabulary) in &self.vocabularies {
            vocabulary.validate().map_err(|error| match error {
                EncodeError::ManifestMismatch { detail, .. } => EncodeError::ManifestMismatch {
                    path: format!("vocabularies.{name}"),
                    detail,
                },
                other => other,
            })?;
        }
        for (path, field) in &self.normalization {
            field.validate(path)?;
        }
        self.support.validate()?;
        Ok(())
    }

    /// 契约摘要：对“摘录自身置空”的规范化 JSON 取 sha256。
    /// 规范化依赖 `BTreeMap` 键序与 serde 的稳定字段序，不依赖内存布局或平台浮点文本。
    pub fn digest(&self) -> String {
        let mut canonical = self.clone();
        canonical.contract_digest = None;
        let bytes = serde_json::to_vec(&canonical).expect("manifest 必须可序列化");
        format!("{:x}", Sha256::digest(bytes))
    }

    /// 取一个数值槽的拟合常数；缺失返回 [`EncodeError::MissingCalibration`]。
    pub fn fitted(&self, path: &str) -> Result<&FittedConstants, EncodeError> {
        self.normalization
            .get(path)
            .and_then(|field| field.fitted.as_ref())
            .ok_or_else(|| EncodeError::MissingCalibration { path: path.to_owned() })
    }

    /// 从 `tswn-pwp calibrate` 的报告组装 manifest。
    ///
    /// - 固定计数尺度的槽（规格第 5 节“机制计数的绝对量”）写入 `FixedCount` 声明，不要求观测；
    /// - 其余数值槽一律 `Fitted`，常数来自校准报告；报告缺字段时这里就失败，
    ///   而不是等编码到具体样本才报错（发布门禁前移）。
    pub fn from_calibration(calibration: &CalibrationReport, profile: &EncoderProfile) -> Result<Self, EncodeError> {
        if calibration.schema != CALIBRATION_SCHEMA || calibration.schema_version != CALIBRATION_SCHEMA_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "calibration.schema".to_owned(),
                expected: format!("{CALIBRATION_SCHEMA} v{CALIBRATION_SCHEMA_VERSION}"),
                actual: format!("{} v{}", calibration.schema, calibration.schema_version),
            });
        }
        if calibration.state_schema_version != MODEL_STATE_SCHEMA_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "calibration.state_schema_version".to_owned(),
                expected: MODEL_STATE_SCHEMA_VERSION.to_string(),
                actual: calibration.state_schema_version.to_string(),
            });
        }
        let registry = crate::runtime::default_custom_runtime_import_config()
            .map_err(|error| EncodeError::ManifestMismatch {
                path: "vocabularies".to_owned(),
                detail: format!("默认注册表无效：{error:?}"),
            })?
            .registry;
        let mut normalization: BTreeMap<String, NormalizationField> = FIXED_COUNT_SLOTS
            .iter()
            .map(|(path, divisor)| {
                (
                    (*path).to_owned(),
                    NormalizationField {
                        transform: TransformKind::FixedCount { divisor: *divisor },
                        fitted: None,
                    },
                )
            })
            .collect();
        for (path, field) in &calibration.fields {
            // 校准报告也含固定计数的分布统计，但这些统计不能覆盖已冻结的线性变换声明。
            if FIXED_COUNT_SLOTS.iter().any(|(fixed_path, _)| *fixed_path == path.as_str()) {
                continue;
            }
            normalization.insert(
                path.clone(),
                NormalizationField {
                    transform: TransformKind::Fitted,
                    fitted: Some(FittedConstants {
                        s_f: field.s_f,
                        c_f: field.c_f,
                        s_f_bits: field.s_f.to_bits(),
                        c_f_bits: field.c_f.to_bits(),
                        count: field.count,
                        min: field.min,
                        max: field.max,
                        p50: field.p50,
                        p99: field.p99,
                        abs_p50: field.abs_p50,
                        abs_p99: field.abs_p99,
                    }),
                },
            );
        }
        Ok(Self {
            schema: ENCODER_MANIFEST_SCHEMA.to_owned(),
            schema_version: ENCODER_MANIFEST_VERSION,
            state_schema_version: MODEL_STATE_SCHEMA_VERSION,
            encoder_version: ENCODER_SEMANTIC_VERSION.to_owned(),
            tensor_layout_version: TENSOR_LAYOUT_VERSION,
            numeric_policy_version: NUMERIC_POLICY_VERSION,
            profile: ProfileSpec::from(profile),
            vocabularies: [
                ("runtime.kind".to_owned(), player_kind_vocabulary(&registry)),
                ("template.kind".to_owned(), player_kind_vocabulary(&registry)),
                ("template.identity.boss_kind".to_owned(), boss_kind_vocabulary()),
            ]
            .into_iter()
            .collect(),
            normalization,
            calibration: CalibrationEvidence {
                schema: calibration.schema.clone(),
                schema_version: calibration.schema_version,
                split: calibration.split.clone(),
                require_label: calibration.require_label,
                samples_seen: calibration.samples_seen,
                samples_selected: calibration.samples_selected,
                input_sha256: calibration.input_sha256.clone(),
                executable_sha256: calibration.executable_sha256.clone(),
                selected_rows_digest: calibration.selected_rows_digest.clone(),
                calibrator: calibration.calibrator.clone(),
            },
            support: SupportDomain::current(),
            contract_digest: None,
        })
    }
}

/// 规格第 5 节的固定计数尺度槽：`(字段路径, 分母)`。分母取 p99，不随容量放大。
pub const FIXED_COUNT_SLOTS: [(&str, f64); 2] = [("global.entity_slot_count", 16.0), ("global.entity_count", 15.0)];

/// `tswn-pwp calibrate` 的报告视图；只读取组装 manifest 需要的字段，
/// 未知字段忽略，因此校准报告可以继续增补统计量而不破坏本视图。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CalibrationReport {
    pub schema: String,
    pub schema_version: u32,
    pub state_schema_version: u32,
    pub split: String,
    pub require_label: bool,
    pub samples_seen: usize,
    pub samples_selected: usize,
    pub input_sha256: String,
    pub executable_sha256: String,
    pub selected_rows_digest: String,
    pub calibrator: String,
    pub fields: BTreeMap<String, CalibrationFieldReport>,
}

/// 校准报告里的单字段统计；与 `tswn_pwp::calibrate::CalibrationField` 同构。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CalibrationFieldReport {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p99: f64,
    pub abs_p50: f64,
    pub abs_p99: f64,
    pub s_f: f64,
    pub c_f: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::capacity::BASELINE_64;

    fn profile() -> EncoderProfile { BASELINE_64 }

    fn calibration_report() -> CalibrationReport {
        serde_json::from_str(
            r#"{
                "schema": "tswn-pwp/encoder-calibration",
                "schema_version": 1,
                "state_schema_version": 1,
                "format_version": 1,
                "split": "train",
                "require_label": true,
                "samples_seen": 100,
                "samples_selected": 90,
                "input_sha256": "aa",
                "executable_sha256": "bb",
                "selected_rows_digest": "cc",
                "calibrator": "tswn-pwp calibrate v1",
                "fields": {
                    "global.round": {"count": 90, "min": 0, "max": 61, "p50": 12, "p99": 61,
                        "abs_p50": 12, "abs_p99": 61, "s_f": 12, "c_f": 61}
                }
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn built_manifest_passes_self_validation() {
        let manifest = EncoderManifest::from_calibration(&calibration_report(), &profile()).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.support.payload_kinds.len(), SUPPORTED_PAYLOAD_KINDS.len());
        assert!(manifest.support.unsupported_payload_kinds.contains(&"covid_boss".to_owned()));
        // 固定计数槽不带拟合常数。
        let fixed = &manifest.normalization["global.entity_slot_count"];
        assert!(fixed.fitted.is_none());
        assert_eq!(fixed.transform, TransformKind::FixedCount { divisor: 16.0 });
        assert!(manifest.fitted("global.round").is_ok());
    }

    #[test]
    fn calibration_statistics_cannot_override_fixed_count_transforms() {
        let mut report = calibration_report();
        for (path, _) in FIXED_COUNT_SLOTS {
            report.fields.insert(path.to_owned(), report.fields["global.round"].clone());
        }
        let manifest = EncoderManifest::from_calibration(&report, &profile()).unwrap();
        manifest.validate().unwrap();
        for (path, divisor) in FIXED_COUNT_SLOTS {
            let field = &manifest.normalization[path];
            assert_eq!(field.transform, TransformKind::FixedCount { divisor });
            assert!(field.fitted.is_none(), "{path} 的统计值不得变成拟合常数");
        }
    }

    #[test]
    fn missing_calibration_field_is_reported_not_defaulted() {
        let mut report = calibration_report();
        report.fields.clear();
        let manifest = EncoderManifest::from_calibration(&report, &profile()).unwrap();
        manifest.validate().unwrap();
        // 自洽但缺常数：构造期（FeatureEncoder::new）才会因为字段表不齐全而拒绝，
        // 这里先确认查询路径返回 MissingCalibration 而不是默认值。
        assert!(matches!(
            manifest.fitted("global.round").unwrap_err(),
            EncodeError::MissingCalibration { .. }
        ));
    }

    #[test]
    fn wrong_calibration_schema_is_rejected() {
        let mut report = calibration_report();
        report.schema = "other".to_owned();
        assert!(EncoderManifest::from_calibration(&report, &profile()).is_err());
        report.schema = CALIBRATION_SCHEMA.to_owned();
        report.state_schema_version = 99;
        assert!(EncoderManifest::from_calibration(&report, &profile()).is_err());
    }

    #[test]
    fn profile_must_satisfy_capacity_invariants() {
        let mut spec = ProfileSpec::from(&profile());
        spec.q_max = 100;
        assert!(spec.validate().is_err(), "q_max 必须覆盖 7*e_max+3");
        let mut spec = ProfileSpec::from(&profile());
        spec.v_max = 100;
        assert!(spec.validate().is_err(), "v_max 必须覆盖 5*l_max");
        let mut spec = ProfileSpec::from(&profile());
        spec.x_max = 100;
        assert!(spec.validate().is_err(), "x_max 必须覆盖上界式");
        let mut spec = ProfileSpec::from(&profile());
        spec.s_max = 8;
        assert!(spec.validate().is_err(), "s_max 必须至少等于 e_max");
    }

    #[test]
    fn profile_x_bound_counts_raw_once_and_rejects_overflow() {
        let mut spec = ProfileSpec::from(&profile());
        let bound = BASELINE_64.v_max
            + 15 * BASELINE_64.h_max
            + 9 * BASELINE_64.e_max
            + 3 * BASELINE_64.q_max
            + 3 * BASELINE_64.s_max
            + 2;
        assert_eq!(bound, 42_754);
        spec.name = "test-exact-x-bound".to_owned();
        spec.x_max = bound;
        assert!(spec.validate().is_ok(), "raw 已计入 15H/9E/3Q/3S，不应重复计费");
        spec.x_max = bound - 1;
        assert!(spec.validate().is_err());
        spec.h_max = usize::MAX;
        spec.x_max = usize::MAX;
        assert!(spec.validate().is_err(), "上界式溢出不能由 usize::MAX 饱和值掩盖");
    }

    #[test]
    fn fitted_constants_reject_bit_mismatch_and_small_scale() {
        let mut constants = FittedConstants {
            s_f: 2.0,
            c_f: 8.0,
            s_f_bits: 2.0f64.to_bits(),
            c_f_bits: 8.0f64.to_bits(),
            count: 10,
            min: -8.0,
            max: 8.0,
            p50: 0.0,
            p99: 6.0,
            abs_p50: 2.0,
            abs_p99: 8.0,
        };
        assert!(constants.validate("x").is_ok());
        constants.s_f_bits = 3.0f64.to_bits();
        assert!(constants.validate("x").is_err(), "bit 表示必须一致");
        constants.s_f_bits = 2.0f64.to_bits();
        constants.s_f = 0.5;
        assert!(constants.validate("x").is_err(), "s_f 必须 ≥ 1");
        constants.s_f = 9.0;
        assert!(constants.validate("x").is_err(), "c_f 必须 ≥ s_f");
    }

    #[test]
    fn digest_is_stable_and_covers_contract() {
        let manifest = EncoderManifest::from_calibration(&calibration_report(), &profile()).unwrap();
        let digest = manifest.digest();
        assert_eq!(digest, manifest.digest());
        assert_eq!(digest.len(), 64);
        let mut changed = manifest.clone();
        changed.calibration.selected_rows_digest = "dd".to_owned();
        assert_ne!(digest, changed.digest(), "换一份校准证据必须换摘要");
    }
}
