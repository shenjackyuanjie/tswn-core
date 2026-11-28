//! 技能的固定 lane、当前/构建等级、尝试顺序及派生 hook 缓存。
//!
//! `fixed_lane` 是技能数据槽位置；`active_order` 保存遍历这些位置的排列。
//! 吞噬等机制可能改等级、顺序或延后游标；使用 loadout 的修改入口以同步 dirty 与 generation。
//! 当前等级为 0 的技能不进入相应 hook 计划，构建等级保留最初导入语义。

use super::*;
use crate::namerena::EffectiveSkillWeight;
use std::sync::Arc;

/// hook 缓存按阶段分区的段数（对应 ProcMask 低 8 位）。
const SKILL_HOOK_COUNT: usize = 8;
static NEXT_SKILL_LOADOUT_BASELINE_ID: AtomicU64 = AtomicU64::new(1);

/// 分配下一个 loadout 基线编号，用来识别外部整体替换技能表的情况。
fn next_skill_loadout_baseline_id() -> u64 { NEXT_SKILL_LOADOUT_BASELINE_ID.fetch_add(1, Ordering::Relaxed) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachedSkillHookEntry {
    hook_index: u8,
    pub skill_id: SkillId,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
    pub post_action_phase: SkillPostActionPhase,
    pub active_order: usize,
    pub fixed_lane: usize,
    pub registration_order: RegistrationOrder,
}

/// 默认 score 技能表在 registry 构造期固化的 hook 元数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreSkillHookPlanEntry {
    pub legacy_key: usize,
    pub hook_index: u8,
    pub skill_id: SkillId,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
    pub post_action_phase: SkillPostActionPhase,
    pub registration_order: RegistrationOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachedBuiltinActionEntry {
    pub fixed_lane: u16,
    pub skill: BuiltinActiveSkill,
}

/// dirty 位：当前等级被修改。
const SKILL_DIRTY_LEVELS: u8 = 1 << 0;
/// dirty 位：强化记录被修改。
const SKILL_DIRTY_BOOSTS: u8 = 1 << 1;
/// dirty 位：主动钩子顺序被修改。
const SKILL_DIRTY_ACTIVE_HOOKS: u8 = 1 << 2;
/// dirty 位：PRE_ACTION 顺序被修改。
const SKILL_DIRTY_PRE_ACTION: u8 = 1 << 3;
/// dirty 位：POST_DAMAGE 顺序被修改。
const SKILL_DIRTY_POST_DAMAGE: u8 = 1 << 4;
/// dirty 位：POST_ACTION 延迟条目被修改。
const SKILL_DIRTY_DEFERRED: u8 = 1 << 5;

#[derive(Debug, PartialEq, Eq)]
struct EffectiveActionWeights {
    // 固定槽位、初始等级和初始权重；克隆体共享基准，读取时使用自己的当前等级。
    entries: Vec<(usize, u32, EffectiveSkillWeight)>,
    redistribution: Option<EffectiveSkillWeight>,
    adjustment: Option<Vec<(usize, EffectiveSkillWeight)>>,
}

/// 分身和首发模板可共享固定槽位。修改阵容或重填 score profile 时按需分离。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SkillLayout {
    skills: SmallVec<[SkillId; 8]>,
    fixed_lane_keys: SmallVec<[usize; 8]>,
    merge_lane_order: SmallVec<[usize; 8]>,
}

#[derive(Debug, Clone)]
/// 一份技能配置及其运行期变动；handler 函数由 registry ID 另行绑定。
pub struct SkillLoadout {
    layout: Arc<SkillLayout>,
    levels: SmallVec<[u32; 8]>,
    build_levels: SmallVec<[u32; 8]>,
    boosts: SmallVec<[Option<SkillBoost>; 8]>,
    boosted: SmallVec<[bool; 8]>,
    active_order: SmallVec<[usize; 8]>,
    pre_action_order: SmallVec<[usize; 8]>,
    post_damage_order: SmallVec<[usize; 8]>,
    post_action_after_states: SmallVec<[(u64, usize); 4]>,
    effective_skill_weights: Option<Arc<EffectiveActionWeights>>,
    // 主动技能热路径只需要固定槽位和已解析的内置类型；u16 槽位让常见八项缓存留在栈内。
    action_cache: SmallVec<[CachedBuiltinActionEntry; 8]>,
    action_cache_ready: bool,
    // 这是纯派生缓存，不参与 loadout 的语义相等性；score 会跨场复用其堆容量。
    hook_cache: Vec<CachedSkillHookEntry>,
    hook_cache_offsets: [u16; SKILL_HOOK_COUNT + 1],
    hook_cache_ready: bool,
    // 缓存有效时只记录有非零等级技能的阶段；失效时保守地保留所有阶段。
    enabled_hook_mask: u8,
    /// 同一份准备模板与 worker 克隆共享此编号，用来识别外部整体替换技能表的情况。
    baseline_id: u64,
    /// 按字段组记录本局写入，只复位真正变化过的 SmallVec 和 hook 缓存。
    battle_dirty: u8,
    /// 会改变技能钩子计划的写入代数，用于同一行动内安全复用 POST_ACTION 分区。
    hook_generation: u32,
}

