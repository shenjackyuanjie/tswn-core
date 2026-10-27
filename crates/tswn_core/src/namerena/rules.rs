//! 名竞内置角色与数值规则。

pub const BOSS_NAMES: [&str; 12] = [
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
];

pub const BOOST_NAMES: [&str; 3] = ["云剑狄卡敢", "云剑穸跄祇", "田一人"];

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
        _ => name,
    }
}

/// 与 0.5.5 中各 Boss 的 `gan()` 返回值一一对应。
pub fn boss_append_attr(name: &str) -> [i32; 8] {
    match name {
        "covid" => [10, 9, 0, 12, 0, 12, 0, 60],
        "lazy" => [0, 88, 10, -20, 0, 50, 0, 120],
        "saitama" => [72, 39, 69, 76, 67, 66, 0, 84],
        "mario" => [0, 38, 31, 46, 28, 18, 15, 69],
        "sonic" => [10, -6, 1000, 0, 10, -15, 6, 0],
        "mosquito" => [-3, 24, 29, 729, 5, 7, 12, -35],
        "yuri" => [26, 31, 46, 9, 40, 5, 32, 24],
        "slime" => [6, 21, 5, 19, 38, 21, 12, 62],
        "ikaruga" => [48, 28, 21, 45, 10, 19, 16, 150],
        "conan" => [0, 48, -16, 20, 0, 41, 30, 22],
        "aokiji" => [40, 30, 40, 10, 35, 4, 40, 96],
        "testsubject" => [0; 8],
        _ => [0; 8],
    }
}

pub fn boost_value(name: &str) -> u32 {
    match name {
        "云剑狄卡敢" => 25,
        "云剑穸跄祇" => 35,
        "田一人" => 18,
        _ => 0,
    }
}

pub fn boss_action_prob_count(name: &str) -> usize {
    match name {
        "covid" | "lazy" | "testsubject" => 0,
        _ => 1,
    }
}

/// 阈值来自 0.5.5 中各 Boss 覆盖的 `gaS()`（240）与 `gaG()`（192），未列出者 84。
/// `piston` 是毒状态在 0.5.5 中的键名。
pub fn boss_immune_threshold(name: &str, key: &str) -> i32 {
    match name {
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
            "assassinate" | "half" | "exchange" | "piston" | "slow" | "ice" => 192,
            _ => 84,
        },
        "mario" => match key {
            "assassinate" => 192,
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
        assert_eq!(boss_append_attr("ikaruga"), [48, 28, 21, 45, 10, 19, 16, 150]);
        assert_eq!(boss_append_attr("conan"), [0, 48, -16, 20, 0, 41, 30, 22]);
        assert_eq!(boss_append_attr("aokiji"), [40, 30, 40, 10, 35, 4, 40, 96]);
        assert_eq!(boss_append_attr("lazy"), [0, 88, 10, -20, 0, 50, 0, 120]);
        assert_eq!(boss_append_attr("covid"), [10, 9, 0, 12, 0, 12, 0, 60]);
        assert_eq!(boss_append_attr("saitama"), [72, 39, 69, 76, 67, 66, 0, 84]);
        assert_eq!(boss_append_attr("testsubject"), [0; 8]);
        assert_eq!(boss_append_attr("unknown"), [0; 8]);
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
