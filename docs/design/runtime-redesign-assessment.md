# Runtime 架构评估与重构建议：保留行为，重整执行内核

- 评估日期：2026-10-14。
- 初评代码基线：`6dd89dc7b193c73b3d010a8846c5c4b1064a24a6`。
- 二次复核基线：`main` 的 `04fc7b0ead0e3dd6a8c49831bc2e0519782082ca`；相对初评仅增加本文，runtime 与测试源码未变化。
- 状态：设计提案，尚未实现；本次仅修订文档。
- 约束：不保留旧 Rust API、hook API、类型布局、模块路径或序列化格式的兼容层；保留有意的战斗与对外可观察行为。
- 方法：初评审阅执行链及核心定向测试；二次复核补充 `tswn_test` 的适配器、共享套件注册、golden 计算与断言、corpus 入口、会话一致性测试、Cargo feature 门控、release 脚本，以及核心 reset/并行测试。未运行 Cargo 测试或性能基准，也未逐个重新验证所有 fixture 的期望值；“已有覆盖”指已读到的源码断言，不是本次通过记录。

## 1. 结论

**建议对 runtime 的执行内核做结构性重构，但不要求从零重写整个 runtime，更不建议把现有 hook 系统换成更通用的事件总线。**

二次复核修正：初评低估了现有行为验收基础。`tswn_test` 已经有 `EngineAdapter`、87 项共享 trace/行为用例和 37 项冻结 stress 用例，后者比较逐回合摘要；另有会话一致性测试与 release 回归脚本。不能把“新建一套回归框架”列为重构前置条件。正确做法是直接复用这些资产，只调整新版引擎的测试接线，并按实际改动补强观察盲区。[R1][r1] [R2][r2] [R3][r3] [R4][r4] [R5][r5]

这一修正改变实施路线和工作量判断，不自动证明重构更迫切。现有代码已有大量行为保护；建议重构的理由仍是执行控制权分散、接口语义重叠，而不是已经证实引擎错误、性能不足或缺少测试。

问题不是“八个 hook 阶段太多”。当前复杂度的主要来源是：同一条规则需要穿过通用 hook、内置技能专用分支、递归效果结算、状态注册顺序及更新结束回调；这些地方共同决定执行顺序，而接口没有完整表达这种顺序。把几个 hook 合并、改名或藏进 trait，并不能减少需要理解的规则。[S1][s1] [S2][s2] [S3][s3]

新版应该是一个**面向本游戏的确定性执行器**：明确的行动流程、分类型的伤害/生命周期结算、集中维护的数据修改入口，以及少量按职责区分的扩展接口。内置机制不必为了经过统一 hook 框架而绕一圈；真正需要动态扩展的机制仍能接入，但不能任意改写整个 runtime。

重构边界是“谁推进规则、谁修改状态、何时允许观察”，不是文件夹的边界。保留或迁移已证明有价值的算法与行为测试；替换组织它们的方式。

### 方案比较

| 方案 | 能解决什么 | 代价与问题 | 判断 |
| --- | --- | --- | --- |
| 只拆文件、合并 Context、减少函数数量 | 局部阅读体验 | 多条执行链、隐式时序与日志控制状态仍然存在 | 不足以解决当前问题 |
| 继续完善通用 hook / event bus / 全面 ECS | 更一般的注册与组合能力 | 本游戏的顺序例外会变成更多 priority、事件重入和特殊上下文；存储优化不能自动解决结算语义 | 不选 |
| **重整执行内核，保留领域算法和既有回归套件** | 统一控制权、显式表达时序、缩小扩展权限 | 复用现有逐回合 golden；仅对迁移涉及的盲区增加状态诊断 | **推荐** |
| 同时重写解析、构建、所有绑定、数据集和 UI | 能统一整个仓库 | 扩大验证面，使数值差异和接口接线错误难以定位 | 不作为此次范围 |

这里的“整体”指统一执行职责，不是改写每个模块。可以在现有 runtime 内按切片重构，也可以用测试适配器接入新实现；两者都不需要生产兼容层。若保留旧实现作参照，只用于迁移测试，不做长期运行回退。

## 2. 当前架构：已经做对了什么，问题在哪里

### 2.1 不应抹掉的基础

当前已经有单一 `CombatRuntime`，实体 arena、世界视图、RC4、效果队列、prepared runner、trace 和模型投影也都有明确的入口。这不是“还没有 runtime，需要从零搭引擎”。[S4][s4]

`PreparedRuntimeRunner` 支持每个 worker 复用实例、恢复基线、重新应用 seed，并复用评分构造中的缓冲区。主循环已有 `CAPTURE_UPDATES` 常量路径、内联小容器和内置静态分派。新方案应保留这种热路径意识，不能以架构整洁为由退回每击分配 trait object、每局重新解析阵容。[S1][s1] [S3][s3] [S5][s5]

现有验收有多个层次：`tswn_test` 的共享 trace/分数/开局断言和逐回合冻结摘要；同 corpus 上的 session/replay 状态一致性；`tswn_core` 的完整 RC4 状态、终局短路、待生成实体、状态清除、精度与判胜定向断言。核心还已有 fresh/reset、并行胜率及评分一致性测试。不能把其中某一层未观察的字段误写成整个项目没有测试，也不能为新版通过而重生成 golden。具体范围和限制见第 9 节。[R1][r1] [R2][r2] [R3][r3] [R4][r4] [R7][r7] [S6][s6] [S7][s7] [S8][s8] [S9][s9] [S10][s10]

### 2.2 可以直接定位的结构性问题

