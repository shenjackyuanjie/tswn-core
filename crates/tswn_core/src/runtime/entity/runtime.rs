use super::*;

#[inline(always)]
fn order_i32(left: &mut i32, right: &mut i32) {
    if *left > *right {
        std::mem::swap(left, right);
    }
}

/// 返回五个整数的中值；七项比较网络只整理会影响中间位置的配对。
#[inline(always)]
fn median_i32_5(mut a: i32, mut b: i32, mut c: i32, mut d: i32, mut e: i32) -> i32 {
    order_i32(&mut a, &mut b);
    order_i32(&mut d, &mut e);
    order_i32(&mut a, &mut d);
    order_i32(&mut b, &mut e);
    order_i32(&mut b, &mut c);
    order_i32(&mut c, &mut d);
    order_i32(&mut b, &mut c);
    c
}

/// 返回三个整数的中值；固定网络避免进入通用切片排序。
#[inline(always)]
fn median_i32_3(mut a: i32, mut b: i32, mut c: i32) -> i32 {
    order_i32(&mut a, &mut b);
    order_i32(&mut b, &mut c);
    order_i32(&mut a, &mut b);
    b
}

/// 行动条状态：当前行动点，达到阈值即可行动。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MoveState {
    pub speed_points: i32,
}

/// 个体策略覆盖；未设置的字段沿用注册表中的玩家类型策略。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerPolicyOverrides {
    pub owner_resolution: Option<OwnerResolutionPolicy>,
    pub damage_share: Option<DamageSharePolicy>,
    pub merge: Option<MergePolicy>,
    pub inherit_owner_def_res: Option<bool>,
}

impl PlayerPolicyOverrides {
    /// 把覆盖项应用到基础策略并返回结果。
    pub fn apply_to(self, mut policies: PlayerKindPolicies) -> PlayerKindPolicies {
        if let Some(owner_resolution) = self.owner_resolution {
            policies.owner_resolution = owner_resolution;
        }
        if let Some(damage_share) = self.damage_share {
            policies.damage_share = damage_share;
        }
        if let Some(merge) = self.merge {
            policies.merge = merge;
        }
        if let Some(inherit_owner_def_res) = self.inherit_owner_def_res {
            policies.inherit_owner_def_res = inherit_owner_def_res;
        }
        policies
    }

    /// 设置伤害共享覆盖。
    pub fn with_damage_share(mut self, damage_share: DamageSharePolicy) -> Self {
        self.damage_share = Some(damage_share);
        self
    }

    /// 设置是否继承 owner 防御与抗性的覆盖。
    pub fn with_inherit_owner_def_res(mut self, inherit_owner_def_res: bool) -> Self {
        self.inherit_owner_def_res = Some(inherit_owner_def_res);
        self
    }
}

/// 保护链接：保护者实体与保护技能等级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProtectLinkRuntime {
    pub owner: EntityIdx,
    pub level: u32,
}

/// 隐匿运行时快照：等级与当时的吸引力、敏捷、防御、抗性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HideRuntime {
    pub level: u32,
    pub attract_bits: u64,
    pub agility: i32,
    pub defense: i32,
    pub resistance: i32,
}

/// 待发暗杀：固定 lane、目标与受伤是否打断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssassinateRuntime {
    pub fixed_lane: usize,
    pub target: EntityIdx,
    pub break_on_damage: bool,
}

/// 反击运行时：待反击标记、上次目标与手动决策成本。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CounterRuntime {
    /// 手动决策时展示的成本，延迟到 update-end 执行时使用同一个值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magic_cost: Option<u32>,
    pub pending: bool,
    pub last_target: Option<EntityIdx>,
    pub last_updates_id: Option<u64>,
}

/// 0.5.5 `SklS11` 的运行时字段：技能等级会被砍手分支改写，耐久从 3 开始递减。
/// 剁手刀（SklS11）运行时：当前技能等级与剩余耐久。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WeaponS11Runtime {
    pub level: u32,
    pub durability: i32,
}

/// 尸体类型标记：无、融合、亡灵或史莱姆分裂残留。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuntimeCorpseKind {
    #[default]
    None,
    Merge,
    Zombie,
    Slime,
}

