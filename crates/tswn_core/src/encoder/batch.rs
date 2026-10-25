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

use crate::encoder::capacity::{BASELINE_64, EncoderProfile};
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
    /// 混合索引行：仅引用列填 -1，其余分类与 ordinal 列填 0。
    Index { columns: usize, reference_columns: u8 },
}

impl Fill {
    fn i32_at(self, index: usize) -> i32 {
        match self {
            Self::Zero => 0,
            Self::RefMinusOne => -1,
            Self::Index {
                columns,
                reference_columns,
            } => {
                if reference_columns & (1u8 << (index % columns)) != 0 {
                    -1
                } else {
                    0
                }
            }
        }
    }
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
            self.e_max, self.t_max, self.r_max, self.h_max, self.l_max, self.s_max, self.q_max, self.v_max, self.x_max,
        ]
    }
}

impl CapacityDims for ProfileSpec {
    fn dims(&self) -> [usize; 9] {
        [
            self.e_max, self.t_max, self.r_max, self.h_max, self.l_max, self.s_max, self.q_max, self.v_max, self.x_max,
        ]
    }
}

/// 已实现的全部张量族（含 mask、presence 与精确旁路）。
///
/// 追加时必须同步规格第 4 节的张量表与 manifest 张量注册表，
/// 并保持“每个张量只登记一次、名称全局唯一”。
pub const TENSOR_SPECS: &[TensorSpec] = &[
    // global
    TensorSpec {
        name: "global_num",
        dtype: Dtype::F32,
        shape: "[6]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "team_mask",
        dtype: Dtype::U8,
        shape: "[T_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "runtime_team_mask",
        dtype: Dtype::U8,
        shape: "[R_max]",
        fill: Fill::Zero,
    },
    // entity
    TensorSpec {
        name: "entity_mask",
        dtype: Dtype::U8,
        shape: "[E_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_template",
        dtype: Dtype::I32,
        shape: "[E_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "entity_team",
        dtype: Dtype::I32,
        shape: "[E_max,2]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "entity_num",
        dtype: Dtype::F32,
        shape: "[E_max,24]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_num_present",
        dtype: Dtype::U8,
        shape: "[E_max,24]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_bool",
        dtype: Dtype::U8,
        shape: "[E_max,10]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_flags",
        dtype: Dtype::U8,
        shape: "[E_max,8]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_kind_flags",
        dtype: Dtype::U8,
        shape: "[E_max,6]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_cat",
        dtype: Dtype::I32,
        shape: "[E_max,5]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "entity_ref",
        dtype: Dtype::I32,
        shape: "[E_max,5]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "entity_ref_present",
        dtype: Dtype::U8,
        shape: "[E_max,5]",
        fill: Fill::Zero,
    },
    // template
    TensorSpec {
        name: "template_mask",
        dtype: Dtype::U8,
        shape: "[H_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_num",
        dtype: Dtype::F32,
        shape: "[H_max,31]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_num_present",
        dtype: Dtype::U8,
        shape: "[H_max,31]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_bool",
        dtype: Dtype::U8,
        shape: "[H_max,5]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_cat",
        dtype: Dtype::I32,
        shape: "[H_max,5]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_cat_present",
        dtype: Dtype::U8,
        shape: "[H_max,5]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_team",
        dtype: Dtype::I32,
        shape: "[H_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "template_player_ref",
        dtype: Dtype::I32,
        shape: "[H_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "template_override_present",
        dtype: Dtype::U8,
        shape: "[H_max,4]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_clone_attr",
        dtype: Dtype::U32,
        shape: "[H_max,8]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "template_clone_weapon_bonus",
        dtype: Dtype::I32,
        shape: "[H_max,8]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "immunity_num",
        dtype: Dtype::F32,
        shape: "[H_max,9]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "immunity_present",
        dtype: Dtype::U8,
        shape: "[H_max,9]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "clan_equal",
        dtype: Dtype::U8,
        shape: "[H_max,H_max]",
        fill: Fill::Zero,
    },
    // lane
    TensorSpec {
        name: "lane_mask",
        dtype: Dtype::U8,
        shape: "[L_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_skill_id",
        dtype: Dtype::I32,
        shape: "[L_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_boost_kind",
        dtype: Dtype::I32,
        shape: "[L_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_template",
        dtype: Dtype::I32,
        shape: "[L_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "lane_key",
        dtype: Dtype::I32,
        shape: "[L_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_num",
        dtype: Dtype::F32,
        shape: "[L_max,4]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_num_present",
        dtype: Dtype::U8,
        shape: "[L_max,4]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "lane_bool",
        dtype: Dtype::U8,
        shape: "[L_max,1]",
        fill: Fill::Zero,
    },
    // list/order
    TensorSpec {
        name: "list_mask",
        dtype: Dtype::U8,
        shape: "[V_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "list_index",
        dtype: Dtype::I32,
        shape: "[V_max,5]",
        fill: Fill::Index {
            columns: 5,
            reference_columns: 0b10010,
        },
    },
    TensorSpec {
        name: "list_position",
        dtype: Dtype::F32,
        shape: "[V_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "order_key",
        dtype: Dtype::U32,
        shape: "[V_max,2]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "order_key_present",
        dtype: Dtype::U8,
        shape: "[V_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "order_rank",
        dtype: Dtype::F32,
        shape: "[V_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "order_rank_present",
        dtype: Dtype::U8,
        shape: "[V_max]",
        fill: Fill::Zero,
    },
    // state
    TensorSpec {
        name: "state_mask",
        dtype: Dtype::U8,
        shape: "[S_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_entity",
        dtype: Dtype::I32,
        shape: "[S_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "state_kind",
        dtype: Dtype::I32,
        shape: "[S_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_cat",
        dtype: Dtype::I32,
        shape: "[S_max,2]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_cat_present",
        dtype: Dtype::U8,
        shape: "[S_max,2]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_hook",
        dtype: Dtype::U8,
        shape: "[S_max,64]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_num",
        dtype: Dtype::F32,
        shape: "[S_max,9]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_num_present",
        dtype: Dtype::U8,
        shape: "[S_max,9]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_ref",
        dtype: Dtype::I32,
        shape: "[S_max,4]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "state_ref_present",
        dtype: Dtype::U8,
        shape: "[S_max,4]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "state_group",
        dtype: Dtype::I32,
        shape: "[S_max,3]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "state_group_present",
        dtype: Dtype::U8,
        shape: "[S_max,3]",
        fill: Fill::Zero,
    },
    // slot
    TensorSpec {
        name: "slot_mask",
        dtype: Dtype::U8,
        shape: "[Q_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "slot_index",
        dtype: Dtype::I32,
        shape: "[Q_max,4]",
        fill: Fill::Index {
            columns: 4,
            reference_columns: 0b0010,
        },
    },
    TensorSpec {
        name: "slot_field_present",
        dtype: Dtype::U8,
        shape: "[Q_max,4]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "slot_value",
        dtype: Dtype::F32,
        shape: "[Q_max,1]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "slot_value_present",
        dtype: Dtype::U8,
        shape: "[Q_max,1]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "slot_template",
        dtype: Dtype::I32,
        shape: "[Q_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "slot_template_present",
        dtype: Dtype::U8,
        shape: "[Q_max]",
        fill: Fill::Zero,
    },
    // extra
    TensorSpec {
        name: "extra_mask",
        dtype: Dtype::U8,
        shape: "[X_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "extra_index",
        dtype: Dtype::I32,
        shape: "[X_max,4]",
        fill: Fill::Index {
            columns: 4,
            reference_columns: 0b0010,
        },
    },
    TensorSpec {
        name: "extra_num",
        dtype: Dtype::F32,
        shape: "[X_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "extra_ref",
        dtype: Dtype::I32,
        shape: "[X_max]",
        fill: Fill::RefMinusOne,
    },
    TensorSpec {
        name: "extra_bool",
        dtype: Dtype::U8,
        shape: "[X_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "extra_cat",
        dtype: Dtype::I32,
        shape: "[X_max]",
        fill: Fill::Zero,
    },
    TensorSpec {
        name: "extra_bits",
        dtype: Dtype::U32,
        shape: "[X_max,2]",
        fill: Fill::Zero,
    },
];

