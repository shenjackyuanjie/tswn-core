//! 编码主路径：状态校验 → 局内重映射 → global / entity / template / lane / list / state 写入。
//!
//! [`FeatureEncoder`] 提供单样本入口和可复用批缓冲入口：
//!
//! ```text
//! FeatureEncoder::new(manifest)                          // 初始化期校验并冻结运行配置
//! FeatureEncoder::encode(state)                          // B=1 便捷入口
//! FeatureEncoder::encode_into(state, batch_index, batch)  // 批槽位写入
//! ```
//!
//! manifest 在初始化阶段校验；目标槽位先恢复 padding，随后检查 schema、引用、
//! 可空分支与容量，再写入字段。失败时目标槽位恢复 padding，相邻槽位保持原内容。
//! 同一 state 在各批位置具有相同的有效编码内容。
//!
//! slot / extra 两族按规格第 14 节写入模型张量与精确旁路。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::encoder::batch::{CapacityDims, EncodedBatch};
use crate::encoder::capacity::{CAPACITY_DIMS, CapacityMeasure, runtime_team_ids};
use crate::encoder::error::EncodeError;
use crate::encoder::manifest::{EncoderManifest, FIXED_COUNT_SLOTS, ProfileSpec, SupportDomain, UNSUPPORTED_PAYLOAD_KINDS};
use crate::encoder::numeric::{TransformKind, normalize_fitted, normalize_fixed_count, to_f32_checked};
use crate::encoder::slots::{SlotScope, SlotSemantic, SlotStorage, resolve_slot};
use crate::encoder::vocab::{
    Vocabulary, boss_kind_vocabulary, player_kind_vocabulary, state_extension_vocabulary, state_legacy_vocabulary,
};
use crate::runtime::entity::RuntimeCorpseKind;
use crate::runtime::extension::{DamageSharePolicy, MergePolicy, OwnerResolutionPolicy};
use crate::runtime::model_state::{
    BattleModelState, MODEL_STATE_SCHEMA_VERSION, ModelPayload, ModelPlayerRuntime, ModelTemplate,
};
use crate::runtime::{EntityIdx, ExtensionRegistry, PlrId, default_custom_runtime_import_config};

mod fields;
mod index;
mod lists;
mod slots_extra;
mod states;
mod tables;
mod validation;

pub use fields::{ENTITY_NUM_SLOTS, IMMUNITY_AXIS, TEMPLATE_NUM_SLOTS, required_normalization};
use fields::{
    corpse_kind_id, damage_share_id, entity_num_present, entity_num_values, merge_id, owner_resolution_id, template_num_present,
    template_num_values,
};
use index::SampleIndex;
use validation::dim_path;

#[cfg(test)]
mod tests;

/// 默认规则注册表；词表派生只做一次。
fn default_registry() -> &'static ExtensionRegistry {
    static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| default_custom_runtime_import_config().expect("默认规则注册表必须有效").registry)
}

/// FeatureEncoder：字段映射、局内引用、presence 与容量预检的唯一实现。
///
/// 纯 Rust 机制编码，可在 `wasm32-unknown-unknown` 上编译，供数据导出器及后续绑定复用。
/// 编码器持有已校验的 manifest、容量档位和冻结词表；编码只借用输入状态，
/// 不推进战斗或消耗 RNG。重复批量编码时应复用 [`EncodedBatch`]，避免逐样本分配全部张量。
#[derive(Debug, Clone)]
pub struct FeatureEncoder {
    manifest: EncoderManifest,
    profile: ProfileSpec,
    vocabularies: BTreeMap<&'static str, Vocabulary>,
}

