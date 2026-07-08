# tswn_core 下一代核心重构计划

> 状态：计划文档
> 范围：`crates/tswn_core` 的 `engine`、`player`、`skill`、`state`、扩展 API 与 custom branch 迁移
> 目标：允许 breaking change 的激进高性能重构，同时保留二次开发迁移路径

---

## 1. 背景与目标

当前 `tswn_core` 主线已经做过多轮热路径优化，包括目标选择降分配、hook 空路径跳过、`PlayerStateStore` 存储整理、固定 30-case 性能基准建设等。这些优化有效，但仍然建立在现有架构上：

- `EngineCore + Storage + WorldState` 分散维护战斗状态；
- `Storage` 通过 `UnsafeCell` 暴露跨实体内部可变性；
- `SkillTrait / StateTrait / SkillArgs / OnDamageFunc` 把过宽能力传入技能和状态；
- `Player` 同时承担构造数据、身份数据、运行时状态、技能、状态、武器、overlay 等职责；
- 内置技能与动态技能都主要走 trait object 宽接口；
- 部分正确性仍依赖 nightly 下的 `mutable-noalias=no` 规避 LLVM 对 `&mut` noalias 的优化假设。

这份计划的目标不是继续做小步局部优化，而是设计一套下一代核心：

- 热路径以 dense arena、阶段调度、静态技能元数据、effect pipeline、scratch buffer 复用为主；
- 内置技能走高性能静态路径；
- 自定义技能、boss、玩家类型、replay 展示等二次开发能力走明确扩展层；
- breaking change 允许发生在 Rust 内部 API 和扩展 API 上；
- CLI 行为、JS/Dart 对齐语义、RC4 消费顺序、replay/winner/score 不应因架构重写而改变。

额外约束：`github/custom` 已经存在二次开发分支，包含 bed2 玩家类型、HP marker、召唤/使魔/merge 行为、replay 显示和大量 runner 测试。新架构必须能让这类分支迁移到稳定扩展 API 上，而不是迫使二开继续 fork `engine/player/skill` 内部结构。

---

## 2. 现状问题

### 2.1 `Storage` 与别名边界

当前 `Storage` 用 `UnsafeCell` 持有 players、groups、alive groups、pending queues 等运行时数据。这个设计让技能和状态可以从 `Arc<Storage>` 中重新取出任意 `Player` 的可变引用，但也导致：

- owner 自己持有 `&mut self` 时，技能/状态仍可通过 `storage.just_get_player_mut(owner)` 再次取得 owner；
- `damage()` 持有 target 的 `&mut self` 时，`on_damage` 回调可以重新取 target 或 caster；
- 正确性边界依赖运行期 discipline，而不是 Rust 类型系统；
- 想移除 `mutable-noalias=no` 时，必须先消除这些同实体重借路径。

`docs/analysis/storage_refactor.md` 中已经把这类风险拆成 owner-phase alias、target/caster phase alias、staged damage、阶段化 context、split components、EventQueue 等方案。下一代架构应直接吸收这些结论，不再把 `Arc<Storage>` 作为默认扩展能力传给所有回调。

### 2.2 `Player` 大对象问题

当前 `Player` 是战斗实体聚合根，集中持有：

- 身份与显示：名字、队伍、显示名覆盖、ID；
- 构造数据：name base、skill id/prop、overlay、weapon；
- 运行时状态：HP/MP/move point、属性、flags；
- 技能容器与状态容器；
- 召唤物、boss、DIY/OL 等特殊运行时逻辑。

这让代码调用直观，但对热路径不友好：

- 每场或每次 prepared runner clone 时，大对象深拷贝成本高；
- 很多路径只需要 `PlayerStatus` 或 team/alive 信息，却必须借整个 `Player`；
- 自定义分支容易通过往 `Player` 或 `PlayerType` 加字段/枚举值扩展功能，导致核心结构持续膨胀；
- 二次开发与核心热路径耦合过紧。