/// 张量的具体 shape（不含 batch 轴）；用于推导每样本元素数与字节数。
pub fn tensor_shape(dims: &[usize; 9], name: &str) -> Option<Vec<usize>> {
    let [e, t, r, h, l, s, q, v, x] = *dims;
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
        "lane_mask" | "lane_skill_id" | "lane_boost_kind" | "lane_template" | "lane_key" => vec![l],
        "lane_num" | "lane_num_present" => vec![l, 4],
        "lane_bool" => vec![l, 1],
        "list_mask" | "list_position" | "order_key_present" | "order_rank" | "order_rank_present" => vec![v],
        "list_index" => vec![v, 5],
        "order_key" => vec![v, 2],
        "state_mask" | "state_entity" | "state_kind" => vec![s],
        "state_cat" | "state_cat_present" => vec![s, 2],
        "state_hook" => vec![s, 64],
        "state_num" | "state_num_present" => vec![s, 9],
        "state_ref" | "state_ref_present" => vec![s, 4],
        "state_group" | "state_group_present" => vec![s, 3],
        "slot_mask" | "slot_template" | "slot_template_present" => vec![q],
        "slot_value" | "slot_value_present" => vec![q, 1],
        "slot_index" | "slot_field_present" => vec![q, 4],
        "extra_mask" | "extra_num" | "extra_ref" | "extra_bool" | "extra_cat" => vec![x],
        "extra_index" => vec![x, 4],
        "extra_bits" => vec![x, 2],
        _ => return None,
    })
}