/// 实体的当前战斗数值与机制运行时字段，由模板经状态处理得到。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
    pub attack: i32,
    pub magic: i32,
    pub magic_point: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_bits: u64,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
    pub kind: PlayerKindId,
    pub owner: EntityIdx,
    pub root_owner: EntityIdx,
    pub team: usize,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
    pub move_state: MoveState,
    pub charge: ChargeRuntime,
    pub accumulate: AccumulateRuntime,
    pub shield: i32,
    pub protect_to: Option<EntityIdx>,
    pub protect_from: Vec<ProtectLinkRuntime>,
    pub protect_pre_defend_skill_count: Option<usize>,
    pub upgrade_active: bool,
    pub hide: Option<HideRuntime>,
    pub assassinate: Option<AssassinateRuntime>,
    pub counter: CounterRuntime,
    /// 玄月奖杯的受击反击登记；与普通反击分开保存。
    pub counter_xuan_yue: CounterRuntime,
    /// 剁手刀的运行时技能等级与耐久。
    pub weapon_s11: Option<WeaponS11Runtime>,
    /// 死亡笔记当前写下的目标。
    pub death_note_mark: Option<EntityIdx>,
    /// 马里奥 Boss 的复活次数；0.5.5 存在技能实例上，不随离散状态清除。
    pub mario_reraise_lives: Option<u32>,
    /// 属性修改器身份的复活次数；0.5.5 存在隐身型 `SklMarioReraise` 子类实例上。
    pub rinick_reraise_lives: Option<u32>,
    /// 柯南 Boss 的推理计数；0.5.5 保存在 `SklConan` 实例上，初始为 -1。
    pub conan_thinking: Option<i32>,
    /// 史莱姆 Boss 的分裂代数；0 为原始 Boss，1 为第一代子体，2 起不再分裂。
    pub slime_split_generation: Option<u32>,
    /// 科研 Boss、助手角色和临时嘲讽覆盖随克隆与存档完整保存。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squall: Option<Box<SquallRuntime>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squall_helper: Option<(EntityIdx, u8, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squall_taunt: Option<(EntityIdx, u64)>,
    /// smdd 的做局层数与技能冷却。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smdd: Option<Box<SmddRuntime>>,
    /// 被子衍生物的行动计数与睡眠、钻被、得意等临时标记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quilt_minion: Option<Box<QuiltMinionState>>,
    /// 独立的逮捕效果保存来源、每跳伤害和剩余跳数。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quilt_arrests: Vec<(EntityIdx, i32, u8)>,
    pub corpse: RuntimeCorpseKind,
}

impl PlayerRuntime {
    /// 从模板构造初始运行态；Boss 与属性修改器身份会初始化专属字段。
    pub fn from_template(
        template: &PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        root_owner: EntityIdx,
    ) -> Self {
        let (flags, policies) = registry
            .player_kind(template.kind)
            .map_or((PlayerKindFlags::NONE, PlayerKindPolicies::default()), |kind| {
                (kind.flags, template.policy_overrides.apply_to(kind.policies))
            });
        let weapon_s11 = template.skills.skills().iter().copied().enumerate().find_map(|(fixed_lane, skill_id)| {
            (registry.skill(skill_id)?.export_name == crate::runtime::DEFAULT_CORE_WEAPON_S11_SKILL_EXPORT)
                .then(|| template.skills.level_at(fixed_lane))
                .flatten()
                .map(|level| WeaponS11Runtime { level, durability: 3 })
        });
        let mario_reraise_lives = (flags.contains(PlayerKindFlags::BOSS) && template.name == "mario").then_some(3);
        let conan_thinking = (flags.contains(PlayerKindFlags::BOSS) && template.name == "conan").then_some(-1);
        let slime_split_generation = (flags.contains(PlayerKindFlags::BOSS) && template.name == "slime").then_some(0);
        Self {
            hp: template.max_hp,
            alive: true,
            attack: template.attack,
            magic: template.magic,
            magic_point: template.magic_point,
            wisdom: template.wisdom,
            speed: template.speed,
            defense: template.defense,
            resistance: template.resistance,
            agility: template.agility,
            at_boost_bits: template.at_boost_bits,
            at_boost_millionths: template.at_boost_millionths,
            attr_sum: template.attr_sum,
            atk_sum: template.atk_sum,
            attract_bits: template.attract_bits,
            kind: template.kind,
            owner,
            root_owner,
            team: template.team,
            flags,
            policies,
            move_state: template.move_state,
            charge: ChargeRuntime::from_permanent(template.permanent_charge),
            accumulate: AccumulateRuntime::default(),
            shield: 0,
            protect_to: None,
            protect_from: Vec::new(),
            protect_pre_defend_skill_count: None,
            upgrade_active: false,
            hide: None,
            assassinate: None,
            counter: CounterRuntime::default(),
            counter_xuan_yue: CounterRuntime::default(),
            weapon_s11,
            death_note_mark: None,
            mario_reraise_lives,
            rinick_reraise_lives: template.rinick.as_ref().is_some_and(|data| data.revive_attrs.is_some()).then_some(2),
            conan_thinking,
            slime_split_generation,
            squall: (flags.contains(PlayerKindFlags::BOSS) && template.name == "Squall")
                .then(|| Box::new(SquallRuntime::default())),
            squall_helper: None,
            squall_taunt: None,
            smdd: (flags.contains(PlayerKindFlags::BOSS) && template.name == "smdd").then(|| Box::new(SmddRuntime::default())),
            quilt_minion: None,
            quilt_arrests: Vec::new(),
            corpse: RuntimeCorpseKind::None,
        }
    }