### 2.3 技能和状态接口过宽

当前 `SkillTrait` 负责主动行动、目标选择、概率、pre/post action、pre/post defend、post damage、die/kill、clear positive、update state 等大量阶段。`SkillArgs` 又直接携带 `PlrId + RC4 + RunUpdates + Arc<Storage>`。

问题：

- 内置技能每次扫描都要通过 trait object 查询 action/proc/priority/target 等元数据；
- custom 技能和内置技能共享同一热路径成本；
- hook 分发常需要临时收集、排序、clone key 列表；
- `Vec<PlrId>` 返回值导致目标选择边界容易分配；
- 扩展能力太大，二开可以直接改任意 player/storage 状态，难以保证时序和别名安全。

### 2.4 世界同步与目标选择敏感

`WorldState` 当前维护 round order、teams、alive、flat_alive、alive set、player team、alive_group_count 等多份派生索引。它有不少必须保留的 JS 兼容细节：

- `flat_alive` 是目标选择顺序来源，不能从 teams 临时重建；
- `round_pos` 在 remove 时有特殊 splice 语义；
- `alive_group_count` 不是普通“当前非空队伍数”；
- pending spawn/revival 在某些路径需要提前可见；
- linked minion、owner death、share damage、death queue 顺序会影响 replay 和分叉点。

下一代架构应让世界顺序只有一个权威来源，同时把这些敏感点固化成测试。

---

## 3. 目标架构

新增 `CombatRuntime` 作为战斗核心，替代当前松散的 `EngineCore + Storage + WorldState` 组合。

```rust,ignore
pub struct CombatRuntime {
    entities: EntityArena,
    world: WorldArena,
    scheduler: PhaseScheduler,
    effects: EffectQueue,
    scratch: BattleScratch,
    rng: RC4,
    updates: RunUpdates,
    extensions: ExtensionRegistry,
}
```

### 3.1 `EntityArena`

`EntityArena` 是所有实体运行时数据的权威存储：

```rust,ignore
pub struct EntityArena {
    runtime: Vec<PlayerRuntime>,
    skills: Vec<SkillLoadout>,
    states: Vec<StateStore>,
    identity: Vec<PlayerIdentityRef>,
    extension_slots: Vec<EntityExtensionSlots>,
    id_to_idx: FastHashMap<PlrId, EntityIdx>,
}
```

设计选择：

- `PlrId` 保留为外部稳定 ID，用于 replay、updates、CLI、JS 对齐；
- `EntityIdx(u32)` 是单场战斗内部 dense index，用于热路径；
- 单场内实体 slot 不复用，避免 spawn/remove/revive 改变 replay 语义；
- 冷数据和热数据拆分，减少 run-to-completion 的 cache 压力。

### 3.2 `WorldArena`

`WorldArena` 统一维护战场顺序和队伍视图：

```rust,ignore
pub struct WorldArena {
    round_order: Vec<EntityIdx>,
    round_pos: usize,
    teams: Vec<TeamRuntime>,
    flat_alive: Vec<EntityIdx>,
    alive_set: GenerationMarkSet,
    player_team: Vec<TeamId>,
    alive_group_count_js: usize,
    pending: PendingQueues,
}
```

设计选择：

- `round_order` 和 `flat_alive` 是顺序真相；
- `teams.alive` 只作为 view/cache，不反向决定 target order；
- pending spawn/revival/remove/death 通过 `RuntimeSyncDelta` 应用；
- `alive_group_count_js` 命名上明确这是 JS 兼容计数，而非普通派生值。

### 3.3 `PhaseScheduler`

`PhaseScheduler` 负责阶段化执行：

```text
tick
  -> sync pending runtime entities
  -> select next actor
  -> pre action hooks
  -> choose action
  -> target selection
  -> action/effects
  -> damage/defend/death phases
  -> run_update_end
  -> sync pending runtime entities
  -> winner check
  -> post action hooks
```