| 观察 | 源码证据 | 架构后果 |
| --- | --- | --- |
| 通用阶段与专用行动分支共存 | `round.rs` 同时组织 PRE/POST hooks 与 `PreparedPlainAction`；主动技能可以 `ProcMask::NONE` | 增加机制前需要先判断走哪条执行方式；这不是两套完整引擎，而是一个引擎中的多种控制路径 |
| 同名阶段并不代表同一种发生点 | 行动者主线使用 `POST_DAMAGE`；`passive.rs` 另有受伤者的专用 post-damage 链 | “攻击完成”与“某实体受伤后”容易混为一谈，不能只凭 mask 理解 owner、时机和次数 |
| 内置实现会绕过普通 handler 表 | `builtin_static_skill_handler` 按 export name 选择内置函数，优先于动态表；部分内置被动绑定 noop | registry 既像通用注册表，又承担内置语义识别；仅把 handler 放到表里并不决定实际执行 |
| 行动后顺序是运行期规则，不是简单排序 | early、按状态游标插入的 deferred、延迟清理、late；中间还要检查技能 generation | 一个 `POST_ACTION` 名字隐藏了一个小型状态机；统一排序无法替代它 |
| Context 承担过多领域职责 | `SkillContext` 同时持有实体、世界、槽、队列、RNG、更新、可选受击值，并暴露护盾、蓄力、守护、复起等操作 | 通用接口不断吸收具体技能；错误阶段设置受击值只能运行时 panic |
| `QueuedEffect` 混合不同抽象层 | 同时有 Damage/Heal/AddState、FireAttack/CovidContact、SpawnWithMessage/Replay | 基础状态操作、规则程序和展示指令缺少边界 |
| 更新对象参与机制控制 | `RunUpdates.id` 用于反击去重，`on_update_end` 驱动反击链，`has_activity` 参与边界判断 | 日志不是纯观察者，不能直接替换成一个什么都不做的 NullLogger |
| 名为模板的数据会在局内变化 | `PlayerTemplate` 的技能和属性会因吞噬、分身等改变；有效属性又保存在 runtime | 初始蓝图、局内基础值、有效值之间的关系不够直观，复位与导出需要额外小心 |

依据：[S1][s1] [S2][s2] [S3][s3] [S11][s11] [S12][s12] [S13][s13] [S14][s14] [S18][s18] [S20][s20]。

这张表是可维护性判断，不是宣称这些分支都是 bug，也不是说动态分派一定慢。许多特殊分支正是为了修复行为差异而存在；应该替换它们的表达方式，而不是顺手删除它们表达的规则。

## 3. “不考虑兼容性，只考虑行为”的具体含义

可以删除或重做：`ProcMask`、`SkillContext`、`StateContext`、注册器/绑定器分离、公开可变 arena、旧槽表示、旧模块路径、旧 DTO 和编码格式。调用方直接改用新入口，不写 deprecated wrapper，不保留旧 ABI，也不要求新旧序列化数据互读。

**不能把“无需兼容旧接口”理解成“可以改掉名字叫 legacy 的规则”。** 当前 `LegacyStep` 就承载正式的名字竞技场行动条；严格大于阈值、随机数调用顺序、状态可见性等是行为，不是接口包袱。[S15][s15]

行为验收按以下层次区分：

| 层次 | 必须保持的内容 | 不必冻结的实现 |
| --- | --- | --- |
| 游戏机制 | 同输入、同 seed、同规则配置下的行动、目标、数值、状态、生命周期、终局和截断语义 | 函数名、trait、mask、字段位置 |
| 确定性 | RNG 消耗的顺序与条件、浮点/整数转换位置、遍历顺序 | 调试输出和内存地址 |
| 展示与会话 | 可见事件含义、相对顺序、目标/数值、分批边界、评分和播放延迟行为 | 字符串占位协议、旧 JSON 字段和旧 DTO 类型；文本不在此次顺带改写 |
| 批量与复用 | fresh/reset、capture/no-capture、不同 worker 数的结果一致性 | 缓存对象布局和分配容量 |
| 研究与模型 | 可观察的机制信息、无副作用读取、标签队伍来源、随机信息隔离 | 旧 tensor/schema 的二进制兼容；新版本明确标识，拒绝静默混用 |
| 扩展 | 能定义自定义主动行为、被动反应、状态、生成实体及局内数据；访问边界可验证 | 旧扩展注册步骤和任意阶段入口 |

对于疑似旧 bug：先补最小复现并明确规则决策，单独变更，不夹带在重构中。只比较最终胜率或只看“多数 seed 对上”不构成通过。

## 4. 建议的新架构

### 4.0 必做边界与可选实现分开

第一轮只要求三件事：把更新对象中的机制控制状态收回执行器；把行动尾链及不同受击路线写成可追踪的流程；把内置/扩展的绑定和调用职责理清。它们可以先在现有数据结构上完成，不以“先写完新框架”为条件。

下图中的 `BattleBlueprint`、`TurnDriver`、`ResolutionMachine` 是职责名称，不是要求立即新增一组框架类型。蓝图与局内数据可以先通过封装和命名区分；通用 continuation 栈、完整 RulesBuilder 编译框架、arena 更换、ECS 和新 crate 拆分都不是首轮前置项。若普通同步函数与有限分支足以表达流程，就保留这种更简单的实现。

```text
名字 / DIY / 阵容 / eval_rq
          │ 现有构建算法，经新的导入层
          ▼
 BattleBlueprint + CompiledRules             不可变，可共享
          │ seed / reset
          ▼
 Battle
 ├─ BattleState                             唯一权威机制数据
 │   ├─ Combatants / RuleState
 │   ├─ WorldOrder / TeamViews
 │   └─ BattleRng
 ├─ Timeline                                行动条、轮询游标
 ├─ TurnDriver                              明确的行动阶段
 ├─ ResolutionMachine                       效果结算、嵌套续体、批次结束任务
 └─ StepMeta                                 活动标记、批次身份、边界信息
          │ 只读的已发生事实
          ├─ Replay / Show projector
          ├─ ModelState projector
          └─ Debug trace / differential trace
```

