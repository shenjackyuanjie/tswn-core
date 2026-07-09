# tswn_core Runtime v2 核心重构实施规格

> 状态：实施规格
> 范围：`crates/tswn_core` 的 runtime、engine、player、skill、state、wasm/show、extension/custom 迁移
> 第一优先级：消除 UB 风险，并与当前 legacy/md5.js 结果严格一致
> 兼容策略：不保留旧 Rust/extension/CLI/wasm API 兼容，只保最终呈现结果与归一化帧级行为一致

---

## 1. 目标与非目标

这次重构不是继续在旧 `EngineCore + Storage + WorldState + Player + SkillTrait` 上做局部优化，而是建设 `CombatRuntime` v2：

- 消除旧 `Storage`/`UnsafeCell` 风格同实体和跨实体重借带来的 UB 风险；
- 移除架构上对 `mutable-noalias=no` 的必要依赖；
- 以当前 legacy Rust 为直接 oracle，保持与 md5.js 已对齐的行为结果；
- 用 arena、静态内置技能路径、phase scheduler、effect pipeline、scratch buffer 复用提升默认路径性能；
- 让 `github/custom` 作为准产品线迁移到 repo 内 experimental extension example/fixture；
- 允许 Rust public API、extension API、CLI/wasm 输入结构、DIY/OL schema、replay schema breaking。

非目标：

- 不保留 `SkillArgs`、`OnDamageFunc`、`Arc<Storage>`、旧 `SkillTrait`/`StateTrait` 作为新 runtime 扩展能力边界；
- 不提供 Legacy Adapter；
- 不承诺旧 `Player` 构造 API、旧 wasm API、旧 replay JSON、旧 DIY/OL JSON 的结构兼容；
- 不为了修正有感 legacy/md5.js 历史行为而改变帧级输出或 RNG 顺序。

---

## 2. 决策表

