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
| `mutable-noalias=no` | 双栈期间允许 legacy 继续依赖；v2 必须在 release `mutable-noalias=yes` 门禁下独立通过，最终切换时再删除全局依赖。 |
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
| wasm/show | wasm API 可 breaking；`examples/index.html` 迁到 v2 replay schema，以视觉可用和核心 golden 为准。 |
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

### 2.1 2026-07 架构复查结论

截至 2026-07-14，`runtime_v2` 已从“最小 fixture 原型”进入行为收敛阶段。core 全量测试在 `mutable-noalias=yes` 下不再保留 runtime_v2 self-golden ignore；CLI 与 release `mutable-noalias=yes` 门禁均通过。`case_d8c6_opening_matches_js_trace`、`case_large_67_summon_opening_matches_js_trace`、完整 `large_01`、`large_02`、`large_36`、`large_67`、`large_70` 和 `large_72` 已在 `mutable-noalias=yes` 下通过；feature-gated 完整 runtime-v2 corpus 已在 release `mutable-noalias=yes` 下达到 118/118 通过。`tests/sqp5900.txt` 六模式 24000 条与 `tests/sqp6000.txt` 六模式实际可生成的 15792 条均已完成 strict diff，TS/Rust 失败、TS 空输出与结果差异全部为 0；按本轮约定的正确性验收范围，Runtime v2 正确性确认完成，但这仍不等于已经满足删除 legacy 的全部安全、性能门槛。`large_36` 暴露的 linked-minion 连续删除游标与 KILL 技能首触发短路已经闭环，`large_02` 暴露的 PoisonTick 致死 replay/score 已进入统一 lethal pipeline；`large_67` 暴露的 Merge 固定槽位、0→正等级 action 队尾和 clone 继承 Summon blueprint 问题已经闭环；`large_70` 暴露的 Clone 属性重建丢失垂死增益问题已通过统一 Runtime v2 属性刷新入口闭环；`large_72` 暴露的 Disperse 防御链、Protect hook 插入顺序与魔法重定向、冻结背刺、Ice/Hide on_damage 时序以及终局 KILL hook/RNG 问题已经闭环；sqp6000 暴露的直接 owner 存活时 root-owned summon 清理、终局状态 hook 截断、实体 ID 空洞计数、LifeWheel/Exchange 后置致死链和 Charge 激活刷新 Haste 倍率问题均已闭环，四个原始输入已归档并接入长期回归。raw 初始化已切换到独立 `PreparedBattleInit`，不再构造 legacy `Runner` 或读取 legacy `WorldState`；CLI `fight`（含 `--out-raw`）、`diff`、`raw`（含 `!test!` 评分/胜率）和独立 `bench` 已默认走 Runtime v2，core `cli_api` 及 C、Python、WASM 高层评分/胜率入口同步完成默认切换；`examples/index.html` 仅调用 v2 normalized run，并已对齐 main 的 replay 语义与视觉效果。低层 legacy 类型仍作为兼容/对账 API 保留，核心全量 Miri 与性能不退步门槛也尚未通过。

可以保留并继续演进：

- `EntityArena`、`PlayerTemplate` / `PlayerRuntime` 的冷热数据拆分；
- typed template/entity/battle slots；
- `ExtensionRegistry` 的 namespace、稳定注册顺序与 capability 数据面；
- `EffectQueue`、受控 context 和已经按 legacy 顺序验证过的局部伤害链。
- `PreparedBattleInit` 的显式构造边界：`Player` facade 与临时 `Storage` 只用于输入解析、build 和蓝图准备，Runtime v2 热路径不持有它们。
- `EntityRecord::refresh_runtime_stats_from_template` 的统一属性刷新边界：模板派生属性变化后重放 Upgrade、Curse、Hide、Charge 与 Accumulate 的运行期修饰，Clone 与 Merge 不再各自手工覆盖 runtime 属性。
- `scripts/check_runtime_v2_noalias.py` 与 `track_test.py --engine runtime-v2` 的门禁边界：workspace 默认已改为 `mutable-noalias=yes`；legacy 若确有兼容需要，必须由对应命令显式覆盖为 `mutable-noalias=no`，不能再由全局配置掩盖。

切换前仍必须完成：