`rules/namerena` 定义本游戏的行动、受击路线、目标选择、顺序、状态和特殊角色规则。kernel 负责推进、提交、保存执行位置及检查边界；它不通过 `"core.xxx"` 字符串猜技能。

建议先留在 `tswn_core` 内，不急着拆新 crate。模块职责可以是：

```text
runtime/
  blueprint.rs       # 已验证蓝图与只读规则绑定
  battle.rs          # 对外推进、reset、稳定边界
  state/             # 实体、局内规则数据、世界顺序
  timeline.rs        # 时间推进与调度结果
  turn.rs            # 一次行动的显式流程
  resolve/           # 分类型结算与 continuation
  mutation.rs        # 协调实体和索引的写入口
  observe/           # trace、展示和模型投影
rules/namerena/
  actions/           # 主动技能与目标准备
  defense.rs         # 威力层、防御层和受击路线
  reactions.rs       # 受伤、致命、击杀、批次结束反应
  statuses.rs        # 状态生命周期与属性生效点
  ordering.rs        # 行动尾链及运行期注册顺序
  numeric.rs         # 精确数值算法
```

这是职责示意，不是必须照抄的目录规范；不要添加冻结目录或禁止某字符串的“架构测试”。测试检查行为和不变量。

### 4.1 数据：蓝图只读，局内基础值可变

`BattleBlueprint` 保存构建基线、初始技能、身份、派生蓝图和规则配置。`Combatant` 保存局内可变的基础属性、当前技能等级、有效属性、HP/MP、行动点、归属及规则状态。吞噬修改后者，不修改全局共享蓝图。

需要注意两类数据不能被错误“去重”：

1. **基础属性与有效属性可能有意暂时不同。** 例如现有冰冻首次加入会触发属性刷新，已有冰冻延长不会重复提交等待生效的倍率。应明确建模待生效状态和刷新点，不改成每次读取都由基础值即时重算。[S16][s16]
2. **顺序和历史标记有时本身就是机制。** 压缩状态占据的注册位置、技能运行期启用顺序、队伍清空后的历史计数必须有真实存储，不能从当前 entries 或存活集合“重建”掉。[S9][s9] [S14][s14]

状态载荷与实际注册身份分开表达：`StateInstanceId` 标识一次登记，payload 保存规则数据；没有大型载荷的压缩状态也可以占据一个轻量顺序节点。业务不再维护一份状态、一份标记、再一份不可见排序记录而没有统一入口。

同一局中实体 ID 稳定且不复用；死亡不等于从 arena 删除。保留生成顺序和预留槽语义，不以数组压紧改变后续编号或轮询。跨 reset 的外部句柄可用 battle epoch 拒绝旧引用。

### 4.2 数据修改：集中协调，但不把所有加减法变成事件

所有跨结构变更通过命名操作协调：生成、登记死亡、复活、移除、归属变化、技能启用、状态登记/移除及属性刷新。操作同时维护实体、world 的有序索引和派生缓存失效。

只涉及当前局部数值的计算仍可直接在 resolver 内完成；不要求 `hp -= x` 都先分配一个命令。关键是外部 handler 和观察者拿不到整个 `&mut BattleState`，不能绕过索引维护。

世界中至少区分“当前存活视图”和“规则使用的历史计数”。`alive_group_count` 可以换一个准确名称，但不能直接替换成 `team_alive` 的非空数量；判胜使用当前存活事实，某些尾链短路所用的历史量另行保留。[S1][s1] [S9][s9]

## 5. 执行流程：让阶段成为程序，不是广播消息

### 5.1 一次推进不一定等于一次攻击

Timeline 返回明确结果，例如“尚无主体行动”“只有解冻事件”“选中了行动者”“已经终止”。TurnDriver 决定哪些结果形成可观察边界。不要让是否分配了 replay frame 决定机制推进。

正式名字竞技场调度保留：逐实体取 step 随机数、行动点严格 `> 2048`、减阈值、冰冻处理，以及按 world roster 而非 arena 空槽数量计算 tick 上限。[S15][s15]

`Minimal` 不再由“角色类型和 speed 是否满足条件”隐式推断。需要低层自定义对局时显式选择规则/调度配置，保留已支持的行为；纯测试用的最小场景也可以直接使用显式的测试配置。

### 5.2 行动主线

下列是应从当前实现提炼的命名步骤，不是把所有技能强行套进完全相同的循环：

```text
AdvanceTimeline
  → BeginBatch / RollSmart
  → ActionStartRules
  → StateInterception
  → PrepareAction（普通 / 强制 / 主动技能 / 状态接管）
  → ExecuteActionAndResolveConsequences
  → EvaluateTerminalTailPolicy
  → RecoverAndActionTail（仅适用时）
  → DrainBatchEndReactions
  → FinishBoundary
```

终局主体行动跳过恢复和行动后链，但不能因此不执行当前实现仍然执行的批次结束处理。状态接管、冰冻无输出返回及只有事件没有主体行动的路径分别建模，不压成一个 `bool`。[S1][s1] [S7][s7]

主动技能是一次被选择的行为，适合直接调用领域函数或已验证的 typed action handler；不应伪装成某个 PRE_ACTION hook。攻击者的攻击后行为与受伤者的受击后行为使用不同名称和输入类型。

### 5.3 行动尾链：保留顺序，删除分散拼接

用独立 `ActionTailCursor` 表达当前已经隐含存在的程序：

```text
Early abilities
  → 按运行期游标穿插 deferred abilities 与 states
  → Remaining deferred abilities
  → Deferred state expiration commit
  → Late abilities
```

游标必须显式保存当前分区、位置和延迟清理项。吞噬在执行中启用技能时，对尚未执行的部分应用登记变更；已执行条目不能重跑。一般状态执行中被移除的条目不能继续运行；“到期但延迟统一移除”则是另一种操作，不能与立即清除混淆。[S1][s1] [S8][s8]

