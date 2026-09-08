# BattleModelState v1：Runtime 状态审计

审计边界是默认规则、初始化完成或完整主回合结束。导出目标是当前机制信息，不是用于恢复整个 Runtime 的存档格式。`ModelStateError` 拒绝不同注册表、非空效果队列、文本槽和无法映射的实体引用；高层 session 在失败后也拒绝导出。

## 字段来源

| Runtime 来源 | 导出与理由 |
| --- | --- |
| `CombatRuntime.round` | 保留 round，区分当前实际推进位置 |
| `EntityArena` | 保留所有实体及 ID 槽总数；包括死亡实体和生成前预留的空槽，后者影响新实体编号 |
| `RuntimeRunner.input_groups` | 保留输入队伍成员，标签依赖此映射，不使用重排后的 runtime team 作为标签 |
| `WorldArena` | 保留 round_order、round_pos、team_roster、team_alive、flat_alive、alive_group_count 及当前 winner_team；这些列表的顺序影响行动和目标选择 |
| `PlayerRuntime` | 保留全部当前属性、alive、移动点、kind、flags、policies、owner/root_owner、team、蓄力、积聚、护盾、保护链、升级、隐藏、暗杀、反击和尸体状态；反击更新批次 ID 单独排除 |
| `PlayerTemplate` | 保留原始属性、max_hp、技能表、类型、team、出生保留槽数、policy overrides、clone_build 和重施/继承策略；身份文本替换为 ModelIdentity |
| `CloneBuildData` | 保留八围、武器属性增量、父/子名字系数 bit、属性调整和 score 技能加成计划，避免分身/合并后无法表达构造参数 |
| `SkillLoadout` | 导出每个 lane 的稳定技能 ID、level、build_level、boost 构成、boosted、fixed_lane_key，以及 merge/active/pre_action/post_damage/deferred 顺序；不是单个技能等级列表 |
| `StateStore` | 保留所有条目、运行期注册顺序、下次注册游标和压缩状态位；压缩的 Shield/Protect/Upgrade/Corpse/Minion 不会因不在 entries 中而丢失 |
| `StateEntry` | 保留 legacy key、extension state ID、hook mask、priority、注册顺序及完整 payload |
| `StatePayload` | 16 个分支穷尽匹配，含毒、魅惑、加速、Boss 感染记录、恢复标志、一拳超人命中者/使魔关系等；Rust 增加新分支会使投影匹配无法编译 |
| `EntitySlotStorage` | 按已审计注册槽导出 Bool/I64/U64/模板；包括召唤计数、记忆目标、蓝图及延迟蓝图参数 |
| `TemplateSlotStorage` / `BattleSlotStorage` | 导出实际存在的机制值，保留槽 ID；未知文本不能静默进入特征 |
| `PhaseScheduler` | 保留 Minimal/LegacyStep 模式和待处理 ice_release_events；读取不清空事件 |
| `EffectQueue` | 边界必须排空；非空时拒绝导出，不将中途执行队列伪装为完整帧末状态 |

## 身份与延迟构造

原名字、id_key_name、clan_name、display_name 不出现在模型状态中。`ModelIdentity` 保存：

- 阵营相等关系的局内编号；编号只用于关系，不是名字编码。
- 内置 Boss 种类和名字决定的主动触发次数。
- BOOST 免疫阈值，以及各类 Boss 状态免疫阈值。

已经存在的召唤模板投影成 `ModelTemplate`。延迟蓝图使用引擎同一个纯计算入口预览，保留派生技能顺序、属性和构造参数，但不写 entity slot、不消耗战斗 RNG。蓝图计算中使用的名字派生随机化只产生机制属性，不导出其随机流或名字哈希。

每次采样表达当时实体及当时可派生的一层蓝图；之后新生成实体的机制状态在后续采样时导出。本 schema 不承诺在没有名字和战斗 RNG 的情况下重建或无限续跑原局，因此不能作为 Runtime 序列化存档使用。

## 明确排除

| 排除项 | 理由 |
| --- | --- |
| prepared seed、`CombatRuntime.rng`、trace 中的 RNG checkpoint | 禁止泄漏战斗随机状态 |
| 最终 winner、最终轮数、progress、截断原因 | 仅作标签/审计，不属于尚未结束局面的输入；state 中的 winner_team 仅是当前事实，采样器拒绝已决状态 |
| `CounterRuntime.last_updates_id` | RunUpdates 的全局或线程局部批次编号，仅用于同一批更新内反击去重；完整回合后后续更新使用新编号。保留 pending/target，但不把分配计数混入特征 |
| skill/slot baseline ID、dirty mask、generation、action/hook cache | 缓存身份、失效计数或派生计划，不是独立战斗机制 |
| `StateStore` 的 hook 汇总、scheduler flags、payload kind 缓存 | 从实际状态条目派生；缓存可能是保守超集，不能把缓存历史当特征 |
| `BattleScratch.selected_actor_round` | 每次行动选择时重写的临时值 |
| handler 函数指针、renderer、registry 实例 | 由默认规则与引擎版本约束；不把进程地址当特征 |
| 展示快照、图标、文本更新、日志、内存容量 | 展示或分配细节；Runtime 更新是否非空只用于确定采样边界 |

## 契约与校验

技能映射由 `MODEL_SKILL_EXPORTS` 固定，当前 42 项占用约定的 1–50 空间中的 1–42。新增处理器或改变含义必须更新审计和 schema，而不能静默重新解释已有数据。

`BattleModelState::validate` 检查实体 ID、空槽范围、输入队伍、世界顺序、owner/root_owner、保护链、暗杀/反击目标、毒/魅惑及感染/Boss 引用。Parquet 校验层另外检查对局行序、标签唯一性、样本轮数、终局排除、样本计数和集合隔离。

测试覆盖同 seed 逐边界比对、重复读取不改变实体/世界/RC4、当前注册表全部技能编号、内置 Boss、DIY 和动态生成、压缩状态与注册游标、实体数组重排、不同线程数生成及续跑。golden/corpus 基线不因本功能重生成。