    /// 从标准数字 score profile 原地复位运行态，并保留保护链向量的容量。
    pub(crate) fn reset_score_profile_from_template(
        &mut self,
        template: &PlayerTemplate,
        owner: EntityIdx,
        hp: i32,
        alive: bool,
    ) {
        debug_assert_eq!(template.kind, PlayerTemplate::DEFAULT_KIND);
        self.hp = hp;
        self.alive = alive;
        self.attack = template.attack;
        self.magic = template.magic;
        self.magic_point = template.magic_point;
        self.wisdom = template.wisdom;
        self.speed = template.speed;
        self.defense = template.defense;
        self.resistance = template.resistance;
        self.agility = template.agility;
        self.at_boost_bits = template.at_boost_bits;
        self.at_boost_millionths = template.at_boost_millionths;
        self.attr_sum = template.attr_sum;
        self.atk_sum = template.atk_sum;
        self.attract_bits = template.attract_bits;
        self.kind = template.kind;
        self.owner = owner;
        self.root_owner = owner;
        self.team = template.team;
        self.flags = PlayerKindFlags::NONE;
        self.policies = PlayerKindPolicies::default();
        self.move_state = template.move_state;
        self.charge = ChargeRuntime::from_permanent(template.permanent_charge);
        self.accumulate = AccumulateRuntime::default();
        self.shield = 0;
        self.protect_to = None;
        self.protect_from.clear();
        self.protect_pre_defend_skill_count = None;
        self.upgrade_active = false;
        self.hide = None;
        self.assassinate = None;
        self.counter = CounterRuntime::default();
        self.counter_xuan_yue = CounterRuntime::default();
        self.weapon_s11 = None;
        self.death_note_mark = None;
        self.mario_reraise_lives = None;
        self.rinick_reraise_lives = None;
        self.conan_thinking = None;
        self.slime_split_generation = None;
        self.quilt_minion = None;
        self.quilt_arrests.clear();
        self.squall = None;
        self.squall_helper = None;
        self.squall_taunt = None;
        self.corpse = RuntimeCorpseKind::None;
    }

    /// 从场前模板原地恢复运行态，并保留保护链向量已经申请的容量。
    pub(crate) fn reset_battle_state_from(&mut self, prepared: &Self) {
        self.hp = prepared.hp;
        self.alive = prepared.alive;
        self.attack = prepared.attack;
        self.magic = prepared.magic;
        self.magic_point = prepared.magic_point;
        self.wisdom = prepared.wisdom;
        self.speed = prepared.speed;
        self.defense = prepared.defense;
        self.resistance = prepared.resistance;
        self.agility = prepared.agility;
        self.at_boost_bits = prepared.at_boost_bits;
        self.at_boost_millionths = prepared.at_boost_millionths;
        self.attr_sum = prepared.attr_sum;
        self.atk_sum = prepared.atk_sum;
        self.attract_bits = prepared.attract_bits;
        self.kind = prepared.kind;
        self.owner = prepared.owner;
        self.root_owner = prepared.root_owner;
        self.team = prepared.team;
        self.flags = prepared.flags;
        self.policies = prepared.policies;
        self.move_state = prepared.move_state;
        self.charge = prepared.charge;
        self.accumulate = prepared.accumulate;
        self.shield = prepared.shield;
        self.protect_to = prepared.protect_to;
        self.protect_from.clear();
        self.protect_from.extend_from_slice(&prepared.protect_from);
        self.protect_pre_defend_skill_count = prepared.protect_pre_defend_skill_count;
        self.upgrade_active = prepared.upgrade_active;
        self.hide = prepared.hide;
        self.assassinate = prepared.assassinate;
        self.counter.clone_from(&prepared.counter);
        self.counter_xuan_yue.clone_from(&prepared.counter_xuan_yue);
        self.weapon_s11 = prepared.weapon_s11;
        self.death_note_mark = prepared.death_note_mark;
        self.mario_reraise_lives = prepared.mario_reraise_lives;
        self.rinick_reraise_lives = prepared.rinick_reraise_lives;
        self.conan_thinking = prepared.conan_thinking;
        self.slime_split_generation = prepared.slime_split_generation;
        self.quilt_minion.clone_from(&prepared.quilt_minion);
        self.quilt_arrests.clone_from(&prepared.quilt_arrests);
        self.squall.clone_from(&prepared.squall);
        self.squall_helper = prepared.squall_helper;
        self.squall_taunt = prepared.squall_taunt;
        self.corpse = prepared.corpse;
    }

    /// 当前攻击倍率。
    pub fn at_boost(&self) -> f64 { f64::from_bits(self.at_boost_bits) }

    /// 当前吸引力。
    pub fn attract(&self) -> f64 { f64::from_bits(self.squall_taunt.map_or(self.attract_bits, |(_, bits)| bits)) }

    /// 是否为使魔。
    pub fn is_minion(&self) -> bool { self.flags.contains(PlayerKindFlags::MINION) }

    /// 是否为参与战斗结算的使魔。
    pub fn is_combat_minion(&self) -> bool { self.flags.contains(PlayerKindFlags::COMBAT_MINION) }