不能为了方便，把整个行动尾链在回合开始时冻结；也不能每次 generation 变化都从头重建并执行。具体新增条目参与本轮还是后续轮，逐种登记路径按现有行为固定，而不是统一规定“新条目永远下回合生效”。

守护触发时直接刷新守护目标，是对领域操作的调用，不是再手工构造一个只保留 Protect 的 POST_ACTION 计划。该调用仍保留现有立即执行的时机。[S17][s17]

## 6. 伤害和生命周期：不能统一成一个万能 Damage

### 6.1 分离四种概念

- **Action**：行动者决定做什么，例如火球、净化、治疗或分身。
- **Resolution request**：开始哪种结算，例如标准受击、已越过威力层的伤害、持续毒伤或自爆。
- **Reaction**：在某个确定位置参与结算，例如反弹、守护、受伤后反击登记、致命救回。
- **Observed event**：已经发生的事实，供日志、动画和分析读取，不反向驱动战斗。

这四者可以都是 Rust enum 或结构体，但不能共享一个任意 payload 的通用消息总线。

### 6.2 受击路线必须显式区分

| 现有行为 | 新版表达 | 不能做的简化 |
| --- | --- | --- |
| 标准攻击处理威力、受击前反应、闪避、整数伤害及后果 | `StandardHit` 路线及明确阶段结果 | 将全部修正写成任意 priority 的 damage modifiers |
| 净化跳过 PRE_DEFEND 和闪避，但仍执行 POST_DEFEND | `DisperseHit` 领域路线，从指定阶段进入 | 给普通攻击传一串可任意组合的布尔开关 |
| 毒伤有自己的伤害/死亡/解除消息时机 | `PoisonTick` 领域操作 | 把它当作每回合施放一次普通攻击 |
| 反弹进入另一条攻击链，并在返回后扣行动点 | 嵌套攻击和 `AfterReflection` 续体 | 仅交换 caster/target 后直接扣 HP |
| 守护转移受击，递归处理保护者并短路原受击 | `RedirectHit` 结果和嵌套结算 | 先将伤害均分后分别扣血 |
| 自爆先置自身 HP，再处理目标，最后处理延迟自身死亡 | `Explosion` 的明确子步骤与续体 | HP 到零立即统一删除实体 |
| 低层直接 Damage 与正式攻击的死亡标记时点不同 | 显式的已审核路线，不靠调度模式隐式决定 | 假设所有入口完全共享一个死亡提交顺序 |

依据：[S12][s12] [S16][s16] [S17][s17]。具体攻击家族中的命中、取整和伤害附带效果继续沿现有实现逐条迁移；这张表不是声称所有家族都只需要上面几步。

接口草图（用于说明约束，不是可编译补丁）：

```rust,ignore
fn resolve_power(input: PowerDefenseInput, cx: &mut PowerDefenseCtx)
    -> PowerDefenseResult; // Continue / Redirect / Reflect / Absorbed

fn resolve_integer_damage(input: DamageDefenseInput, cx: &mut DamageDefenseCtx)
    -> DamageDefenseResult;

fn resolve_fatal(input: FatalInput, cx: &mut FatalCtx)
    -> FatalResolution;
```

`PowerDefenseCtx` 不能调用 `set_damage`，`DamageDefenseCtx` 不能修改浮点威力，`FatalCtx` 不暴露整份 world。不要用一个带多个 `Option` 的 Context 再做运行时阶段检查。

### 6.3 嵌套顺序：保留 queue 语义，不机械改成 FIFO 或 LIFO

当前 `push` 追加队尾，`push_nested` 插入队首；连续两次 nested push 会反序执行。更重要的是，每个 handler 后可能立即调用 `drain_effects_into`，它排空的是共享队列，而不只是“这个 handler 刚生成的子树”。[S3][s3] [S12][s12] [S20][s20]

因此新执行器应有有序工作队列与显式 continuation 栈，区分：追加工作、前置有序序列、开始嵌套结算、排空当前队列、返回父结算。不能只选一个 `VecDeque` 或 stack 就假定行为等价。

第一次实现可以保留同步函数调用和明确的队列 drain 点，不必先做通用字节码 VM。需要消除深递归时，再把这些调用点逐个转成有限的 continuation enum。队列里原本已有伤害/生成操作、handler 又插入嵌套工作时的执行次序，要用专门用例锁定。

### 6.4 死亡不是一个布尔条件

当前正式攻击中可能出现 `hp == 0 && alive == true` 的中间状态；自爆也利用了延迟死亡。新版需要区分 HP 降低、致命处理、死亡登记、关联实体清理、救回/复活及最终尸体处理，不能把 `alive` 简化成 `hp > 0`。[S16][s16]

每条路线明确微步骤及其中可见的状态，不强求所有路线都“先全部标死”或“最后统一提交”。状态操作可以集中实现，但调用时点由领域路线决定。

击杀反应有独立资格判断：非自杀、施加者仍满足存活条件、仍有敌人或相关 pending spawn。待生成实体的阵营要按当前规则解析，不能只读取模板中暂存的 team。亡灵成功后不再继续吞噬并消耗下一次 RNG；这些规则由结果类型表达短路，而非广播 KILL 后让所有订阅者自行猜测。[S6][s6]

判胜在规定边界发布；内部反应是否继续则使用该反应对应的终局策略。两者不能混成一个到处调用的 `is_battle_over()`。

## 7. 扩展：合并定义与实现，保留受限能力

### 7.1 构建时完成绑定

`RulesBuilder::compile()` 一次性完成元数据、实现、payload 类型、访问能力、排序规则和生成蓝图的验证，产出不可变 `CompiledRules`。存在未实现的状态/效果、无效引用或不支持的组合时，在开局前返回明确错误。

内置能力编译成 enum tag 或直接函数入口；扩展能力绑定到对应 typed handler。运行期不再按 export name 重新识别内置技能，不再要求 noop handler 证明一个由专用链执行的技能“已经实现”。

