use std::sync::Arc;

use crate::engine::storage::{SkillId, Storage};
use crate::engine::update::RunUpdates;
use crate::player::{PlayerStatus, PlrPtr};

#[derive(Debug, Clone, Copy)]
pub struct Skill {
    /// 是否被增强过
    pub boosted: bool,
    /// 等级
    level: u32,
    /// 类型
    skill_type: SkillType,
    /// 目标
    pub target: Option<PlrPtr>,
}

impl Skill {
    pub fn new_from_type_id(level: u32, id: u8) -> Self {
        Self {
            boosted: false,
            level,
            skill_type: SkillType::new_from_skill_type_id(id),
            target: None,
        }
    }

    pub fn set_target(&mut self, target: PlrPtr) { self.target = Some(target); }

    pub fn get_target(&self) -> Option<PlrPtr> { self.target }

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

    pub fn boost_level(&mut self, level: u32) -> bool {
        if self.boosted {
            self.level += level;
            false
        } else {
            self.level += level;
            self.boosted = true;
            true
        }
    }

    /// 获取技能等级
    pub fn level(&self) -> u32 { self.level }

    pub fn update_state(&self, status: &mut PlayerStatus) {
        match self.skill_type {
            SkillType::Accumulate { acc } => {
                status.at_boost *= acc;
            }
            SkillType::Charge => {
                status.at_boost *= 3.0;
            }
            SkillType::Iron => {
                status.attract *= 1.12;
            }
            SkillType::Hide => {
                status.attract /= 10.0;
                if self.level > 63 {
                    let boost_level = (self.level - 63) as i32;
                    status.agility += boost_level;
                    status.defense += boost_level;
                    status.resistance += boost_level;
                }
            }
            SkillType::Upgrade => {
                // 全属性 +30
                status.attack += 30;
                status.defense += 30;
                status.agility += 30;
                status.magic += 30;
                status.resistance += 30;
                // 但是这俩只加 20
                status.speed += 20;
                status.wisdom += 20;
            }
            SkillType::CharmState { charmed_group } => {
                todo!("魅惑我还不知道咋写")
            }
            SkillType::CurseState => {
                status.atk_sum *= 4;
            }
            SkillType::HasteState { faster } => {
                status.speed *= faster;
            }
            SkillType::IceState { .. } => {
                status.set_frozen(true);
            }

            _ => (),
        }
    }

    #[allow(clippy::single_match)]
    pub fn pre_step(&mut self, step: i32, updates: &mut RunUpdates, status: &mut PlayerStatus) -> i32 {
        match &mut self.skill_type {
            SkillType::IceState { frozen_step } => {
                if step > 0 {
                    if *frozen_step > 0 {
                        *frozen_step -= step as u32;
                    } else if (step + status.move_point) >= 2048 {
                        // destroy
                        let target = self.target.expect("no target");

                        return 0;
                    }
                }
            }
            _ => {}
        }
        step
    }

    pub fn destroy(&self, ) { todo!() }
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
    pub skill_store: Vec<SkillId>,
    /// 全局状态
    storage: Arc<Storage>,
    /// 更新状态的
    /// (其他人加到自己身上的)
    pub update_states: Vec<SkillId>,
    /// meta??
    pub meta: Vec<SkillId>,
    // 自己的状态
    /// step 之前
    pub pre_step: Vec<SkillId>,
    /// 动作之前
    pub pre_action: Vec<SkillId>,
    /// 动作之后
    pub post_action: Vec<SkillId>,
    /// 防御之前
    pub pre_defend: Vec<SkillId>,
    /// 防御之后
    pub post_defend: Vec<SkillId>,
    /// 伤害之后
    pub post_damage: Vec<SkillId>,
    /// 死亡之后
    pub post_death: Vec<SkillId>,
    /// 干掉目标之后
    pub post_kill: Vec<SkillId>,
}

