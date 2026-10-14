# Runtime 架构评估与重构建议：保留行为，重写执行内核

- 评估日期：2026-10-14。
- 评估基线：`main` 的 `6dd89dc7b193c73b3d010a8846c5c4b1064a24a6`。
- 状态：设计提案，尚未实现；本次提交仅增加本文。
- 约束：不保留旧 Rust API、hook API、类型布局、模块路径或序列化格式的兼容层；保留有意的战斗与对外可观察行为。
- 方法：审阅当前执行链、数据结构、上下文、队列、复用入口及代表性行为测试。未运行 Cargo 测试或性能基准；下文不把静态判断当成已验证的性能收益，也不把建议增加的测试写成已通过。

## 1. 结论

**建议整体重构 runtime 的执行内核，但不建议推倒整个 tswn-core，更不建议把现有 hook 系统换成更通用的事件总线。**

问题不是“八个 hook 阶段太多”。当前复杂度的主要来源是：同一条规则需要穿过通用 hook、内置技能专用分支、递归效果结算、状态注册顺序及更新结束回调；这些地方共同决定执行顺序，而接口没有完整表达这种顺序。把几个 hook 合并、改名或藏进 trait，并不能减少需要理解的规则。[S1][s1] [S2][s2] [S3][s3]

新版应该是一个**面向本游戏的确定性执行器**：明确的行动流程、分类型的伤害/生命周期结算、集中维护的数据修改入口，以及少量按职责区分的扩展接口。内置机制不必为了经过统一 hook 框架而绕一圈；真正需要动态扩展的机制仍能接入，但不能任意改写整个 runtime。

重构边界是“谁推进规则、谁修改状态、何时允许观察”，不是文件夹的边界。保留或迁移已证明有价值的算法与行为测试；替换组织它们的方式。

### 方案比较

| 方案 | 能解决什么 | 代价与问题 | 判断 |
| --- | --- | --- | --- |
| 只拆文件、合并 Context、减少函数数量 | 局部阅读体验 | 多条执行链、隐式时序与日志控制状态仍然存在 | 不足以解决当前问题 |
| 继续完善通用 hook / event bus / 全面 ECS | 更一般的注册与组合能力 | 本游戏的顺序例外会变成更多 priority、事件重入和特殊上下文；存储优化不能自动解决结算语义 | 不选 |
| **重写执行内核，保留领域算法和行为基线** | 统一控制权、显式表达时序、缩小扩展权限 | 需要严肃的逐边界差分验证 | **推荐** |
| 同时重写解析、构建、所有绑定、数据集和 UI | 能统一整个仓库 | 扩大验证面，使数值差异和接口接线错误难以定位 | 不作为此次范围 |

这里的“整体”指目标架构完整替换，不是一次提交重写全部代码。实施可以分段，但正式运行只保留一条新执行链；旧引擎仅作为迁移期间的测试参照，不做长期运行回退。

## 2. 当前架构：已经做对了什么，问题在哪里

### 2.1 不应抹掉的基础

当前已经有单一 `CombatRuntime`，实体 arena、世界视图、RC4、效果队列、prepared runner、trace 和模型投影也都有明确的入口。这不是“还没有 runtime，需要从零搭引擎”。[S4][s4]

`PreparedRuntimeRunner` 支持每个 worker 复用实例、恢复基线、重新应用 seed，并复用评分构造中的缓冲区。主循环已有 `CAPTURE_UPDATES` 常量路径、内联小容器和内置静态分派。新方案应保留这种热路径意识，不能以架构整洁为由退回每击分配 trait object、每局重新解析阵容。[S1][s1] [S3][s3] [S5][s5]

现有测试不只验证胜者，也覆盖 RNG 完整状态、终局短路、待生成实体、状态清除、精度及批量判胜一致性。这些是重构资产，不能为了让新版通过而直接重生成 golden。[S6][s6] [S7][s7] [S8][s8] [S9][s9] [S10][s10]

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

只读蓝图和编译规则可以共享；每个 worker 独占可变 Battle、RNG 和 scratch。第一版先实现容易审计的正确 reset，再迁移 dirty reset 和容量复用优化。

reset 的验收不是“HP 回满”：必须清理新增实体、技能变化、状态顺序、隐藏压缩标记、归属关系、世界游标、扩展数据、批次任务、RNG、规则局内数据和缓存失效状态。以 `fresh(seed)` 对比经历复杂上一局后的 `reset(seed)`，还要验证评分场景更换 roster 的路径。[S5][s5]

性能目标是防止显著回退，不预先宣称会加速。对相同构建模式、相同硬件与 seed 集，分别测普通 1v1、多队伍、召唤/复活密集、自定义规则、fresh/reset、capture/no-capture 的吞吐、尾延迟、分配次数和峰值内存。先记录当前基线，再确定允许的回退预算；不要拿未测量的百分比作结论。

### 8.5 模型状态不是确定性续跑存档

当前 `BattleModelState` 有意排除战斗 RNG，且只审计默认规则及初始化/完整主回合边界；它不是完整可恢复快照。新版保持这一隔离。[S19][s19]

内部差分或未来续跑存档可以包含完整 RNG、规则版本和执行位置，但使用单独类型，不能流入模型输入。模型投影保留局内机制、身份关系和原输入队伍映射，不泄漏未来胜者、随机流、trace 或进程分配编号。新 schema 可直接升级，不做旧格式转换层，但不把旧数据静默解释成新版。