调度器的目标是：

- 所有 hook 都有明确 phase；
- phase 内只暴露最小能力 context；
- 跨实体副作用不直接借全局 mutable storage，而是进入 effect queue；
- 内置 hook plan 可缓存，动态扩展 hook 只在实际注册后进入 fallback。

### 3.4 `EffectQueue`

所有跨实体副作用都表达成 effect：

```rust,ignore
pub enum Effect {
    Damage(DamageEffect),
    Heal(HealEffect),
    AddState(AddStateEffect),
    ClearState(ClearStateEffect),
    Spawn(SpawnEffect),
    Revive(ReviveEffect),
    Remove(RemoveEffect),
    Replay(ReplayEffect),
    Custom(CustomEffect),
}
```

设计选择：

- effect 不是异步乱序队列；默认在当前 phase 的指定 flush 点按原语义立即处理；
- 伤害链用 effect pipeline 替换旧 `OnDamageFunc`；
- summon share damage、boss infection、absorb heal、poison tick 等都应迁移成 effect；
- custom effect 通过 extension registry 声明 handler。

### 3.5 `BattleScratch`

热路径临时数据统一复用：

```rust,ignore
pub struct BattleScratch {
    targets: TargetBuf,
    scores: ScoreBuf,
    hooks: HookBuf,
    effects: SmallVec<[Effect; 8]>,
    clear_states: SmallVec<[StateKindId; 4]>,
}
```

目标：

- 目标选择常见 1v1/2v2 不分配；
- post_defend/post_damage 不反复构造 heap Vec；
- score buffer、skip indices、pending view 等可复用；
- no-capture benchmark 路径不构造显示字符串。

---

## 4. 核心数据模型

### 4.1 Player 拆分

`Player` 不再作为战斗热路径聚合根，而是拆成：

```rust,ignore
pub struct PlayerTemplate {
    identity: PlayerIdentity,
    build: PlayerBuildData,
    initial_runtime: RuntimeInit,
    extension_data: ExtensionBlob,
}

pub struct PlayerRuntime {
    status: PlayerStatus,
    flags: PlayerFlags,
    move_state: MoveState,
    kind: PlayerKindId,
    team: TeamId,
}

pub struct PlayerIdentity {
    raw_name: String,
    display_name: Option<String>,
    clan_name: Option<String>,
    id_name_override: Option<String>,
}
```

默认取舍：

- `name_base` 改为 `[u8; 128]`；
- 固定 40 技能槽改为紧凑数组或 boxed slice；
- identity/display/export/DIY 走冷路径；
- `Player` 旧构造 API 可作为 facade 存在，但不再是 runtime 内部核心类型。

### 4.2 Player Kind 扩展

替换不可扩展的 enum 风格：

```rust,ignore
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct PlayerKindId(u16);

pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub name: &'static str,
    pub build_hooks: PlayerBuildHooks,
    pub phase_hooks: PlayerKindPhaseHooks,
    pub replay: PlayerReplaySpec,
}
```

用途：

- 内置 Normal/Boss/Minion/Shadow/Zombie 等用固定 kind；
- custom branch 的 bed2 player type 通过 `PlayerKindSpec` 注册；
- 不再要求二开修改核心 `PlayerType` enum。

### 4.3 Skill 元数据

```rust,ignore
pub struct SkillMeta {
    pub id: SkillId,
    pub name: &'static str,
    pub export_name: &'static str,
    pub proc_mask: ProcMask,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
    pub ops: SkillOps,
}

pub enum SkillRuntime {
    Builtin(BuiltinSkillRuntime),
    Custom(CustomSkillBox),
}
```

目标：

