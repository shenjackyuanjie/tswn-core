//! 批缓冲：按张量分配的 C 连续存储与批槽位定位。
//!
//! 布局约定（外部评审 3.2）：**不能把 B 份“完整单样本包”直接拼接**。每个张量单独分配，
//! 批槽位写入时元素偏移为 `batch_index × 该张量每样本元素数`，因此 `entity_num` 等张量
//! 满足 `[B, ...]` 的 C 连续要求，两份同族样本之间不会夹着其他张量。
//!
//! 填充规则（规格第 4 节）：数值、分类、bit、mask 的未写入位置为 0；引用张量为 -1；
//! mask=0 的位置不参与聚合、注意力或 softmax。复用缓冲时 [`EncodedBatch::clear_slot`]
//! 先把目标槽位恢复成填充值，保证不会残留上一个样本的数据。

use std::collections::BTreeMap;

use crate::encoder::capacity::{EncoderProfile, BASELINE_64};
use crate::encoder::error::EncodeError;
use crate::encoder::manifest::ProfileSpec;

/// 张量 dtype；后缀与导出容器的 `.bin` 文件名一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtype {
    F32,
    I32,
    U8,
    U32,
}

impl Dtype {
    /// 文件名后缀（如 `entity_num.f32.bin`）。
    pub fn suffix(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::I32 => "i32",
            Self::U8 => "u8",
            Self::U32 => "u32",
        }
    }

    /// 元素字节数。
    pub fn size(self) -> usize {
        match self {
            Self::F32 | Self::I32 | Self::U32 => 4,
            Self::U8 => 1,
        }
    }
}

/// 未写入位置的填充值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    /// 数值、分类、bit、mask：0。
    Zero,
    /// 引用张量：-1（防止消费端把 padding 当有效引用 gather）。
    RefMinusOne,
}

/// 一个输出张量的静态描述。
///
/// 每样本元素数依赖容量档位（如 `entity_num` 是 `E_max×24`），因此不进 const 注册表，
/// 由 [`tensor_shape`] 在分配时推导；注册表只固定名称、dtype、轴含义与填充值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TensorSpec {
    pub name: &'static str,
    pub dtype: Dtype,
    /// 轴含义（`E_max` 等为容量占位）；只用于 manifest 与文档，不参与分配。
    pub shape: &'static str,
    pub fill: Fill,
}

/// 容量九维的公共视图；常量档位与 manifest 声明档位都实现它，
/// 避免批缓冲同时认识两种 profile 类型。
pub trait CapacityDims {
    fn dims(&self) -> [usize; 9];
}

impl CapacityDims for EncoderProfile {
    fn dims(&self) -> [usize; 9] {
        [
            self.e_max,
            self.t_max,
            self.r_max,
            self.h_max,
            self.l_max,
            self.s_max,
            self.q_max,
            self.v_max,
            self.x_max,
        ]
    }
}

impl CapacityDims for ProfileSpec {
    fn dims(&self) -> [usize; 9] {
        [
            self.e_max,
            self.t_max,
            self.r_max,
            self.h_max,
            self.l_max,
            self.s_max,
            self.q_max,
            self.v_max,
            self.x_max,
        ]
    }
}