- **已完成**：state hook 执行器按 `StateStore` generation 动态刷新后续 hook，状态增删会在当前 phase 内影响后续 state hook；
- 尚未被大样本/custom 命中的内置技能/状态组合；plain 主动静态 dispatch 当前覆盖 26/26；Assassinate 已补齐 pre-action 顺序、潜行 pending、冻结目标与强制背刺路径并修复 `case_d8c6`，Summon 已补齐 blueprint、remembered entity、首次 spawn、死亡后复活、charge、固定技能槽、伤害分摊、owner 分摊致死 replay 和 clone blueprint 继承，Zombie 已补齐 KILL 静态 dispatch、尸体标记、蓝图生成、Clone 继承、MP/RNG/replay 顺序与 spawn 前 ID 空洞，Merge 已补齐固定槽位逐位抬级、0→正等级 action 队尾和终局 KILL gate 语义并修复完整 `large_67` / `large_72`；KILL 技能链已按 legacy 在首个真实触发后短路；feature-gated 完整 runtime-v2 corpus 的 118 个 case 已全部通过；后续风险转为 custom golden、状态生命周期边界和 RNG 回归；
- 内置技能借用 extension handler 的过渡路径继续收敛为静态 dispatch；
- 已清理过期的 v2 self-golden ignore；后续新增 runner 回归必须优先使用 legacy/v2 strict diff 或稳定行为断言，不能把 v2 自身输出当作 parity 门禁。
- 压力 strict-diff 首次发现且尚未闭环的输入不得只保留在 `target` 临时目录；必须原样复制到 `crates/tswn_test/cases/runtime_v2_stress/`，记录来源、模式、首差异和处理状态。修复后必须将该输入接入长期 Runtime v2 严格回归，并继续保留原始 input。
- **已完成（2026-07-14 复验）**：`tests/sqp5900.txt` 最终六种模式各 4000 条、共 24000 条全部执行；`summary.json` 为 `ts_failures=0`、`rust_failures=0`、`ts_empty_outputs=0`、`diff_failures=0`。
- **已完成（2026-07-14）**：`tests/sqp6000.txt` 以每模式上限 4000 条执行；受号库组合数限制，实际六种模式各生成 2632 条、共 15792 条，全部执行且 `ts_failures=0`、`rust_failures=0`、`ts_empty_outputs=0`、`diff_failures=0`。本轮发现的四个输入已归档到 `crates/tswn_test/cases/runtime_v2_stress/` 并接入上述 118 项 release `mutable-noalias=yes` corpus。完成 sqp5900 与 sqp6000 两套验收后，按约定可视为正确性无误。
- **默认入口已完成，最终删除未完成**：CLI `fight`（含 `--out-raw`）、`diff`、`raw`（含 `!test!` 评分/胜率）和独立 `bench` 默认入口已切换到 Runtime v2；core `cli_api` 与 C、Python、WASM 的无 runtime 参数高层评分/胜率入口同步改走 v2，WASM `WinRateSession` 保留原协议但内部使用 `PreparedRuntimeV2Runner`；`examples/index.html` 只调用 v2 normalized replay adapter。低层 `Runner`、`PreparedRunner`、`FightSession` 等兼容对象和 CLI 显式 `--runtime legacy` 对账入口仍保留。2026-07-14 正式 no_debug 性能复测已确认 fixed30、stress_multi 与 Runtime v2 batch probe 均退步，核心全量 Miri 仍被 legacy alias UB 阻塞，因此整个重构计划尚未完成。

### 2.2 修订后的近期实施顺序

1. 保持 `case_d8c6`、`case_large_67_summon_opening_matches_js_trace`、完整 `large_01`、`large_02`、`large_36`、`large_67`、`large_70` 与 `large_72` 在 debug/release `mutable-noalias=yes` 下持续通过；当前完整 runtime-v2 corpus 为 118/118，`tswn_test` 分片拆分后已复跑完整 release corpus 通过，后续每个行为闭环仍继续运行完整 corpus 门禁，任何 frame/RNG 回归立即阻塞；
2. **已完成**：建立独立 `PreparedBattleInit`，自行复刻 raw 分组、同队 upgrade、build、seed/RNG、初始 world views、loadout 与 summon/shadow blueprint 准备；删除 v2 runtime 构造对 legacy `Runner` / `WorldState` 的依赖和静默同步失败；
3. 补齐尚未被 corpus 命中的内置技能/状态生命周期，并为 RNG 短路、on_damage 时序和状态叠加补精确单测；
4. **已完成**：重写 state hook 执行器，使当前 phase 内状态 generation 变化立即影响后续 hook；
5. **默认入口已完成**：CLI `fight`（含 `--out-raw`）、`diff`、`raw`（含 `!test!`）和独立 `bench` 已默认切到 v2，C、Python、WASM 高层评分/胜率入口及 show 页面同步完成；后续只继续收敛显式 legacy fallback、低层兼容对象和最终删除；
6. **已执行但未通过**：2026-07-14 已运行性能与 Miri/alias 门禁；Runtime v2 独立 `mutable-noalias=yes` 门禁通过，但核心全量 Miri 被 legacy `Storage` alias UB 阻塞，性能门禁也确认退步。先闭环两项，再执行长时间 stress 和删除 legacy runtime。

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
- `examples/index.html` 可视化播放的核心帧表现。

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
- `StateEntry` 已带最小 payload，当前覆盖 `FireMagHalfSteps`，用于后续 fire/summon explode 公式 parity；
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

v2 replay 数据结构可重做，`examples/index.html` 同步迁移到 v2 schema。

验收比较归一化帧序列，不比较旧 JSON 原始结构。归一化帧应覆盖：

- actor/action；
- visible HP/MP/status changes；
- damage/heal/state add/clear；
- death/revive/spawn/remove；
- summon/merge/custom display；
- winner/score/final frame。

### 7.2 examples/index.html

`examples/index.html` 以视觉可用为准：

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
- 已实现 legacy `Runner` 与 v2 的真实双跑归一化，比较 winner、score 汇总、RNG checkpoint、entity/team/HP/MP/DEF/MDF/alive、WorldArena 派生视图和 replay frame；
- 已实现 run 级首差异 `StrictRunDiff`，并通过 `tswn-cli runtime-v2 parity` 输出 `matched`、`first_diff`、`legacy`、`v2` 机器可读 JSON；
- legacy 侧已从 `Player::action` / default / forced 路径直接记录结构化 action boundary，禁止从中文 replay 文本反推；下一步需用真实 parity 样本继续校准 boss/state 特殊行动的 target/amount 语义；
- 接入 fixed golden、track_case_miner 大样本、custom golden；
- 明确任何 diff 失败阻塞切换。