impl SkillStore {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            skill_store: vec![],
            storage,
            update_states: vec![],
            meta: vec![],
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
        for skill_id in self.skill_store.iter() {
            let skill = self.storage.get_skill(*skill_id).expect("skill not found");
            let skill_type = &skill.skill_type;
            match skill_type {
                SkillType::Counter => {
                    self.post_damage.push(*skill_id);
                }
                SkillType::Defend => {
                    self.post_defend.push(*skill_id);
                }
                SkillType::Hide => {
                    self.post_damage.push(*skill_id);
                    self.pre_action.push(*skill_id);
                }
                SkillType::Merge => {
                    self.post_kill.push(*skill_id);
                }
                SkillType::Protect => {
                    self.post_action.push(*skill_id);
                }
                SkillType::Reflect => {
                    self.pre_defend.push(*skill_id);
                }
                SkillType::Reraise => {
                    self.post_death.push(*skill_id);
                }
                SkillType::Shield => {
                    self.pre_action.push(*skill_id);
                }
                SkillType::Upgrade => {
                    self.post_damage.push(*skill_id);
                }
                SkillType::Zombie => {
                    self.post_kill.push(*skill_id);
                }
                // TODO: BOSS 技能
                SkillType::Slime => {
                    self.post_damage.push(*skill_id);
                }
                // TODO: 武器技能
                SkillType::DeathNote => {
                    self.post_damage.push(*skill_id);
                }

                _ => (),
            }
        }
    }

    /// 最后一个技能 boost
    pub fn boost_last(&mut self) {
        for skill_id in self.skill_store.iter_mut().rev() {
            let skill = self.storage.just_get_skill_mut(*skill_id).expect("skill not found");
            if skill.boost_if_not() {
                break;
            }
        }
    }

    /// 添加技能
    pub fn add_skill(&mut self, skill: SkillId) { self.skill_store.push(skill); }
}

/// 技能类型
/// 需要和游戏中的技能类型对应
///
/// 因为不知道啥时候会加新的, 所以务必带上 `#[non_exhaustive]`
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SkillType {
    /// 火球术
    Fire,
    /// 冰冻术
    Ice { frozen_step: u32 },
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
    Accumulate { acc: f64 },

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

    // 各种状态
    /// 被魅惑
    CharmState { charmed_group: u32 },
    /// 被诅咒
    CurseState,
    /// 疾走状态
    HasteState { faster: i32 },
    /// 被冻结
    IceState { frozen_step: u32 },
    /// 被迟缓
    SlowState { step: u32 },

    // boss
    /// 懒惰状态
    LazyState,

    // TODO: BOSS 技能
    /// 史莱姆(分裂)
    Slime,

    // TODO: 武器技能
    /// 死亡笔记
    DeathNote,
    /// Rinck 的修改器 (属性修改器)
    RinickModifier,
}

impl SkillType {
    pub fn new_from_skill_type_id(id: u8) -> Self {
        match id {
            0 => Self::Fire,
            1 => Self::Ice { frozen_step: 1024 },
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
            20 => Self::Accumulate { acc: 1.7 },

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

    /// 是否是普通技能
    pub fn is_normal_skill(&self) -> bool {
        matches!(
            self,
            SkillType::Fire
                | SkillType::Ice { .. }
                | SkillType::Thunder
                | SkillType::Quake
                | SkillType::Absorb
                | SkillType::Poison
                | SkillType::Rapid
                | SkillType::Critical
                | SkillType::Plague
                | SkillType::Life
                | SkillType::Berserk
                | SkillType::Charm
                | SkillType::Haste
                | SkillType::Slow
                | SkillType::Curse
                | SkillType::Heal
                | SkillType::Revive
                | SkillType::Disperse
                | SkillType::Iron
                | SkillType::Charge
                | SkillType::Accumulate { .. }
                | SkillType::Assassinate
                | SkillType::Summon
                | SkillType::Clone
                | SkillType::Shadow
                | SkillType::Defend
                | SkillType::Protect
                | SkillType::Reflect
                | SkillType::Reraise
                | SkillType::Shield
                | SkillType::Counter
                | SkillType::Merge
                | SkillType::Zombie
                | SkillType::Upgrade
                | SkillType::Hide
        )
    }

    pub fn is_normal_state(&self) -> bool {
        matches!(
            self,
            SkillType::SlowState { .. }
                | Self::CurseState
                | Self::IceState { .. }
                | Self::CharmState { .. }
                | Self::HasteState { .. }
                | Self::LazyState
        )
    }

    /// 是否是 BOSS 技能
    pub fn is_boss_skill(&self) -> bool { matches!(self, SkillType::Slime) }

    /// 是否是武器技能
    pub fn is_weapon_skill(&self) -> bool { matches!(self, SkillType::DeathNote) }
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