/// 本模块已实现的张量注册表：**global / entity / template 三族**（含 mask 与 presence）。
///
/// lane / state / slot / list / extra 四族以及 `order_key`/`extra_*` 等张量按 handoff
/// 的分块计划在后续提交追加；追加时必须同步规格第 4 节的张量表与 manifest 张量注册表，
/// 并保持“每个张量只登记一次、名称全局唯一”。
pub const TENSOR_SPECS: &[TensorSpec] = &[
    // global
    TensorSpec { name: "global_num", dtype: Dtype::F32, shape: "[6]", fill: Fill::Zero },
    TensorSpec { name: "team_mask", dtype: Dtype::U8, shape: "[T_max]", fill: Fill::Zero },
    TensorSpec { name: "runtime_team_mask", dtype: Dtype::U8, shape: "[R_max]", fill: Fill::Zero },
    // entity
    TensorSpec { name: "entity_mask", dtype: Dtype::U8, shape: "[E_max]", fill: Fill::Zero },
    TensorSpec { name: "entity_template", dtype: Dtype::I32, shape: "[E_max]", fill: Fill::RefMinusOne },
    TensorSpec { name: "entity_team", dtype: Dtype::I32, shape: "[E_max,2]", fill: Fill::RefMinusOne },
    TensorSpec { name: "entity_num", dtype: Dtype::F32, shape: "[E_max,24]", fill: Fill::Zero },
    TensorSpec { name: "entity_num_present", dtype: Dtype::U8, shape: "[E_max,24]", fill: Fill::Zero },
    TensorSpec { name: "entity_bool", dtype: Dtype::U8, shape: "[E_max,10]", fill: Fill::Zero },
    TensorSpec { name: "entity_flags", dtype: Dtype::U8, shape: "[E_max,8]", fill: Fill::Zero },
    TensorSpec { name: "entity_kind_flags", dtype: Dtype::U8, shape: "[E_max,6]", fill: Fill::Zero },
    TensorSpec { name: "entity_cat", dtype: Dtype::I32, shape: "[E_max,5]", fill: Fill::Zero },
    TensorSpec { name: "entity_ref", dtype: Dtype::I32, shape: "[E_max,5]", fill: Fill::RefMinusOne },
    TensorSpec { name: "entity_ref_present", dtype: Dtype::U8, shape: "[E_max,5]", fill: Fill::Zero },
    // template
    TensorSpec { name: "template_mask", dtype: Dtype::U8, shape: "[H_max]", fill: Fill::Zero },
    TensorSpec { name: "template_num", dtype: Dtype::F32, shape: "[H_max,31]", fill: Fill::Zero },
    TensorSpec { name: "template_num_present", dtype: Dtype::U8, shape: "[H_max,31]", fill: Fill::Zero },
    TensorSpec { name: "template_bool", dtype: Dtype::U8, shape: "[H_max,5]", fill: Fill::Zero },
    TensorSpec { name: "template_cat", dtype: Dtype::I32, shape: "[H_max,5]", fill: Fill::Zero },
    TensorSpec { name: "template_cat_present", dtype: Dtype::U8, shape: "[H_max,5]", fill: Fill::Zero },
    TensorSpec { name: "template_team", dtype: Dtype::I32, shape: "[H_max]", fill: Fill::RefMinusOne },
    TensorSpec { name: "template_player_ref", dtype: Dtype::I32, shape: "[H_max]", fill: Fill::RefMinusOne },
    TensorSpec { name: "template_override_present", dtype: Dtype::U8, shape: "[H_max,4]", fill: Fill::Zero },
    TensorSpec { name: "template_clone_attr", dtype: Dtype::U32, shape: "[H_max,8]", fill: Fill::Zero },
    TensorSpec { name: "template_clone_weapon_bonus", dtype: Dtype::I32, shape: "[H_max,8]", fill: Fill::Zero },
    TensorSpec { name: "immunity_num", dtype: Dtype::F32, shape: "[H_max,9]", fill: Fill::Zero },
    TensorSpec { name: "immunity_present", dtype: Dtype::U8, shape: "[H_max,9]", fill: Fill::Zero },
    TensorSpec { name: "clan_equal", dtype: Dtype::U8, shape: "[H_max,H_max]", fill: Fill::Zero },
];

/// 张量的具体 shape（不含 batch 轴）；用于推导每样本元素数与字节数。
pub fn tensor_shape(dims: &[usize; 9], name: &str) -> Option<Vec<usize>> {
    let [e, t, r, h, _l, _s, _q, _v, _x] = *dims;
    Some(match name {
        "global_num" => vec![6],
        "team_mask" => vec![t],
        "runtime_team_mask" => vec![r],
        "entity_mask" | "entity_template" => vec![e],
        "entity_team" => vec![e, 2],
        "entity_num" | "entity_num_present" => vec![e, 24],
        "entity_bool" => vec![e, 10],
        "entity_flags" => vec![e, 8],
        "entity_kind_flags" => vec![e, 6],
        "entity_cat" | "entity_ref" | "entity_ref_present" => vec![e, 5],
        "template_mask" | "template_team" | "template_player_ref" => vec![h],
        "template_num" | "template_num_present" => vec![h, 31],
        "template_bool" | "template_cat" | "template_cat_present" => vec![h, 5],
        "template_override_present" => vec![h, 4],
        "template_clone_attr" | "template_clone_weapon_bonus" => vec![h, 8],
        "immunity_num" | "immunity_present" => vec![h, 9],
        "clan_equal" => vec![h, h],
        _ => return None,
    })
}