完成标准：

- legacy 自身可生成归一化帧；
- fixed/custom golden 可稳定复跑；
- diff 输出能定位到 frame/action/RNG 节点。

### 阶段 B：custom diff 审计

- 已在 `docs/analysis/custom_runtime_v2_migration.md` 对 `github/custom` 相对 main 做首版 diff 归类；
- 已产出 custom 改动清单和 v2 落点；
- 已补 bed2 registry/template/import fixture，覆盖 `custom.bed2` kind、固定 summon skill、HP marker slot、`bed2[...]` / `@bed2` marker 最小 v2 导入、Player facade id-name 归一化桥接、typed summon template payload 读取后 spawn、`push_summon_from_template_slot` / `push_summon_from_template_slot_with_message` helper、grouped raw bed2 roster、mixed legacy/bed2 raw roster 到 `PreparedCombatTemplate` 的 helper，以及 `RuntimeV2Runner` 对 bed2-only / mixed roster、raw namerena fixture 形状、seed 初始 RNG 与初始 world/order 的正式构造、单回合归一化和 run-until-winner 归一化入口；raw 构造已统一经 `PreparedBattleInit` 完成，不再搭建 legacy `Runner`；
- 已补 custom summon 复合 fixture，覆盖 root-owner 路由、owner/summon 伤害共享、charged summon per-template policy override 关闭 share damage、spawn 后技能与 move_state 保留，以及 `push_summon_recast_from_entity_slot` / `push_summon_recast_from_template_slot` 原实体复活复用，并固化 remembered summon 存活/缺少读取 capability 时不静默重建；`SkillLoadout` 已拆分 fixed lanes 与 active order，并以 `summon_default_skill_loadout` 固化 legacy `[fire, fire, explode]` 固定槽位；同时补 `SpawnWithMessage` / `ReviveWithMessage` 路径，`run_summon_recast_from_template_slot` 已作为正式 handler 从 typed template slot 读取真实 summon payload 后输出 legacy 的 `[0]使用[血祭]` + `召唤出[1]` 帧序列；`FireAttack` effect 与 `run_summon_fire_skill` 已覆盖 summon fire 子技能 replay、`get_at(true) * (1.5 + fire_mag)` 魔法伤害公式、target-side pre/post defend、回避和 fire_mag 递增；`SummonExplode` effect 已覆盖 legacy 自爆 replay、自身死亡、`get_at(true) * (4.0 + fire_mag)` 魔法伤害公式、target-side `PRE_DEFEND` 攻击量改写/截断、magic attack 回避 RNG / replay、target-side `POST_DEFEND` 伤害改写、`ShieldState` 护盾吸收/耗尽 payload、`CurseState` r63 触发伤害倍增/未触发耗 RNG/damage<=0 不耗 RNG、`IronState` 吸收伤害降到 1 / 防御降到 0 / 击破清 payload 并输出取消 replay / post_action step 递减与自然解除 replay、`PoisonState` post_action 毒性发作、持续伤害、自然解除与致死不释放、`HasteState` / `CharmState` / `SlowState` post_action step 递减、死亡静默清理、自然解除换行 replay 与 legacy 210 优先级顺序、`ChargeRuntime` 已接入 skill late post_action phase，覆盖 step 递减、过期清理 at_boost 以及晚于普通 state post_action 的 legacy 尾部顺序；`AccumulateRuntime` 已接入主动技能 handler，覆盖 Charge 加成下的 move_point +900、charge_bonus 倍率、Charge late 清理后保留聚气倍率，以及 clear-positive 中聚气优先于蓄力的消息顺序与 reset multiplier；runtime/state 正面清理已通过 `EntityRecord::clear_positive_messages` 合成单一数据面，覆盖 Accumulate、Charge、Haste、Shield、Iron 的清理与 priority 排序，`QueuedEffect::DisperseAttack` / `DisperseHit` 已复用该数据面覆盖使用净化 replay、魔法攻击 pre/post defend、回避、minion atp 翻倍、legacy damage 后立即清正面/扣 MP、lethal 命中先输出 Haste 解除再进入 DIE/KILL hook，`run_disperse_skill` 可在 round `PRE_ACTION` 通过 `SkillContext::selected_target` 使用当前已选目标，`score_disperse_target` 已复刻 smart/random 目标评分公式并由 `attr_sum` / `atk_sum` / `attract` 数据面支撑，Disperse round `PRE_ACTION` 已接入 legacy smart roll 消耗、多候选抽样、评分排序与 selected target 透传；BOSS/BOOST kind 的 fire immune RNG、命中存活且未免疫目标后的 fire_mag 半层递增，以及目标 DIE/KILL 链后再执行自爆者 DIE 的顺序；`PlayerRuntime::get_at` 已复刻 legacy `get_at(true/false)` 的 RNG 取值公式并承载 magic/magic_point/wisdom/at_boost/agility 数据面，raw import 与 strict diff 已覆盖 MP；
- 已补 custom minion owner cleanup fixture，覆盖 owner 致死或显式 remove 时 linked minion 按实体顺序死亡、移出 round/alive views 并输出消失帧；并补 `next_minion_name_from_entity_slot` / `push_minion_from_template_with_allocated_name` / `push_minion_from_template_slot_with_allocated_name` / `minion_display_index_for_entity` helper，固化 root owner entity slot 计数、child minion 复用 root owner counter、legacy `?N` 展示序号解析、从 typed template slot 读取真实 minion 模板后按 legacy/custom 文案 spawn，并保留 payload move_state；`run_shadow_minion_from_template_slot` 与 `run_zombie_minion_from_template_slot` 已作为正式 handler 固化 `[0]使用[幻术]` + `召唤出[1]`、换行 + `[0][召唤亡灵]` + `[2]变成了[1]` 外显帧序列；silent spawn 已支持只生成实体而不额外输出占位 spawn 帧；缺少 capability 时返回结构化错误；
- 已把 linked minion owner death cleanup 纳入 custom runner strict-diff golden，覆盖消失帧、winner 与 WorldArena 派生视图；
- 已把 merge 纳入 custom runner strict-diff golden，覆盖吞噬/属性上升帧、score，以及 JS `k1` 语义下按 fixed-lane key 提升 owner 既有技能等级（不复制 target 技能 ID）；
- 已补 custom runner multi-round normalized run golden，覆盖 `RuntimeV2Runner::run_until_winner_normalized_rounds`、guard 状态、累计 score 与逐回合 strict diff；
- 已从 custom large / fight_multi 真实 raw 输入抽出初始化 parity fixture，覆盖 seed RNG、team 编号与 WorldArena 初始派生视图；旧 large / fight_multi prefix/terminal v2 self golden 已删除，保留真实 legacy oracle 与 `track_test.py --engine runtime-v2` 作为后续收敛门槛；
- 为关键行为设计 repo 内 extension fixture；
- 标出需要 capability 例外的跨实体读取点。