| 主题 | 决策 |
| --- | --- |
| 第一优先级 | UB 安全与结果一致都不可牺牲；性能和扩展能力排在二者之后。 |
| 行为 oracle | 以当前 legacy Rust 为 v2 直接 oracle；当前 legacy 已与 md5.js 一致。若后续发现 legacy 与 md5.js 不一致，本计划内 legacy 优先，md5.js 差异另开专项。 |
| 结果粒度 | 归一化 replay/update 帧级序列、winner、score、RNG 全量严格一致；原始 JSON 结构可变。 |
| RNG | 内置路径必须与 legacy/md5.js 完全一致；extension/custom RNG 通过受控 API；内置热路径可直接调用 RC4 保性能。 |
| 旧 bug | 只有不改变帧级展示和 RNG 时才修；有感变化不在本次重构中修。 |
| API 兼容 | Rust public API、extension API、CLI/wasm 输入结构都可 breaking；只保最终结果和展示。 |
| Legacy Adapter | 不做。旧技能、状态、custom 行为直接按新 API / 新 runtime 迁移。 |
| 双栈 | legacy/v2 只用于开发对账；v2 四项门槛过后切换即删 legacy。 |
| `mutable-noalias=no` | 最终必须移除，并证明 v2 不依赖它。 |
| unsafe | 性能优先但必须安全；unsafe 集中在少数模块，写专门设计说明，核心测试全量 Miri。 |
| extension 稳定性 | 先 experimental；v2 切换并验证 custom 后再考虑稳定。 |
| custom 地位 | `github/custom` 是准产品线；main 重构必须承担关键行为迁移。 |
| custom 审计 | 以 `github/custom` 相对 main 的 git diff 为准，逐项归类到 kind/skill/effect/replay/runner。 |
| custom 交付 | custom 作为 repo 内 extension example/fixture 跟 main 一起测试。 |
| extension 能力 | 支持自定义 skill、skill 行为、player type；默认局部可读，custom 需要跨实体时通过 capability 逐项放开。 |
| custom 性能 | 第一版只做通用 extension；不建专门 custom 快路径，热点后续凭数据优化。 |
| ID 策略 | 内置 ID 固定；extension 通过 namespace 动态分配，动态 ID 只要求单次 registry/build 内稳定；name/export_name 冲突报错。 |
| EntityIdx | 混合复用：默认不复用，只有经 strict diff 证明无影响的实体类别白名单复用；u32 上限溢出直接 panic。 |
| revive | 复活身份语义按 md5.js/legacy 保持。 |
| cold data | battle 构造时复制必要冷数据，runtime 自给自足。 |
| Player facade | 保留一个输入解析/测试辅助外壳；不是稳定 API，不参与 runtime 热路径；正式 runtime 输入是 `PreparedCombatTemplate`。 |
| DIY/OL | schema 可重做；只要求归一化 roundtrip / 展示结果一致。 |
| wasm/show | wasm API 可 breaking；`show.html` 迁到 v2 replay schema，以视觉可用和核心 golden 为准。 |
| 技能槽 | 内部可变槽；导入/导出/replay 层模拟旧槽位语义。 |
| PlayerKind | 单一 `PlayerKindId` + 可组合 policy/flags，避免 kind 爆炸。 |
| Skill 路径 | 内置技能全部迁到静态路径；custom 走 registry/extension fallback。 |
| Skill 迁移顺序 | 行为优先；先迁移行为敏感技能并建立 diff 基线，再做性能专门化。 |
| 目标选择 | 内部过程、RNG 节点、外显目标必须完全复刻 legacy/md5.js。 |
| StateStore | `SmallVec`/dense index 优先，同时保留 legacy 注册顺序作为行为键。 |
| hook 顺序 | 新统一 priority/order 是设计目标；若与 strict diff 冲突，以归一化帧级/RNG 一致为准。 |
| hook plan 可见性 | phase 中状态变化必须立即影响后续 hook plan。 |
| EffectQueue | 可内部批处理，但 flush 后必须逐项模拟 legacy/md5.js 顺序；嵌套 effect 深度优先。 |
| CustomEffect | 可改实体和提交 update，但 RNG 必须走受控 API。 |
| Scheduler | 可定义新的清晰 phase；pending 可见性逐点复刻 legacy/md5.js。 |
| WorldArena | 复刻 md5.js 数据结构语义，不强行统一成单一世界真相。 |
| Context | 少数通用 ctx 类型 + method/capability 限制；内置热路径可直接改实体，extension 通过受控 API/effect。 |
| error model | extension 边界 `Result`；内置热路径用 panic/debug assert；非法 effect 所有模式 panic；extension panic 在 runner/wasm 边界捕获。 |
| Trace | 可选诊断能力，默认无 trace 路径零成本；trace 粒度 frame 级。 |
| 并行 | 单场内只允许无 RNG、无 update 的纯 score/候选目标等局部纯算并行。 |
| 依赖 | 手写优先；只有原型证明明确性能收益才引入新依赖；不默认引入 ECS。 |
| 分支策略 | v2 长期分支隔离开发，main 漂移末期同步解决。 |
| 版本 | 继续 0.x 发布，切换时 bump `0.x+1`。 |
| 文档 | 用户 changelog 简洁；开发者 migration guide 详细。 |

---

## 3. 行为与 oracle 规格

### 3.1 Strict diff 是第一阶段

实施顺序必须先做 oracle/strict diff，再写 v2 runtime。v2 不能用“最终看起来差不多”作为验收依据。

strict diff 比较：

- 归一化 replay/update 帧级序列；
- winner；
- score；
- RNG 消费序列或等价 checkpoint；
- action/frame 边界；
- custom golden 中 bed2/summon/merge/minion/replay 关键行为。

不要求比较：

- 原始 JSON 字段名和嵌套结构；
- v2 内部 `WorldArena`、`EntityArena`、scratch buffer 的存储形态；
- 非 trace 构建中的内部 checkpoint 数据。

任何 strict diff 失败都阻塞切换，不设 allowlist。

### 3.2 Oracle 权威

- 当前 legacy Rust 是 v2 直接 oracle；
- md5.js 是背景权威，因为当前 legacy 已经与 md5.js 一致；
- 若后续发现 legacy 与 md5.js 不一致，本计划内不临时改 oracle，先按 legacy 继续，另开 md5.js 对齐专项。

### 3.3 有感行为不得改变

以下有感行为必须保持：

- RC4/RNG 消费顺序；
- 目标选择顺序和 score 过程；
- replay/update 归一化帧顺序；
- HP/MP、死亡、复活、召唤、merge、share damage 的展示结果；
- winner、score、round/frame 结果；
- `show.html` 可视化播放的核心帧表现。

