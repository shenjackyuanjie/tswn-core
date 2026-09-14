# 战斗状态导出与胜率数据生成

本文规定第一版状态导出和数据生成契约；字段审计见 [battle-model-state-audit.md](battle-model-state-audit.md)，运行命令见 [生成器说明](../../crates/tswn_winprob_dataset/README.md)。

## 目标与边界

最终目标是在战斗进行中，根据当前机制状态预测各输入队伍最终获胜概率。本轮交付 Rust 状态导出、无展示推进、Parquet 数据生成和校验，不包含模型训练、推理、Python/WASM session 查询或网页 UI。

预测对象是最终胜者。`max_rounds` 和 `no_progress` 只是生成时的计算保护：截断局仍保存状态，标签为空，不强行标负、不增加“未决胜者”类别。默认训练读取器排除空标签；训练前必须检查截断比例和分布，以免把有标签子集误认为完整对局分布。

覆盖当前默认规则中的普通角色、内置 Boss、已支持的 DIY、分身、幻影、使魔、丧尸及复活。第三方注册表或未审计状态不在契约内，导出时明确报错。导入成功但执行失败的低层自定义配置会终止数据生成并保存复现信息。

## 状态与 Rust 接口

`runtime::model_state::BattleModelState` 是版本化、类型化的机制数据。包含输入队伍、实体、模板、当前属性、技能、状态载荷、引用关系、世界顺序、调度信息和运行时槽。整数、布尔值和浮点 bit 按原精度保留；暂不统一转成 f32 或固定张量。

```rust
BattleSession::model_state() -> Result<BattleModelState, ModelStateError>
BattleModelSession::new(raw, options) -> CliApiResult<BattleModelSession>
BattleModelSession::from_prepared(prepared, seed, max_rounds) -> CliApiResult<BattleModelSession>
BattleModelSession::next_frame() -> CliApiResult<Option<BattleModelFrame>>
BattleModelSession::model_state() -> Result<BattleModelState, ModelStateError>
BattleModelSession::result() -> Option<BattleModelOutcome>
```

`from_prepared` 的 `eval_rq` 已固化在 prepared 中，不重复接收可能冲突的配置。状态只能在初始化完成或完整回合结束后读取；高层 session 保证此边界。直接调用 `RuntimeRunner::model_state()` 的调用方必须遵守相同约束；效果队列未排空时拒绝导出。

模型状态不包含战斗 seed、RC4 状态、未来结果、最终进度、日志、图标、显示名或原始名字哈希。名字和阵营文本只存在审计输入中；名字决定的免疫阈值、Boss 身份、阵营相等关系及召唤蓝图派生属性进入状态。蓝图预览是只读计算，不推进战斗 RNG，也不写入运行时缓存。

输出标签和 `input_team_index` 均指输入队伍；运行时队伍编号及魅惑的临时阵营另行表达。实体 ID 是引用键，不应作为连续数值特征。调整实体数组排列必须同步处理引用，并保留真正影响战斗的行动顺序、存活列表顺序和游标。

技能编号空间固定为 1–50。当前默认注册表实际有 42 项，按现有顺序映射到 1–42，43–50 预留；`MODEL_SKILL_EXPORTS` 与测试锁定映射。技能 ID、legacy key 和 fixed lane 是不同概念，不能互换。

`BattleSession` 和 `BattleModelSession` 共用逐回合驱动。胜者优先于轮数保护，无进展上限仍为当前实体数量（至少 1）乘 16。回放会话在空回合后也更新差分快照；无展示会话仅返回可见帧末的边界元数据，不构造展示 DTO。底层 Runtime 更新仍用于判断可见帧。

## 两遍运行与分层抽样

每套有序阵容可运行多个独立派生 seed。生成器的阵容抽取、战斗 seed 和抽样随机序列使用不同域的 SHA-256 派生，绝不从战斗 RNG 抽取随机数。主 seed、稳定任务编号和用途决定结果，线程数不参与派生。

第一遍只记录可见帧索引、实际推进轮数和最终结果。第二遍使用同一 prepared 阵容、seed 和保护配置重跑，仅导出选中的状态；逐帧核对边界，并核对最终结果。

默认每局最多 K=8 个样本：

1. 保留初始状态，已在初始化结束的对局只记对局信息。
2. 将最终推进轮数 T 划为 K−1 个等宽区间；帧末轮数 r 的桶为 `min(floor(r*(K-1)/T), K-2)`。
3. 每个非空桶均匀随机抽一帧，再从其余未选帧中均匀补足名额。
4. 只抽可见帧末，不抽动作中间或空回合，不抽已经产生胜者的终局，不重复抽同一帧。
5. 短局允许少于 K 个样本。截断终止的可见帧仍可抽取，因为没有已知胜者。