    /// 是否存活且有体力。
    pub fn active(&self) -> bool { self.alive && self.hp > 0 }

    /// 尝试支付 MP：不足或未激活返回 false，成功则扣除。
    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        let require_mp = randomer.r3x3() as i32;
        // 能力卡减耗只作用于当前技能的作用域，不改变掷骰消费。
        #[cfg(feature = "cards")]
        let require_mp = (require_mp - crate::runtime::forced_action::mp_reduction(self.root_owner.0)).max(0);
        if self.magic_point < require_mp {
            return false;
        }
        self.magic_point -= require_mp;
        true
    }

    /// 取攻击威力（物理或魔法），按中值网络抽样并应用攻击倍率。
    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.magic } else { self.attack };
        let a = median_i32_5(
            randomer.r127() as i32,
            randomer.r127() as i32,
            randomer.r127() as i32,
            atk + 64,
            atk,
        ) as f64;
        let b = median_i32_3(randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64) as f64;
        // 得意层各贡献 15% 攻击威力，独立于聚气。
        let pride = self.quilt_minion.as_ref().map_or(0, |s| s.pride.len());
        let at = a * b * self.at_boost() * (1.0 + 0.15 * pride as f64);
        // 能力卡在攻击力之后独立乘算，不额外消耗随机流。
        #[cfg(feature = "cards")]
        let at = at * crate::runtime::forced_action::attack_multiplier(self.root_owner.0);
        at
    }

    /// 魔法防御：抗性加 64。
    pub fn magic_defense(&self) -> i32 { self.resistance + 64 }

    /// 魔法命中：魔法加敏捷。
    pub fn magic_accuracy(&self) -> i32 { self.magic + self.agility }

    /// 魔法闪避：抗性加敏捷。
    pub fn magic_dodge(&self) -> i32 { self.resistance + self.agility }

    /// 闪避判定：按命中差计算概率并消费一个随机字节。
    pub fn dodge(accuracy: i32, dodge_value: i32, randomer: &mut RC4) -> bool {
        let chance = {
            let temp = 24 + dodge_value - accuracy;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= chance
    }
}

#[cfg(test)]
mod median_tests {
    use super::{median_i32_3, median_i32_5};

    #[test]
    fn fixed_median_networks_match_sort_with_duplicates() {
        for a in -2..=2 {
            for b in -2..=2 {
                for c in -2..=2 {
                    let mut three = [a, b, c];
                    three.sort_unstable();
                    assert_eq!(median_i32_3(a, b, c), three[1]);

                    for d in -2..=2 {
                        for e in -2..=2 {
                            let mut five = [a, b, c, d, e];
                            five.sort_unstable();
                            assert_eq!(median_i32_5(a, b, c, d, e), five[2]);
                        }
                    }
                }
            }
        }
    }
}

/// 蓄力运行时：激活标记、剩余步数与桂月常驻标记。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChargeRuntime {
    pub active: bool,
    pub post_action_active: bool,
    pub step: i32,
    /// 桂月奖杯注入的常驻 `charge` 标记：只参与状态存在性判定，不带攻击倍率且永不过期。
    pub permanent: bool,
}

impl ChargeRuntime {
    pub(crate) fn from_permanent(permanent: bool) -> Self {
        Self {
            active: permanent,
            post_action_active: false,
            step: 0,
            permanent,
        }
    }
}

/// 聚气运行时：激活标记与聚气/蓄力叠加倍率。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccumulateRuntime {
    pub active: bool,
    pub acc_bits: u64,
    pub charge_bonus_bits: u64,
}

impl Default for AccumulateRuntime {
    /// 默认聚气运行态：未激活、聚气倍率 1.7、无蓄力叠加。
    fn default() -> Self {
        Self {
            active: false,
            acc_bits: 1.7000000476837158_f64.to_bits(),
            charge_bonus_bits: 0.0_f64.to_bits(),
        }
    }
}

impl AccumulateRuntime {
    /// 当前聚气倍率。
    pub fn acc(self) -> f64 { f64::from_bits(self.acc_bits) }

    /// 蓄力叠加到聚气上的额外倍率。
    pub fn charge_bonus(self) -> f64 { f64::from_bits(self.charge_bonus_bits) }

    /// 设置聚气倍率，以 f64 位模式存储。
    fn set_acc(&mut self, acc: f64) { self.acc_bits = acc.to_bits(); }

    /// 设置蓄力叠加到聚气上的额外倍率，以 f64 位模式存储。
    fn set_charge_bonus(&mut self, charge_bonus: f64) { self.charge_bonus_bits = charge_bonus.to_bits(); }
}

/// 一个实体的完整记录：模板、运行态、状态存储与实体槽。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRecord {
    pub template: PlayerTemplate,
    pub runtime: PlayerRuntime,
    pub states: StateStore,
    pub slots: EntitySlotStorage,
}