名字、导出名和局部 ID 仍可用于配置与诊断，但只能在构建时解析。绑定必须与所属 `CompiledRules` 一致，不把不同 registry 的局部 ID 混用。[S2][s2] [S3][s3]

### 7.2 减少自由度，而不是只减少接口数量

优先保留以下职责接口：主动行为、行动资格/接管、威力层受击、整数伤害层、受伤后反应、致命/击杀结果、行动尾处理。它们不是可任意 OR 的生命周期 bitmask，也不允许每个参与者拿到万能 Context。

这些接口数量可能仍接近当前阶段数量。这不构成失败：需要消除的是同一接口多种含义、任意重入和越权修改，而不是追求 hook 数字变小。

可信内置规则直接调用内部领域操作；自定义 Rust 扩展使用受限读视图、专用局内存储和验证过的操作请求。现有跨实体读取及槽修改限制所表达的行为要迁移，不能因“不兼容旧 API”而无声取消。它们是组织边界，不是针对恶意同进程 native 代码的安全沙箱。[S11][s11]

暂不引入脚本 VM、热加载、通用插件 ABI 或任意事件订阅。只有真实新需求出现时再增加能力。

## 8. 观察、批量执行、复位与模型出口

### 8.1 先抽离更新对象里的机制状态

建议把 `RunUpdates` 当前承担的以下内容移到执行器：

| 现有内容 | 新归属 |
| --- | --- |
| `id` 与反击批次去重关系 | 局内 `ResolutionBatchId`，保持相同嵌套工作的批次归属 |
| `on_update_end` | `ResolutionMachine` 的批次结束任务 |
| `has_activity` 及决定推进边界的标记 | `StepMeta`，无论是否捕获展示都维护 |
| 无帧模式保留的最后防御事件身份 | 明确的防御结果/事件摘要；展示去重部分交给 projector |
| 文本、占位符、延迟与展示帧存储 | Replay / Show projector |

反击的批次身份不能简单替换成攻击次数或全局日志计数。现有批次结束链还有最多 64 轮处理的规则；需要逐项固定达到上限时的 pending 状态和后续行为，不能随手改成“无限排空”或“限制 64 次攻击”。[S13][s13] [S18][s18]

观察者只接受已发生事实及只读快照，不获得可变 RNG。`FullCapture`、`NoCapture`、调试 trace、模型采样必须运行同一机制程序。NoCapture 可以不分配展示 payload，但不能跳过批次任务、活动标记和规则需要的摘要。

### 8.2 稳定边界与失败

对外区分“推进到了边界”“已终局”“达到调用方限制”“执行失败”。资源保护上限不能伪装为胜者。运行失败后禁止继续推进或导出假装完整的状态，直到显式 reset；取消应在约定安全点生效。

规则本身已有的上限（如反击处理轮数）与防止失控扩展耗尽资源的执行预算分开配置、记录和测试。没有经过行为决策，不以新预算截断原本合法对局。

通常完整主回合/初始化边界要求效果与执行续体都已处理完；对于按规则被停止的批次尾任务，明确其丢弃或留存规则，不能把残留队列藏在“稳定快照”里。

### 8.3 RNG 和数值

保留单场确定性随机流及初始化消耗顺序；不改成每技能一条随机流，不为“提前准备目标”多取随机数，不让日志或模型读取取随机数。

差分比较 `i`、`j` 和完整置换数组，而不是只看两个游标或最终 seed。现有测试中 `1.7000000476837158` 与 `1.7` 会导致一次伤害取整结果从 86 变为 85；删除浮点 bit 信息、提前转为定点或移动 ceil/floor 位置都不是无害清理。[S6][s6] [S10][s10]

目标选择同样保留“何时冻结候选列表、何时重新读取目标状态、何时消耗随机数”的规则，不统一改成每击重新选目标或行动开始时一次性选完所有目标。

### 8.4 Reset 与批量性能

只读蓝图和编译规则可以共享；每个 worker 独占可变 Battle、RNG 和 scratch。优先保留既有复用路径；只有状态组织变化导致必须改写 reset 时，才先采用容易审计的实现，再恢复 dirty reset 和容量复用优化。

reset 的验收不是“HP 回满”：必须清理新增实体、技能变化、状态顺序、隐藏压缩标记、归属关系、世界游标、扩展数据、批次任务、RNG、规则局内数据和缓存失效状态。已有 `reusable_runner_reset_matches_fresh_runner` 在 32 个 seed 上比较 completion summary 与输入队伍胜负；并行和评分回归也已存在。迁移时首先保留这些断言，仅在改动了 reset/状态归属的切片上补充逐边界状态比较或特定污染场景，不另建重复套件。[S5][s5] [R7][r7]

性能目标是防止显著回退，不预先宣称会加速。对相同构建模式、相同硬件与 seed 集，分别测普通 1v1、多队伍、召唤/复活密集、自定义规则、fresh/reset、capture/no-capture 的吞吐、尾延迟、分配次数和峰值内存。先记录当前基线，再确定允许的回退预算；不要拿未测量的百分比作结论。

### 8.5 模型状态不是确定性续跑存档

当前 `BattleModelState` 有意排除战斗 RNG，且只审计默认规则及初始化/完整主回合边界；它不是完整可恢复快照。新版保持这一隔离。[S19][s19]

内部差分或未来续跑存档可以包含完整 RNG、规则版本和执行位置，但使用单独类型，不能流入模型输入。模型投影保留局内机制、身份关系和原输入队伍映射，不泄漏未来胜者、随机流、trace 或进程分配编号。新 schema 可直接升级，不做旧格式转换层，但不把旧数据静默解释成新版。

## 9. 行为验收：以现有 tswn_test 为主线

### 9.1 二次复核确认的现有资产