完成标准：

- bed2、summon、merge、minion、HP marker、wasm replay 行为都有验收 case；
- 每项 custom 关键行为都有 v2 extension 落点。

### 阶段 C：Runtime v2 骨架

- 新增 `CombatRuntime`、`PreparedCombatTemplate`、`EntityArena`、`WorldArena`、`PhaseScheduler`、`EffectQueue`、`BattleScratch`；
- 保留 `Player` 输入/测试 facade，但 battle start 前转换成 template；
- 已把 legacy `round_pos`、step RNG、speed/move-point 阈值推进迁入 plain Runtime v2 scheduler；`PreparedBattleInit` 已独立复刻 raw 分组、同队 upgrade、按 id-name build、seed RC4 消费、move point、初始 world views、普通玩家运行时属性与 summon/shadow blueprint，并显式报告初始化错误；过期自指 v2 golden 已删除；当前 feature-gated 完整 runtime-v2 corpus 已在 release `mutable-noalias=yes` 下达到 118/118，通过结果可作为后续回归门禁。

完成标准：

- 最小 case winner/score/frame/RNG 通过；
- v2 路径不暴露 `Arc<Storage>` 或旧 `SkillArgs` 能力。

### 阶段 D：World/Scheduler 行为复刻

- 复刻 legacy/md5.js 的 world 派生结构和 pending 可见性；
- 建立新的 phase，但保持归一化帧和 RNG strict diff；
- 固化 target selection、round_pos、alive_group_count、pending spawn/revival/remove/death 行为。
- `WorldArena` 已改为 legacy `round_pos: i32` 与 `rem_euclid` 推进语义，删除实体时按 legacy 规则调整位置；
- plain scheduler 已按 legacy `main_round` 的 `entity_count * 4` tick 上限执行 step roll，只有 move point 严格大于 2048 才提交行动；纯 v2/custom fixture 暂保留现有 handler 驱动路径，避免未迁完的普通玩家 loadout 影响 custom 验收；
- damage、poison、disperse、self-death、remove、linked-minion cleanup 已统一通过 `mark_dead` 同步 round/alive 派生视图；owner 死亡时先按实体顺序清理 linked minion、再移除 owner，保持连续删除下的 legacy `round_pos` 调整顺序；
- revive 已改为追加到 `round_order` 尾部，且复活空队伍不恢复历史 `alive_group_count`，与 legacy/JS 语义一致；
- scheduler 的当前 corpus 行动、目标选择与 pending 可见性已通过真实 legacy/v2 run parity；仍需用新增样例覆盖 corpus 未触达的技能组合，而不能仅以 world 单测替代 run parity。

完成标准：

- track_case_miner 小样本 strict diff 通过；
- 目标选择和 pending 行为 golden 覆盖。

### 阶段 E：Player/Kind/Extension