旧行为中疑似 bug 的语义，只有在不改变上述结果和 RNG 时才允许修。

---

## 4. Runtime v2 架构

### 4.1 核心结构

新增 `CombatRuntime` 作为 v2 战斗核心：

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

正式 runtime 输入收束为：

```rust,ignore
pub struct PreparedCombatTemplate {
    players: Vec<PlayerTemplate>,
    registry: ExtensionRegistry,
    // battle-level init data
}
```

`Player` 只作为输入解析和测试辅助 facade，不是稳定 API，也不进入热路径。

### 4.2 EntityArena

```rust,ignore
pub struct EntityArena {
    runtime: Vec<PlayerRuntime>,
    skills: Vec<SkillLoadout>,
    states: Vec<StateStore>,
    identity: Vec<PlayerIdentity>,
    extension_slots: Vec<EntityExtensionSlots>,
    id_to_idx: FastHashMap<PlrId, EntityIdx>,
}

pub struct EntityIdx(u32);
```

规则：

- `EntityIdx` 默认单场不复用；
- 只有 strict diff 证明无影响的实体类别可进入复用白名单；
- `EntityIdx(u32)` 溢出视为不可恢复 bug，直接 panic；
- revive 身份语义按 md5.js/legacy；
- remove 后是否保留 tombstone 由 strict diff 和 replay 需求决定；
- battle 构造时复制必要 identity/display/export 冷数据，runtime 自给自足。

### 4.3 Player 拆分

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
```

要求：

- 输入长度沿用现有限制；构造出来的 name/display 不应超限；
- 不新增超长 fallback；
- 技能槽内部可变长度，但导入/导出/replay 层模拟旧 slot/merge 语义；
- DIY/OL schema 可重做，只要求归一化 roundtrip / 展示结果一致。

### 4.4 PlayerKind

`PlayerKindId` 保持单一 kind，差异能力通过 policy/flags 组合：

```rust,ignore
pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub name: &'static str,
    pub policies: PlayerKindPolicies,
    pub hooks: PlayerKindHooks,
    pub replay: PlayerReplaySpec,
}
```

Boss、Minion、Bed2 等不要通过 kind 爆炸表达。需要组合语义时使用 kind + policy/flags。

### 4.5 WorldArena

`WorldArena` 不强行做单一真相模型，而是复刻 md5.js/legacy 的多份派生结构语义：

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

要求：

- pending spawn/revival/remove/death 的可见性逐点复刻 legacy/md5.js；
- `flat_alive`、`round_order`、`teams.alive` 的同步顺序由 strict diff 固化；
- `round_pos` remove/splice、`alive_group_count_js` 等历史语义不做有感清理。

### 4.6 PhaseScheduler

允许设计新的清晰 phase，但 phase 输出必须 strict diff 通过。

原则：

- phase boundary 是实现模型，不是可改变行为的理由；
- forced pre_action、assassinate 空 target、protect pending target、post_action 混排等行为必须通过归一化帧和 RNG 对账；
- 状态/技能变化对当前 phase 后续 hook plan 立即可见；
- hook priority/order 可统一建模，但只要 diff 不过，就必须调整到等价 legacy 行为。

### 4.7 EffectQueue

`EffectQueue` 是同步 effect pipeline，不是异步乱序队列。

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

规则：

- 内部可批处理；
- flush 后必须逐项模拟 legacy/md5.js 顺序；
- effect handler 产生新 effect 时深度优先；
- 非法 effect、非法目标、非法 slot 视为不可恢复 bug，所有模式 panic；
- `CustomEffect` 可改实体和提交 update，但 RNG 必须走受控 API。

### 4.8 Context 与实体修改

Context 不做每个 phase 一个大类型爆炸，采用少数通用 ctx + method/capability 限制。

规则：

- 内置热路径可直接修改当前安全范围内实体；
- extension 默认只能局部读取 phase 相关实体和目标；
- custom 准产品能力需要跨实体信息时，通过 capability 逐项放开；
- extension 跨实体修改通过 effect 或受控 API；
- 旧 `Arc<Storage>`、`&mut Player` 跨实体能力不得重新暴露。

### 4.9 Trace

`RuntimeTrace` 是可选诊断能力。

- 默认无 trace 路径必须零成本或近似零成本；
- trace 粒度为 frame 级；
- trace 已记录 action/frame、RNG checkpoint、update、winner、score 汇总；
- phase/effect 深度 trace 不作为默认要求，可按调试需要追加。

---

## 5. Skill / State / Extension

### 5.1 内置 Skill 静态路径

内置技能全部迁到静态路径；custom 技能走 registry/extension fallback。

```rust,ignore
pub struct SkillMeta {
    pub id: SkillId,
    pub name: &'static str,
    pub export_name: &'static str,
    pub proc_mask: ProcMask,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
}