| 现有资产 | 实际断言或用途 | 复用方式与边界 |
| --- | --- | --- |
| `EngineAdapter` + `test_engine_suite!` | 构造、主回合输出、胜者、可选队伍与 RNG 游标；宏注册 87 项共享行为用例 | 新版实现同一测试适配器即可接共享 suite，不保留旧生产 API |
| `suite/` 与 fixtures | 规范化回放、事件顺序、分数、胜者及特定开局；含大型、多队伍和召唤场景 | 保持现有期望值；87 是仓库定义的用例清单，不表示 87 个用例全都断言完整状态或完整回放 |
| 37 项 stress + `golden.json` | 输入 SHA-256、`eval_rq`、胜者队伍、回合数、总分、最终 RC4 `i/j`、每回合摘要 | 冻结 golden 作为独立期望；不是只比较最终胜率 |
| `battle_session_replay_parity_on_frozen_corpus` | 同 37 份输入的初始状态、逐帧 payload、最终状态、终止原因、胜者和推进计数 | 已检查 clone/summon/shadow/zombie、苏生/守护/魅惑及空 Runtime 回合；另有 round budget/icons 用例 |
| 核心定向与 batch 测试 | 完整 RC4 数组、具体数值/顺序、存活索引；32 个 seed 的 reset/fresh completion；不同 worker 的胜率/评分统计及分块边界 | 继续保留；摘要级、状态级和单机制断言作用不同，不互相替代 |
| `check_runtime_release.py --corpus` | release 核心、`no_debug` 核心、release CLI 定向测试和 release corpus | 已有发布验证入口，不需要重建同类调度脚本 |

依据：[R1][r1] [R2][r2] [R3][r3] [R4][r4] [R5][r5] [R7][r7] [R9][r9] [S6][s6] [S9][s9]。

**37 项 stress 的逐回合摘要确实覆盖了推进边界。** `capture_case` 每推进一次都生成 digest，包括该回合没有可见事件的情况。digest 输入含 round index、事件数、事件 kind、message、caster name、score、回合后 RC4 `i/j` 和 winner team。摘要比较失败时已经能报告首个不一致回合；共享 trace 的比较器也已有邻近行上下文与 trace/RC4 调试开关。初评不能把这些诊断能力当作待新建资产。[R2][r2] [R3][r3]

不过需准确区分覆盖粒度：

- `EngineAdapter::rc4_state` 与 `Rc4Golden` 只有 `i/j`，没有完整置换数组或完整消耗计数。`NormalizedRngCheckpoint` 另外在非 `no_debug` 下保存 `byte_count`，也不保存置换数组；核心定向测试中则已有完整 RC4 比较。因此“corpus 已覆盖完整随机状态”和“项目完全没有完整随机状态测试”都不准确。[R1][r1] [R2][r2] [R8][r8] [S6][s6]
- stress digest 不直接包含 HP/MP、技能/状态注册顺序、pending 队列、裸 target ID 或动画 delay；其中部分信息通过事件消息间接体现。session/replay 测试会比较更完整的帧和状态，但它比较的是同一实现的两种入口，不是另一个独立引擎的正确性证明。[R2][r2] [R3][r3] [R4][r4]
- 共享 trace 会按已有规则规范化标记和空白，部分用例还有名称噪声归一化；不能把所有共享 trace 通过表述为“每个展示字节都已验证”。保持现有规范化边界，不在迁移时扩大忽略范围来掩盖差异。[R3][r3] [R9][r9]

这些是断言的观察边界，不是已经发现的缺陷，也不等于对应机制没有通过黑盒回归被覆盖。

### 9.2 新 runtime 如何直接接入

共享的 87 项用例保持输入、断言与 fixture 不变，新增测试侧 `NewRuntimeEngine: EngineAdapter`，然后用 `test_engine_suite!(NewRuntimeEngine)` 注册。原地替换实现时也可直接改现有 `RuntimeEngine` 的接线。适配器输出现有 `EventSnapshot` 与回合边界；不要在新生产 API 中实现旧 RuntimeRunner/Context 兼容层。

37 项 stress 需要一次很小的测试接线改造：`capture_case<E: EngineAdapter>` 已经泛型化，但 `build_runtime_runner` 和公开 `assert_runtime_matches_frozen_golden*` 仍绑定当前 `RuntimeEngine`。将构造后的 runner 注入现有泛型捕获器，或泛型化相应 helper，即可让新版消费**同一份** `STRESS_CASES` 和 `golden.json`。不能误说“只新增适配器，现有 37 项就会自动测试新版”。构造必须保留各 case 的 `eval_rq` 和 `unescape_x02`，这些输入差异不能丢失。[R2][r2] [R10][r10]

复用原 digest 实现的编码、字段顺序和最终摘要算法，不另写一个近似格式，不因 DTO 改名重算期望值。输入 hash 允许的换行规范化继续沿现有实现。

有必要观察新旧隐藏状态时，才在同一 corpus 输入或已有最小回归上添加**测试专用**记录：先复用 `NormalizedOutcome`，再按此次改动补充完整 RNG、状态登记顺序、批次任务等缺失字段。只在失败时按需展开微步骤；不把全量续跑存档、通用 trace 平台或另一套基线生成器当作先决条件。[R8][r8]

已有 `scripts/tswn_diff.py round` 用于复核历史 Bun/tswn 的 round 分歧，但需要 PostgreSQL、Bun 和外部 `md5.js`。它可在需要时辅助溯源，不是运行冻结 corpus 的依赖，也不应作为每个重构切片的必跑条件。[R11][r11]

### 9.3 按实际改动补强，不重复建设

以下是迁移验收清单，不是“当前缺失测试清单”。标记为需检查的组合，先查已有 fixture/断言，只有观察粒度不足才加测试；本次静态审阅不宣称已穷尽全仓测试。