- 拆 `PlayerTemplate` / `PlayerRuntime` / identity cold data；
- 建立 experimental `ExtensionRegistry`；
- 实现 namespace ID、priority hook、链式 handler、typed slots；
- 已接入 `SkillLoadout` 纯数据面，默认空 loadout 并随 template/spawn 进入实体；
- `CustomRuntimeV2ImportConfig` 已同时携带 registry/import 配置与 skill handler 绑定，custom runner 构造时统一安装实现，不再由 CLI 层二次补线；
- `RuntimeV2ReadyError` 已在 custom runner 构造期扫描活动实体和所有 `PlayerTemplate` template slot，聚合缺失 handler 的 skill/export/source；runner 执行入口也会再次守卫 ready 状态；
- default profile 对已实现的 summon、summon-fire、summon-explode、possess 自动安装 handler；已注册但未实现的 `custom.minion.heal` 在输入实际引用时明确拒绝构造，不再静默运行；
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
- `run_summon_recast_from_template_slot_with_config`、`run_shadow_minion_from_template_slot_with_config`、`run_zombie_minion_from_template_slot_with_config` 已把 summon/shadow/zombie fixture 闭包提升为可配置正式 handler，复用 typed template slot、entity slot counter 与 legacy replay 顺序；`run_possess_skill` 已补最小 v2 possess 数据面，覆盖 `[0]使用[附体]`、目标进入 Berserk payload、已有狂暴 step +4，以及 shadow/minion caster 自身移除；
- `SummonExplode` 已接入 target-side `PRE_DEFEND` / `POST_DEFEND` skill/state hook，context 可暴露并改写当前攻击量或最终伤害，并携带 incoming caster/target 元数据；`StatePayload::ShieldValue` 与 `run_shield_post_defend_state` 已覆盖 ShieldState 护盾吸收/耗尽数据面，并纳入 clear-positive state 清理但不输出取消消息；`StatePayload::Curse` 与 `run_curse_post_defend_state` 已覆盖 CurseState 的 r63 判定、伤害倍增 replay、未触发 RNG 消耗和 damage<=0 跳过 RNG；`StatePayload::Iron` 与 `run_iron_post_defend_state` 已覆盖 IronState 的吸收削伤、防御 replay 判定、击破换行/打消 replay、damage<=0 不改状态、post_action step 递减、step<=0 清理、自然解除时 speed_points 调整和换行 replay，并纳入 clear-positive priority 400 打消消息；`StatePayload::Poison` 与 poison tick effect 已覆盖 PoisonState 毒性发作 replay、持续伤害、count/atp 递减、自然解除 replay、死亡静默跳过，以及致死时按 damage → 击倒 replay/50 score → DIE/KILL hook 的统一 lethal pipeline 结算且不输出解除；`StatePayload::Haste` / `StatePayload::Charm` / `StatePayload::Slow` 与对应 post_action handler 已覆盖疾走/魅惑/迟缓 step 递减、自然解除 replay、死亡静默清理和与 Iron 同层的 legacy 210 优先级；`StatePayload::Haste` 已纳入 clear-positive priority 300，死亡时清理但不输出取消消息；
- Reflect 已迁入正式 `PRE_DEFEND` skill hook：概率失败只消费 `r255`，触发后清零原攻击量、提交完整魔法反射攻击，并在反射伤害/死亡链结束后扣除 480 行动力；
- Curse 主动技能已迁入 plain 静态 dispatch：目标抽样完整复刻空目标 RNG 消费，命中走统一魔法攻击链，并在伤害落地后、POST_DAMAGE 与 DIE/KILL 之前执行 on_damage，完成 BOSS/BOOST 阻断、默认 `prob=42/multiply=2`、charge 加成、重复叠加和 `[1]被[诅咒]了` replay；
- Heal 已迁入 plain 静态 dispatch：`AllyAlive` 抽样、smart 有效目标和 `(missing_hp + negative_count * 64) * attr_sum` 评分、`get_at(true)/60` 回复、8 级以上每次 -1 衰减均已接入；治疗后按 berserk → charm → curse → ice → poison → slow 顺序输出解除消息，清除 Fire 等无消息负面 meta，并从当前 template 基线恢复 Curse 放大的 `atk_sum`、按剩余 Haste/Lazy state 重算 speed；
- Disperse 已迁入 plain 静态 dispatch：敌方 `all_alive + pickSkipRange` 抽样和评分后直接进入既有 `DisperseAttack` 完整魔法攻击/回避/clear-positive/MP 扣减链，不再要求内置主动技能借用 extension handler；
- Fire 已迁入 plain 静态 dispatch：Fire/Poison 共用默认敌方 `all_alive + pickSkipRange` 抽样与 legacy smart/random 评分 helper；Fire 直接复用 `FireAttack` 的 `get_at(true) * (1.5 + fire_mag)`、pre/post defend、回避、伤害与命中后半层叠加链，无需技能 handler；
- Poison 已迁入 plain 静态 dispatch：统一魔法攻击链新增 Poison on-damage 分支，严格保留 `damage <= 4` 短路、目标存活/免疫检查后第二次 `get_at(true) * 1.2000000476837158`、state 创建/叠加、count 重置为 4 和 `[1][中毒]` replay；默认 profile 已正式注册 `core.state.poison` 与 post-action handler，Fire/Ice/Poison 的 BOSS/BOOST 免疫判断收束为统一 status immunity helper；
- Assassinate 已迁入 plain 静态 dispatch：导入 legacy `pre_action` 顺序，复刻 smart + Poison 无 RNG 短路、专用目标抽样评分、charge 行动力加成、潜行锁定目标、受伤识破、死亡目标清理和下一次行动强制背刺；背刺使用三次 `get_at(true)` 最大值乘四，并跳过普通 agility dodge；`case_d8c6` 已在 debug/release `mutable-noalias=yes` 下通过；
- Summon 已迁入 plain 静态 dispatch：smart HP `<80` 与存活 remembered entity 均无 RNG 短路，typed slots 保存 blueprint/remembered entity，首次 spawn 与死亡后原实体 revive 复用同一生命周期；charge 仍消费 `r255` 后把行动力覆盖为 2048，使魔固定 Fire/Fire/SummonExplode 槽并保持 shuffled active order，非 charge 时伤害按半数分摊给直接 owner，owner 因分摊致死时补齐 legacy 击倒 replay 且不误删当前召唤物，SummonExplode 复用静态 effect pipeline；clone spawn 会继承 Summon blueprint，实际召唤前按当前 owner build 刷新防御/魔防派生属性，避免 Clone/Merge 后使用陈旧静态模板；首次召唤还会保留 legacy build 产生且永不复用的实体 ID 空洞；
- Zombie 已迁入 plain KILL 静态 dispatch：combat minion 目标无 RNG 短路，普通目标先执行 `r63` 概率判定，再按 blueprint 是否存在决定只标记尸体或继续执行 MP gate；成功生成时继承正式 Zombie blueprint、消费 `r255 * 4` 行动力、保留 spawn 前实体 ID 空洞并按 `[0][召唤亡灵]` / `[2]变成了[1]` 顺序输出 replay；Clone 会继承 Zombie blueprint；
- Charm 下的敌方目标选择已按 legacy roster 语义修正：只替换行动者的 effective team，候选实体仍按实际 team 过滤，因此被魅惑行动者自身仍可进入默认敌方技能、Berserk 与 Exchange 的候选集合；生命之轮已补 seed `33554642@!` 严格回归，该修复使完整 `large_01` strict parity 通过；
- Merge 已复刻 JS `k1` 语义：`FixedLane` 按槽位位置逐位抬级，`DropUnmappedSkills` 保留 fixed key 映射，标准 kind 默认采用 `FixedLane`；0→正等级技能会从旧 action 位置移除并按 fixed lane 遍历顺序追加到队尾，属性、MP 和 move point 转移后完整 `large_67` 已在 release `mutable-noalias=yes` 下通过；
- plain 内置主动静态 dispatch 当前覆盖 26/26；
- `StateStore` 改 `SmallVec`/dense index + legacy order key；
- scheduler 已能按 generation 重建 state hook plan；state hook 执行器已改为每个 handler 后检查 `StateStore` generation，发生变化时重建后续 hook 列表，避免 phase 开始时冻结整段计划。