impl EntityRecord {
    #[inline]
    /// 是否可行动：存活有体力且未被冻结。
    pub fn is_active(&self) -> bool { self.runtime.active() && !self.states.is_frozen() }

    #[inline]
    /// 含状态修正与垂死增益的有效速度。
    pub fn effective_speed(&self) -> i32 {
        let mut speed = self.states.effective_speed(self.template.speed);
        // 得意的速度增益叠加，但不会创建疾走状态。
        if let Some(s) = &self.runtime.quilt_minion {
            speed += self.template.speed * s.pride.len() as i32 / 10;
        }
        if self.runtime.upgrade_active {
            // 增益量独立乘算，保留原始技能等级。
            #[cfg(feature = "cards")]
            let power = crate::runtime::forced_action::ability_value(self.template.team, "core.skill.upgrade").1;
            #[cfg(not(feature = "cards"))]
            let power: f64 = 1.0;
            speed += (20.0 * power).round() as i32;
        }
        speed
    }

    /// 应用派生属性到模板并同步运行态。
    pub fn apply_derived_stats(&mut self, stats: CloneDerivedStats) {
        self.template.apply_derived_stats(stats);
        self.apply_runtime_stats_from_template();
    }

    /// 从模板重算运行态，应用隐匿、垂死、聚气与状态修正。
    pub fn refresh_runtime_stats_from_template(&mut self) {
        if self.template.rinick.is_some()
            && let Some(build) = &mut self.template.clone_build
        {
            let name_factor = build.name_factor();
            let stats = CloneBuildData::derive_rinick_floor(&mut build.attrs, name_factor);
            self.template.apply_derived_stats(stats);
        }
        self.apply_runtime_stats_from_template();
    }

    /// 把模板八围与派生数值同步到运行态：套用垂死增益、重算 at_boost 与仇恨修正，并按隐匿等级折算属性。
    fn apply_runtime_stats_from_template(&mut self) {
        #[cfg(not(feature = "no_debug"))]
        let probe_pending_haste = crate::debug::probe_refresh()
            .filter(|needle| self.template.name.contains(needle))
            .and_then(|_| {
                self.states
                    .entries()
                    .iter()
                    .find_map(StateEntry::haste_runtime_value)
                    .filter(|(faster, effective_faster, _)| faster != effective_faster)
            });
        self.states.refresh_effective_haste_faster();
        #[cfg(not(feature = "no_debug"))]
        if let Some(haste) = probe_pending_haste {
            eprintln!(
                "[refresh_probe:runtime] name={} pending={haste:?}\n{}",
                self.template.name,
                std::backtrace::Backtrace::force_capture(),
            );
        }
        let hide_level = self.runtime.hide.map(|hide| hide.level);
        self.runtime.attack = self.template.attack;
        self.runtime.magic = self.template.magic;
        self.runtime.wisdom = self.template.wisdom;
        self.runtime.speed = self.template.speed;
        self.runtime.defense = self.template.defense;
        self.runtime.resistance = self.template.resistance;
        self.runtime.agility = self.template.agility;
        self.runtime.attr_sum = self.template.attr_sum;
        self.runtime.atk_sum = self.states.effective_atk_sum(self.template.atk_sum);
        self.runtime.attract_bits = self.template.attract_bits;
        self.runtime.attract_bits = self.states.effective_attract(f64::from_bits(self.runtime.attract_bits)).to_bits();
        if self.runtime.upgrade_active {
            // 增益量独立乘算，保留原始技能等级。
            #[cfg(feature = "cards")]
            let power = crate::runtime::forced_action::ability_value(self.template.team, "core.skill.upgrade").1;
            #[cfg(not(feature = "cards"))]
            let power: f64 = 1.0;
            self.runtime.attack += (30.0 * power).round() as i32;
            self.runtime.defense += (30.0 * power).round() as i32;
            self.runtime.agility += (30.0 * power).round() as i32;
            self.runtime.magic += (30.0 * power).round() as i32;
            self.runtime.resistance += (30.0 * power).round() as i32;
            self.runtime.speed += (20.0 * power).round() as i32;
            self.runtime.wisdom += (20.0 * power).round() as i32;
        }
        self.refresh_runtime_at_boost();
        if let Some(level) = hide_level {
            self.runtime.hide = Some(HideRuntime {
                level,
                attract_bits: self.runtime.attract_bits,
                agility: self.runtime.agility,
                defense: self.runtime.defense,
                resistance: self.runtime.resistance,
            });
            #[cfg(feature = "cards")]
            let hide_power = crate::runtime::forced_action::ability_value(self.template.team, "core.skill.hide").1;
            #[cfg(not(feature = "cards"))]
            let hide_power: f64 = 1.0;
            self.runtime.attract_bits = (self.runtime.attract() / (10.0 * hide_power)).to_bits();
            if level > 63 {
                let boost = ((level - 63) as f64 * hide_power).round() as i32;
                self.runtime.agility += boost;
                self.runtime.defense += boost;
                self.runtime.resistance += boost;
            }
        }
    }