| 重构切片 | 先复用的现有资产 | 仅在需要时补强 |
| --- | --- | --- |
| 行动条、主动/强制行动、状态接管 | 共享 suite、逐回合 digest、scheduler 与终局定向测试 | 被改动路径的行动点、主体行动边界和完整 RNG 对照 |
| 受击、反弹、守护、净化、毒、自爆 | 共享/corpus 黑盒回归及 `plain_*`、`summon_explode_*` 核心测试 | 若改变 drain 点或递归方式，比较队列已有工作时的顺序和延迟死亡中间态 |
| 吞噬与行动尾链 | 共享 trace、corpus、状态 generation/kill/charge 等定向测试 | 迁移游标时核对已执行条目不重跑、运行期登记及立即清除/到期清除差别 |
| 抽离反击与更新批次 | counter 及伤害回归，capture/no-capture 执行路径 | 核对批次身份、改变目标、批次尾任务和 64 轮上限的精确边界；先审计已有断言 |
| 实体/世界视图与判胜 | corpus、`winner_path_divergence_tests`、会话最终状态 | 已变动的死亡/复活/归属入口在同一边界保持索引和历史计数语义 |
| reset 与批量复用 | `reusable_runner_reset_matches_fresh_runner`、评分缺陷回归、并行/分块测试 | 仅在移动状态或改复位算法时补逐边界状态、残留任务与污染上一局场景 |
| 展示、会话与模型 | session/replay 全 corpus parity、模型审计与核心模型测试 | 更换 projector/控制状态后增加跨观察模式比较；模型继续排除 RNG |
| 自定义规则和绑定 | 核心 custom、handler、capability 测试及既有绑定验收 | 改成新接口后保留对应行为断言；未绑定状态/效果的构建期验证按新边界补充 |

补强时以首个分歧定位为目标，不要求给每个执行阶段永久保存一份新的 golden。内存容量、地址和缓存不是行为；局内批次关系、状态登记游标及有意的历史计数则不能被归一化掉。

### 9.4 必须启用的回归入口

`crates/tswn_test/Cargo.toml` 的 `[[test]] name = "runtime"` 指向 `runtime_corpus.rs`，并声明 `required-features = ["runtime-corpus"]`。普通 `cargo test --workspace` 或不带 feature 的 `cargo test -p tswn_test` 不会执行这 124 项 corpus 目标；但 `tswn_test` 的默认库测试仍会执行其中的会话测试等，不能说默认命令完全不碰 stress 输入。[R1][r1] [R4][r4] [R6][r6]

正式实施时优先使用现有脚本，并补足它没有运行的组合：

```sh
# 既有入口：release / no_debug 核心、CLI 与 release corpus
python scripts/check_runtime_release.py --corpus

# 明确运行完整 corpus 的 debug 构建
cargo test -p tswn_test --features runtime-corpus --test runtime

# 补充组合：让 tswn_test 的被测 tswn_core 也开启 no_debug
cargo test -p tswn_test --features runtime-corpus,tswn_core/no_debug --test runtime --release

# 默认库测试包含 session/replay 跨模块验收；同时回归整个工作区
cargo test --workspace
cargo +nightly fmt --check
```

完整 release corpus 的独立命令为 `cargo test -p tswn_test --features runtime-corpus --test runtime --release`，已由脚本的 `--corpus` 分支调用。脚本不带 `--corpus` 时只对 corpus 执行 `--no-run`；脚本中的 `no_debug` 是核心定向测试，不等于已经执行了 `no_debug` 的完整 corpus。[R5][r5]

这些是从现有配置整理的实施命令，不是本次运行记录。二次复核时本机 Windows 执行连接仍离线，当前容器没有 Cargo 且无法解析 GitHub 域名，未能实跑 Rust 回归；不把源码检查或此前项目中的通过记录当作本次通过。

## 10. 实施顺序与停止条件

### 阶段 A：启用既有门禁，接入被测实现

先运行 `check_runtime_release.py --corpus` 和必要的 debug/no_debug 组合，记录基线 SHA、构建模式及实际执行的测试目标；不要用默认 `cargo test` 代替完整 corpus。保持 87 项共享用例、37 项 stress 输入和 golden 不变。

新版通过 `EngineAdapter` 接共享套件，调整 stress helper 的构造接线以复用既有泛型捕获器。当前代码已经能定位 trace 行或首个不一致回合；只有首批重构涉及的内部状态无法定位时才扩展测试观察记录，不先建设另一套差分框架。

通过条件：确认新实现实际被现有 suite/corpus 调用，没有误跑旧实现、遗漏 feature 或扩大规范化范围；本阶段的交付是测试接线与基线运行记录，而不是新框架。若尚无新实现，则先完成当前基线记录，接线随首个实现切片交付。

### 阶段 B：抽离控制状态，收拢数据修改

优先搬走 `RunUpdates` 的反击批次/回调/活动标记；建立蓝图与局内状态的区分，统一生成、死亡、复活、状态登记等写入口。此时领域公式尽量不变。

通过条件：捕获与不捕获模式逐边界一致，reset 与 fresh 一致；观察器不能改变 RNG 或推进。

### 阶段 C：先贯通高风险的受击与生命周期切片

用新结算器贯通标准攻击、反弹、守护、净化、毒、自爆、致命救回和击杀链。先把共享队列 drain 与递归返回时点表达清楚，再考虑 continuation 优化。

通过条件：组合链的微步骤、事件、RNG 与结果全量对上。若差分仍无法解释，不继续批量搬运更多技能。

### 阶段 D：接管完整行动与动态尾链

迁移 Timeline、行动选择、状态接管、ActionTailCursor 和剩余内置规则；把自定义机制迁移到构建期绑定的 typed 接口。删除通用/内置双分派与旧万能 Context，而非留着成为第二条路径。

通过条件：全部行为矩阵通过；新增一项技能无需同时修改通用 Context、核心字符串分派和独立的 post-damage 名单。该条件通过代码审阅验证，不写冻结源码形状的测试。