impl Default for SkillLoadout {
    /// 空技能表：无技能与缓存，基线编号新建，启用掩码保守置为全量。
    fn default() -> Self {
        Self {
            layout: Arc::default(),
            levels: SmallVec::new(),
            build_levels: SmallVec::new(),
            boosts: SmallVec::new(),
            boosted: SmallVec::new(),
            active_order: SmallVec::new(),
            pre_action_order: SmallVec::new(),
            post_damage_order: SmallVec::new(),
            post_action_after_states: SmallVec::new(),
            effective_skill_weights: None,
            action_cache: SmallVec::new(),
            action_cache_ready: false,
            hook_cache: Vec::new(),
            hook_cache_offsets: [0; SKILL_HOOK_COUNT + 1],
            hook_cache_ready: false,
            enabled_hook_mask: u8::MAX,
            baseline_id: next_skill_loadout_baseline_id(),
            battle_dirty: 0,
            hook_generation: 0,
        }
    }
}

impl PartialEq for SkillLoadout {
    /// 语义相等：只比较技能、等级、强化与权重等数据字段，忽略 hook 缓存等纯派生内容。
    fn eq(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.levels == other.levels
            && self.build_levels == other.build_levels
            && self.boosts == other.boosts
            && self.boosted == other.boosted
            && self.active_order == other.active_order
            && self.pre_action_order == other.pre_action_order
            && self.post_damage_order == other.post_damage_order
            && self.post_action_after_states == other.post_action_after_states
            && self.effective_skill_weights == other.effective_skill_weights
    }
}

impl Eq for SkillLoadout {}

