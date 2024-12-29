#[derive(Debug, Clone)]
pub struct Skill {
    /// 是否被增强过
    boosted: bool,
    /// 等级
    level: u32,
    /// 类型
    skill_type: SkillType,
}

#[derive(Debug, Clone, Copy)]
pub enum SkillType {
    /// 火球术
    Fire,
    /// 冰冻术
    Ice,
    /// 雷击术
    Thunder,
    /// 地裂术
    Quake,
    /// 吸血攻击
    Absorb,
    /// 投毒
    Poison,
    /// 连击
    Rapid,
    /// 会心一击
    Critical,
    /// 瘟疫
    Poison2,
    /// 生命之轮
    Life,
    /// 狂暴
    Berserk,
    /// 魅惑
    Charm,
    /// 加速术
    Haste,
    /// 减速术
    Slow,
    /// 诅咒
    Curse,
    /// 治愈魔法
    Heal,
    /// 苏生术
    Revive,
    /// 净化
    Disperse,
    /// 铁壁
    Iron,
    /// 蓄力
    Charge,
    /// 聚气
    Accumulate,
    /// 潜行
    Sneak,
    /// 血祭
    BloodSacrifice,
    /// 分身
    Clone,
    /// 幻术
    Illusion,
    /// 防御
    Defend,
    /// 守护
    Protect,
    /// 伤害反弹
    Reflect,
    /// 护身符
    Amulet,
    /// 护盾
    Shield,
    /// 反击
    Counter,
    /// 吞噬
    Devour,
    /// 召唤亡灵
    Summon,
    /// 垂死抗争
    Reraise,
    /// 隐匿
    Hide,
    /// 无 (35-40)
    None,
}

impl SkillType {
    pub fn new_from_skill_type_id(id: u8) -> Self {
        match id {
            0 => Self::Fire,
            1 => Self::Ice,
            2 => Self::Thunder,
            3 => Self::Quake,
            4 => Self::Absorb,
            5 => Self::Poison,
            6 => Self::Rapid,
            7 => Self::Critical,
            8 => Self::Poison2,
            9 => Self::Life,
            10 => Self::Berserk,
            11 => Self::Charm,
            12 => Self::Haste,
            13 => Self::Slow,
            14 => Self::Curse,
            15 => Self::Heal,
            16 => Self::Revive,
            17 => Self::Disperse,
            18 => Self::Iron,
            19 => Self::Charge,
            20 => Self::Accumulate,
            21 => Self::Sneak,
            22 => Self::BloodSacrifice,
            23 => Self::Clone,
            24 => Self::Illusion,
            25 => Self::Defend,
            26 => Self::Protect,
            27 => Self::Reflect,
            28 => Self::Amulet,
            29 => Self::Shield,
            30 => Self::Counter,
            31 => Self::Devour,
            32 => Self::Summon,
            33 => Self::Reraise,
            34 => Self::Hide,
            _ => Self::None,
        }
    }
}

/*
const char skillNameMap[] = {
    "火球术", "冰冻术", "雷击术", "地裂术", "吸血攻击", "投毒", "连击",
    "会心一击", "瘟疫", "生命之轮", "狂暴术", "魅惑", "加速术", "减速术",
    "诅咒", "治愈魔法", "苏生术", "净化", "铁壁", "蓄力", "聚气",
    "潜行", "血祭", "分身", "幻术", "防御", "守护", "伤害反弹",
    "护身符", "护盾", "反击", "吞噬", "召唤亡灵", "垂死抗争", "隐匿",
    "啧", "啧", "啧", "啧", "啧"};
string skillNameMap_2[35] = {
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击",
    "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速",
    "诅咒", "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气",
    "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹",
    "护符", "护盾", "反击", "吞噬", "召灵", "垂死", "隐匿"};
    */