pub enum BuiltinSkillRuntime {
    // enum/match or equivalent static dispatch
}
```

迁移顺序：

1. 行为敏感技能优先；
2. 每组迁移后补 strict diff/golden；
3. 行为基线稳定后再做性能专门化。

目标选择的内部 score、RNG 节点、目标顺序必须完全复刻 legacy/md5.js。

### 5.2 StateStore

```rust,ignore
pub struct StateStore {
    entries: SmallVec<[StateEntry; 8]>,
    index: StateIndex,
    hook_mask: ProcMask,
    generation: u32,
    cached_plans: CachedStatePlans,
}
```

要求：

- `SmallVec`/dense index 优先；
- 保留 legacy 注册顺序作为行为键；
- phase 中状态变化立即影响后续 hook plan；
- clear、post_action、post_defend、post_damage 等顺序由 strict diff 固化。

### 5.3 Extension Registry

extension API 先标 experimental：

```rust,ignore
pub trait TswnExtension {
    fn name(&self) -> &'static str;
    fn version(&self) -> ExtensionVersion;
    fn register(&self, registry: &mut ExtensionRegistryBuilder) -> Result<(), ExtensionError>;
}
```

能力：

- register player kind；
- register skill / skill behavior；
- register state；
- register effect handler；
- register replay/show renderer；
- reserve typed slots；
- declare capability for broader custom reads.

规则：

- 内置 ID 固定；
- extension 用 namespace 动态分配 ID；
- 动态 ID 只需单次 registry/build 内稳定；
- name/export_name 冲突直接报错；
- 多 extension/hook/policy 使用 priority，同 priority 按注册顺序；
- handler 链式执行。

### 5.4 Typed Slots

支持三类 slot：

- template slot；
- battle slot；
- entity slot。

phase 临时数据走 `BattleScratch`，不做 phase slot。

slot 初始化失败只发生在构造/注册边界，返回 `Result`。prepared runner / batch clone 的 slot clone/reset 策略不预先固定：先实现显式 hook 与 `T: Clone` 两种原型，用 microbench/runner clone 数据选择更快方案。

---

## 6. Custom 准产品线迁移

### 6.1 审计方法

以 `github/custom` 相对 main 的 git diff 为准，逐项归类：

- player kind / player policy；
- skill / skill behavior；
- effect / damage / summon / merge policy；
- replay/show/HP marker；
- runner / large / fight_multi fixture。

审计产物必须列出：

- 原 custom 改动点；
- v2 extension 落点；
- strict diff 或 golden 验收 case；
- 是否需要 capability 例外；
- 是否需要后续性能优化。

### 6.2 迁移落点

| custom 主题 | v2 落点 |
| --- | --- |
| bed2 player type | `PlayerKindSpec` + policy/flags |
| bed2 HP marker | replay/show renderer + template/entity slot |
| bed2 summon flow / recast | summon policy + effect handler |
| summon clone damage route to root owner | owner resolution / damage policy |
| summon damage/share behavior | damage share policy + damage effect hook |
| merge lane mapping | merge policy / skill lane policy |
| drop unmapped skills on merge | merge policy |
| remove minion heal sharing | player kind policy or damage share policy |
| wasm replay HP report | v2 replay schema + show renderer |
| custom runner tests | repo 内 extension fixture + strict diff |

custom 第一版只通过通用 extension 实现，不建专门快路径。性能热点后续用 perf 数据决定是否专门化。

---

## 7. Wasm / Replay / DIY / OL

### 7.1 Replay schema

v2 replay 数据结构可重做，`show.html` 同步迁移到 v2 schema。

验收比较归一化帧序列，不比较旧 JSON 原始结构。归一化帧应覆盖：

- actor/action；
- visible HP/MP/status changes；
- damage/heal/state add/clear；
- death/revive/spawn/remove；
- summon/merge/custom display；
- winner/score/final frame。

### 7.2 show.html

`show.html` 以视觉可用为准：

- 可以迁移 wasm API；
- 可以迁移 replay schema；
- 需要少量核心 replay case 的 golden 视觉/DOM 测试；
- golden 覆盖加载、播放、关键帧展示，不要求大规模截图矩阵。

### 7.3 DIY / OL

DIY/OL schema 可随 v2 重做。

要求：

- 归一化 roundtrip 结果一致；
- 旧 slot/merge 语义在导入/导出/replay 层模拟；
- 需要 developer migration guide 说明新入口和旧结构破坏面。

---

## 8. Safety / Unsafe / 依赖 / 性能

### 8.1 UB 与 unsafe

新 runtime 必须从设计上消除旧 `Storage` 风格任意重借。短期桥接只允许在以下条件同时满足时存在：

- 明确游戏需求或性能需求；
- 封装边界小；
- 有 unsafe 设计说明；
- 核心 Miri 覆盖；
- strict diff 通过。

unsafe 策略：

- 可以为性能使用；
- 必须集中到少数模块；
- 每个 unsafe 模块写设计说明；
- 最终核心 tests 全量 Miri；
- 切换时移除 `mutable-noalias=no` 的架构必要性。

### 8.2 依赖

默认手写 `Vec` / `SmallVec` / `foldhash` 等直接结构。

新依赖引入流程：

1. 做小型原型或 microbench；
2. 证明有明确性能收益；
3. 确认不增加 strict diff 风险；
4. 再引入。

不默认引入 ECS 或完整调度框架。

### 8.3 并行

单场内只允许无 RNG、无 update 的纯计算局部并行，例如候选目标评分预计算。并行结果必须先收集，再按 legacy/md5.js 顺序消费。

第一版可以只预留接口，不强制启用单场内并行。

### 8.4 性能门槛

切换硬门槛是“不退步”，不是必须达到百分比提升。

必须覆盖：

- fixed cases；
- stress_multi；
- no_debug release；
- no-capture fight path；
- prepared runner / batch clone；
- custom extension fixture。

20%/15% 等提升目标可作为阶段优化目标，但不是 v2 切换阻塞项。

---

## 9. 实施阶段

### 提交粒度与提交信息

实施过程中必须按“完成一块可审查内容就提交一次”的方式推进，避免把多个独立阶段或多个子系统混在同一个 commit。

提交信息采用约定式提交，格式：

```text
feat(runtime): 中文一句话描述