impl SkillLoadout {
    /// 从技能 ID 列表构造装载，等级统一为 1。
    pub fn from_skills(skills: impl IntoIterator<Item = SkillId>) -> Self {
        let skills = skills.into_iter().collect::<SmallVec<[SkillId; 8]>>();
        let levels = std::iter::repeat_n(1, skills.len()).collect::<SmallVec<[u32; 8]>>();
        let build_levels = levels.clone();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let boosted = std::iter::repeat_n(false, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let merge_lane_order = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        let post_damage_order = (0..skills.len()).collect();
        Self {
            layout: Arc::new(SkillLayout {
                skills,
                fixed_lane_keys,
                merge_lane_order,
            }),
            levels,
            build_levels,
            boosts,
            boosted,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
            effective_skill_weights: None,
            action_cache: SmallVec::new(),
            action_cache_ready: false,
            hook_cache: Vec::new(),
            hook_cache_offsets: [0; SKILL_HOOK_COUNT + 1],
            hook_cache_ready: false,
            enabled_hook_mask: u8::MAX,
            baseline_id: next_skill_loadout_baseline_id(),
            battle_dirty: 0,
            hook_generation: 0,
        }
    }

    /// 从 (技能, 等级) 列表构造装载。
    pub fn from_skill_levels(skills: impl IntoIterator<Item = (SkillId, u32)>) -> Self {
        let (skills, levels): (SmallVec<[SkillId; 8]>, SmallVec<[u32; 8]>) = skills.into_iter().unzip();
        let build_levels = levels.clone();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let boosted = std::iter::repeat_n(false, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let merge_lane_order = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        let post_damage_order = (0..skills.len()).collect();
        Self {
            layout: Arc::new(SkillLayout {
                skills,
                fixed_lane_keys,
                merge_lane_order,
            }),
            levels,
            build_levels,
            boosts,
            boosted,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
            effective_skill_weights: None,
            action_cache: SmallVec::new(),
            action_cache_ready: false,
            hook_cache: Vec::new(),
            hook_cache_offsets: [0; SKILL_HOOK_COUNT + 1],
            hook_cache_ready: false,
            enabled_hook_mask: u8::MAX,
            baseline_id: next_skill_loadout_baseline_id(),
            battle_dirty: 0,
            hook_generation: 0,
        }
    }

    /// 从 (技能, 等级, 强化) 列表构造；构建等级取强化前的基线。
    pub fn from_skill_levels_and_boosts(skills: impl IntoIterator<Item = (SkillId, u32, Option<SkillBoost>)>) -> Self {
        let mut skill_ids = SmallVec::<[SkillId; 8]>::new();
        let mut levels = SmallVec::<[u32; 8]>::new();
        let mut boosts = SmallVec::<[Option<SkillBoost>; 8]>::new();
        for (skill_id, level, boost) in skills {
            skill_ids.push(skill_id);
            levels.push(level);
            boosts.push(boost);
        }
        let build_levels = levels
            .iter()
            .zip(&boosts)
            .map(|(level, boost)| boost.as_ref().map_or(*level, SkillBoost::base_level))
            .collect::<SmallVec<[u32; 8]>>();
        let boosted = boosts
            .iter()
            .map(|boost| matches!(boost, Some(SkillBoost::LastBoost(_) | SkillBoost::SlotBoost { .. })))
            .collect();
        let fixed_lane_keys = (0..skill_ids.len()).collect();
        let merge_lane_order = (0..skill_ids.len()).collect();
        let active_order = (0..skill_ids.len()).collect();
        let post_damage_order = (0..skill_ids.len()).collect();
        Self {
            layout: Arc::new(SkillLayout {
                skills: skill_ids,
                fixed_lane_keys,
                merge_lane_order,
            }),
            levels,
            build_levels,
            boosts,
            boosted,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
            effective_skill_weights: None,
            action_cache: SmallVec::new(),
            action_cache_ready: false,
            hook_cache: Vec::new(),
            hook_cache_offsets: [0; SKILL_HOOK_COUNT + 1],
            hook_cache_ready: false,
            enabled_hook_mask: u8::MAX,
            baseline_id: next_skill_loadout_baseline_id(),
            battle_dirty: 0,
            hook_generation: 0,
        }
    }

    /// 原地重填标准 score profile 的固定 35-lane 技能表，复用上一场的堆容量。
    pub fn reset_score_profile(
        &mut self,
        skill_ids: &[Option<SkillId>; 35],
        hook_plan: &[ScoreSkillHookPlanEntry],
        hook_plan_needs_active_sort: bool,
        levels: &[u32; 35],
        boosted: &[bool; 35],
        boosts: &[Option<SkillBoost>; 35],
        action_order: &[u32; 40],
    ) {
        self.invalidate_hook_cache();
        self.effective_skill_weights = None;
        let layout = Arc::make_mut(&mut self.layout);
        layout.skills.clear();
        self.levels.clear();
        self.build_levels.clear();
        self.boosts.clear();
        self.boosted.clear();
        layout.fixed_lane_keys.clear();
        layout.merge_lane_order.clear();
        self.active_order.clear();
        self.pre_action_order.clear();
        self.post_damage_order.clear();
        self.post_action_after_states.clear();

        let mut lane_by_key = [usize::MAX; 35];
        for key in 0..35 {
            let Some(skill_id) = skill_ids[key] else {
                continue;
            };
            let lane = layout.skills.len();
            lane_by_key[key] = lane;
            layout.skills.push(skill_id);
            self.levels.push(levels[key]);
            self.build_levels.push(boosts[key].as_ref().map_or(levels[key], SkillBoost::base_level));
            self.boosts.push(boosts[key].clone());
            self.boosted.push(boosted[key]);
            layout.fixed_lane_keys.push(key);
            layout.merge_lane_order.push(lane);
        }

        let mut active_position_by_key = [usize::MAX; 35];
        for &key in action_order {
            let key = key as usize;
            if key < lane_by_key.len() && lane_by_key[key] != usize::MAX {
                active_position_by_key[key] = self.active_order.len();
                let fixed_lane = lane_by_key[key];
                self.active_order.push(fixed_lane);
                if let Some(skill) = BuiltinActiveSkill::from_legacy_key(key) {
                    self.action_cache.push(CachedBuiltinActionEntry {
                        fixed_lane: u16::try_from(fixed_lane).expect("runtime score 主动技能槽位超出 u16"),
                        skill,
                    });
                }
            }
        }
        self.action_cache_ready = true;
        for key in [29usize, 34] {
            if levels[key] > 0 && lane_by_key[key] != usize::MAX {
                self.pre_action_order.push(lane_by_key[key]);
            }
        }
        for key in [30usize, 33, 34, 21] {
            if levels[key] > 0 && lane_by_key[key] != usize::MAX {
                self.post_damage_order.push(lane_by_key[key]);
            }
        }

        for plan in hook_plan {
            let fixed_lane = lane_by_key[plan.legacy_key];
            let active_order = active_position_by_key[plan.legacy_key];
            if fixed_lane == usize::MAX || active_order == usize::MAX {
                continue;
            }
            self.hook_cache.push(CachedSkillHookEntry {
                hook_index: plan.hook_index,
                skill_id: plan.skill_id,
                target_policy: plan.target_policy,
                priority: plan.priority,
                post_action_phase: plan.post_action_phase,
                active_order,
                fixed_lane,
                registration_order: plan.registration_order,
            });
        }
        if hook_plan_needs_active_sort {
            self.hook_cache
                .sort_by_key(|entry| (entry.hook_index, entry.priority, entry.active_order, entry.registration_order));
        }
        self.finish_hook_cache_offsets();
        self.battle_dirty = 0;
        self.hook_generation = 0;
    }

    /// 每个 worker 独立持有基线，分身共享只在同一 worker 增减引用。
    pub fn isolate_worker_layout(&mut self) { Arc::make_mut(&mut self.layout); }

    /// 将当前技能表封为可复用 runner 的场前基线。
    pub fn mark_battle_baseline(&mut self) {
        self.battle_dirty = 0;
        self.hook_generation = 0;
    }

    /// 恢复一场战斗会修改的技能字段；未发生修改时完全跳过 SmallVec 深拷贝。
    pub fn reset_battle_fields_from(&mut self, prepared: &Self) {
        if self.baseline_id != prepared.baseline_id {
            self.clone_from(prepared);
            return;
        }
        let dirty = self.battle_dirty;
        if dirty == 0 {
            return;
        }

        debug_assert_eq!(self.layout.skills, prepared.layout.skills);
        debug_assert_eq!(self.layout.fixed_lane_keys, prepared.layout.fixed_lane_keys);
        debug_assert_eq!(self.layout.merge_lane_order, prepared.layout.merge_lane_order);
        if dirty & SKILL_DIRTY_LEVELS != 0 {
            self.levels.clone_from(&prepared.levels);
        }
        if dirty & SKILL_DIRTY_BOOSTS != 0 {
            self.build_levels.clone_from(&prepared.build_levels);
            self.boosts.clone_from(&prepared.boosts);
            self.boosted.clone_from(&prepared.boosted);
        }
        if dirty & SKILL_DIRTY_ACTIVE_HOOKS != 0 {
            self.active_order.clone_from(&prepared.active_order);
            self.action_cache.clone_from(&prepared.action_cache);
            self.action_cache_ready = prepared.action_cache_ready;
            self.hook_cache.clone_from(&prepared.hook_cache);
            self.hook_cache_offsets = prepared.hook_cache_offsets;
            self.hook_cache_ready = prepared.hook_cache_ready;
        }
        if dirty & (SKILL_DIRTY_LEVELS | SKILL_DIRTY_ACTIVE_HOOKS) != 0 {
            self.enabled_hook_mask = if self.hook_cache_ready {
                prepared.enabled_hook_mask
            } else {
                u8::MAX
            };
        }
        if dirty & SKILL_DIRTY_PRE_ACTION != 0 {
            self.pre_action_order.clone_from(&prepared.pre_action_order);
        }
        if dirty & SKILL_DIRTY_POST_DAMAGE != 0 {
            self.post_damage_order.clone_from(&prepared.post_damage_order);
        }
        if dirty & SKILL_DIRTY_DEFERRED != 0 {
            self.post_action_after_states.clone_from(&prepared.post_action_after_states);
        }
        self.battle_dirty = 0;
        self.hook_generation = prepared.hook_generation;
    }

    /// 召唤实体沿用已准备的缓存；外部模板的技能全部登记后才构建缓存。
    pub fn prepare_registered_hook_cache(&mut self, registry: &ExtensionRegistry) {
        if !self.hook_cache_ready && self.layout.skills.iter().all(|&id| registry.skill(id).is_some()) {
            self.prepare_hook_cache(registry);
        }
    }

    /// 预计算八类技能钩子的稳定顺序，避免每次行动重复扫描并排序完整技能表。
    pub fn prepare_hook_cache(&mut self, registry: &ExtensionRegistry) {
        if self.hook_cache_ready {
            return;
        }
        self.action_cache.clear();
        self.hook_cache.clear();
        let mut action_cache_supported = true;
        for (active_order, &fixed_lane) in self.active_order.iter().enumerate() {
            let skill_id = *self
                .layout
                .skills
                .get(fixed_lane)
                .unwrap_or_else(|| panic!("runtime skill active order references missing lane: {fixed_lane}"));
            let spec = registry
                .skill(skill_id)
                .unwrap_or_else(|| panic!("unknown runtime skill id in loadout: {}", skill_id.0));
            if let Some(skill) = registry.builtin_active_skill(skill_id) {
                if let Ok(fixed_lane) = u16::try_from(fixed_lane) {
                    self.action_cache.push(CachedBuiltinActionEntry { fixed_lane, skill });
                } else {
                    action_cache_supported = false;
                }
            }
            let mut hooks = spec.hook_mask.0 & ((1 << SKILL_HOOK_COUNT) - 1);
            while hooks != 0 {
                let hook_index = hooks.trailing_zeros() as u8;
                hooks &= hooks - 1;
                self.hook_cache.push(CachedSkillHookEntry {
                    hook_index,
                    skill_id: spec.id,
                    target_policy: spec.target_policy,
                    priority: spec.priority,
                    post_action_phase: spec.post_action_phase,
                    active_order,
                    fixed_lane,
                    registration_order: spec.registration_order,
                });
            }
        }
        self.hook_cache
            .sort_by_key(|entry| (entry.hook_index, entry.priority, entry.active_order, entry.registration_order));
        if !action_cache_supported {
            self.action_cache.clear();
        }
        self.action_cache_ready = action_cache_supported;
        self.finish_hook_cache_offsets();
    }

    /// 重建 hook 缓存的分段偏移与启用掩码：按 `hook_index` 顺序登记各区起止，并标记含非零等级技能的阶段。
    fn finish_hook_cache_offsets(&mut self) {
        let mut cursor = 0usize;
        self.enabled_hook_mask = 0;
        for hook_index in 0..SKILL_HOOK_COUNT {
            self.hook_cache_offsets[hook_index] = u16::try_from(cursor).expect("runtime skill hook cache exceeds u16 range");
            while let Some(entry) = self.hook_cache.get(cursor).filter(|entry| usize::from(entry.hook_index) == hook_index) {
                if self.levels[entry.fixed_lane] > 0 {
                    self.enabled_hook_mask |= 1 << hook_index;
                }
                cursor += 1;
            }
        }
        self.hook_cache_offsets[SKILL_HOOK_COUNT] = u16::try_from(cursor).expect("runtime skill hook cache exceeds u16 range");
        self.hook_cache_ready = true;
    }

    /// 只在已知该阶段无非零等级的技能时返回 false；未缓存的扩展阶段仍走通用查询。
    #[inline]
    pub fn may_have_enabled_hook(&self, hook: ProcMask) -> bool {
        let absent_hooks = u64::from(!self.enabled_hook_mask);
        hook.0 & !absent_hooks != 0
    }

    /// 仅按当前等级重算启用掩码；缓存未就绪时跳过。
    fn refresh_enabled_hook_mask(&mut self) {
        if !self.hook_cache_ready {
            return;
        }
        let mut enabled_hooks = 0;
        for entry in &self.hook_cache {
            if self.levels[entry.fixed_lane] > 0 {
                enabled_hooks |= 1 << entry.hook_index;
            }
        }
        self.enabled_hook_mask = enabled_hooks;
    }

    pub fn cached_hook_entries(&self, hook: ProcMask) -> Option<&[CachedSkillHookEntry]> {
        if !self.hook_cache_ready || !hook.0.is_power_of_two() {
            return None;
        }
        let hook_index = hook.0.trailing_zeros() as usize;
        if hook_index >= SKILL_HOOK_COUNT {
            return None;
        }
        if self.enabled_hook_mask & (1 << hook_index) == 0 {
            return Some(&[]);
        }
        let start = usize::from(self.hook_cache_offsets[hook_index]);
        let end = usize::from(self.hook_cache_offsets[hook_index + 1]);
        Some(&self.hook_cache[start..end])
    }

    pub fn cached_builtin_actions(&self) -> Option<&[CachedBuiltinActionEntry]> {
        self.action_cache_ready.then_some(self.action_cache.as_slice())
    }

    /// 作废 hook 与行动缓存：清空条目、偏移归零，启用掩码保守置为全量。
    fn invalidate_hook_cache(&mut self) {
        self.action_cache.clear();
        self.action_cache_ready = false;
        self.hook_cache.clear();
        self.hook_cache_offsets.fill(0);
        self.hook_cache_ready = false;
        self.enabled_hook_mask = u8::MAX;
    }

    /// 按固定 lane 排列的技能 ID。
    pub fn skills(&self) -> &[SkillId] { &self.layout.skills }

    /// 各固定 lane 的当前等级。
    pub fn levels(&self) -> &[u32] { &self.levels }

    /// 指定 lane 的当前等级；越界返回 None。
    pub fn level_at(&self, fixed_lane: usize) -> Option<u32> { self.levels.get(fixed_lane).copied() }

    /// 指定 lane 的构建等级（导入基线）；越界返回 None。
    pub fn build_level_at(&self, fixed_lane: usize) -> Option<u32> { self.build_levels.get(fixed_lane).copied() }

    /// 指定 lane 的强化记录；越界或未强化返回 None。
    pub fn boost_at(&self, fixed_lane: usize) -> Option<&SkillBoost> { self.boosts.get(fixed_lane).and_then(Option::as_ref) }

    /// 指定 lane 是否已强化；越界返回 None。
    pub fn boosted_at(&self, fixed_lane: usize) -> Option<bool> { self.boosted.get(fixed_lane).copied() }

    /// 指定 lane 的固定键（legacy 兼容编号）；越界返回 None。
    pub fn fixed_lane_key_at(&self, fixed_lane: usize) -> Option<usize> { self.layout.fixed_lane_keys.get(fixed_lane).copied() }

    /// 吞噬配对使用的 lane 顺序。
    pub fn merge_lane_order(&self) -> &[usize] { &self.layout.merge_lane_order }

    /// 设置当前等级；等级进出 0 时刷新 hook 启用掩码，lane 越界返回 false。
    pub fn set_level_at(&mut self, fixed_lane: usize, level: u32) -> bool {
        let Some(current) = self.levels.get_mut(fixed_lane) else {
            return false;
        };
        if *current == level {
            return true;
        }
        let eligibility_changed = (*current == 0) != (level == 0);
        *current = level;
        if eligibility_changed {
            self.refresh_enabled_hook_mask();
        }
        self.mark_hook_mutation(SKILL_DIRTY_LEVELS);
        true
    }

    /// 主动技能的遍历顺序（lane 排列）。
    pub fn active_order(&self) -> &[usize] { &self.active_order }

    pub fn with_effective_skill_weights(
        mut self,
        weights: Option<&[(usize, EffectiveSkillWeight)]>,
        redistribution: Option<EffectiveSkillWeight>,
        adjustment: Option<&[(usize, EffectiveSkillWeight)]>,
    ) -> Self {
        self.effective_skill_weights = weights.map(|weights| {
            Arc::new(EffectiveActionWeights {
                entries: weights
                    .iter()
                    .filter_map(|&(key, weight)| {
                        let lane = self.layout.fixed_lane_keys.iter().position(|&candidate| candidate == key)?;
                        let level = self.levels[lane];
                        (level > 0 && weight.value() > 0.0).then_some((lane, level, weight))
                    })
                    .collect(),
                redistribution,
                adjustment: adjustment.map(|adjustment| {
                    adjustment
                        .iter()
                        .filter_map(|&(key, cut)| {
                            let lane = self.layout.fixed_lane_keys.iter().position(|&candidate| candidate == key)?;
                            Some((lane, cut))
                        })
                        .collect()
                }),
            })
        });
        self
    }

    pub fn effective_action_weights(&self) -> Option<impl Iterator<Item = (usize, f64)> + '_> {
        self.effective_skill_weights.as_ref().map(|weights| {
            weights.entries.iter().filter_map(|&(lane, initial_level, weight)| {
                let level = self.levels[lane];
                (level > 0 && self.active_order.contains(&lane))
                    .then(|| (lane, weight.value() * f64::from(level) / f64::from(initial_level)))
            })
        })
    }

    pub fn effective_skill_redistribution(&self) -> f64 {
        self.effective_skill_weights
            .as_ref()
            .and_then(|weights| weights.redistribution)
            .map_or(0.0, EffectiveSkillWeight::value)
            .min(1.0)
    }

    pub fn uses_compact_effective_skills(&self) -> bool {
        self.effective_skill_weights.as_ref().is_some_and(|weights| weights.adjustment.is_some())
    }

    pub fn effective_skill_cut(&self, lane: usize) -> Option<f64> {
        self.effective_skill_weights
            .as_ref()?
            .adjustment
            .as_ref()?
            .iter()
            .find(|&&(candidate, _)| candidate == lane)
            .map(|&(_, cut)| cut.value())
    }

    pub fn uses_effective_skill_weights(&self) -> bool { self.effective_skill_weights.is_some() }

    /// PRE_ACTION 阶段的 lane 顺序。
    pub fn pre_action_order(&self) -> &[usize] { &self.pre_action_order }

    /// POST_DAMAGE 阶段的 lane 顺序。
    pub fn post_damage_order(&self) -> &[usize] { &self.post_damage_order }

    /// 延后到状态之后执行的 POST_ACTION 条目：(状态游标, lane)。
    pub fn post_action_after_states(&self) -> &[(u64, usize)] { &self.post_action_after_states }

    pub const fn hook_generation(&self) -> u32 { self.hook_generation }

    /// 是否没有任何技能。
    pub fn is_empty(&self) -> bool { self.layout.skills.is_empty() }

    /// 技能数量，即固定 lane 数。
    pub fn len(&self) -> usize { self.layout.skills.len() }

    /// 按各 lane 的强化类型重建一套克隆等级，供分身使用。
    pub fn rebuilt_for_clone(&self) -> Self {
        let mut clone = self.clone();
        let mut eligibility_changed = false;
        for lane in 0..clone.levels.len() {
            let clamped = self.build_levels[lane].min(self.levels[lane]);
            clone.levels[lane] = match &self.boosts[lane] {
                None | Some(SkillBoost::Normal(_)) => clamped,
                Some(SkillBoost::LastBoost(_)) => clamped.saturating_mul(2),
                Some(SkillBoost::SlotBoost { boost, .. }) => clamped.saturating_add((*boost).min(clamped)),
            };
            eligibility_changed |= self.levels[lane] > 0 && clone.levels[lane] == 0;
        }
        clone
            .pre_action_order
            .retain(|lane| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        clone
            .post_action_after_states
            .retain(|(_, lane)| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        if eligibility_changed {
            clone.refresh_enabled_hook_mask();
        }
        clone
    }

    pub fn rebuilt_for_score_clone(&self, plan: &super::ScoreCloneSkillBoostPlan) -> Self {
        let mut clone = self.clone();
        let mut eligibility_changed = false;
        for lane in 0..clone.levels.len() {
            let key = clone.layout.fixed_lane_keys[lane];
            clone.levels[lane] = self.build_levels[lane].min(self.levels[lane]);
            eligibility_changed |= self.levels[lane] > 0 && clone.levels[lane] == 0;
            clone.boosts[lane] = None;
            clone.boosted[lane] = key < 64 && plan.initially_boosted_mask & (1u64 << key) != 0;
        }

        if let Some(lane) = clone
            .active_order
            .iter()
            .rev()
            .copied()
            .find(|lane| clone.layout.fixed_lane_keys[*lane] < 25 && clone.levels[*lane] > 0 && !clone.boosted[*lane])
        {
            let base = clone.levels[lane];
            clone.levels[lane] = base.saturating_mul(2);
            clone.boosted[lane] = true;
            clone.boosts[lane] = Some(SkillBoost::LastBoost(base));
        }

        for &(key, max_boost) in plan.slot_boosts.iter().flatten() {
            let key = usize::from(key);
            let Some(lane) = clone.layout.fixed_lane_keys.iter().position(|candidate| *candidate == key) else {
                continue;
            };
            if clone.levels[lane] == 0 || clone.boosted[lane] {
                continue;
            }
            let base = clone.levels[lane];
            let boost = u32::from(max_boost).min(base);
            clone.levels[lane] = base.saturating_add(boost);
            clone.boosted[lane] = true;
            clone.boosts[lane] = Some(SkillBoost::SlotBoost { base, boost });
        }

        clone
            .pre_action_order
            .retain(|lane| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        clone
            .post_action_after_states
            .retain(|(_, lane)| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        if eligibility_changed {
            clone.refresh_enabled_hook_mask();
        }
        clone
    }

    /// 从主动顺序移除指定 lane，并使 hook 缓存失效。
    pub fn disable_action_lane(&mut self, fixed_lane: usize) {
        let before = self.active_order.len();
        self.active_order.retain(|lane| *lane != fixed_lane);
        if self.active_order.len() != before {
            self.invalidate_hook_cache();
            self.mark_hook_mutation(SKILL_DIRTY_ACTIVE_HOOKS);
        }
    }

    /// 设置主动顺序；引用不存在的 lane 会 panic。
    pub fn with_active_order(mut self, active_order: impl IntoIterator<Item = usize>) -> Self {
        self.active_order = active_order.into_iter().collect();
        self.invalidate_hook_cache();
        assert!(
            self.active_order.iter().all(|idx| *idx < self.layout.skills.len()),
            "runtime skill active order must reference existing fixed lanes"
        );
        self
    }

    /// 设置 PRE_ACTION 顺序；引用不存在的 lane 会 panic。
    pub fn with_pre_action_order(mut self, pre_action_order: impl IntoIterator<Item = usize>) -> Self {
        self.pre_action_order = pre_action_order.into_iter().collect();
        assert!(
            self.pre_action_order.iter().all(|idx| *idx < self.layout.skills.len()),
            "runtime skill pre-action order must reference existing fixed lanes"
        );
        self
    }

    /// 设置 POST_DAMAGE 顺序；引用不存在的 lane 会 panic。
    pub fn with_post_damage_order(mut self, post_damage_order: impl IntoIterator<Item = usize>) -> Self {
        self.post_damage_order = post_damage_order.into_iter().collect();
        assert!(
            self.post_damage_order.iter().all(|idx| *idx < self.layout.skills.len()),
            "runtime skill post-damage order must reference existing fixed lanes"
        );
        self
    }

    /// 设置延后 POST_ACTION 条目并按游标排序；引用不存在的 lane 会 panic。
    pub fn with_post_action_after_states(mut self, post_action_after_states: impl IntoIterator<Item = (u64, usize)>) -> Self {
        self.post_action_after_states = post_action_after_states.into_iter().collect();
        assert!(
            self.post_action_after_states.iter().all(|(_, idx)| *idx < self.layout.skills.len()),
            "runtime deferred post-action order must reference existing fixed lanes"
        );
        self.post_action_after_states.sort_by_key(|(cursor, _)| *cursor);
        self
    }

    /// 登记一个延后 POST_ACTION 条目；同一 lane 重复登记被忽略。
    pub fn register_post_action_after_states(&mut self, fixed_lane: usize, state_order_cursor: u64) {
        assert!(
            fixed_lane < self.layout.skills.len(),
            "runtime deferred post-action order must reference an existing fixed lane"
        );
        if self.post_action_after_states.iter().any(|(_, lane)| *lane == fixed_lane) {
            return;
        }
        self.post_action_after_states.push((state_order_cursor, fixed_lane));
        self.post_action_after_states.sort_by_key(|(cursor, _)| *cursor);
        self.mark_hook_mutation(SKILL_DIRTY_DEFERRED);
    }

    /// 确保指定 lane 出现在 PRE_ACTION 顺序中。
    pub fn ensure_pre_action_lane(&mut self, fixed_lane: usize) {
        assert!(
            fixed_lane < self.layout.skills.len(),
            "runtime skill pre-action order must reference existing fixed lanes"
        );
        if !self.pre_action_order.contains(&fixed_lane) {
            self.pre_action_order.push(fixed_lane);
            self.battle_dirty |= SKILL_DIRTY_PRE_ACTION;
        }
    }

    /// 从 PRE_ACTION 顺序中移除指定 lane。
    pub fn remove_pre_action_lane(&mut self, fixed_lane: usize) {
        let before = self.pre_action_order.len();
        self.pre_action_order.retain(|lane| *lane != fixed_lane);
        if self.pre_action_order.len() != before {
            self.battle_dirty |= SKILL_DIRTY_PRE_ACTION;
        }
    }

    /// 设置固定键；长度必须与技能数一致，否则 panic。
    pub fn with_fixed_lane_keys(mut self, fixed_lane_keys: impl IntoIterator<Item = usize>) -> Self {
        Arc::make_mut(&mut self.layout).fixed_lane_keys = fixed_lane_keys.into_iter().collect();
        assert_eq!(
            self.layout.fixed_lane_keys.len(),
            self.layout.skills.len(),
            "runtime fixed lane keys must match skill loadout length"
        );
        self
    }

    /// 设置已强化标记；长度必须与技能数一致，否则 panic。
    pub fn with_boosted_flags(mut self, boosted: impl IntoIterator<Item = bool>) -> Self {
        self.boosted = boosted.into_iter().collect();
        assert_eq!(
            self.boosted.len(),
            self.layout.skills.len(),
            "runtime boosted flags must match skill loadout length"
        );
        self
    }

    /// 复刻 legacy `boost_last()`：从行动顺序末尾寻找首个未强化的正等级技能。
    pub fn boost_last_active_except_key(&mut self, excluded_fixed_key: usize) -> bool {
        for &lane in self.active_order.iter().rev() {
            if self.layout.fixed_lane_keys[lane] == excluded_fixed_key || self.levels[lane] == 0 || self.boosted[lane] {
                continue;
            }
            let base = self.levels[lane];
            self.levels[lane] = base.saturating_mul(2);
            self.build_levels[lane] = base;
            self.boosts[lane] = Some(SkillBoost::LastBoost(base));
            self.boosted[lane] = true;
            self.mark_hook_mutation(SKILL_DIRTY_LEVELS | SKILL_DIRTY_BOOSTS);
            return true;
        }
        false
    }

    /// 设置吞噬配对顺序；引用不存在的 lane 会 panic。
    pub fn with_merge_lane_order(mut self, merge_lane_order: impl IntoIterator<Item = usize>) -> Self {
        Arc::make_mut(&mut self.layout).merge_lane_order = merge_lane_order.into_iter().collect();
        assert!(
            self.layout.merge_lane_order.iter().all(|idx| *idx < self.layout.skills.len()),
            "runtime merge lane order must reference existing fixed lanes"
        );
        self
    }

    /// 按吞噬策略从来源合并技能等级；任一 lane 提升时返回 true。
    pub fn merge_fixed_lanes_from(&mut self, source: &Self, policy: MergePolicy) -> bool {
        match policy {
            MergePolicy::None => false,
            MergePolicy::FixedLane | MergePolicy::FixedLaneHalfResources => {
                // legacy 的 Merge 只会按双方 `slot_skill` 的位置配对；store 中额外注册的
                // 分摊伤害等技能不属于固定槽位，不能挤进普通技能槽位。
                self.merge_slot_levels_from(source.layout.merge_lane_order.iter().map(|&lane| source.levels[lane]))
            }
            MergePolicy::DropUnmappedSkills => {
                let mut changed = false;
                for owner_idx in 0..self.levels.len() {
                    let fixed_lane_key = self.layout.fixed_lane_keys[owner_idx];
                    let Some(source_idx) =
                        source.layout.fixed_lane_keys.iter().position(|source_key| *source_key == fixed_lane_key)
                    else {
                        continue;
                    };
                    changed |= self.merge_level_at(owner_idx, source.levels[source_idx]);
                }
                changed
            }
        }
    }

    /// 按固定槽位顺序合并来源等级，双方都存在的槽位才参与比较。
    pub fn merge_slot_levels_from(&mut self, source_levels: impl IntoIterator<Item = u32>) -> bool {
        let mut changed = false;
        for (position, level) in source_levels.into_iter().take(self.layout.merge_lane_order.len()).enumerate() {
            changed |= self.merge_level_at(self.layout.merge_lane_order[position], level);
        }
        changed
    }

    /// 把来源等级合并到指定 lane：仅在提升时写入；从 0 抬到正等级时重排行动序与 POST_DAMAGE 序，并作废 hook 缓存。
    fn merge_level_at(&mut self, owner_idx: usize, source_level: u32) -> bool {
        let owner_level = &mut self.levels[owner_idx];
        if source_level <= *owner_level {
            return false;
        }
        let was_zero = *owner_level == 0;
        *owner_level = source_level;
        self.mark_hook_mutation(SKILL_DIRTY_LEVELS);
        if was_zero {
            self.active_order.retain(|lane| *lane != owner_idx);
            self.active_order.push(owner_idx);
            // legacy 会在 Merge 把零级槽位抬为正等级时注册新钩子，因此同优先级的
            // post_damage 新钩子应排在原有活跃钩子之后，与固定槽位编号无关。
            self.post_damage_order.retain(|lane| *lane != owner_idx);
            self.post_damage_order.push(owner_idx);
            self.invalidate_hook_cache();
            self.battle_dirty |= SKILL_DIRTY_ACTIVE_HOOKS | SKILL_DIRTY_POST_DAMAGE;
        }
        true
    }

    #[inline]
    fn mark_hook_mutation(&mut self, dirty: u8) {
        self.battle_dirty |= dirty;
        self.hook_generation = self.hook_generation.wrapping_add(1);
    }
}