impl FeatureEncoder {
    /// 构造编码器：manifest 自洽性 + 与当前实现的交叉核对。
    ///
    /// 任何一端过期（版本身份、容量不变量、词表与默认注册表不一致、数值槽不齐全、
    /// 变换类型不符、支持域不符）都拒绝，不自动采用“最新版本”。
    pub fn new(manifest: EncoderManifest) -> Result<Self, EncodeError> {
        manifest.validate()?;
        let registry = default_registry();
        let derived: [(&'static str, Vocabulary); 5] = [
            ("runtime.kind", player_kind_vocabulary(registry)),
            ("template.kind", player_kind_vocabulary(registry)),
            ("template.identity.boss_kind", boss_kind_vocabulary()),
            ("state.legacy_order_key", state_legacy_vocabulary()),
            ("state.extension_state_id", state_extension_vocabulary(registry)),
        ];
        let mut vocabularies = BTreeMap::new();
        for (name, vocabulary) in derived {
            let declared = manifest.vocabularies.get(name).ok_or_else(|| EncodeError::MissingVocabulary {
                path: format!("vocabularies.{name}"),
            })?;
            if declared != &vocabulary {
                return Err(EncodeError::ManifestMismatch {
                    path: format!("vocabularies.{name}"),
                    detail: "词表与默认注册表派生结果不一致".to_owned(),
                });
            }
            vocabularies.insert(name, vocabulary);
        }
        for (path, kind) in required_normalization() {
            let field = manifest
                .normalization
                .get(path)
                .ok_or_else(|| EncodeError::MissingCalibration { path: path.to_owned() })?;
            if field.transform != kind {
                return Err(EncodeError::ManifestMismatch {
                    path: path.to_owned(),
                    detail: format!("manifest 声明的变换 {:?} 与实现登记的 {:?} 不符", field.transform, kind),
                });
            }
        }
        if manifest.support != SupportDomain::current() {
            return Err(EncodeError::ManifestMismatch {
                path: "support".to_owned(),
                detail: "支持域与当前实现不符".to_owned(),
            });
        }
        Ok(Self {
            profile: manifest.profile.clone(),
            manifest,
            vocabularies,
        })
    }

    pub fn manifest(&self) -> &EncoderManifest { &self.manifest }

    pub fn profile(&self) -> &ProfileSpec { &self.profile }

    /// 分配一份单样本批缓冲并编码，成功后由调用方拥有全部张量。
    ///
    /// 输入必须是当前 schema 下尚未决出的状态，且引用、载荷和容量均符合 manifest。
    /// 任何校验或写入错误直接返回，不交付半成品；连续编码多个样本宜改用 [`Self::encode_into`]。
    pub fn encode(&self, state: &BattleModelState) -> Result<EncodedBatch, EncodeError> {
        let mut batch = EncodedBatch::new(&self.profile, 1);
        self.encode_into(state, 0, &mut batch)?;
        Ok(batch)
    }

    /// 将只读状态编码进调用方持有的一个批槽位，相邻槽位保持原内容。
    ///
    /// `out` 必须与本编码器的九维容量完全一致，`batch_index` 从零起。通过调用校验后
    /// 先清除旧内容；状态校验或字段写入失败时，目标槽位整体恢复 padding，可继续复用。
    /// profile 不匹配或 `batch_index` 越界属于调用错误，不修改任何槽位。
    /// 本方法复用输出张量，但索引与变长关系的临时容器仍按样本构造。
    pub fn encode_into(&self, state: &BattleModelState, batch_index: usize, out: &mut EncodedBatch) -> Result<(), EncodeError> {
        if out.dims() != &self.profile.dims() {
            return Err(EncodeError::ManifestMismatch {
                path: "profile".to_owned(),
                detail: "批缓冲容量与 manifest 声明不一致".to_owned(),
            });
        }
        out.clear_slot(batch_index)?;
        let result = (|| -> Result<(), EncodeError> {
            let measured = self.validate_state(state)?;
            let index = self.build_index(state)?;
            self.write_global(state, &index, batch_index, out)?;
            self.write_entity_family(state, &index, batch_index, out)?;
            self.write_template_family(&index, batch_index, out)?;
            self.write_lane_family(&index, batch_index, out)?;
            self.write_list_family(state, &index, batch_index, out)?;
            self.write_state_family(state, &index, batch_index, out)?;
            self.write_slot_family(state, &index, batch_index, out)?;
            self.write_extra_family(state, &index, batch_index, out)?;
            let extra_rows = out.u8_row_mut("extra_mask", batch_index)?.iter().filter(|value| **value != 0).count();
            if extra_rows > measured.x {
                return Err(EncodeError::CapacityExceeded {
                    path: "extra".to_owned(),
                    actual: extra_rows,
                    limit: measured.x,
                });
            }
            Ok(())
        })();
        if result.is_err() {
            // 词表、免疫分类与数值校验也可能在写入过程中失败，不能留下已置 1 的 mask。
            out.clear_slot(batch_index)?;
        }
        result
    }

    // ---------------------------------------------------------------- 数值助手

    /// 逐字段 `N_f`；manifest 缺该字段或声明不是 fitted 时报错，不套用默认常数。
    fn fitted_value(&self, value: f64, path: &str, prefix: &str) -> Result<f32, EncodeError> {
        // 必须检查变换前的原值；否则 ±Inf 会被 clamp 伪装成有限的 ±4。
        if !value.is_finite() {
            return Err(EncodeError::NonFiniteValue {
                path: format!("{prefix}{path}"),
            });
        }
        let field = self.manifest.normalization.get(path).ok_or_else(|| EncodeError::MissingCalibration {
            path: format!("{prefix}{path}"),
        })?;
        let fitted = field.fitted.as_ref().ok_or_else(|| EncodeError::ManifestMismatch {
            path: format!("{prefix}{path}"),
            detail: "声明为 fitted 但缺少常数".to_owned(),
        })?;
        let transformed = normalize_fitted(value, fitted.s_f, fitted.c_f);
        to_f32_checked(transformed, &format!("{prefix}{path}"))
    }

    /// 固定计数尺度；分母来自 `FIXED_COUNT_SLOTS`，不随容量放大。
    fn fixed_count_value(&self, value: f64, path: &str, prefix: &str) -> Result<f32, EncodeError> {
        let divisor = FIXED_COUNT_SLOTS
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, divisor)| *divisor)
            .ok_or_else(|| EncodeError::ManifestMismatch {
                path: format!("{prefix}{path}"),
                detail: "未登记的固定计数槽".to_owned(),
            })?;
        to_f32_checked(normalize_fixed_count(value, divisor), &format!("{prefix}{path}"))
    }

    fn dense_id(&self, domain: &'static str, raw: u32, path: &str) -> Result<i32, EncodeError> {
        self.vocabularies[domain].dense_id(raw, path)
    }
}

fn normalize_position(ordinal: usize, length: usize) -> f32 { ordinal as f32 / length.saturating_sub(1).max(1) as f32 }