完成标准：

- 行为敏感技能 strict diff 通过；
- 内置路径不依赖宽 trait object 查询；
- 目标选择内部 RNG/score 顺序完全复刻。

### 阶段 G：Effect pipeline 与伤害链

- 用 effect pipeline 替换 `OnDamageFunc`；
- damage/heal/state/spawn/revive/remove/replay/custom/fire-attack/summon-explode effect 接入；
- 已补齐 damage/heal/spawn/state/revive/remove/replay/custom effect 的实体存在性校验，非法 caster/target panic；
- `Revive` / `Remove` effect 已同步维护 v2 `round_order`，为 pending revive/remove 可见性对齐 legacy 铺底；
- `WorldArena` 已接入 `team_alive` / `flat_alive` / `alive_group_count` 派生存活视图，并随 spawn/revive/remove/death/heal 复活同步维护；
- `PreparedBattleInit` 已在 seed 初始化后直接生成 team 编号、`round_order`、`team_alive`、`flat_alive` 与 `alive_group_count` 所需初始视图，不再从 legacy `WorldState` 回填；large / fight_multi 初始化 parity fixture 与 corpus 门禁保持无退步；
- `NormalizedOutcome` 已纳入 defense/resistance，strict diff 可覆盖 custom summon 继承 owner 防御/魔防的数据面；
- `ExtensionRegistry` 已补 skill name / export_name -> v2 `SkillId` 查找面，为 DIY/OL/custom parser 把 overlay 技能名导入 v2 `SkillLoadout` 铺底，避免依赖 legacy skill id 与 v2 registry 顺序偶然一致；
- `CustomBed2Import` / `RuntimeV2Runner` 已补 parser-facing summon/shadow/zombie overlay 导入入口、组合 minion overlay 导入入口和 `CustomRuntimeV2ImportConfig` profile 入口，可从 bed2-only 与 mixed roster/runner 的 bed2 raw `ol.summon` 解析 attrs、inherit_owner_def_res 与 summon fire/explode active order，并可从 `ol.shadow` 解析 attrs 与 possess active order、从 `ol.zombie` 解析 attrs 与 skill export 前缀映射，把 typed `PlayerTemplate` payload 写入 v2 template slot；core 已新增 `default_custom_runtime_v2_import_config`，默认 custom v2 profile 已注册 summon/shadow/zombie overlay 所需 kind、template slot、remembered summon entity slot 与默认 skill export，并可经默认 mixed raw runner 导入三类 `ol` template payload；默认 mixed raw runner 已安装 summon recast handler、summon fire/explode 子技能 handler 与 minion possess handler，`default_custom_runtime_v2_normalized_run` 可实际执行 bed2 `ol.summon`、spawned summon 火球术/自爆并输出对应 legacy 前缀帧；显式 profile 与默认 profile 两组 `custom_runtime_v2_*` / `default_custom_runtime_v2_*` 专用外层 helper 已落地，并在 core helper 层统一校验 `max_rounds > 0`；CLI 已暴露 `runtime-v2 normalized-run` JSON 命令并补结构化 JSON golden 与 zero max_rounds 错误覆盖；C API 已暴露 `tswn_default_custom_runtime_v2_normalized_run_json` 并补结构化 JSON golden 与 zero max_rounds 错误覆盖；Python 已暴露 `default_custom_runtime_v2_normalized_run` dict 入口并补 dict golden 与 zero max_rounds 错误覆盖；wasm 已暴露 `default_custom_runtime_v2_normalized_run` typed 入口并补 typed view golden 固定 rounds / RNG / entity stats / action / frame / `UpdateTypeView` 字段形状，以及 zero max_rounds `INVALID_INPUT` 错误覆盖，作为默认 custom v2 normalized run 绑定入口，先提供 custom profile raw import 与 normalized run 调用面，不替换现有 legacy API；
- core 已新增 `default_custom_runtime_v2_parity_report`，CLI 已暴露 `runtime-v2 parity`；该入口直接双跑 legacy/v2 并报告真实首差异，后续行为迁移必须优先用它验证，而不是新增 v2 self golden；
- CLI `fight`（含 `--out-raw`）、`diff` 与 `raw`（含 `!test!` 评分/胜率）已默认使用 Runtime v2，legacy 仅保留显式 `--runtime legacy` fallback；v2 普通输出、raw 聚合日志、赢家输入索引和玩家状态摘要均复用 runtime 自身实体/世界数据，最小样例与含幻影、分身、clan 的 `large_51` 已固定 legacy/v2 逐行一致；批量层新增 seed-independent `PreparedBattleRoster`、可复用 `PreparedRuntimeV2Runner` 和无逐回合向量积累的 completion runner，显式覆盖 benchmark `eval_rq=6`、ProfileWinChance seed 调度、普通/`!` 评分以及 4-worker 汇总确定性；高轮数对账进一步修复反弹攻击丢失 Ice/Curse/Poison `on_damage` 回调和 Exchange 忽略 charm effective team 两项缺口，单线程 score/胜率各 1000 局累计结果已对齐；`PlayerTemplate` 新增 `id_key_name` / clan 冷身份数据，运行期子实体 spawn 与 summon revive 会重建或保留自洽身份，不需要为 CLI 回查 legacy `Storage`；独立 bench 与正式 no_debug 性能门槛仍待收敛；
- 致死 `Damage` effect 已按 damage -> die(target) -> kill(caster) 顺序执行 `DIE` / `KILL` skill/state hook，并把真实被击杀目标通过 `SkillContext::selected_target` 透传给 `KILL` skill hook；已补真实 lethal damage 路径下 zombie handler 使用被击杀目标而非 fallback victim 的回归 fixture，供后续 minion strict-diff parity 复用；
- `PlayerRuntime` 已记录 `owner` / `root_owner` / `PlayerKindPolicies`，`Spawn` effect 会把新实体挂到 caster/root-owner 链路上；
- `Damage` effect 已接入 `OwnerResolutionPolicy::RootOwner`，summon/root-owner 伤害可转打 root owner 并在解析目标上触发致死 hook；
- `Damage` effect 已接入 `DamageSharePolicy::ShareToOwner`，子实体受伤时可同步扣 owner 并在共享致死时触发 owner 致死 hook；
- `Damage` effect 已接入 `DamageSharePolicy::ShareToSummons`，owner 受伤时可按实体顺序同步扣存活子实体；
- `Damage` effect 已接入 linked minion cleanup，owner 致死时按实体顺序清理存活 minion 并同步 `round_order` / alive views；
- `Spawn` effect 已接入 `PlayerKindPolicies::inherit_owner_def_res`，custom summon 可在生成时继承 owner 防御/魔防数据面；
- `CustomEffect` / skill / state handler 已通过各自 context 暴露受控 RNG 消费 API，不直接暴露 `RC4` 本体；
- `Merge` effect 已接入固定槽位等级合并数据面：`MergePolicy::None` 禁止合并，`FixedLane` 与兼容保留的 `DropUnmappedSkills` 都按 JS `k1` 位置只提升 owner 既有技能等级、忽略 target 独有槽位；成功合并时输出换行、吞噬与属性上升帧；
- 普通攻击链已增加内部 `PlainAttackOnDamage` 分派，Curse 与 covid/lazy on_damage 都在扣血后、POST_DAMAGE 前执行；该路径不暴露 legacy `OnDamageFunc` 或可重借 `Player` 指针；
- effect batch 后逐项复刻 legacy flush；
- 嵌套 effect 深度优先；
- 清掉新 runtime 中 `just_get_player_mut` 风格重借。