#[derive(Debug)]
enum TensorBuffer {
    F32(Vec<f32>),
    I32(Vec<i32>),
    U8(Vec<u8>),
    U32(Vec<u32>),
}

impl TensorBuffer {
    fn allocate(dtype: Dtype, len: usize, fill: Fill) -> Self {
        match (dtype, fill) {
            (Dtype::F32, _) => Self::F32(vec![fill.i32_at(0) as f32; len]),
            (Dtype::I32, Fill::Zero) => Self::I32(vec![0; len]),
            (Dtype::I32, Fill::RefMinusOne) => Self::I32(vec![-1; len]),
            (Dtype::I32, Fill::Index { .. }) => Self::I32((0..len).map(|index| fill.i32_at(index)).collect()),
            (Dtype::U8, _) => Self::U8(vec![0; len]),
            (Dtype::U32, _) => Self::U32(vec![0; len]),
        }
    }

    fn clear(&mut self, start: usize, len: usize, fill: Fill) {
        match self {
            Self::F32(values) => {
                let value = fill.i32_at(0) as f32;
                for slot in &mut values[start..start + len] {
                    *slot = value;
                }
            }
            Self::I32(values) => {
                for (index, slot) in values[start..start + len].iter_mut().enumerate() {
                    *slot = fill.i32_at(index);
                }
            }
            Self::U8(values) => {
                for slot in &mut values[start..start + len] {
                    *slot = 0;
                }
            }
            Self::U32(values) => {
                for slot in &mut values[start..start + len] {
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

#[derive(Debug)]
struct TensorSlot {
    per_sample: usize,
    fill: Fill,
    buffer: TensorBuffer,
}

/// 一批拥有独立存储的编码结果；每个张量按 `batch × 每样本元素数` 单独连续存放。
///
/// 分配后所有位置都是 padding。读取和可写行视图都借用本缓冲；可写访问仅检查张量名、
/// dtype 与批下标，不维护 mask、presence 或引用语义，完整写入宜通过编码器入口完成。
#[derive(Debug)]
pub struct EncodedBatch {
    dims: [usize; 9],
    batch: usize,
    slots: BTreeMap<&'static str, TensorSlot>,
}

impl EncodedBatch {
    /// 按九维容量和批大小为每个张量分配存储，并预填 padding。
    ///
    /// `batch=0` 合法，此时没有可写批槽位。此入口不执行 manifest 门禁，也不限制总字节数；
    /// 调用方应预先校验容量并控制批大小，防止 shape 乘积溢出或分配过大。
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
    pub fn spec(name: &str) -> Option<&'static TensorSpec> { TENSOR_SPECS.iter().find(|spec| spec.name == name) }

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
            return Err(EncodeError::BatchSlotOutOfRange {
                batch: batch_index,
                limit: self.batch,
            });
        }
        for slot in self.slots.values_mut() {
            let start = batch_index * slot.per_sample;
            slot.buffer.clear(start, slot.per_sample, slot.fill);
        }
        Ok(())
    }

    fn slot(&self, name: &str) -> Result<&TensorSlot, EncodeError> {
        self.slots.get(name).ok_or_else(|| EncodeError::UnknownTensor { name: name.to_owned() })
    }

    fn slot_mut(&mut self, name: &str) -> Result<&mut TensorSlot, EncodeError> {
        self.slots
            .get_mut(name)
            .ok_or_else(|| EncodeError::UnknownTensor { name: name.to_owned() })
    }

    /// 借用 f32 张量的全部批数据，按 batch 主序排列，不复制元素。
    /// 未知张量或 dtype 不符均返回 [`EncodeError::UnknownTensor`]。
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

    /// 将整段张量复制成调用方拥有的小端字节数组，长度与 [`Self::tensor_byte_len`] 一致。
    ///
    /// 包含全部批槽位和 padding；f32 按原始 IEEE bit 导出，不作文本转换或归一化。
    /// 每次调用都会新分配，导出器不应依赖主机字节序。
    pub fn tensor_bytes(&self, name: &str) -> Result<Vec<u8>, EncodeError> {
        let mut bytes = Vec::with_capacity(self.tensor_byte_len(name)?);
        match &self.slot(name)?.buffer {
            TensorBuffer::F32(values) => values.iter().for_each(|value| bytes.extend(value.to_le_bytes())),
            TensorBuffer::I32(values) => values.iter().for_each(|value| bytes.extend(value.to_le_bytes())),
            TensorBuffer::U8(values) => bytes.extend(values),
            TensorBuffer::U32(values) => values.iter().for_each(|value| bytes.extend(value.to_le_bytes())),
        }
        Ok(bytes)
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
            return Err(EncodeError::BatchSlotOutOfRange {
                batch: batch_index,
                limit: self.batch,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
