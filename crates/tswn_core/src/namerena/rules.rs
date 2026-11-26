//! 名竞内置角色与数值规则。

/// 埼玉退场判定使用的身份名。
pub const SAITAMA_RETIREMENT_WINNER: &str = "SAITAMA";

/// 内置 Boss 名字表；用于身份分类、默认技能与免疫规则查询。
pub const BOSS_NAMES: [&str; 22] = [
    "mario",
    "sonic",
    "mosquito",
    "yuri",
    "slime",
    "ikaruga",
    "conan",
    "aokiji",
    "lazy",
    "covid",
    "saitama",
    "testsubject",
    "thanos",
    "wheel",
    SAITAMA_RETIREMENT_WINNER,
    "Squall",
    "quilt",
    "smdd",
    "新纪元",
    "咦",
    "TigerStar",
    "Shabby_fish",
];

/// 加成角色名字表；对应 [`boost_value`] 的加成来源。
pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

/// Boss 战斗名归一化：埼玉统一为小写，其余原样返回。
pub fn boss_combat_name(name: &str) -> &str {
    match name {
        SAITAMA_RETIREMENT_WINNER => "saitama",
        _ => name,
    }
}

/// Boss 身份覆盖：仅埼玉返回固定小写身份。
pub fn boss_combat_identity_override(identity: &str) -> Option<&'static str> {
    match identity {
        "SAITAMA@!" => Some("saitama@!"),
        _ => None,
    }
}

/// Boss 的中文展示名；未知名按原名返回。
pub fn boss_display_name(name: &str) -> &str {
    match name {
        "mario" => "马里奥",
        "sonic" => "索尼克",
        "mosquito" => "蚊",
        "yuri" => "尤里",
        "slime" => "史莱姆",
        "ikaruga" => "斑鸠",
        "conan" => "柯南",
        "aokiji" => "青雉",
        "lazy" => "懒癌",
        "covid" => "新冠病毒",
        "saitama" => "一拳超人",
        "testsubject" => "实验体 #C8",
        "thanos" => "灭霸",
        "wheel" => "生命之轮",
        "quilt" => "被子",
        "smdd" => "曙光",
        "新纪元" => "新纪元",
        "咦" => "咦",
        "TigerStar" => "豹星",
        "Shabby_fish" => "沙冰鱼",
        _ => name,
    }
}

/// 各 Boss 的八围加成，与原版 `appendAttr`（0.5.5 中为 `gan()`）对应。
pub fn boss_append_attr(name: &str) -> [i32; 8] {
    match boss_combat_name(name) {
        "covid" => [10, 9, 0, 12, 0, 12, 0, 60],
        "lazy" => [0, 88, 10, -20, 0, 50, 0, 120],
        "saitama" => [72, 39, 69, 76, 67, 66, 0, 84],
        "mario" => [0, 38, 31, 46, 28, 18, 15, 69],
        "sonic" => [10, -6, 1000, 0, 10, -15, 6, 0],
        "mosquito" => [-3, 24, 29, 729, 5, 7, 12, -35],
        "yuri" => [26, 31, 46, 9, 40, 5, 32, 24],
        "slime" => [6, 21, 5, 19, 38, 21, 12, 62],
        "ikaruga" => [48, 28, 21, 45, 10, 19, 33, 150],
        "conan" => [0, 48, -33, 20, 0, 41, 30, 22],
        "aokiji" => [40, 30, 40, 10, 35, 4, 40, 96],
        "testsubject" => [0; 8],
        "thanos" => [39, 36, 12, 16, 0, 37, 20, 113],
        "wheel" => [23, 135, 0, 19, 27, 89, 10, 39],
        _ => [0; 8],
    }
}