K=1 时只保留初始状态。最终进度只写审计列，不能作为输入特征。单局缓存规模为可见帧边界列表加最多 K 个选中状态；不缓存整局状态历史。

## 输入、切分和 Parquet

`tswn-winprob-dataset generate` 支持固定对局文件/目录，或名字池抽取指定队伍人数的阵容。这两种模式互斥。文本沿用引擎解析规则；实际战斗保留输入顺序，原 seed 行由派生 seed 替换。名字池去重，单局不重复抽同一条目。

按规范化阵容哈希切分为 train/validation/test，比例 80/10/10。规范化忽略 seed、排序队伍与队内成员，但保留成员重复和队伍边界。同阵容所有 seed、所有顺序变体共享集合；不同集合允许出现相同名字。规范化只服务于切分，不改变引擎输入。比例是哈希分桶比例，小样本不保证三组都非空。

Rust 直接输出嵌套 Parquet。Arrow/Parquet/serde_arrow 依赖仅在独立生成器 crate 中。schema 从 Rust 类型生成，避免某个分片未出现罕见状态就遗漏字段；载荷使用 kind 和可空的类型字段，不依赖 Arrow Union 或整段 JSON 字符串。无载荷枚举在 Arrow schema 中使用普通字符串，避免 PyArrow 读取跨行组的嵌套字典时报错；Parquet 物理字典压缩仍可使用。文件使用 ZSTD 压缩，写入缓冲达到阈值时刷新行组。

每个分片有两张表：

| 表 | 内容 |
| --- | --- |
| `battles.parquet` | 对局编号、阵容编号、seed、split、终止结果和样本数 |
| `samples.parquet` | 对局编号、帧边界、审计 progress、可空 winner_team_index 和嵌套 state |

训练读取器只选择 `state` 和 `winner_team_index`，默认过滤空标签。原始输入、阵容哈希、seed、split 和 progress 都不能进入训练特征。

## 确定性、恢复和失败

任务顺序为阵容输入顺序乘每阵容对局数。默认每 1000 局一个固定编号分片；worker 可以并行处理不同分片，片内行序固定。相同输入、配置和可执行文件在不同线程数下得到相同逻辑数据。

manifest 记录原输入、参数、schema 版本及可执行文件 SHA-256。线程数和输出位置不属于语义配置。首次生成要求空目录；续跑必须显式 `--resume` 且 manifest 相符。可执行文件摘要同时约束引擎源码、依赖和构建选项，重新编译后不能混入旧任务。

分片先写临时目录，写完两张表、同步文件、生成摘要、回读验证后改名提交。完整分片校验成功则跳过；未提交分片重做；已提交分片损坏则停止并保留现场。操作系统文件锁防止多个进程写入同一输出目录。

Runtime 错误、panic、导出错误、胜者不能唯一映射或两遍运行不一致都停止任务。失败不是截断；失败分片不提交，已完成分片保留。复现信息记录在临时分片的 `active-battle.json`，全局错误记录在 `failure.json`。

`validate` 校验文件摘要、schema、行序、队伍及实体引用、标签、每局样本数、进度和切分。汇总已决/截断、终止原因、队伍数量及集合分布。成功数据集中的失败局数量为零，因为失败会阻止任务完成。

## 后续训练和网页约束

张量化实现位置已定：**encoder 只在 Rust 实现一次，另加导出通道**。数据集仍以原始 `state` 为
唯一事实来源，格式不变；Python 侧只做训练、评估与指标，不重新实现一套编码逻辑，避免训练与
Rust 推理两份实现漂移。需要张量时由 Rust 读取 Parquet 并导出（或由 WASM/Python 绑定调用同一个
encoder），保证训练与推理共用同一份数值语义。

先用已决训练集建立 HP 等简单基线，再评估实体/队伍聚合模型。队伍独立评分后直接 softmax 无法充分表达第三队对前两队相对胜率的影响，应加入全局或对手上下文。获胜资格已按引擎语义固定：只有恰好一个队伍仍有存活实体才判胜，全部阵亡也不判胜；复活是内置主动技能，因此“某帧该队 0 存活”不等于出局，不能按本体存活屏蔽队伍，也不能用 `alive_group_count` 当 mask。定义、源码定位与实测分布见 [判胜语义](../mechanics/winner.md)。

概率对应生成分布下对未观察随机性的估计；同一个完整输入和 battle seed 的引擎结果是确定的。不能在采样状态处重新播种来伪造续局分布，也不能要求每一帧的预测置信度必然单调上升。训练、校准和最终测试应使用独立切分，并按对局聚合评估，避免相邻样本被当成独立对局。

网页会预取回放，收到帧不等于已经显示该帧；终局还可能提前释放 WASM session。后续推理结果必须在生成对应帧时与 generation/frame 索引一起保存，并在该帧播放完成后显示，不能在订阅回调中临时查询已被推进或释放的 session。本轮不添加该接口或 UI。