完成标准：

- damage/death/revive/share/summon/merge strict diff 通过；
- 非法 effect 所有模式 panic；
- Miri 覆盖核心 unsafe/alias 路径。

### 阶段 H：wasm/show/DIY/OL 迁移

- 已补 v2 `RuntimeFrame` 核心 replay/show renderer golden，并覆盖 custom HP marker payload，作为 `examples/index.html` 迁移前的最小帧展示对账面；
- 已补 HP marker 结构化 replay view 适配，`"[0]还剩[2]点血"` 即使 HP 未变化也会向 wasm/show 输出 `show_hp` 和 `Data` part；
- wasm 已新增默认 custom v2 normalized-run typed 入口 `default_custom_runtime_v2_normalized_run`，JS 侧通过该入口取得 v2 归一化 rounds/actions/frames 结构；
- `show-wasm.js` 已新增显式 `buildV2NormalizedReplay()` adapter，可把 v2 default custom normalized run 的 rounds/actions/frames 转成当前 show-compatible replay shape，供后续 DOM/golden 对账和默认路径切换；
- `examples/index.html` 已只调用 v2 normalized replay adapter 的 `buildV2NormalizedReplay()`；页面和 `show-wasm.js` 均不再引用 legacy `FightSession`，历史 `engine` / `runtime` 参数只在分享链接中清理；
- 已补 `show-wasm.test.mjs` 纯 JS adapter 与 HTML chunk 测试，固定 v2 normalized run 到 show-compatible players / states / rows / clips / sequential HP bar / recover HP bar / multi-target sidebar / winner row / summoned entity first-appearance / removed entity disappearance shape 的转换和 `buildFrameRows()` 渲染；
- 已把 `examples/index.html` 的 URL input 与分享链接清洗逻辑抽到 `show-routing.js`，并补 `show-routing.test.mjs` 固定非法 input、历史 runtime 参数清理与 share URL 行为；
- 已补 `show-page-contract.test.mjs` 固定 `examples/index.html` 的 v2 DOM 节点、module script、唯一 adapter 调用和分享链接契约，作为无浏览器依赖的最小页面 wiring golden；
- **已完成**：展示入口已从 `show.html` 重命名为 `examples/index.html` 并迁到 v2 replay schema；同步移植 main 在 `d266cfe` 后的 part 级 clip、生命之轮、机制死亡、百分比伤害 HP 条与配色重构；
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
- `examples/index.html` 核心 golden 全过；
- fixed/stress/no_debug performance 不退步。

