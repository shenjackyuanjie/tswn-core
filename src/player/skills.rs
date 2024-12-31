use crate::player::Player;

#[derive(Debug, Clone)]
pub struct Skill {
    /// 是否被增强过
    pub boosted: bool,
    /// 等级
    level: u32,
    /// 类型
    skill_type: SkillType,
}

impl Skill {
    pub fn new_from_type_id(level: u32, id: u8) -> Self {
        Self {
            boosted: false,
            level,
            skill_type: SkillType::new_from_skill_type_id(id),
        }
    }

    /// 如果没 boost, 那就 boost 一下
    /// true: boost 成功
    /// false: 已经 boost 过了
    pub fn boost_if_not(&mut self) -> bool {
        if self.boosted {
            false
        } else {
            self.boosted = true;
            self.level *= 2;
            true
        }
    }
}

/// ```dart
/// MList<PreStepEntry> presteps = new MList<PreStepEntry>();
/// MList<PreActionEntry> preactions = new MList<PreActionEntry>();
/// MList<PostActionEntry> postactions = new MList<PostActionEntry>();
/// MList<PreDefendEntry> predefends = new MList<PreDefendEntry>();
/// MList<PostDefendEntry> postdefends = new MList<PostDefendEntry>();
/// MList<PostDamageEntry> postdamages = new MList<PostDamageEntry>();
/// MList<DieEntry> dies = new MList<DieEntry>();
/// MList<KillEntry> kills = new MList<KillEntry>();
/// ```
#[derive(Debug, Clone, Default)]
pub struct SkillStore {
    /// 实际存储 skill 的地方
    pub skill_store: Vec<Skill>,
    /// step 之前
    pub pre_step: Vec<u32>,
    /// 动作之前
    pub pre_action: Vec<u32>,
    /// 动作之后
    pub post_action: Vec<u32>,
    /// 防御之前
    pub pre_defend: Vec<u32>,
    /// 防御之后
    pub post_defend: Vec<u32>,
    /// 伤害之后
    pub post_damage: Vec<u32>,
    /// 死亡之后
    pub post_death: Vec<u32>,
    /// 干掉目标之后
    pub post_kill: Vec<u32>,
}

impl SkillStore {
    pub fn new() -> Self {
        Self {
            skill_store: vec![],
            pre_step: vec![],
            pre_action: vec![],
            post_action: vec![],
            pre_defend: vec![],
            post_defend: vec![],
            post_damage: vec![],
            post_death: vec![],
            post_kill: vec![],
        }
    }

    fn clear_proc(&mut self) {
        self.pre_step.clear();
        self.pre_action.clear();
        self.post_action.clear();
        self.pre_defend.clear();
        self.post_defend.clear();
        self.post_damage.clear();
        self.post_death.clear();
        self.post_kill.clear();
    }

    pub fn update_proc(&mut self) {
        self.clear_proc();
        for (i, skill) in self.skill_store.iter().enumerate() {
            let skill_type = &skill.skill_type;
            let i = i as u32;
            match skill_type {
                SkillType::Counter => {
                    self.post_damage.push(i);
                }
                SkillType::Defend => {
                    self.post_defend.push(i);
                }
                SkillType::Hide => {
                    self.post_damage.push(i);
                    self.pre_action.push(i);
                }
                SkillType::Merge => {
                    self.post_kill.push(i);
                }
                SkillType::Protect => {
                    self.post_action.push(i);
                }
                SkillType::Reflect => {
                    self.pre_defend.push(i);
                }
                SkillType::Reraise => {
                    self.post_death.push(i);
                }
                SkillType::Shield => {
                    self.pre_action.push(i);
                }
                SkillType::Upgrade => {
                    self.post_damage.push(i);
                }
                SkillType::Zombie => {
                    self.post_kill.push(i);
                }
                // TODO: BOSS 技能
                SkillType::Slime => {
                    self.post_damage.push(i);
                }
                // TODO: 武器技能
                SkillType::DeathNote => {
                    self.post_damage.push(i);
                }

                _ => (),
            }
        }
    }

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill in self.skill_store.iter_mut().rev() {
            if skill.boost_if_not() {
                break;
            }
        }
    }

    /// 添加技能
    pub fn add_skill(&mut self, skill: Skill) { self.skill_store.push(skill); }
}

/// 技能类型
/// 需要和游戏中的技能类型对应
///
/// 因为不知道啥时候会加新的, 所以务必带上 `#[non_exhaustive]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
    Plague,
    /// 生命之轮
    Life,
    /// 狂暴术
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
    Assassinate,

    /// 血祭
    Summon,
    /// 分身
    Clone,
    /// 幻术
    Shadow,

    /// 防御
    Defend,
    /// 守护
    Protect,
    /// 伤害反弹
    Reflect,
    /// 护身符
    Reraise,
    /// 护盾
    Shield,
    /// 反击
    Counter,
    /// 吞噬
    Merge,
    /// 召唤亡灵
    Zombie,
    /// 垂死抗争
    Upgrade,
    /// 隐匿
    Hide,

    /// 无 (35-40)
    None,

    // TODO: BOSS 技能
    /// 史莱姆(分裂)
    Slime,

    // TODO: 武器技能
    /// 死亡笔记
    DeathNote,
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
            8 => Self::Plague,
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

            21 => Self::Assassinate,

            22 => Self::Summon,
            23 => Self::Clone,
            24 => Self::Shadow,

            25 => Self::Defend,
            26 => Self::Protect,
            27 => Self::Reflect,
            28 => Self::Reraise,
            29 => Self::Shield,
            30 => Self::Counter,
            31 => Self::Merge,
            32 => Self::Summon,
            33 => Self::Upgrade,
            34 => Self::Hide,

            35..40 => Self::None,
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