## 9. 行为验收矩阵

“已有依据”表示本次读到了实现或测试，并不表示本次执行了它们。

| 场景 | 必须比较的结果 | 已有依据 / 新版需要补充 |
| --- | --- | --- |
| 行动条等于/大于阈值、解冻、预留生成空槽 | actor、行动点、边界、RNG | `scheduler.rs`；扩展边界组合用例 |
| 主动技能、普通/强制攻击、状态接管 | 主体行动、目标、RNG 与尾链是否执行 | `round.rs`；逐路线 golden |
| 终局普通行动 | 不恢复 MP、不再执行行动后状态、事件尾部 | `plain_terminal_round_tests.rs` |
| 击杀最后一敌 / 尚有敌人 / 冻结敌人 / pending spawn | 击杀链资格、生成阵营、RNG | `plain_kill_hook_tests.rs` |
| 亡灵成功与失败后吞噬 | 短路、尸体类型、后续 RNG | 同上 |
| 反弹、递归守护、净化、毒、自爆 | 入路、嵌套顺序、属性、伤害、死亡时点、事件 | `combat/effects.rs`、`defense.rs`；补共享队列已有工作的组合用例 |
| HP 为零但尚未登记死亡、致命救回、关联使魔清理 | 微步骤可见状态、world 顺序、最终存活 | `defense.rs`；补跨路线组合用例 |
| 吞噬中途启用技能、early/deferred/state/late | 运行期顺序、只执行一次、状态延迟清理 | `round.rs`；补完整 action-tail trace |
| 立即清除后续状态 | 被移除状态不再执行 | `state_hook_generation_tests.rs` |
| 反击批次内多次受击、改变目标、连锁到上限 | 去重范围、RNG、批次尾任务和残留状态 | `passive.rs`；补 64 轮边界用例 |
| 队伍清空后被魅惑施法者复活 | 存活索引与实体一致、无提前胜者、历史计数不被误替换 | `winner_path_divergence_tests.rs` |
| at_boost 浮点尾数 | bits、取整位置、伤害 | `plain_at_boost_precision_tests.rs` |
| Fresh vs reset，包括更换评分 roster | 每边界完整机制状态与 RNG | `runner.rs`；补污染上一局的 reset 用例 |
| FullCapture / NoCapture / trace / model | 同一推进边界与机制状态，不因观察方式改变战斗 | `round.rs`、`update.rs`、模型审计；补跨模式差分 |
| 自定义规则、无效绑定、无权限读取、生成蓝图 | 行为可表达、开局前错误、访问拒绝、引用正确 | `extension.rs`、Context 和 readiness；迁移为新接口测试 |
| 模型读取与内部 checkpoint | 重复读取无副作用；模型不含随机秘密；标签映射正确 | `battle-model-state-audit.md` |

### 差分方式

固定旧实现 SHA，构建测试专用参照程序或保存逐边界基线。新旧代码可以通过独立进程输出同一种测试归一化记录，无需在生产 API 中兼容旧类型。

记录至少包含：主体行动、规则微步骤（用于定位首个分歧）、实体存在/死亡/归属、基础和有效属性、完整技能与状态顺序、world 列表及游标、待生效数据、事件序列、当前终局/截断状态和完整 RNG。允许忽略容量、地址、派生缓存和进程全局日志 ID；局内批次关系、注册游标等会影响机制的信息不能被归一化丢掉。

将游戏行为的变化与展示格式的变化分开比较。现有 golden 不自动重生成；发生差异时缩小到最早的规则步骤，再补最小回归。不同 worker 数使用相同的 seed 分配和结果排序验证，不能只比较聚合胜率。

正式实施时建议至少运行：

```sh
cargo test -p tswn_core
cargo test -p tswn_core --features no_debug
cargo test --workspace
cargo +nightly fmt --check
```

这些是后续实施的验证命令，不是本次运行记录。绑定/目标平台构建、差分工具和性能基准还需按具体实施提交补充。

## 10. 实施顺序与停止条件

### 阶段 A：把行为边界写成可执行基线

交付固定 SHA 的参照、归一化记录、跨 capture/reset 的差分工具，以及本矩阵中的高风险组合用例。先分清空推进、事件边界、主体行动和终局，不动正式数值规则。

通过条件：能定位到第一个行为分歧，而不只是得到一个不同胜率。参照来自测试构建，不在生产中引入双 runtime 开关。

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

本次确认了当前存在的控制路径、接口职责重叠和若干必须保留的细粒度行为；没有证明现有 runtime 全部正确，没有完成全部技能的穷尽审计，也没有测得任何性能提升。测试文件名与断言是行为证据，不是本次测试通过记录。

最终建议是：**启动一次有明确行为基线的 runtime 内核重构。保留规则的复杂性，去掉表达这些规则时额外制造的复杂性。**

不要把目标设成“hook 从八个减到三个”，也不要“所有东西都是事件”。更好的目标是：阅读一个主动技能能够找到它的完整主体，阅读一种受击路线能够看到其全部阶段，阅读行动尾链能够解释吞噬和状态穿插，关闭日志不会改变控制流程，复位不会漏掉机制状态。

## 源码依据

以下链接固定到评估 SHA；正文中的“当前”均指该版本。

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
