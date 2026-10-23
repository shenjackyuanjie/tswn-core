//! # 名竞构建属性 (stats)
//!
//! 本模块定义 [`PlayerStats`] 纯数据结构，承接输入构建和 Runtime 模板初始化。
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
//! | `magic_point`| i32    | 魔法值              |
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
//! use tswn_core::namerena::PlayerStats;
//!
//! let mut status = PlayerStats::default();
//! status.hp = 100;
//! status.max_hp = 100;
//! status.attack = 50;
//! println!("{}", status);
//! // 输出: PlayerStats{正常,存活 分数: 0, hp: 100 移动点数: 0 sums:0,0,0 攻|50 防|0 速|0 敏|0 魔|0 magic_point|0 抗|0 智|0 }
//! ```

/// 名字构建完成后的属性与初始状态值，供 Runtime 导入或导出配置使用。
///
/// 这是独立拥有的纯数据，不会跟随战斗内伤害、状态或吞噬自动更新；
/// 当前战斗展示值应读取 Runtime 快照。字段可独立赋值，构造方负责维持体力、
/// 存活标记与派生总和的一致性，默认值不等于一个已完成构建的可战斗玩家。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStats {
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
    pub magic_point: i32,
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

impl PlayerStats {
    /// 检查是否被冻结
    #[inline]
    pub fn frozed(&self) -> bool { self.frozen }
    /// 检查是否存活
    #[inline]
    pub fn alive(&self) -> bool { self.alive }
    /// 返回原始移动点数；兼容旧调用，新代码直接读取 `move_point` 字段。
    #[deprecated(note = "请直接读取 move_point 字段")]
    #[inline]
    pub fn spsum(&self) -> i32 { self.move_point }
    /// 判断移动点是否严格超过行动阈值；不检查冻结、存活或体力，也不扣除移动点。
    #[inline]
    pub fn check_move(&self) -> bool { self.move_point > crate::runtime::MOVE_POINT_THRESHOLD }

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

    /// 获取智力；兼容旧调用，新代码直接读取 `wisdom` 字段。
    #[inline]
    #[deprecated(note = "self.wisdom")]
    pub fn itl(&self) -> i32 { self.wisdom }
}

/// 默认实现，创建一个初始状态的玩家状态。
impl Default for PlayerStats {
    fn default() -> Self {
        PlayerStats {
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
            magic_point: 0,
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
impl std::fmt::Display for PlayerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlayerStats{{{},{} 分数: {}, hp: {} 移动点数: {} sums:{},{},{} 攻|{} 防|{} 速|{} 敏|{} 魔|{} magic_point|{} 抗|{} 智|{} }}",
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
            self.magic_point,
            self.resistance,
            self.wisdom
        )
    }
}
