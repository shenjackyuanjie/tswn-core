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

pub fn boss_append_attr(name: &str) -> [i32; 8] {
    match name {
        "covid" => [10, 9, 0, 12, 0, 12, 0, 60],
        "lazy" => [0, 88, 10, -20, 0, 50, 0, 120],
        "saitama" => [72, 39, 69, 76, 67, 66, 0, 84],
        "mario" => [20, 5, 15, 10, 20, 5, 0, 50],
        "sonic" => [10, 5, 40, 20, 10, 5, 0, 50],
        "mosquito" => [5, 5, 20, 30, 5, 5, 0, 80],
        "yuri" => [10, 10, 10, 10, 30, 30, 0, 50],
        "slime" => [5, 20, 5, 5, 5, 20, 0, 100],
        "ikaruga" => [15, 15, 10, 10, 15, 15, 0, 50],
        "conan" => [10, 10, 15, 15, 10, 10, 0, 50],
        "aokiji" => [30, 30, 10, 10, 30, 30, 0, 50],
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