---

## 10. 验证命令与门禁

### 10.1 基础测试

```powershell
cargo test -p tswn_core --lib
cargo test -p tswn_core --test engine_core
cargo test -p tswn_core --bin tswn-cli
cargo test -p tswn_core --features no_debug --lib
cargo test -p tswn_test
python track_test.py -q
python scripts/check_runtime_v2_noalias.py
# v2 完整 corpus 是 release mutable-noalias=yes 门禁；当前 118/118 通过
python track_test.py --engine runtime-v2 -q
```

### 10.2 strict diff

日常轻量：

```powershell
cargo run -p tswn_core --bin tswn-cli -- runtime-v2 parity -r "left@red`n`nright@blue" --max-rounds 8
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --max-cases-per-mode 64 --keep-going
```

切换前完整：

```powershell
cargo run -p tswn_core --features aux_bins --bin track_case_miner -- -q --modes 1v1,2v2,3v3v3,ffa --ffa-sizes 4,6,8 --case-offset-per-mode 0 --max-cases-per-mode 2000 --keep-going
```

strict diff 工具还必须覆盖 fixed/custom golden。任何失败阻塞切换。

### 10.3 Miri

核心 `tswn_core` tests 全量 Miri。若耗时过长，允许日常 CI 分层执行，但切换 PR 必须全量通过。

2026-07-14 实测记录：

- workspace 默认已切换为 `-Z mutable-noalias=yes`；
- 安装 nightly Miri 后执行 `cargo +nightly miri test -p tswn_core`，共发现 567 项测试，但在第 21 项 `cli_api_default_custom_runtime_v2_parity_report_matches_converged_first_round` 进入 legacy oracle 时失败；
- Miri 在 `crates/tswn_core/src/engine/storage.rs:384` 报告 `Storage::get_player` 从 `UnsafeCell<Player>` 建立共享引用会移除仍受保护的独占 tag，调用链来自 legacy `Player::attacked`；这是实际 alias/UB 阻塞，不得记录为通过；
- 独立 Runtime v2 release `mutable-noalias=yes` 门禁已通过：core 407 项、CLI 12 项、完整 corpus 118 项均为 0 失败；但它不能替代“核心 `tswn_core` tests 全量 Miri”。

结论：Miri 已实际执行，当前门槛为**未通过**；删除 legacy 前必须消除或隔离上述 legacy alias 路径，并重新跑完整 567 项。

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

2026-07-14 实测记录（release、`no_debug`、`mutable-noalias=yes`）：

- fixed30 单线程 390000 局：overall `75.285 us/battle`、`13282.79 battles/s`；相对 `docs/perf/fixed_cases_30_results/perf_cases.json` 的 0.3.10 基线 `68.952 us/battle` 慢 9.2%；
- fixed30 单线程 `stress_multi`：`153.453 us/battle`，相对基线 `141.170 us/battle` 慢 8.7%；
- fixed30 自动线程：overall `9.881 us/battle`、`101200.04 battles/s`。该工具当前调用 legacy `prepared_win_rate`，用于固定样本基线，不应误记为 Runtime v2 本体性能；
- Runtime v2/legacy batch probe 以 13000 局复测且结果一致性断言通过：score wall v2 `3.491 s`、legacy `1.113 s`（v2 慢 213.6%），其中 init 慢 427.6%、fight 慢 15.7%；win-rate wall v2 `273.2 ms`、legacy `209.0 ms`（v2 慢 30.7%），其中 init 慢 60.0%、fight 慢 36.4%。

结论：性能测试已实际执行，但 fixed/stress 与 batch/prepared runner 均存在可复现退步，当前性能门槛为**未通过**；下一阶段优先分析 `PreparedRuntimeV2Runner` 的 score 初始化与重复 roster/template 构造，再复跑同一口径。

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
- 展示可用：`examples/index.html` v2 schema 核心 golden 通过；
- custom 迁移：`github/custom` 审计出的关键行为有 repo 内 extension example/fixture；
- 性能不退步：fixed/stress/no_debug/perf clone 路径不退步；
- 删除旧栈：正式路径无 legacy runtime，旧 `Storage`/`SkillArgs`/`OnDamageFunc` 不作为新扩展能力边界；
- 文档完整：changelog、developer migration guide、unsafe/runtime design、custom migration 均更新。