/// 0.5.5 中各 Boss 的 `ac()` 直接写入 `k1` 的默认技能等级（`f`）。
///
/// 编号沿用通用技能键；只列出能以通用技能表达的条目，其余专属技能
/// （如 `SklAokijiIceAge`、`SklConan`）由各自机制处理。这些等级不参与
/// 名字推导，也不触发暴击式翻倍。
pub fn boss_default_skill_levels(name: &str) -> &'static [(usize, u32)] {
    match boss_combat_name(name) {
        "aokiji" => &[(1, 80)],
        "mosquito" => &[(4, 100)],
        "sonic" => &[(6, 48), (7, 48), (30, 48)],
        "yuri" => &[(25, 48), (27, 48)],
        "thanos" => &[(29, 80)],
        "wheel" => &[(30, 48)],
        _ => &[],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 0.5.5 中各 Boss `ac()` 写入 `k1`、随后由通用扫描进入 `k4` 的主动技能。
///
/// 通用技能沿用现有内置实现；专属技能（`IceAge`、`IkarugaAttack`、`MarioGet`、
/// `YuriControl`、`Saitama`）由 Boss 机制分支结算。
pub enum BossActionSkill {
    Fire,
    Ice,
    Absorb,
    Rapid,
    Critical,
    IceAge,
    IkarugaAttack,
    MarioGet,
    YuriControl,
    Saitama,
    /// Squall 的科研技能与研究助手统一进入专属结算器。
    Squall,
    /// 被子的钻出被子、多重魂、占领与鱼我所欲也统一进入专属结算器。
    Quilt,
    /// smdd 的十个做局技能统一进入专属结算器。
    Smdd,
    /// 新纪元的猎豹与人海技能统一进入专属结算器。
    Xjy,
    /// 咦与拜谢的行动统一进入专属结算器。
    Yi,
    /// TigerStar 的行动统一进入专属结算器。
    Tiger,
    /// 沙冰鱼的行动统一进入专属结算器。
    Fish,
}

/// 按 0.5.5 中 `k4` 的插入顺序返回 Boss 主动技能及基础等级。
///
/// `k4` 只收集 `ActionSkill` 实例，所以防御、反射等被动不会出现在这里；
/// 马里奥的火球术初始等级为 0，吃到火焰花后由状态载荷改写为 120。
pub fn boss_action_skills(name: &str) -> &'static [(BossActionSkill, u32)] {
    match boss_combat_name(name) {
        "aokiji" => &[(BossActionSkill::IceAge, 70), (BossActionSkill::Ice, 80)],
        "ikaruga" => &[(BossActionSkill::IkarugaAttack, 48)],
        "mario" => &[(BossActionSkill::MarioGet, 63), (BossActionSkill::Fire, 0)],
        "mosquito" => &[(BossActionSkill::Absorb, 100)],
        "saitama" => &[(BossActionSkill::Saitama, 0)],
        "sonic" => &[(BossActionSkill::Rapid, 48), (BossActionSkill::Critical, 48)],
        "yuri" => &[(BossActionSkill::YuriControl, 256)],
        _ => &[],
    }
}

/// 加成角色的加成值；非加成角色返回 0。
pub fn boost_value(name: &str) -> u32 {
    match name {
        "云剑狄卡敢" => 25,
        "云剑穸跄祇" => 35,
        "田一人" => 18,
        _ => 0,
    }
}

/// 0.5.5 中 `k3` 使用 `SklAttack` 的 Boss；它们的默认攻击可以走魔法分支。
pub fn boss_uses_skill_attack(name: &str) -> bool { matches!(boss_combat_name(name), "aokiji" | "ikaruga") }

/// Boss 行动概率的抽样条目数；`covid` / `lazy` / `testsubject` 为 0，其余为 1。
pub fn boss_action_prob_count(name: &str) -> usize {
    match boss_combat_name(name) {
        "covid" | "lazy" | "testsubject" => 0,
        _ => 1,
    }
}