- 具体修改点 1
- 具体修改点 2
- 验证方式或对账结果

Co-authored-by: Codex <codex@openai.com>
```

规则：

- 新能力使用 `feat(模块): 中文描述`；
- 修复行为、diff、测试或文档问题使用 `fix(模块): 中文描述`；
- 纯文档调整使用 `docs(模块): 中文描述`；
- 纯测试补充使用 `test(模块): 中文描述`；
- commit body 必须写清具体修改内容、涉及门禁、验证命令或未验证原因；
- 每个 commit 只覆盖一个清晰模块或一块行为闭环，例如 oracle、custom 审计、runtime 骨架、world/scheduler、skill/static path、effect pipeline、wasm/show 迁移。

### 阶段 A：Oracle 与 strict diff

- 建立 legacy/v2 对账框架；
- 定义归一化 replay/update 帧；
- 已在 v2 `NormalizedOutcome` / `strict_diff` 中记录并比较 winner、score 汇总、RNG checkpoint、entity/HP/alive、WorldArena 派生视图、action/frame；
- 接入 fixed golden、track_case_miner 大样本、custom golden；
- 明确任何 diff 失败阻塞切换。

完成标准：

- legacy 自身可生成归一化帧；
- fixed/custom golden 可稳定复跑；
- diff 输出能定位到 frame/action/RNG 节点。

### 阶段 B：custom diff 审计

- 已在 `docs/analysis/custom_runtime_v2_migration.md` 对 `github/custom` 相对 main 做首版 diff 归类；
- 已产出 custom 改动清单和 v2 落点；
- 已补 bed2 registry/template/import fixture，覆盖 `custom.bed2` kind、固定 summon skill、HP marker slot、`bed2[...]` / `@bed2` marker 最小 v2 导入、Player facade id-name 归一化桥接、typed summon template payload 读取后 spawn、`push_summon_from_template_slot` / `push_summon_from_template_slot_with_message` helper、grouped raw bed2 roster、mixed legacy/bed2 raw roster 到 `PreparedCombatTemplate` 的 helper，以及 `RuntimeV2Runner` 对 bed2-only / mixed roster、raw namerena fixture 形状、seed 初始 RNG 与 legacy 初始 world/order 的正式构造、单回合归一化和 run-until-winner 归一化入口；
- 已补 custom summon 复合 fixture，覆盖 root-owner 路由、owner/summon 伤害共享、spawn 后技能保留与 `push_summon_recast_from_entity_slot` 原实体复活复用，并固化 remembered summon 存活/缺少读取 capability 时不静默重建；同时补 `SpawnWithMessage` / `ReviveWithMessage` 路径，允许 summon handler 输出 legacy 的 `[0]使用[血祭]` + `召唤出[1]` 帧序列；
- 已补 custom minion owner cleanup fixture，覆盖 owner 致死或显式 remove 时 linked minion 按实体顺序死亡、移出 round/alive views 并输出消失帧；并补 `next_minion_name_from_entity_slot` / `minion_display_index_for_entity` helper，固化 root owner entity slot 计数、child minion 复用 root owner counter、legacy `?N` 展示序号解析、缺少 `ReadAllies` 时返回结构化错误；
- 已把 linked minion owner death cleanup 纳入 custom runner strict-diff golden，覆盖消失帧、winner 与 WorldArena 派生视图；
- 已把 merge 纳入 custom runner strict-diff golden，覆盖吞噬/属性上升帧、score 与 fixed-lane 技能槽继承；
- 已补 custom runner multi-round normalized run golden，覆盖 `RuntimeV2Runner::run_until_winner_normalized_rounds`、guard 状态、累计 score 与逐回合 strict diff；
- 已从 custom large / fight_multi 真实 raw 输入抽出初始化 parity golden，覆盖 v2 raw runner 对 legacy seed RNG、team 编号与 WorldArena 初始派生视图的对齐；
- 为关键行为设计 repo 内 extension fixture；
- 标出需要 capability 例外的跨实体读取点。

完成标准：

- bed2、summon、merge、minion、HP marker、wasm replay 行为都有验收 case；
- 每项 custom 关键行为都有 v2 extension 落点。

### 阶段 C：Runtime v2 骨架

- 新增 `CombatRuntime`、`PreparedCombatTemplate`、`EntityArena`、`WorldArena`、`PhaseScheduler`、`EffectQueue`、`BattleScratch`；
- 保留 `Player` 输入/测试 facade，但 battle start 前转换成 template；
- 实现最小 1v1，并接入 strict diff。

完成标准：

- 最小 case winner/score/frame/RNG 通过；
- v2 路径不暴露 `Arc<Storage>` 或旧 `SkillArgs` 能力。

### 阶段 D：World/Scheduler 行为复刻

- 复刻 legacy/md5.js 的 world 派生结构和 pending 可见性；
- 建立新的 phase，但保持归一化帧和 RNG strict diff；
- 固化 target selection、round_pos、alive_group_count、pending spawn/revival/remove/death 行为。

完成标准：

- track_case_miner 小样本 strict diff 通过；
- 目标选择和 pending 行为 golden 覆盖。

### 阶段 E：Player/Kind/Extension

- 拆 `PlayerTemplate` / `PlayerRuntime` / identity cold data；
- 建立 experimental `ExtensionRegistry`；
- 实现 namespace ID、priority hook、链式 handler、typed slots；
- 已接入 `SkillLoadout` 纯数据面，默认空 loadout 并随 template/spawn 进入实体；
- 实现 custom repo 内 example/fixture 的基础能力。

完成标准：

- custom 审计关键能力能表达；
- extension panic 在 runner/wasm 边界捕获；
- 注册/构造错误走 `Result`。

### 阶段 F：Skill/State 静态化

- 内置技能全部迁静态路径；
- custom skill 走 registry fallback；
- 已接入 `SkillSpec::hook_mask` 与按 loadout/registry 生成的 `SkillHookPlan`，作为静态/自定义 skill 调度入口；
- 已接入 `SkillHandlers` / `SkillContext` 和 runtime skill hook 执行入口，handler 可产出 update 或投递 effect queue；
- `run_minimal_round` 已执行 actor 的 `PRE_ACTION` / `PRE_DAMAGE` / `POST_DAMAGE` / `POST_ACTION` skill hook，并将 skill update/effect 与基础攻击及 state hook 合入同一 frame；
- 已接入 `StateHandlers` / `StateContext` 和 runtime state hook 执行入口，legacy-only state entry 在 v2 handler 分发中跳过；
- `run_minimal_round` 已在基础攻击后执行 actor 的 `POST_ACTION` state hook，并将 state update/effect 合入同一 frame；
- `run_minimal_round` 已在基础攻击前后执行 actor 的 `PRE_DAMAGE` / `POST_DAMAGE` state hook，并按 pre-damage -> damage -> post-damage -> post-action 顺序合帧；
- `StateStore` 改 `SmallVec`/dense index + legacy order key；
- hook plan 变化当前 phase 立即可见。

完成标准：

- 行为敏感技能 strict diff 通过；
- 内置路径不依赖宽 trait object 查询；
- 目标选择内部 RNG/score 顺序完全复刻。

### 阶段 G：Effect pipeline 与伤害链

- 用 effect pipeline 替换 `OnDamageFunc`；
- damage/heal/state/spawn/revive/remove/replay/custom effect 接入；
- 已补齐 damage/heal/spawn/state/revive/remove/replay/custom effect 的实体存在性校验，非法 caster/target panic；
- `Revive` / `Remove` effect 已同步维护 v2 `round_order`，为 pending revive/remove 可见性对齐 legacy 铺底；
- `WorldArena` 已接入 `team_alive` / `flat_alive` / `alive_group_count` 派生存活视图，并随 spawn/revive/remove/death/heal 复活同步维护；
- raw namerena runner 已在 seed 初始化后同步 legacy `WorldState` 的 team 编号、`round_order`、`team_alive`、`flat_alive` 与 `alive_group_count`，避免 large / fight_multi runner golden 在初始世界顺序上偏移；
- `NormalizedOutcome` 已纳入 defense/resistance，strict diff 可覆盖 custom summon 继承 owner 防御/魔防的数据面；
- 致死 `Damage` effect 已按 damage -> die(target) -> kill(caster) 顺序执行 `DIE` / `KILL` skill/state hook；
- `PlayerRuntime` 已记录 `owner` / `root_owner` / `PlayerKindPolicies`，`Spawn` effect 会把新实体挂到 caster/root-owner 链路上；
- `Damage` effect 已接入 `OwnerResolutionPolicy::RootOwner`，summon/root-owner 伤害可转打 root owner 并在解析目标上触发致死 hook；
- `Damage` effect 已接入 `DamageSharePolicy::ShareToOwner`，子实体受伤时可同步扣 owner 并在共享致死时触发 owner 致死 hook；
- `Damage` effect 已接入 `DamageSharePolicy::ShareToSummons`，owner 受伤时可按实体顺序同步扣存活子实体；
- `Damage` effect 已接入 linked minion cleanup，owner 致死时按实体顺序清理存活 minion 并同步 `round_order` / alive views；
- `Spawn` effect 已接入 `PlayerKindPolicies::inherit_owner_def_res`，custom summon 可在生成时继承 owner 防御/魔防数据面；
- `CustomEffect` / skill / state handler 已通过各自 context 暴露受控 RNG 消费 API，不直接暴露 `RC4` 本体；
- `Merge` effect 已接入 `MergePolicy::FixedLane` / `DropUnmappedSkills` 的固定技能槽位合并数据面，并在成功合并时输出吞噬/属性上升帧；
- effect batch 后逐项复刻 legacy flush；
- 嵌套 effect 深度优先；
- 清掉新 runtime 中 `just_get_player_mut` 风格重借。

完成标准：

- damage/death/revive/share/summon/merge strict diff 通过；
- 非法 effect 所有模式 panic；
- Miri 覆盖核心 unsafe/alias 路径。

### 阶段 H：wasm/show/DIY/OL 迁移

- 已补 v2 `RuntimeFrame` 核心 replay/show renderer golden，并覆盖 custom HP marker payload，作为 `show.html` 迁移前的最小帧展示对账面；
- 已补 HP marker 结构化 replay view 适配，`"[0]还剩[2]点血"` 即使 HP 未变化也会向 wasm/show 输出 `show_hp` 和 `Data` part；
- `show.html` 迁到 v2 replay schema；
- wasm API 可 breaking；
- DIY/OL schema 重做；
- 增加核心 show golden。

完成标准：

- 核心 replay case 可加载、播放、关键帧展示一致；
- DIY/OL 归一化 roundtrip 一致；
- migration guide 记录新入口和破坏面。

### 阶段 I：切换与删除 legacy

切换 PR 必须同时完成：

- v2 成为正式路径；
- 删除 legacy runtime；
- 删除 Legacy Adapter 计划残留；
- 移除 `mutable-noalias=no` 的架构必要性；
- 更新 changelog 和 developer migration guide。

切换硬门槛：

- 核心 tests 全量 Miri；
- strict diff 大样本 + fixed/custom golden 全过；
- `show.html` 核心 golden 全过；
- fixed/stress/no_debug performance 不退步。

---

## 10. 验证命令与门禁

### 10.1 基础测试

```powershell
cargo test -p tswn_core --lib
cargo test -p tswn_core --test engine_core
cargo test -p tswn_core --bin tswn-cli
cargo test -p tswn_core --features no_debug --lib
python track_test.py -q
```

### 10.2 strict diff

日常轻量：

```powershell
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --max-cases-per-mode 64 --keep-going
```

切换前完整：

```powershell
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --modes 1v1,2v2,3v3v3,ffa --ffa-sizes 4,6,8 --case-offset-per-mode 0 --max-cases-per-mode 2000 --keep-going
```

strict diff 工具还必须覆盖 fixed/custom golden。任何失败阻塞切换。

### 10.3 Miri

核心 `tswn_core` tests 全量 Miri。若耗时过长，允许日常 CI 分层执行，但切换 PR 必须全量通过。

### 10.4 show golden

少量核心 replay case：

- 加载成功；
- 可播放；
- 关键帧 DOM/截图 golden 一致；
- custom HP marker / summon / merge 展示可用。

### 10.5 性能

```powershell
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_perf_cases -- --case-dir docs/perf/fixed_cases_30 --out-dir target/perf_cases_v2_t1 --bench-runs 13000 --thread 1 -q
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_perf_cases -- --case-dir docs/perf/fixed_cases_30 --out-dir target/perf_cases_v2_t0 --bench-runs 13000 --thread 0 -q
```

硬门槛：

- fixed cases 不退步；
- stress_multi 不退步；
- no_debug release 不退步；
- batch/prepared runner clone 无异常退步；
- 默认无 trace 路径无可测常驻成本。

---

## 11. 文档与发布

切换时版本继续走 0.x 线，bump `0.x+1`。

必须更新：

- 用户 changelog：简洁说明用户可见结果保持一致、wasm/show 已迁移、开发 API breaking；
- developer migration guide：详细列 Rust API、extension API、wasm/CLI、DIY/OL、replay schema 的破坏面和替代入口；
- unsafe/runtime design：记录 unsafe 集中模块、alias 边界、Miri 门禁；
- custom migration：记录 `github/custom` diff 审计表、extension 落点和 fixture。

---

## 12. 最终验收

v2 合入并删除 legacy 前必须满足：

- UB 安全：核心全量 Miri 通过，unsafe 设计说明完整，`mutable-noalias=no` 不再是必要条件；
- 行为一致：legacy/v2 strict diff 大样本 + fixed/custom golden 全过，RNG 完全一致；
- 展示可用：`show.html` v2 schema 核心 golden 通过；
- custom 迁移：`github/custom` 审计出的关键行为有 repo 内 extension example/fixture；
- 性能不退步：fixed/stress/no_debug/perf clone 路径不退步；
- 删除旧栈：正式路径无 legacy runtime，旧 `Storage`/`SkillArgs`/`OnDamageFunc` 不作为新扩展能力边界；
- 文档完整：changelog、developer migration guide、unsafe/runtime design、custom migration 均更新。