    /// 激活垂死增益并推进 400 行动点；已激活时返回 false。
    pub fn activate_upgrade_runtime(&mut self) -> bool {
        if self.runtime.upgrade_active {
            return false;
        }
        self.states.register_compressed_legacy_state(CompressedLegacyState::Upgrade);
        self.runtime.upgrade_active = true;
        self.runtime.move_state.speed_points += 400;
        self.refresh_runtime_stats_from_template();
        true
    }

    /// 撤销垂死增益；未激活时返回 false。
    pub fn clear_upgrade_runtime(&mut self) -> bool {
        if !self.runtime.upgrade_active {
            return false;
        }
        self.runtime.upgrade_active = false;
        self.states.clear_compressed_legacy_state(CompressedLegacyState::Upgrade);
        self.refresh_runtime_stats_from_template();
        true
    }

    /// 激活蓄力并登记行动后回调，同时重算属性。
    pub fn activate_charge_runtime(&mut self) {
        if !self.runtime.charge.post_action_active {
            self.states.register_charge_post_action();
        }
        self.runtime.charge.step += 2;
        self.runtime.charge.active = true;
        self.runtime.charge.post_action_active = true;
        // legacy 的 Charge.act 会立即调用 update_states；除了启用蓄力倍率，
        // 也会让先前叠加、等待完整属性重算的疾走倍率在本次行动后生效。
        self.refresh_runtime_stats_from_template();
    }

    /// 行动后递减蓄力步数，归零时撤销并重算属性。
    pub fn tick_charge_post_action(&mut self) -> bool {
        if !self.runtime.charge.post_action_active || self.runtime.charge.permanent {
            return false;
        }

        self.runtime.charge.step -= 1;
        if self.runtime.charge.step <= 0 {
            self.runtime.charge.active = false;
            self.runtime.charge.post_action_active = false;
            // legacy 的 Charge.post_action 会调用 update_states；除了撤销攻击倍率，
            // 还必须让等待下一次完整重算的疾走倍率正式生效。
            self.refresh_runtime_stats_from_template();
        }
        true
    }

    /// 立即清除蓄力；未激活或为常驻标记时返回 false。
    pub fn clear_charge_runtime(&mut self) -> bool {
        if !self.runtime.charge.active || self.runtime.charge.permanent {
            return false;
        }

        self.runtime.charge.active = false;
        self.runtime.charge.post_action_active = false;
        // legacy 的 Charge.clear_positive_runtime 同样会调用 update_states。
        self.refresh_runtime_stats_from_template();
        true
    }

    /// 激活聚气；蓄力中额外获得倍率与 500 行动点。
    pub fn activate_accumulate_runtime(&mut self) -> bool {
        if self.runtime.accumulate.active {
            return false;
        }

        let charge_active = self.runtime.charge.active;
        self.runtime.accumulate.active = true;
        self.runtime.accumulate.set_charge_bonus(if charge_active { 1.0 } else { 0.0 });
        if charge_active {
            self.runtime.move_state.speed_points += 500;
        }
        // legacy 的 Accumulate.act 会调用 update_states；除了更新聚气攻击倍率，
        // 也必须让先前叠加、等待完整属性重算的疾走倍率在本次行动后生效。
        self.refresh_runtime_stats_from_template();
        self.runtime.move_state.speed_points += 400;
        true
    }

    /// 清除聚气并复位倍率；未激活时返回 false。
    pub fn clear_accumulate_runtime(&mut self) -> bool {
        if !self.runtime.accumulate.active {
            return false;
        }

        self.runtime.accumulate.active = false;
        self.runtime.accumulate.set_acc(1.600000023841858);
        self.runtime.accumulate.set_charge_bonus(0.0);
        // legacy 的 Accumulate.clear_positive_runtime 同样会调用 update_states。
        self.refresh_runtime_stats_from_template();
        true
    }