- 内置技能扫描时先看 `SkillMeta`，避免通过 trait object 问 `has_action_impl/proc_kinds/priority`；
- 统一生成 factory、名字解析、export 名、DIY active/passive 列表；
- custom 技能仍可注册，但走 fallback；
- 内置高频技能可逐步迁移到 `SkillKind + runtime payload` 静态分发。

### 4.4 State Store

```rust,ignore
pub struct StateStore {
    entries: SmallVec<[StateEntry; 8]>,
    index: StateIndex,
    hook_mask: ProcMask,
    generation: u32,
    cached_plans: CachedStatePlans,
}
```

目标：

- 常见状态数量小，优先 small storage；
- hook plan 按 generation 懒重建；
- 同优先级仍按 JS 注册 order；
- clear list 返回 `SmallVec`，避免 0-2 个清理项也分配。

---

## 5. 扩展与二次开发 API

### 5.1 新增 `extension` 模块

公开稳定扩展入口：

```rust,ignore
pub trait TswnExtension {
    fn name(&self) -> &'static str;
    fn version(&self) -> ExtensionVersion;
    fn register(&self, registry: &mut ExtensionRegistryBuilder);
}

pub struct ExtensionRegistryBuilder {
    pub fn register_player_kind(&mut self, spec: PlayerKindSpec);
    pub fn register_skill(&mut self, meta: SkillMeta, factory: CustomSkillFactory);
    pub fn register_state(&mut self, spec: StateSpec);
    pub fn register_effect_handler(&mut self, kind: EffectKind, handler: EffectHandler);
    pub fn register_replay_renderer(&mut self, renderer: ReplayRendererHook);
}
```

要求：

- 扩展不能直接持有或访问 `Arc<Storage>`；
- 扩展不能要求 `&mut Player` 贯穿跨实体调用；
- 扩展通过 context 读取世界状态，通过 effect 修改跨实体状态；
- 无扩展时默认 main 热路径只检查 bitmask，不进入 dyn dispatch；
- extension crate 只能依赖公开扩展模块和 facade，不依赖 `engine` 内部实现细节。

### 5.2 阶段化 Context

替换旧 `SkillArgs` / `OnDamageFunc`：

```rust,ignore
pub struct ActionCtx<'a> {
    pub actor: EntityIdx,
    pub rng: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
    pub world: WorldView<'a>,
    pub targets: TargetView<'a>,
    pub effects: &'a mut EffectQueue,
    pub scratch: &'a mut BattleScratch,
    pub ext: ExtensionView<'a>,
}

pub struct DamageCtx<'a> {
    pub caster: EntityIdx,
    pub target: EntityIdx,
    pub rng: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
    pub world: WorldView<'a>,
    pub effects: &'a mut EffectQueue,
    pub ext: ExtensionView<'a>,
}
```

能力分层：

- `WorldView`：只读 alive、team、pending、round order；
- `EntityMut`：只修改当前 phase 安全开放的 owner/target 字段；
- `EffectQueue`：提交跨实体副作用；
- `ExtensionView`：访问扩展自己的 typed slot。

### 5.3 Extension Typed Slots

扩展不能直接往核心 struct 加字段，而应保留 typed slot：

```rust,ignore
pub struct ExtensionSlot<T> {
    id: ExtensionSlotId,
    _marker: PhantomData<T>,
}

impl ExtensionRegistryBuilder {
    pub fn reserve_entity_slot<T: 'static>(&mut self, name: &'static str) -> ExtensionSlot<T>;
    pub fn reserve_battle_slot<T: 'static>(&mut self, name: &'static str) -> BattleSlot<T>;
}
```

用途：

- custom player kind runtime state；
- summon/merge/minion policy 数据；
- replay 展示配置；
- bed2 HP marker；
- 扩展私有 flags 和临时状态。

### 5.4 Custom Branch 迁移映射

`github/custom` 当前主题应迁移为：