/// 阈值来自各 Boss 的强免疫（240）、普通免疫（192）和兜底档（84）。
/// 返回 0 表示恒不免疫且不抽样；`piston` 是毒状态在 0.5.5 中的键名。
pub fn boss_immune_threshold(name: &str, key: &str) -> i32 {
    match boss_combat_name(name) {
        "saitama" => match key {
            "half" | "exchange" => 240,
            "berserk" | "slow" | "ice" => 192,
            _ => 84,
        },
        "covid" => match key {
            "charm" | "berserk" | "exchange" => 192,
            _ => 84,
        },
        "lazy" => match key {
            "assassinate" | "half" | "curse" | "exchange" => 192,
            _ => 84,
        },
        "conan" => match key {
            "charm" => 240,
            "assassinate" | "berserk" | "half" | "curse" | "exchange" | "slow" | "ice" => 192,
            _ => 84,
        },
        "ikaruga" => match key {
            "curse" => 0, // 原版 Ikaruga 对诅咒恒不免疫，且不抽取随机数。
            "assassinate" | "half" | "exchange" | "piston" | "slow" | "ice" => 192,
            _ => 84,
        },
        "mario" => match key {
            "assassinate" => 192,
            "disperse" => 0, // 原版马里奥对净化恒不免疫，也不抽随机数。
            _ => 84,
        },
        "mosquito" => match key {
            "assassinate" | "disperse" => 240,
            "berserk" | "charm" => 192,
            _ => 84,
        },
        "slime" | "sonic" => match key {
            "piston" => 192,
            _ => 84,
        },
        "yuri" => 84,
        "testsubject" => match key {
            "berserk" => 256,
            _ => 84,
        },
        _ => match key {
            "assassinate" | "charm" | "berserk" | "half" | "curse" | "exchange" | "slow" | "ice" => 192,
            _ => 84,
        },
    }
}

