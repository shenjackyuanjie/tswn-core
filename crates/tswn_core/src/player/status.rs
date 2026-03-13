//! # 玩家状态 (status)
//!
//! 本模块定义 [`PlayerStatus`] 结构体，存储玩家的各种属性和状态。
//!
//! ## 字段说明
//!
//! | 字段        | 类型   | 说明                |
//! |-------------|--------|---------------------|
//! | `frozen`    | bool   | 是否被冻结           |
//! | `alive`     | bool   | 是否存活             |
//! | `point`     | u32    | 分数                |
//! | `move_point` | i32    | 移动点数            |
//! | `hp`        | i32    | 当前生命值           |
//! | `max_hp`    | i32    | 最大生命值           |
//! | `attack`    | i32    | 攻击力              |
//! | `defense`   | i32    | 防御力              |
//! | `speed`     | i32    | 速度                |
//! | `agility`   | i32    | 敏捷                |
//! | `magic`     | i32    | 魔法                |
//! | `mp`        | i32    | 魔法值              |
//! | `resistance`| i32    | 抗性                |
//! | `wisdom`    | i32    | 智力                |
//! | `at_boost`  | f64    | 攻击加成倍率        |
//! | `attract`   | f64    | 吸引力              |
//! | `attr_sum`  | u32    | 属性总和            |
//! | `atk_sum`   | i32    | 攻击总和            |
//! | `all_sum`   | u32    | 全部总和            |
//!
//! ## 方法说明
//!
//! - **查询方法** — `frozed()`、`alive()`、`check_move()`
//! - **设置方法** — `set_frozen()`、`set_alive()`、`set_point()`
//! - **已弃用方法** — `spsum()`、`mdf()`、`itl()`（请使用字段直接访问）
//!
//! ## Display 实现
//!
//! 实现了 `Display` trait，用于格式化输出玩家状态信息。
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::status::PlayerStatus;
//!
//! let mut status = PlayerStatus::default();
//! status.hp = 100;
//! status.max_hp = 100;
//! status.attack = 50;
//! println!("{}", status);
//! // 输出: PlayerStatus{正常,存活 分数: 0, hp: 100 移动点数: 0 sums:0,0,0 攻|50 防|0 速|0 敏|0 魔|0 mp|0 抗|0 智|0 }
//! ```

/// 玩家状态结构体，存储玩家的各种属性和状态。
#[derive(Clone, Copy, Debug)]
pub struct PlayerStatus {
    /// 是否被冻结
    pub frozen: bool,
    /// 是否存活
    pub alive: bool,
    /// 分数
    pub point: u32,
    /// 移动点数
    pub move_point: i32,
    /// 当前生命值
    pub hp: i32,
    /// 最大生命值
    pub max_hp: i32,
    /// 攻击力
    pub attack: i32,
    /// 防御力
    pub defense: i32,
    /// 速度
    pub speed: i32,
    /// 敏捷
    pub agility: i32,
    /// 魔法
    pub magic: i32,
    /// 魔法值
    pub mp: i32,
    /// 抗性
    pub resistance: i32,
    /// 智力
    pub wisdom: i32,
    /// 攻击加成倍率
    pub at_boost: f64,
    /// 吸引力
    pub attract: f64,
    /// 属性总和
    pub attr_sum: u32,
    /// 攻击总和
    pub atk_sum: i32,
    /// 全部总和
    pub all_sum: u32,
}

impl PlayerStatus {
    /// 检查是否被冻结
    #[inline]
    pub fn frozed(&self) -> bool { self.frozen }
    /// 检查是否存活
    #[inline]
    pub fn alive(&self) -> bool { self.alive }
    /// 获取移动点数 (已弃用，请使用 move_point())
    #[deprecated(note = "请使用 move_point()")]
    #[inline]
    pub fn spsum(&self) -> i32 { self.move_point }
    /// 检查是否可以移动
    #[inline]
    pub fn check_move(&self) -> bool { self.move_point > crate::player::MOVE_POINT_THRESHOLD }

    /// 设置冻结状态
    pub fn set_frozen(&mut self, val: bool) { self.frozen = val }
    /// 设置存活状态
    pub fn set_alive(&mut self, val: bool) { self.alive = val }
    /// 设置分数
    pub fn set_point(&mut self, val: u32) { self.point = val }

    /// 获取抗性 (已弃用，请使用 self.resistance)
    #[inline]
    #[deprecated(note = "self.resistance")]
    pub fn mdf(&self) -> i32 { self.resistance }

    /// 获取智利 (已弃用，请使用 self.wisdom)
    #[inline]
    #[deprecated(note = "self.wisdom")]
    pub fn itl(&self) -> i32 { self.wisdom }
}

/// 默认实现，创建一个初始状态的玩家状态。
impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            frozen: false,
            alive: true,
            point: 0,
            move_point: 0,
            hp: 0,
            max_hp: 0,
            attack: 0,
            defense: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            mp: 0,
            resistance: 0,
            wisdom: 0,
            at_boost: 1.0,
            attract: 32768.0,
            attr_sum: 0,
            atk_sum: 0,
            all_sum: 0,
        }
    }
}

/// Display 实现，用于格式化输出玩家状态。
impl std::fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStatus{{{},{} 分数: {}, hp: {} 移动点数: {} sums:{},{},{} 攻|{} 防|{} 速|{} 敏|{} 魔|{} mp|{} 抗|{} 智|{} }}",
            if self.frozen { "冻结" } else { "正常" },
            if self.alive { "存活" } else { "死亡" },
            self.point,
            self.hp,
            self.move_point,
            self.attr_sum,
            self.atk_sum,
            self.all_sum,
            self.attack,
            self.defense,
            self.speed,
            self.agility,
            self.magic,
            self.mp,
            self.resistance,
            self.wisdom
        )
    }
}