| custom branch 改动 | 新架构落点 |
| --- | --- |
| `feat: add bed2 player type` | `register_player_kind(PlayerKindSpec)` |
| configurable bed2 HP marker | `ReplayRendererHook + PlayerReplaySpec` |
| bed2 summon flow / recast | `SummonPolicy + EffectHandler` |
| bed2 summon damage handling | `DamageSharePolicy + DamageEffectHook` |
| route summon clone damage to root owner | `OwnerResolutionPolicy` |
| map merge skills to minion lanes | `MergePolicy / SkillLanePolicy` |
| drop unmapped skills on summon merge | `MergePolicy` |
| remove minion heal sharing | `DamageSharePolicy` 或 player kind policy |
| wasm replay HP report | `ReplayRendererHook` |
| custom runner tests | extension fixture tests |

迁移目标不是把 custom 分支硬合进 main，而是让 custom 以扩展包形式表达这些规则。

---

## 6. 兼容桥

### 6.1 Legacy Adapter

为迁移保留一个短期 adapter：

```rust,ignore
pub trait LegacySkillAdapter {
    fn legacy_act(&mut self, ctx: LegacyActionCompatCtx);
    fn legacy_post_damage(&mut self, ctx: LegacyDamageCompatCtx);
}
```

约束：

- adapter 只存在一个迁移周期；
- adapter 不重新暴露 `Arc<Storage>`；
- adapter 用新 context 模拟旧 `SkillArgs` 常见能力；
- adapter 默认不进入内置技能热路径；
- adapter 的目的是让 custom 分支先编译、再逐个迁移成原生 extension API。

### 6.2 旧接口迁移表

| 旧接口 | 新接口 |
| --- | --- |
| `register_skill_factory(id, factory)` | `register_skill(meta, factory)` |
| `SkillTrait::act(Vec<PlrId>, ..., SkillArgs)` | `SkillOps::act(TargetBuf, ActionCtx)` |
| `StateTrait::post_damage(..., SkillArgs)` | `StateSpec + PhaseHook::PostDamage` |
| `OnDamageFunc` | `DamageEffectHook` |
| `BossHandler` | `PlayerKindSpec + PhaseHook + EffectHandler` |
| `HookPipeline` | `ExtensionRegistry` engine phase hook |
| `PlayerType` enum 扩展 | `PlayerKindId` registry |
| replay 特殊显示 | `ReplayRendererHook` |
| summon/merge 特化 | `SummonPolicy / MergePolicy` |

---

## 7. 实施阶段

### 阶段 A：扩展契约先行

先定义未来稳定扩展 API，不急着重写热路径：

- 新增 `extension` 模块和 `ExtensionRegistryBuilder`；
- 给现有 `register_skill_factory / register_boss_handler / HookPipeline` 标记 legacy；
- 新增 `docs/extension_migration.md`，以 bed2/custom branch 作为迁移样例；
- 定义 `PlayerKindId / SkillId / StateKindId / EffectKind` 的编号策略；
- 明确 extension crate 不能依赖 `engine` 内部模块。

完成标准：

- main 编译通过；
- custom branch 现有改动点都能在迁移文档里找到新落点；
- 不要求本阶段有性能收益。

### 阶段 B：Runtime v2 骨架

- 新增 `runtime_v2`；
- 实现 `CombatRuntime / EntityArena / WorldArena / EffectQueue / BattleScratch`；
- legacy runtime 保留，可并行 diff；
- `PreparedCombatTemplate` 从旧 prepared template 派生；
- `RuntimeTrace` 记录 actor、phase、effect、RC4 checkpoint、update frame。

完成标准：

- v2 可跑最小 `1v1 a vs b`；
- winner、round count、updates frame 数与 legacy 一致；
- 测试可同输入跑 legacy/v2 diff。

### 阶段 C：World / Scheduler 替换