### 阶段 E：切换调用方，优化并清理

直接修改 CLI、会话、C/Python/WASM 绑定及模型/批量入口使用新 Battle；不写旧 API 转发层。依据已测基线恢复缓存和复用优化。保留黄金用例和独立差分资产，移除正式构建中的旧执行内核及迁移脚手架。

通过条件：工作区与必要目标平台验证完成，模型/展示出口经过审计，性能回退在事先约定预算内，没有为了通过测试而重新定义规则。

**停止扩大重构范围的条件：** 如果阶段 A–C 表明收拢控制状态和关键 resolver 已经解决主要维护问题，可以保留现有 arena 和算法模块的实现，不为“新版必须全新”继续改写。目标是消除多处控制同一行为，不是重写行数。

## 11. 本次评估的边界与最终建议

二次复核确认：初评没有审阅项目的主要跨引擎验收层 `tswn_test`，因此把阶段 A 写成新建基线/差分能力并遗漏 `runtime-corpus` 执行入口是不准确的，现已修正。现有套件应作为重构主验收资产，而非被替代。

本次确认了执行职责重叠、已有断言的具体观察范围及少数可按需补强的盲区；没有证明现有 runtime 错误，没有完成全部技能和 fixture 的穷尽审计，也没有测得性能收益。推荐结构性重构不等于要求所有数据结构和模块重写。

最终建议是：**复用现有 `tswn_test`，分切片重整 runtime 的执行内核。保留规则的复杂性，去掉表达这些规则时额外制造的复杂性。**

不要把目标设成“hook 从八个减到三个”，也不要“所有东西都是事件”。更好的目标是：阅读一个主动技能能够找到它的完整主体，阅读一种受击路线能够看到其全部阶段，阅读行动尾链能够解释吞噬和状态穿插，关闭日志不会改变控制流程，复位不会漏掉机制状态。

## 源码依据

S 系列链接固定到初评代码 SHA；R 系列固定到二次复核 SHA。两者 runtime 与测试源码相同，后者仅多出初版评估文档。

- [S1：行动主线与行动尾链][s1]
- [S2：扩展元数据、ProcMask 与注册契约][s2]
- [S3：readiness、内置静态分派与 handler 后 drain][s3]
- [S4：Runtime 模块与已有分层][s4]
- [S5：PreparedRuntimeRunner 与 reset][s5]
- [S6：击杀资格、pending spawn、亡灵/吞噬及 RNG 测试][s6]
- [S7：终局行动短路测试][s7]
- [S8：状态变更后的执行测试][s8]
- [S9：存活索引与历史计数测试][s9]
- [S10：浮点精度影响伤害的测试][s10]
- [S11：SkillContext 的借用范围与专用操作][s11]
- [S12：效果类型及具体结算路线][s12]
- [S13：RunUpdates 的数据与控制职责][s13]
- [S14：实体基底、运行期状态与顺序模型][s14]
- [S15：行动条及 hook 计划][s15]
- [S16：伤害应用、死亡中间态与属性刷新][s16]
- [S17：守护和受击前分段处理][s17]
- [S18：专用受伤链、反击批次及 64 轮处理上限][s18]
- [S19：模型状态投影与随机信息隔离审计][s19]
- [S20：队列、handler 类型与展示出口定义][s20]
- [R1：测试适配器与 87 项共享 suite 注册][r1]
- [R2：37 项 stress 清单、digest 与冻结 golden 断言][r2]
- [R3：RuntimeEngine 接线、trace 收集/规范化与差异上下文][r3]
- [R4：完整 stress 输入的 session/replay 一致性][r4]
- [R5：release/no_debug/corpus 验证脚本][r5]
- [R6：runtime-corpus feature 门控与测试目标][r6]
- [R7：reset/fresh、评分、并行及分块回归][r7]
- [R8：已有行为归一化记录及 RNG 观察范围][r8]
- [R9：共享完整 trace、分数和开局断言示例][r9]
- [R10：runtime corpus 集成入口][r10]
- [R11：历史差分与跨语言验证工具的现有依赖说明][r11]
- [R12：冻结 golden 的版本、数量与摘要格式][r12]

[s1]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/round.rs
[s2]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/extension.rs
[s3]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/lifecycle.rs
[s4]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/mod.rs
[s5]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/runner.rs
[s6]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/tests/plain_kill_hook_tests.rs
[s7]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/tests/plain_terminal_round_tests.rs
[s8]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/tests/state_hook_generation_tests.rs
[s9]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/tests/winner_path_divergence_tests.rs
[s10]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/tests/plain_at_boost_precision_tests.rs
[s11]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/effect/skill_context.rs
[s12]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/effects.rs
[s13]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/update.rs
[s14]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/docs/core-guide/runtime-model.md
[s15]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/scheduler.rs
[s16]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/defense.rs
[s17]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/defense.rs#L28-L290
[s18]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/combat/passive.rs
[s19]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/docs/design/battle-model-state-audit.md
[s20]: https://github.com/shenjackyuanjie/tswn-core/blob/6dd89dc7b193c73b3d010a8846c5c4b1064a24a6/crates/tswn_core/src/runtime/effect.rs

[r1]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/src/lib.rs
[r2]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/src/golden.rs
[r3]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/src/suite/mod.rs
[r4]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/src/battle_session.rs
[r5]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/scripts/check_runtime_release.py
[r6]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/Cargo.toml
[r7]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_core/src/runtime/batch.rs
[r8]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_core/src/runtime/normalized.rs
[r9]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/src/suite/simple.rs
[r10]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/runtime_corpus.rs
[r11]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/scripts/README.md
[r12]: https://github.com/shenjackyuanjie/tswn-core/blob/04fc7b0ead0e3dd6a8c49831bc2e0519782082ca/crates/tswn_test/cases/runtime_stress/golden.json