/// 三个可取中位数的值的中位数；用于属性派生等三值聚合场景。
pub fn median<T>(x: T, y: T, z: T) -> T
where
    T: Ord + Copy,
{
    if x < y {
        if y < z {
            y
        } else if x < z {
            z
        } else {
            x
        }
    } else if x < z {
        x
    } else if y < z {
        z
    } else {
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boss_append_attr_matches_artifact_tables() {
        assert_eq!(boss_append_attr("mario"), [0, 38, 31, 46, 28, 18, 15, 69]);
        assert_eq!(boss_append_attr("sonic"), [10, -6, 1000, 0, 10, -15, 6, 0]);
        assert_eq!(boss_append_attr("mosquito"), [-3, 24, 29, 729, 5, 7, 12, -35]);
        assert_eq!(boss_append_attr("yuri"), [26, 31, 46, 9, 40, 5, 32, 24]);
        assert_eq!(boss_append_attr("slime"), [6, 21, 5, 19, 38, 21, 12, 62]);
        assert_eq!(boss_append_attr("ikaruga"), [48, 28, 21, 45, 10, 19, 33, 150]);
        assert_eq!(boss_append_attr("conan"), [0, 48, -33, 20, 0, 41, 30, 22]);
        assert_eq!(boss_append_attr("aokiji"), [40, 30, 40, 10, 35, 4, 40, 96]);
        assert_eq!(boss_append_attr("lazy"), [0, 88, 10, -20, 0, 50, 0, 120]);
        assert_eq!(boss_append_attr("covid"), [10, 9, 0, 12, 0, 12, 0, 60]);
        assert_eq!(boss_append_attr("saitama"), [72, 39, 69, 76, 67, 66, 0, 84]);
        assert_eq!(boss_append_attr("testsubject"), [0; 8]);
        assert_eq!(boss_append_attr("thanos"), [39, 36, 12, 16, 0, 37, 20, 113]);
        assert_eq!(boss_append_attr("wheel"), [23, 135, 0, 19, 27, 89, 10, 39]);
        assert_eq!(boss_append_attr("unknown"), [0; 8]);
    }

    #[test]
    fn boss_default_skill_levels_match_ac_tables() {
        assert_eq!(boss_default_skill_levels("aokiji"), &[(1, 80)]);
        assert_eq!(boss_default_skill_levels("mosquito"), &[(4, 100)]);
        assert_eq!(boss_default_skill_levels("sonic"), &[(6, 48), (7, 48), (30, 48)]);
        assert_eq!(boss_default_skill_levels("yuri"), &[(25, 48), (27, 48)]);
        assert_eq!(boss_default_skill_levels("thanos"), &[(29, 80)]);
        assert_eq!(boss_default_skill_levels("wheel"), &[(30, 48)]);
        for name in [
            "mario",
            "slime",
            "ikaruga",
            "conan",
            "lazy",
            "covid",
            "saitama",
            "testsubject",
            "unknown",
        ] {
            assert!(boss_default_skill_levels(name).is_empty(), "{name} 不应有通用默认技能");
        }
    }

    #[test]
    fn boss_action_skills_match_k4_tables() {
        assert_eq!(
            boss_action_skills("aokiji"),
            &[(BossActionSkill::IceAge, 70), (BossActionSkill::Ice, 80)]
        );
        assert_eq!(boss_action_skills("ikaruga"), &[(BossActionSkill::IkarugaAttack, 48)]);
        assert_eq!(
            boss_action_skills("mario"),
            &[(BossActionSkill::MarioGet, 63), (BossActionSkill::Fire, 0)]
        );
        assert_eq!(boss_action_skills("mosquito"), &[(BossActionSkill::Absorb, 100)]);
        assert_eq!(boss_action_skills("saitama"), &[(BossActionSkill::Saitama, 0)]);
        assert_eq!(
            boss_action_skills("sonic"),
            &[(BossActionSkill::Rapid, 48), (BossActionSkill::Critical, 48)]
        );
        assert_eq!(boss_action_skills("yuri"), &[(BossActionSkill::YuriControl, 256)]);
        for name in ["conan", "covid", "lazy", "slime", "testsubject", "thanos", "wheel", "unknown"] {
            assert!(boss_action_skills(name).is_empty(), "{name} 的 k4 应为空");
        }
    }

    #[test]
    fn boss_uses_skill_attack_only_for_aokiji_and_ikaruga() {
        assert!(boss_uses_skill_attack("aokiji"));
        assert!(boss_uses_skill_attack("ikaruga"));
        for name in BOSS_NAMES.iter().copied().filter(|name| !matches!(*name, "aokiji" | "ikaruga")) {
            assert!(!boss_uses_skill_attack(name), "{name} 的 k3 不是 SklAttack");
        }
        assert!(!boss_uses_skill_attack("unknown"));
    }

    #[test]
    fn boss_immune_threshold_follows_boss_specific_lists() {
        assert_eq!(boss_immune_threshold("conan", "charm"), 240);
        assert_eq!(boss_immune_threshold("conan", "ice"), 192);
        assert_eq!(boss_immune_threshold("conan", "fire"), 84);
        assert_eq!(boss_immune_threshold("ikaruga", "piston"), 192);
        assert_eq!(boss_immune_threshold("ikaruga", "charm"), 84);
        assert_eq!(boss_immune_threshold("mario", "assassinate"), 192);
        assert_eq!(boss_immune_threshold("mario", "charm"), 84);
        assert_eq!(boss_immune_threshold("mosquito", "assassinate"), 240);
        assert_eq!(boss_immune_threshold("mosquito", "disperse"), 240);
        assert_eq!(boss_immune_threshold("mosquito", "berserk"), 192);
        assert_eq!(boss_immune_threshold("slime", "piston"), 192);
        assert_eq!(boss_immune_threshold("sonic", "piston"), 192);
        assert_eq!(boss_immune_threshold("yuri", "berserk"), 84);
        assert_eq!(boss_immune_threshold("saitama", "half"), 240);
        assert_eq!(boss_immune_threshold("covid", "charm"), 192);
        assert_eq!(boss_immune_threshold("lazy", "curse"), 192);
        assert_eq!(boss_immune_threshold("aokiji", "charm"), 192);
        assert_eq!(boss_immune_threshold("unknown", "charm"), 192);
    }
}