- `sync_runtime_entities` 改为 `PendingQueues -> RuntimeSyncDelta -> WorldArena.apply_delta()`；
- 固定同步顺序：revival → roster revived scan → spawn → death_queue → pending_remove → fallback；
- `flat_alive` 成为目标选择唯一源；
- `TargetView + BattleScratch` 替代热路径临时 `Vec`；
- pending spawn 提前可见逻辑固化为 `WorldArena` API。

完成标准：

- engine runner tests 与 `engine_core` 等价用例通过；
- custom branch 中新增 runner case 可迁移为共享 fixture；
- `ActionTargets` 顺序 golden 覆盖 charm、pending spawn、pending revival、EnemyAlive skip。

### 阶段 D：Player Runtime 拆分

- 拆 `PlayerTemplate / PlayerRuntime / PlayerIdentity`；
- `PlayerKindSpec` 接管 boss/bed2/custom type 差异；
- summon/minion/clone/revive 改为 template/runtime 双层构造；
- `PlayerOverlay` 解析输出到 `PlayerTemplate` 和 extension slots；
- `to_diy / to_ol_json / replay display` 通过 cold facade 实现。

完成标准：

- DIY/OL roundtrip 不退步；
- bed2 这类 custom player kind 能用 registry 表达；
- 战斗循环不再 clone 完整 `Player` 大对象。

### 阶段 E：Skill / State 元数据化

- 引入 `SkillMeta` 总表；
- 内置技能迁到 `BuiltinSkillRuntime` 静态分发；
- `CustomSkillBox` 支持扩展技能；
- `StateStore` 改 dense entries + generation cached hook plan；
- `post_action / post_defend / post_damage` 使用 cached phase plan。

完成标准：

- 内置技能扫描不再依赖宽 trait object；
- custom 技能可注册并参与 action/defend/damage/death phase；
- 同优先级 `order`、post_action cursor、post_defend skill/state 合并顺序保持一致。

### 阶段 F：Effect Pipeline 替换伤害链

- 伤害链改为 `DamageEffect -> apply_damage_core -> on_damage hooks -> on_damaged -> post_damage -> death phase`；
- 吸血、冰冻、中毒、感染、净化、使魔分摊、boss 特效迁为 effect；
- `SummonPolicy / MergePolicy / DamageSharePolicy` 作为扩展点开放；
- `clear_positive_runtime`、owner self-modify 不再通过 storage 重借 owner。

完成标准：

- 技能/状态实现不再调用 `just_get_player_mut`；
- `SkillArgs` 和 `OnDamageFunc` 从新 runtime 消失；
- custom branch 的 summon/merge/minion damage 行为可用 policy/effect 迁移。

### 阶段 G：删除 Legacy Runtime

- `EngineCore` facade 指向 `CombatRuntime`；
- 删除或隔离旧 `Storage`；
- 删除 `mutable-noalias=no` 的架构必要性；
- legacy adapter 保留一个版本周期后移除；
- 更新 architecture、extension migration、performance、breaking changes 文档。

完成标准：

- no_debug release 不包含 legacy 对照路径；
- extension examples 覆盖 custom branch 关键能力；
- changelog 明确标注 breaking API 和迁移路径。

---

## 8. 行为边界

以下语义不可改变：

- RC4 消费顺序，包括 smart、prob、空目标、单目标 score、EnemyAlive、dodge、mp、boss prob；
- `flat_alive` 目标选择顺序；
- `round_pos` remove/splice 调整语义；
- `alive_group_count` 的 JS 兼容语义；
- pending spawn/revival 的提前可见性；
- `sync_runtime_entities` 的 revival/spawn/death/remove/fallback 顺序；
- `run_update_end` 的 `mem::take` 分批和 64 guard；
- 行动中已决胜时跳过 recover/newline/post_action；
- post_action early/state/deferred/late 混排；
- post_defend skill/state priority 合并；
- protect 的 effective group、split pre_defend、pending target 规则；
- assassinate 的 pending target、forced pre_action、空 target act 语义；
- linked minion / owner death / share damage 顺序；
- DIY clone、SkillBoost、slot_skill、merge lane 语义；
- CLI/replay 输出字段和排序，除非另行作为 breaking output 记录。