enum TensorBuffer {
    F32(Vec<f32>),
    I32(Vec<i32>),
    U8(Vec<u8>),
    U32(Vec<u32>),
}

impl TensorBuffer {
    fn allocate(dtype: Dtype, len: usize, fill: Fill) -> Self {
        match (dtype, fill) {
            (Dtype::F32, Fill::Zero) => Self::F32(vec![0.0; len]),
            (Dtype::F32, Fill::RefMinusOne) => Self::F32(vec![-1.0; len]),
            (Dtype::I32, Fill::Zero) => Self::I32(vec![0; len]),
            (Dtype::I32, Fill::RefMinusOne) => Self::I32(vec![-1; len]),
            (Dtype::U8, _) => Self::U8(vec![0; len]),
            (Dtype::U32, _) => Self::U32(vec![0; len]),
        }
    }

    fn clear(&mut self, start: usize, fill: Fill) {
        match self {
            Self::F32(values) => {
                let value = match fill {
                    Fill::Zero => 0.0,
                    Fill::RefMinusOne => -1.0,
                };
                for slot in &mut values[start..] {
                    *slot = value;
                }
            }
            Self::I32(values) => {
                let value = match fill {
                    Fill::Zero => 0,
                    Fill::RefMinusOne => -1,
                };
                for slot in &mut values[start..] {
                    *slot = value;
                }
            }
            Self::U8(values) => {
                for slot in &mut values[start..] {
                    *slot = 0;
                }
            }
            Self::U32(values) => {
                for slot in &mut values[start..] {
                    *slot = 0;
                }
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::F32(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::U8(values) => values.len(),
            Self::U32(values) => values.len(),
        }
    }
}

struct TensorSlot {
    per_sample: usize,
    fill: Fill,
    buffer: TensorBuffer,
}

/// 一批编码结果；每个张量按 `batch × 每样本元素数` 单独连续存放。
pub struct EncodedBatch {
    dims: [usize; 9],
    batch: usize,
    slots: BTreeMap<&'static str, TensorSlot>,
}

impl EncodedBatch {
    /// 按 manifest 声明的容量分配缓冲，并预填 padding 值。
    pub fn new<C: CapacityDims>(profile: &C, batch: usize) -> Self {
        let dims = profile.dims();
        let slots = TENSOR_SPECS
            .iter()
            .map(|spec| {
                let per_sample = tensor_shape(&dims, spec.name).map_or(0, |shape| shape.iter().product());
                (
                    spec.name,
                    TensorSlot {
                        per_sample,
                        fill: spec.fill,
                        buffer: TensorBuffer::allocate(spec.dtype, per_sample * batch, spec.fill),
                    },
                )
            })
            .collect();
        Self { dims, batch, slots }
    }

    /// 用冻结档位 `BASELINE_64` 分配；测试与一次性编码的便捷入口。
    pub fn baseline(batch: usize) -> Self { Self::new(&BASELINE_64, batch) }

    pub fn batch(&self) -> usize { self.batch }

    pub fn dims(&self) -> &[usize; 9] { &self.dims }

    /// 张量静态描述；未注册返回 None。
    pub fn spec(name: &str) -> Option<&'static TensorSpec> {
        TENSOR_SPECS.iter().find(|spec| spec.name == name)
    }

    /// 该张量的元素数（含 batch 轴）。
    pub fn tensor_len(&self, name: &str) -> Result<usize, EncodeError> { Ok(self.slot(name)?.buffer.len()) }

    /// 该张量的字节数；导出侧用它校验 `byte_length == product(shape) × sizeof(dtype)`。
    pub fn tensor_byte_len(&self, name: &str) -> Result<usize, EncodeError> {
        let spec = Self::spec(name).ok_or_else(|| EncodeError::UnknownTensor { name: name.to_owned() })?;
        Ok(self.tensor_len(name)? * spec.dtype.size())
    }

    /// 把一个批槽位恢复成 padding 值；复用缓冲时必须在写入前调用。
    pub fn clear_slot(&mut self, batch_index: usize) -> Result<(), EncodeError> {
        if batch_index >= self.batch {
            return Err(EncodeError::BatchSlotOutOfRange { batch: batch_index, limit: self.batch });
        }
        for slot in self.slots.values_mut() {
            let start = batch_index * slot.per_sample;
            slot.buffer.clear(start, slot.fill);
        }
        Ok(())
    }

    fn slot(&self, name: &str) -> Result<&TensorSlot, EncodeError> {
        self.slots
            .get(name)
            .ok_or_else(|| EncodeError::UnknownTensor { name: name.to_owned() })
    }

    fn slot_mut(&mut self, name: &str) -> Result<&mut TensorSlot, EncodeError> {
        self.slots
            .get_mut(name)
            .ok_or_else(|| EncodeError::UnknownTensor { name: name.to_owned() })
    }

    /// 整段只读视图（导出与测试用）。
    pub fn f32_all(&self, name: &str) -> Result<&[f32], EncodeError> {
        match &self.slot(name)?.buffer {
            TensorBuffer::F32(values) => Ok(values),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn i32_all(&self, name: &str) -> Result<&[i32], EncodeError> {
        match &self.slot(name)?.buffer {
            TensorBuffer::I32(values) => Ok(values),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn u8_all(&self, name: &str) -> Result<&[u8], EncodeError> {
        match &self.slot(name)?.buffer {
            TensorBuffer::U8(values) => Ok(values),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn u32_all(&self, name: &str) -> Result<&[u32], EncodeError> {
        match &self.slot(name)?.buffer {
            TensorBuffer::U32(values) => Ok(values),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    /// 批槽位可写行；偏移即 `batch_index × per_sample`。
    pub fn f32_row_mut(&mut self, name: &str, batch_index: usize) -> Result<&mut [f32], EncodeError> {
        self.check_batch(name, batch_index)?;
        let slot = self.slot_mut(name)?;
        let start = batch_index * slot.per_sample;
        match &mut slot.buffer {
            TensorBuffer::F32(values) => Ok(&mut values[start..start + slot.per_sample]),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn i32_row_mut(&mut self, name: &str, batch_index: usize) -> Result<&mut [i32], EncodeError> {
        self.check_batch(name, batch_index)?;
        let slot = self.slot_mut(name)?;
        let start = batch_index * slot.per_sample;
        match &mut slot.buffer {
            TensorBuffer::I32(values) => Ok(&mut values[start..start + slot.per_sample]),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn u8_row_mut(&mut self, name: &str, batch_index: usize) -> Result<&mut [u8], EncodeError> {
        self.check_batch(name, batch_index)?;
        let slot = self.slot_mut(name)?;
        let start = batch_index * slot.per_sample;
        match &mut slot.buffer {
            TensorBuffer::U8(values) => Ok(&mut values[start..start + slot.per_sample]),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    pub fn u32_row_mut(&mut self, name: &str, batch_index: usize) -> Result<&mut [u32], EncodeError> {
        self.check_batch(name, batch_index)?;
        let slot = self.slot_mut(name)?;
        let start = batch_index * slot.per_sample;
        match &mut slot.buffer {
            TensorBuffer::U32(values) => Ok(&mut values[start..start + slot.per_sample]),
            _ => Err(EncodeError::UnknownTensor { name: name.to_owned() }),
        }
    }

    /// 先查张量名再查批范围；与 `slot_mut` 分开以便同时借用。
    fn check_batch(&self, name: &str, batch_index: usize) -> Result<(), EncodeError> {
        self.slot(name)?;
        if batch_index >= self.batch {
            return Err(EncodeError::BatchSlotOutOfRange { batch: batch_index, limit: self.batch });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_have_shapes() {
        let mut seen = std::collections::BTreeSet::new();
        for spec in TENSOR_SPECS {
            assert!(seen.insert(spec.name), "张量名重复：{}", spec.name);
            assert!(
                tensor_shape(&BASELINE_64.dims(), spec.name).is_some(),
                "{} 缺少 shape 推导",
                spec.name
            );
        }
    }

    #[test]
    fn allocation_matches_shape_product_and_padding_values() {
        let batch = EncodedBatch::baseline(2);
        for spec in TENSOR_SPECS {
            let shape = tensor_shape(&BASELINE_64.dims(), spec.name).unwrap();
            let product: usize = shape.iter().product();
            assert_eq!(
                batch.tensor_len(spec.name).unwrap(),
                product * 2,
                "{} 的元素数必须等于 shape 之积 × batch",
                spec.name
            );
            match spec.fill {
                Fill::Zero => match spec.dtype {
                    Dtype::F32 => assert!(batch.f32_all(spec.name).unwrap().iter().all(|value| *value == 0.0)),
                    Dtype::I32 => assert!(batch.i32_all(spec.name).unwrap().iter().all(|value| *value == 0)),
                    Dtype::U8 => assert!(batch.u8_all(spec.name).unwrap().iter().all(|value| *value == 0)),
                    Dtype::U32 => assert!(batch.u32_all(spec.name).unwrap().iter().all(|value| *value == 0)),
                },
                Fill::RefMinusOne => {
                    assert!(
                        batch.i32_all(spec.name).unwrap().iter().all(|value| *value == -1),
                        "{} 的 padding 必须是 -1",
                        spec.name
                    );
                }
            }
        }
    }

    #[test]
    fn batch_slot_offset_is_per_tensor() {
        let mut batch = EncodedBatch::baseline(3);
        batch.u8_row_mut("entity_mask", 1).unwrap()[0] = 7;
        batch.i32_row_mut("entity_ref", 2).unwrap()[3] = 9;
        let mask = batch.u8_all("entity_mask").unwrap();
        assert_eq!(mask[0], 0);
        assert_eq!(mask[BASELINE_64.e_max], 7, "槽位 1 的偏移必须是 e_max");
        assert_eq!(mask[2 * BASELINE_64.e_max], 0);
        let reference = batch.i32_all("entity_ref").unwrap();
        assert_eq!(reference[3], -1);
        assert_eq!(reference[2 * 5 * BASELINE_64.e_max + 3], 9);
    }

    #[test]
    fn clear_slot_restores_padding() {
        let mut batch = EncodedBatch::baseline(1);
        {
            let mask = batch.u8_row_mut("entity_mask", 0).unwrap();
            mask[0] = 1;
            mask[5] = 1;
        }
        {
            let reference = batch.i32_row_mut("entity_ref", 0).unwrap();
            reference[0] = 3;
        }
        batch.clear_slot(0).unwrap();
        assert!(batch.u8_all("entity_mask").unwrap().iter().all(|value| *value == 0));
        assert!(batch.i32_all("entity_ref").unwrap().iter().all(|value| *value == -1));
    }

    #[test]
    fn byte_len_matches_shape_times_dtype_size() {
        let batch = EncodedBatch::baseline(4);
        for spec in TENSOR_SPECS {
            let shape = tensor_shape(&BASELINE_64.dims(), spec.name).unwrap();
            let product: usize = shape.iter().product();
            let expected = product * 4 * spec.dtype.size();
            assert_eq!(batch.tensor_byte_len(spec.name).unwrap(), expected, "{}", spec.name);
        }
    }

    #[test]
    fn row_accessors_reject_out_of_range_batch_and_unknown_tensor() {
        let mut batch = EncodedBatch::baseline(2);
        assert!(matches!(
            batch.u8_row_mut("entity_mask", 2).unwrap_err(),
            EncodeError::BatchSlotOutOfRange { batch: 2, limit: 2 }
        ));
        assert!(matches!(
            batch.u8_row_mut("extra_index", 0).unwrap_err(),
            EncodeError::UnknownTensor { .. }
        ));
    }

    #[test]
    fn wrong_dtype_access_is_rejected() {
        let batch = EncodedBatch::baseline(1);
        // entity_mask 是 u8；用 f32 视图访问必须报未知张量而不是 reinterpret。
        assert!(matches!(batch.f32_all("entity_mask").unwrap_err(), EncodeError::UnknownTensor { .. }));
    }
}
