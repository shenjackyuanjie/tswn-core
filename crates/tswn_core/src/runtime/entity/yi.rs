//! 咦 Boss 的行动计数与拜谢印记。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct YiRuntime {
    /// 实际行动次数；第一次行动必定是咦拜涂地。
    pub turn: u32,
    /// 猴咦射日累计次数。
    pub sun_count: u32,
    /// 冗余记录拥有者。
    pub owner: Option<EntityIdx>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BaixieRuntime {
    /// 已经被这只拜谢拜过的角色。
    pub thanked: Vec<EntityIdx>,
    /// 冗余记录拥有者。
    pub owner: Option<EntityIdx>,
}