---

## 9. 验证计划

### 9.1 Main 基础测试

```powershell
cargo test -p tswn_core --lib
cargo test -p tswn_core --test engine_core
cargo test -p tswn_core --bin tswn-cli
cargo test -p tswn_core --features no_debug --lib
python track_test.py -q
```

### 9.2 Legacy/V2 Diff

新增测试工具，同输入同时跑 legacy 和 v2，比较：

- winner；
- round count；
- score；
- updates frame 数；
- 前 N 条 update；
- RC4 checkpoint；
- `flat_alive` 和 `round_pos`；
- pending queue flush 点。

### 9.3 Custom 迁移基线

不直接把 custom branch 代码合进 main，但要提取其行为为 fixture：

- bed2 player type；
- bed2 HP marker；
- summon recast；
- merge lane mapping；
- minion damage/share behavior；
- wasm replay HP report；
- custom runner large/fight_multi case。

每个 custom 主题至少要有一个 extension example 或 migration fixture。

### 9.4 JS/Rust 对账

分层运行：

```powershell
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --max-cases-per-mode 64 --keep-going
cargo run -p tswn_core --bin track_diy_roundtrip -- --quiet --max-cases-per-mode 64 --keep-going
```

最终切换前运行：

```powershell
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --modes 1v1,2v2,3v3v3,ffa --ffa-sizes 4,6,8 --case-offset-per-mode 0 --max-cases-per-mode 2000 --keep-going
```

验收：

- 不新增 diff signature；
- 已知失败 idx 不提前；
- fixed case legacy/v2 完全一致后才能删除 legacy。

### 9.5 性能基准

主基准：

```powershell
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_perf_cases -- --case-dir docs/perf/fixed_cases_30 --out-dir target/perf_cases_v2_t1 --bench-runs 13000 --thread 1 -q
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_perf_cases -- --case-dir docs/perf/fixed_cases_30 --out-dir target/perf_cases_v2_t0 --bench-runs 13000 --thread 0 -q
```

目标：

- 无扩展、内置技能路径：`core_1v1_2v2` 中位数改善 20%+；
- `overall` 中位数改善 15%+；
- `stress_multi` 不退步，目标改善 10%+；
- 默认 main 路径不被 custom fallback 拖慢；
- no-capture fight path 不做 display string formatting；
- batch-rate / pair / score 长跑内存稳定，无全局 cache 线性增长。

---

## 10. 验收标准

最终合并前必须满足：

- 新 runtime 在固定 legacy/v2 diff case 上行为完全一致；
- `track_test.py -q` 无新增退步；
- JS/Rust miner 不新增 diff signature；
- DIY/OL roundtrip 不退步；
- custom branch 的 bed2 关键行为能通过 extension API 表达；
- 无扩展 main 默认路径不进入 dyn extension dispatch；
- `SkillArgs / OnDamageFunc / Arc<Storage>` 不再是新 runtime 的扩展能力边界；
- `mutable-noalias=no` 不再是架构正确性的必要条件；
- 文档包含 extension migration、breaking change、性能基准解释；
- changelog 用中文记录重构影响和迁移建议。

---

## 11. 默认取舍

- 不引入外部 ECS 框架，采用手写 dense arena；
- 不在单场战斗内部并行，保持 deterministic；
- 内置技能为性能服务，扩展技能为二开友好服务，两条路径分层；
- breaking change 优先发生在 Rust 内部 API，不主动破坏 CLI/JS 行为；
- custom branch 迁移优先通过 extension API，不鼓励继续 fork engine/player 内部结构；
- 行为对账优先于性能目标，性能通过 layout/cache/specialization 逐步回收；
- 先设计扩展契约，再重写 runtime，避免重构完成后 custom branch 无迁移出口。