    /// 清除正面运行时状态（聚气、蓄力、垂死），返回按优先级排序的消息。
    pub fn clear_positive_runtime_messages(&mut self) -> Vec<(i32, &'static str)> {
        let emit_state_cancel = self.runtime.alive && self.runtime.hp > 0;
        let mut messages = Vec::new();
        if self.clear_accumulate_runtime() {
            messages.push((100, "[1]的[聚气]被打消了"));
        }
        if self.clear_charge_runtime() {
            messages.push((200, "[1]的[蓄力]被中止了"));
        }
        if self.clear_upgrade_runtime() && emit_state_cancel {
            messages.push((500, "[1]的[垂死]属性被打消"));
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    /// 清除运行时与状态两类正面效果，合并后按优先级排序。
    pub fn clear_positive_messages(&mut self) -> Vec<(i32, &'static str)> {
        let mut messages = self.clear_positive_runtime_messages();
        messages.extend(self.clear_positive_state_messages());
        if let Some(s) = &mut self.runtime.quilt_minion {
            if !s.pride.is_empty() {
                s.pride.clear();
                messages.push((0, "[1]从[得意]中解除"));
            }
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    /// 清除正面状态（含护盾），返回按优先级排序的消息。
    pub fn clear_positive_state_messages(&mut self) -> Vec<(i32, &'static str)> {
        let emit_state_cancel = self.runtime.alive && self.runtime.hp > 0;
        self.runtime.shield = 0;
        self.states.clear_compressed_legacy_state(CompressedLegacyState::Shield);
        let generation = self.states.generation();
        let messages = self.states.clear_positive_states_with_ordered_messages(emit_state_cancel);
        if self.template.rinick.is_none() || generation != self.states.generation() {
            self.refresh_runtime_stats_from_template();
        }
        messages
    }

    /// 恢复固定 roster 中实体的可变战斗状态，同时保留模板冷数据的既有分配。
    pub fn reset_battle_state_from(&mut self, prepared: &Self) {
        self.template.reset_battle_fields_from(&prepared.template);
        self.runtime.reset_battle_state_from(&prepared.runtime);
        self.states.reset_battle_state_from(&prepared.states);
        self.slots.reset_battle_state_from(&prepared.slots);
    }

    /// 重算运行态攻击倍率：非永久蓄力乘 3、聚气乘累计倍率、马里奥蘑菇乘 1.5，并同步整数百万分比。
    fn refresh_runtime_at_boost(&mut self) {
        let mut at_boost = f64::from_bits(self.template.at_boost_bits);
        // 攻击倍率来自 SklCharge.ar；桂月的常驻标记没有 ar，只满足存在性判定。
        if self.runtime.charge.active && !self.runtime.charge.permanent {
            at_boost *= 3.0;
        }
        if self.runtime.accumulate.active {
            at_boost *= self.runtime.accumulate.acc() + self.runtime.accumulate.charge_bonus();
        }
        // 马里奥吃到蘑菇后 SklMarioGet 让攻击力乘 1.5。
        if self
            .states
            .entry(crate::runtime::profile::PLAIN_MARIO_BOSS_STATE_KEY)
            .and_then(StateEntry::mario_boss_value)
            .is_some_and(|(grow, _)| grow > 0)
        {
            at_boost *= 1.5;
        }
        self.runtime.at_boost_bits = at_boost.to_bits();
        self.runtime.at_boost_millionths = at_boost_to_millionths(at_boost);
    }
}

/// 实体容器：按 [`EntityIdx`] 寻址，可为生成预留空槽。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityArena {
    entities: Vec<Option<EntityRecord>>,
}

impl EntityArena {
    /// 用默认注册表从模板构造 arena。
    pub fn from_templates(players: Vec<PlayerTemplate>) -> Self {
        Self::from_templates_with_registry(players, &ExtensionRegistry::default())
    }

    /// 用注册表从模板构造 arena，并准备技能 hook 缓存。
    pub fn from_templates_with_registry(players: Vec<PlayerTemplate>, registry: &ExtensionRegistry) -> Self {
        let entities = players
            .into_iter()
            .enumerate()
            .map(|(idx, mut template)| {
                if template.skills.skills().iter().all(|skill_id| registry.skill(*skill_id).is_some()) {
                    template.skills.prepare_hook_cache(registry);
                }
                let owner = EntityIdx(idx as u32);
                let runtime = PlayerRuntime::from_template(&template, registry, owner, owner);
                let mut states = StateStore::default();
                if runtime.is_minion() {
                    states.register_compressed_legacy_state(CompressedLegacyState::Minion);
                }
                Some(EntityRecord {
                    template,
                    runtime,
                    states,
                    slots: EntitySlotStorage::from_registry(registry),
                })
            })
            .collect();
        Self { entities }
    }

    /// 槽范围长度，包含为空槽预留的位置。
    pub fn len(&self) -> usize { self.entities.len() }

    /// 槽范围是否为空。
    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    /// 封存 prepared runner 的技能基线，后续未修改的技能表不再逐局深拷贝。
    pub(crate) fn mark_battle_baseline(&mut self) {
        for entity in self.entities.iter_mut().flatten() {
            entity.template.skills.mark_battle_baseline();
            entity.slots.mark_battle_baseline();
        }
    }

    /// 从准备好的 arena 恢复下一局，并丢弃上一局生成的召唤物、分身和实体空洞。
    pub fn reset_battle_state_from(&mut self, prepared: &Self) {
        self.entities.truncate(prepared.entities.len());
        if self.entities.len() < prepared.entities.len() {
            self.entities.resize_with(prepared.entities.len(), || None);
        }
        for (current, prepared) in self.entities.iter_mut().zip(&prepared.entities) {
            match (current.as_mut(), prepared.as_ref()) {
                (Some(current), Some(prepared)) => current.reset_battle_state_from(prepared),
                _ => current.clone_from(prepared),
            }
        }
    }

    /// 恢复 score 对局：固定 target 完整复位，尾部动态 profile 留给随后的构造应用。
    ///
    /// 动态 profile 的模板、runtime、状态和槽位随后会在同一应用循环中覆盖或清空；
    /// 这里不提前触碰它们，避免跨两个循环重复扫描实体数据。
    pub(crate) fn reset_score_battle_state_from(&mut self, prepared: &Self, fixed_count: usize) {
        self.entities.truncate(prepared.entities.len());
        if self.entities.len() < prepared.entities.len() {
            self.entities.resize_with(prepared.entities.len(), || None);
        }
        for (index, (current, prepared)) in self.entities.iter_mut().zip(&prepared.entities).enumerate() {
            match (current.as_mut(), prepared.as_ref()) {
                (Some(current), Some(prepared)) if index < fixed_count => current.reset_battle_state_from(prepared),
                (Some(_), Some(_)) => {}
                _ => current.clone_from(prepared),
            }
        }
    }

    /// 按下标取实体；越界或空槽返回 None。
    pub fn get(&self, idx: EntityIdx) -> Option<&EntityRecord> { self.entities.get(idx.0 as usize).and_then(Option::as_ref) }

    /// 可变按下标取实体；越界或空槽返回 None。
    pub fn get_mut(&mut self, idx: EntityIdx) -> Option<&mut EntityRecord> {
        self.entities.get_mut(idx.0 as usize).and_then(Option::as_mut)
    }

    /// 按模板的预留 ID 数计算下一个生成下标。
    pub fn next_spawn_idx(&self, template: &PlayerTemplate) -> EntityIdx {
        Self::next_spawn_idx_from_slot_count(self.entities.len(), template)
    }

    /// 按给定槽数与预留数计算生成下标；溢出会 panic。
    pub fn next_spawn_idx_from_slot_count(slot_count: usize, template: &PlayerTemplate) -> EntityIdx {
        let reserved =
            usize::try_from(template.reserved_player_ids_before_spawn).expect("runtime reserved player id count overflow");
        let idx = slot_count.checked_add(reserved).expect("runtime entity slot count overflow");
        EntityIdx(idx.try_into().expect("runtime entity index overflow"))
    }

    /// 从模板生成一个无属主实体，返回新下标。
    pub fn spawn_from_template(&mut self, template: PlayerTemplate, registry: &ExtensionRegistry) -> EntityIdx {
        self.spawn_from_template_with_owner(template, registry, None, None)
    }

    /// 从模板生成实体并指定 owner 与根 owner；队伍、clan 与防抗继承按 owner 解析。
    pub fn spawn_from_template_with_owner(
        &mut self,
        mut template: PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: Option<EntityIdx>,
        root_owner: Option<EntityIdx>,
    ) -> EntityIdx {
        let idx = self.next_spawn_idx(&template);
        let reserved =
            usize::try_from(template.reserved_player_ids_before_spawn).expect("runtime reserved player id count overflow");
        self.entities.extend(std::iter::repeat_n(None, reserved));
        template.id = idx.0 as usize + 1;
        let owner = owner.unwrap_or(idx);
        let root_owner = root_owner.unwrap_or(owner);
        if let Some(owner_idx) = Some(owner).filter(|owner_idx| *owner_idx != idx) {
            let owner_entity = self
                .get(owner_idx)
                .unwrap_or_else(|| panic!("unknown runtime spawn owner entity: {}", owner_idx.0));
            // legacy 的 `queue_spawn(owner, child)` 会把子实体放进 owner 当前所在的世界队伍。
            // 蓝图中的队伍只是导入期占位值，不能决定运行期归属。
            template.team = owner_entity.runtime.team;
            let policies = template.effective_policies(registry);
            if policies.inherit_owner_def_res {
                template.defense = owner_entity.runtime.defense;
                template.resistance = owner_entity.runtime.resistance;
            }
            let root_clan_name = self
                .get(root_owner)
                .unwrap_or_else(|| panic!("unknown runtime spawn root owner entity: {}", root_owner.0))
                .template
                .clan_name
                .clone();
            template.set_runtime_clan_name(root_clan_name);
        }
        if template.skills.skills().iter().all(|skill_id| registry.skill(*skill_id).is_some()) {
            template.skills.prepare_hook_cache(registry);
        }
        let runtime = PlayerRuntime::from_template(&template, registry, owner, root_owner);
        let mut states = StateStore::default();
        if runtime.is_minion() {
            states.register_compressed_legacy_state(CompressedLegacyState::Minion);
        }
        self.entities.push(Some(EntityRecord {
            template,
            runtime,
            states,
            slots: EntitySlotStorage::from_registry(registry),
        }));
        idx
    }

    /// 遍历现有实体及其下标，跳过空槽。
    pub fn iter(&self) -> impl Iterator<Item = (EntityIdx, &EntityRecord)> {
        self.entities
            .iter()
            .enumerate()
            .filter_map(|(idx, entity)| entity.as_ref().map(|entity| (EntityIdx(idx as u32), entity)))
    }
}
