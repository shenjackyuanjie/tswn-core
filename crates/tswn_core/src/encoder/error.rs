//! encoder 错误类型。
//!
//! 所有错误都带**字段路径**（第 3 节的完整字段路径或容量维度名），便于离线导出时定位到行与字段；
//! 容量与引用错误还带原始值，不允许把错误行静默删除或截断后继续。

use std::fmt::{self, Display, Formatter};

/// 编码或 manifest 校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// schema 名称或版本不符（manifest 协议身份、state schema、encoder 语义版本）。
    SchemaMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    /// `world.winner_team` 非空的已决状态；结果泄漏门禁。
    AlreadyDecided { path: String },
    /// 容量超限；禁止截断实体、lane、状态或蓝图。
    CapacityExceeded { path: String, actual: usize, limit: usize },
    /// 缺少该字段的归一化常数；不允许用 0/1 或相邻字段顶替。
    MissingCalibration { path: String },
    /// 数值通道出现 NaN/Inf。
    NonFiniteValue { path: String },
    /// 压缩状态标志的高 3 个保留位被置 1；不得静默清除。
    ReservedFlagBitSet { path: String },
    /// 分类原值不在冻结词表内；不得折叠为 PAD。
    UnknownCategory { path: String, raw: String },
    /// 同一列表内出现重复分类键（如重复的 immunity status）。
    DuplicateCategory { path: String },
    /// manifest 未声明该分类域的词表。
    MissingVocabulary { path: String },
    /// 引用不是本局实体（悬空或跨域同号）。
    InvalidReference { path: String, raw: String },
    /// 结构性输入错误（输入队伍为空、kind 与载荷分支不匹配等）。
    InvalidState { path: String },
    /// Boss 专属载荷 kind；本轮不支持。
    UnsupportedPayloadKind { path: String, kind: String },
    /// 未登记或与白名单不符的槽语义。
    UnknownSlotSemantics { path: String },
    /// 槽值分支无效（全 None 或多个 Some）。
    InvalidSlotValue { path: String },
    /// manifest 自洽性检查失败或与实现登记表不一致。
    ManifestMismatch { path: String, detail: String },
    /// 批槽位越界。
    BatchSlotOutOfRange { batch: usize, limit: usize },
    /// 访问了未注册的张量名。
    UnknownTensor { name: String },
}

impl Display for EncodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch { path, expected, actual } => {
                write!(f, "{path}: schema 不符，期望 {expected}，实际 {actual}")
            }
            Self::AlreadyDecided { path } => write!(f, "{path}: 已决状态禁止编码（结果泄漏）"),
            Self::CapacityExceeded { path, actual, limit } => {
                write!(f, "{path}: 容量超限，实际 {actual} > 上限 {limit}，禁止截断")
            }
            Self::MissingCalibration { path } => write!(f, "{path}: 缺少归一化常数（MissingCalibration）"),
            Self::NonFiniteValue { path } => write!(f, "{path}: 数值非有限（NaN/Inf）"),
            Self::ReservedFlagBitSet { path } => write!(f, "{path}: 压缩状态保留位被置 1"),
            Self::UnknownCategory { path, raw } => write!(f, "{path}: 未知分类原值 {raw}"),
            Self::DuplicateCategory { path } => write!(f, "{path}: 分类键重复"),
            Self::MissingVocabulary { path } => write!(f, "{path}: manifest 未声明该分类域词表"),
            Self::InvalidReference { path, raw } => write!(f, "{path}: 无效引用 {raw}"),
            Self::InvalidState { path } => write!(f, "{path}: 输入状态不满足编码前提"),
            Self::UnsupportedPayloadKind { path, kind } => write!(f, "{path}: 不支持的载荷 kind {kind}"),
            Self::UnknownSlotSemantics { path } => write!(f, "{path}: 未登记或与白名单不符的槽语义"),
            Self::InvalidSlotValue { path } => write!(f, "{path}: 槽值分支无效"),
            Self::ManifestMismatch { path, detail } => write!(f, "{path}: {detail}"),
            Self::BatchSlotOutOfRange { batch, limit } => {
                write!(f, "批槽位 {batch} 越界（批大小 {limit}）")
            }
            Self::UnknownTensor { name } => write!(f, "未注册的张量 {name}"),
        }
    }
}

impl std::error::Error for EncodeError {}
