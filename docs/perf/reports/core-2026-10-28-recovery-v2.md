# 非 PGO 核心源码优化：recovery-v2 独立重测

## 结论状态与证据边界

**主评估85集已完成，其中FFA41的配对吞吐中位提升7.8658%，耗时下降7.2922%（MAD 1.3005pp，7/7轮更快）。独立留出FFA20的吞吐中位提升5.3967%，耗时下降5.1204%（MAD 1.7978pp，7/7轮更快）。两套预登记混合FFA组的吞吐中位均超过5%，但Boss FFA10仅提升3.9948%，未达到5%；不能宣称所有FFA分层或案例达标。** 前一轮工作区及原始证据丢失后，本轮从公开源码重新构建候选、生成独立留出集并重新测量。旧报告的耗时、吞吐、分配、采样、事件覆盖及测试计数均不作为本轮证据，不补写或混合旧数值。

本轮机器为 AMD EPYC 9V74，不能将此前另一台机器的结果当作本机实测。最终结论仅适用于本轮记录的构建、输入、原始轮次区间与运行条件；组级中位收益不等同于每例或所有机制均受益。

- 可复现基线合并提交：`fcc03f4390b0dff912d9d31ca97b6ed9efcbf0ca`，生产逻辑对应主线 `7f6a7b17`；主线版本未变化。
- 已发布重建候选：`de1a5fae39b9727115f75db927757a5829fd7f9d`，源码树`16e51d046b36ae7d717e22eb4457da815e378f1d`；二进制摘要与输入绑定以本轮测量manifest为准。上述源码是本轮完整计时的冻结实现。
- 无PGO、无训练画像、无`target-cpu=native`、无额外`RUSTFLAGS`。
- 武器仍由用户本地开发，本轮不改武器生产代码，也不使用武器案例或dummy代替；不声称真实武器机制覆盖。

## 核心改动与安全语义

| 改动 | 源码位置 | 保留的语义与核验点 |
|---|---|---|
| 标量排除范围替代临时索引向量 | `crates/tswn_core/src/rc4.rs`及combat目标选择调用点 | 保留旧实现只读取首项和数量的行为，不借优化“修正”非连续排除列表；公开`pick_skip_range`接口保留 |
| 小范围精确倒数取余 | `crates/tswn_core/src/rc4.rs` | 仅优化1..255上界的16位样本；包括上界1仍消费两个随机字节，0和较大范围保持原路径；核对取余值及后续RNG状态 |
| 空伤害阶段钩子快速路径 | `crates/tswn_core/src/runtime/combat/round.rs` | 仅已知空技能缓存跳过拥有型快照；未知缓存完整扫描；技能执行后重新读取状态mask，保留同阶段新增状态效果；不跳过非空扩展钩子 |
| 存活队伍索引 | `crates/tswn_core/src/runtime/world.rs` | 缓存首次包含实体的队伍；初始化、复位、复活、召唤与删除同步更新；稀疏ID保留扫描回退，重复/错队视图保留旧首次匹配语义 |
| roster总数缓存 | 同上 | 累计roster槽位而非存活人数；死亡不减，新增真实roster槽位才增，重建时重新计数 |

World新增缓存属于派生数据；相等比较仍按原公开逻辑状态，不能把缓存容量差当成战斗行为差。源码阅读只能确认实现意图，不能替代回归测试、trace对照或完整行为摘要核验。组合优化的端到端收益也不等于每项改动各自的贡献，本轮没有预登记逐项消融结论。

## 冻结测量计划

两侧使用相同 Rust 1.99.1、release fat LTO、codegen-units=1、`no_debug`与mimalloc，绑定单CPU 2。性能计时不与编译或诊断并行，CPU采样也独占运行；计数诊断不使用其耗时。计时中不修改生产代码、输入或运行上限。若计时后发现必须修正的正确性问题，应重新冻结并完整重测，不沿用旧候选计时。

| 序列 | 案例 | 每例局数 | 正式轮数 | 原始轮次 | 状态 |
|---|---:|---:|---:|---|---|
| 同二进制A/A噪声 | 固定集FFA9 | 6,000 | 5 | `[0,6000)` | 完成 |
| 主评估矩阵 | 扩展85 | 6,000 | 7 | `[0,6000)` | 完成 |
| 独立确认矩阵 | 新普通/Boss32 | 12,000 | 7 | `[2000000,2012000)` | 完成 |

每组另有第0轮warmup，不纳入正式统计。正式7轮重复相同输入与相同seed区间，是配对重复计时样本，不是7套独立随机seed；不把重复执行的总局数当作独立统计样本数。独立确认来自新roster与2M起始区间，而不是重复轮次数量。按案例排序，相邻运行A/B，方向由`(round + case_index) % 2`交替。固定计划结束即停止，不重复运行直到达到目标，不根据性能剔除案例、轮次或guard局。

主评估沿用确定性85集；新32集使用独立`recovery-v2`盐值，plain/Boss两族×FFA4/6/8/12/16、2v2、4v4、3v3v3八布局×两个重复，共20例FFA、12例队伍战。全部12种Boss身份均被预登记选入，但身份出现不证明其每种技能实际触发。生成器以完整无序raw角色多重集检查与既有30+85+48集不重叠；单个Boss保留名字会复用，不宣称角色级完全独立。原始轮次用于`seed:PROFILE_START+round@!`，不是把2M直接当最终seed编号。

## 统计口径与已完成结果

计时使用公开Runtime API，从准备模板之前到批量计算及模板释放之后；包含准备、初始化、战斗和释放，不包含输入文件IO、文本分组解析及JSON输出。标准`perf_runtime`忽略输入文本seed行、使用batch原始轮次；因此压力文件名称不保证复现原始挖掘seed或专属分支。独立`perf_runtime_range`明确拒绝文本seed，使用`--start`区间，不能混用两种入口。

每轮先在组内累加A/B秒数，再计算耗时变化`100×(B/A−1)`和吞吐变化`100×(A/B−1)`，分别取正式轮次中位数。MAD为耗时变化相对其中位数的绝对偏差中位数，单位pp。不是把各案例百分比直接平均，也不以“中位耗时之比”替代逐轮配对headline。MAD不是置信区间，A/A抖动不能直接从A/B收益中相减。

| 范围 | 配对吞吐中位 | 配对耗时中位 | 耗时MAD | 更快轮数 | errors / guard |
|---|---:|---:|---:|---:|---|
| A/A FFA9 | -0.8832% | +0.8911% | 0.7817pp | 1/5 | 0 / 0 |
| 扩展85全部 | +8.0945% | -7.4884% | 0.5644pp | 7/7 | 0 / 1 |
| 扩展FFA41 | +7.8658% | -7.2922% | 1.3005pp | 7/7 | 0 / 1 |
| 扩展duel1 | +11.2351% | -10.1003% | 3.3941pp | 6/7 | 0 / 0 |
| 扩展队伍战43 | +7.8728% | -7.2982% | 0.2000pp | 7/7 | 0 / 0 |
| 新32全部 | +5.6205% | -5.3214% | 0.8069pp | 7/7 | 0 / 1 |
| 新FFA20 | +5.3967% | -5.1204% | 1.7978pp | 7/7 | 0 / 1 |
| 新队伍战12 | +6.5008% | -6.1040% | 0.6868pp | 7/7 | 0 / 0 |
| 新普通FFA10 | +5.1451% | -4.8934% | 1.7060pp | 7/7 | 0 / 0 |
| 新Boss FFA10（不足5%） | +3.9948% | -3.8413% | 1.2804pp | 7/7 | 0 / 1 |

以下及附件列出完整逐轮、逐案例与布局分层，正负中位数个数、最差回退、所有异常值，以及Boss/普通交叉分层。不用总体收益替代FFA目标，不用混合FFA掩盖Boss FFA不足。1v1按`group_sizes == [1,1]`归duel，其余非FFA归队伍战；分组交叉，不可相加为独立样本总数。

### A/A噪声控制

同一基线二进制A/A的FFA9五轮耗时变化为+0.2784%、+3.3295%、+0.8911%、−5.0594%、+1.6727%。中位+0.8911%、MAD0.7817pp，只有1/5轮B标签的组内合计时间更短。这些是无源码差异时的顺序/环境波动，不是优化收益，也不直接从A/B结果相减。组内各例MAD可明显更大，说明组级稳定不等于每例稳定。

### 主评估85的布局分层与弱收益案例

两侧完整正式轮次行为摘要匹配，错误0。每轮每侧guard1均在`generated__ffa16__02.txt`，保留其全部时间和结果，不剔除或更改保护上限。FFA41七轮耗时变化为−8.5927%、−5.0571%、−6.6731%、−8.9292%、−7.2922%、−8.4562%、−5.9741%；全部为负。

| 布局 | 案例 | 配对吞吐中位 | 配对耗时中位 | 耗时MAD | 更快轮数 |
|---|---:|---:|---:|---:|---:|
| 2v2 | 10 | +7.2154% | -6.7298% | 3.7594pp | 7/7 |
| 3v3v3 | 22 | +8.3582% | -7.7135% | 0.7132pp | 7/7 |
| 4v4 | 6 | +8.7191% | -8.0199% | 1.7114pp | 7/7 |
| ffa12 | 6 | +10.1862% | -9.2445% | 1.2670pp | 7/7 |
| ffa16 | 6 | +7.8233% | -7.2556% | 1.1874pp | 7/7 |
| ffa4 | 9 | +6.8856% | -6.4420% | 2.4422pp | 6/7 |
| ffa6 | 8 | +6.6278% | -6.2159% | 1.3783pp | 7/7 |
| ffa8 | 12 | +8.8690% | -8.1465% | 0.9773pp | 7/7 |
| 1v1 | 1 | +11.2351% | -10.1003% | 3.3941pp | 6/7 |
| 2v3 | 5 | +5.9801% | -5.6427% | 0.8588pp | 7/7 |

85/85例配对耗时中位为负，没有正中位回退例；但其中12例吞吐中位不足5%，不能宣称所有案例超过5%。单轮回退仍保留，以下为吞吐中位最弱6例。MAD大于或接近效应的条目尤其不应被包装为稳定的小幅胜出。

| 案例 | 配对吞吐中位 | 配对耗时中位 | 耗时MAD | 更快轮数 |
|---|---:|---:|---:|---:|
| `stress__3v3v3-ea03011602664398.txt` | +0.8206% | -0.8140% | 6.1878pp | 5/7 |
| `stress__score_round_7.txt` | +1.2397% | -1.2246% | 6.0146pp | 4/7 |
| `generated__ffa6__06.txt` | +2.1718% | -2.1256% | 3.0661pp | 5/7 |
| `generated__3v3v3__06.txt` | +3.0112% | -2.9232% | 3.1928pp | 6/7 |
| `generated__2v2__03.txt` | +3.7729% | -3.6357% | 4.2886pp | 5/7 |
| `generated__ffa4__02.txt` | +3.9574% | -3.8067% | 3.4365pp | 6/7 |

完整安全数值附件：[A/A汇总](core-2026-10-28-recovery-v2/aa-control-summary.csv)、[A/A配对逐轮](core-2026-10-28-recovery-v2/aa-control-paired-rounds.csv)、[主85汇总](core-2026-10-28-recovery-v2/primary85-summary.csv)、[主85配对逐轮](core-2026-10-28-recovery-v2/primary85-paired-rounds.csv)。逐轮表包括warmup标记，正式统计必须排除warmup；未删异常值。新留出32结果另列，不替换主评估结果。

### 独立新32确认：Boss FFA未达到5%

全部预登记轮次结束后停止，没有追加“跑到达标”的测量。两侧完整行为摘要匹配，错误0；每轮每侧guard1在`boss__ffa6__02.txt`，不删除该案例或其耗时。新的roster与原始2M区间提供独立确认，但7轮内部仍重复同一seed区间。

**混合FFA20吞吐+5.3967%，普通FFA10为+5.1451%，Boss FFA10仅+3.9948%。** Boss全部16例（含队伍战）的+5.5389%不能替代Boss FFA目标。普通FFA与混合FFA刚超过5%的样本中位不等于已建立“真实收益至少5%”的统计下界，本报告未构造这类置信保证。

| 分层/布局 | 案例 | 配对吞吐中位 | 配对耗时中位 | 耗时MAD | 更快轮数 |
|---|---:|---:|---:|---:|---:|
| 2v2 | 4 | +6.9602% | -6.5072% | 0.8533pp | 6/7 |
| 3v3v3 | 4 | +7.7586% | -7.2000% | 2.6319pp | 6/7 |
| 4v4 | 4 | +5.7877% | -5.4710% | 1.8804pp | 6/7 |
| ffa12 | 4 | +4.6537% | -4.4468% | 0.2719pp | 7/7 |
| ffa16 | 4 | +7.5236% | -6.9972% | 2.6078pp | 6/7 |
| ffa4 | 4 | +8.9710% | -8.2325% | 1.7079pp | 7/7 |
| ffa6 | 4 | +5.8172% | -5.4974% | 1.3067pp | 7/7 |
| ffa8 | 4 | +4.6565% | -4.4493% | 2.7946pp | 5/7 |
| plain | 16 | +5.2396% | -4.9787% | 1.5925pp | 7/7 |
| plain_team | 6 | +5.5046% | -5.2174% | 1.7887pp | 6/7 |
| boss | 16 | +5.5389% | -5.2482% | 0.9505pp | 7/7 |
| boss_team | 6 | +7.4512% | -6.9345% | 1.0625pp | 7/7 |

FFA8与FFA12布局吞吐中位分别+4.6565%、+4.6537%，也不足5%。完整7轮FFA耗时变化如下，所有轮次保留：

| 轮次 | 全部FFA20 | 普通FFA10 | Boss FFA10 |
|---|---:|---:|---:|
| 1 | -5.1204% | -6.5994% | -3.4542% |
| 2 | -3.1984% | -4.7208% | -1.5425% |
| 3 | -6.1967% | -3.3696% | -9.0955% |
| 4 | -6.9565% | -6.8879% | -7.0306% |
| 5 | -5.0020% | -4.8934% | -5.1218% |
| 6 | -3.3226% | -2.8483% | -3.8413% |
| 7 | -8.3444% | -12.4699% | -3.5890% |

32/32案例配对耗时中位下降，0例正中位回退；但11例吞吐中位不足5%。最弱`plain__ffa8__01.txt`仅+0.0613%，耗时MAD3.5397pp且只有4/7轮更快，视为接近噪声，不能包装为可靠改善。以下保留最弱6例，其他单轮回退完整列在数值附件。

| 案例 | 配对吞吐中位 | 配对耗时中位 | 耗时MAD | 更快轮数 |
|---|---:|---:|---:|---:|
| `plain__ffa8__01.txt` | +0.0613% | -0.0612% | 3.5397pp | 4/7 |
| `plain__3v3v3__02.txt` | +1.4119% | -1.3923% | 5.2057pp | 5/7 |
| `boss__ffa6__02.txt` | +2.2991% | -2.2474% | 1.1145pp | 5/7 |
| `boss__ffa16__01.txt` | +2.7597% | -2.6856% | 5.9015pp | 4/7 |
| `boss__ffa4__01.txt` | +3.0843% | -2.9921% | 5.3970pp | 6/7 |
| `plain__4v4__02.txt` | +3.4230% | -3.3097% | 2.8648pp | 5/7 |

附件：[新32汇总](core-2026-10-28-recovery-v2/confirm32-summary.csv)、[新32全部配对逐轮](core-2026-10-28-recovery-v2/confirm32-paired-rounds.csv)。

### 单案例单轮极值（未剔除）

逐案例中位全部更快不代表每次运行均更快。主85最慢单例配对发生在第4轮`stress__2v2-554f4128af707167.txt`：A 0.178592357 s、B 0.240993259 s，耗时 **+34.9404%**；同轮`stress__ffa_8-3d155c15a9d6ec4b.txt`出现−38.0489%的最快极值。独立32最慢配对为第6轮`boss__2v2__02.txt`：A 0.159033412 s、B 0.208879441 s，耗时 **+31.3431%**；最快为第7轮`plain__ffa16__01.txt`的−22.5657%。这些短案例的双向波动全部保留，不能以单次极值宣称稳定退化或加速，也没有将它们当成异常值删除。完整分布见逐轮附件。

## 行为、测试与工具门禁

| 验证 | 本轮状态 |
|---|---|
| 核心库测试 | 655项通过；不重复计入后续工作区汇总 |
| 新留出生成器 | 6项Python单测通过；确定性、32例范围、去重、manifest与不覆盖已有输出 |
| 最终工作区测试 | 32个结果target合计975通过、5失败、7 ignored；不是全绿 |
| release默认runtime | 474通过、181 filtered，0失败 |
| release no_debug runtime | 470通过、180 filtered，0失败 |
| release CLI runtime | 6通过、85 filtered，0失败 |
| release corpus | 124通过、0失败；整个release验收脚本退出0 |
| Python工具单测 | 本轮合并14项通过；与上列生成器6项有重叠，不重复相加 |
| trace helper编译与编码单测 | 编译完成，2项编码单测通过 |
| nightly fmt / clippy | 两者均退出0；clippy有警告，见下文 |
| 配对运行行为摘要 | A/A、主85与新32确认均完成且一致；布局、区间、total、wins、errors、guard逐项检查 |
| 新32实际事件与trace对照 | 完成；32行两侧JSONL逐字一致，1024局，errors/guard均0 |

工作区5项DS4 golden失败已在基线独立复现。每项失败记录中第一对assert的left/right字节数组经CRLF→LF归一化后精确一致；这个核验只针对所提取的第一对数组，不宣称已分析所有错误trace。原始测试仍失败，不能把手工归一化改写为测试通过，也未重生成golden。后续release、核心库、helper等调用可能覆盖相同测试，上表不应相加冒充唯一测试总数；ignored与filtered分别报告。

最终nightly fmt退出0。clippy退出0但不是零警告：core库31条，core库测试40条（包含31条重复），不可相加称71个唯一问题。本轮新增两条非阻断风格建议为倒数常量的`manual_div_ceil`和World缓存条件的`collapsible_if`。为了保持实测代码冻结，不在计时期间清理这些风格建议；不声称它们已经修复。

独立诊断默认每例32局、start=2M，不用于计时。流式SHA-256编码有固定整数宽度、长度前缀和分段标记；包含公开消息语义字段及顺序、on_update_end、行动与胜者、RNG的i/j/main_val、实体alive/hp/team/owner/shield等选定状态与存活顺序。排除全局updates.id、指针和环境。capture还与同区间no-capture摘要比较；仅输出basename及固定事件类别，不公开含角色全名的原始消息。

关键词消息计数不是hook调用次数，更不是完整机制覆盖。最终应明确实际观测到的机制、没有触发的Boss能力和诊断范围；相同摘要/trace也不是所有输入或全部内部状态的等价证明。达到100,000回合保护上限的局仍保留，guard与初始化错误分别报告。

### 本轮已完成的独立trace诊断

新32例每侧各32局，共1,024局，原始round为`[2000000,2000032)`。两侧32行JSONL逐字一致；重新解析后的所有字段也相等。累计48,570回合、47,515次行动、179,922条消息，错误0、guard0；每例capture与同区间no-capture摘要一致。SHA-256与全部计数见[安全逐例trace表](core-2026-10-28-recovery-v2/trace-safe.csv)。这些是本轮重新生成和执行的数据，不是旧工作区结果。

| 消息关键词类别 | 命中条数 | 命中案例数 / 32 |
|---|---:|---:|
| summon（召唤） | 792 | 27 |
| merge（吞噬） | 204 | 21 |
| revive（复活） | 29 | 9 |
| clone（分身） | 470 | 27 |
| counter（反击） | 304 | 26 |
| freeze（冰冻） | 763 | 22 |
| death（死亡） | 156 | 22 |
| zombie（僵尸） | 0 | 0 |

以上只统计公开消息中相应关键词，可能漏掉同机制的其他措辞，也可能由不同来源产生；death的156不是完整死亡次数，freeze不能直接当作冰冻施加次数，counter不能当作武器反击证明。未观察到zombie关键词，不声称该机制覆盖。32例包含12种Boss身份，但本轮未逐Boss专属能力分类，不能据装配或总消息数宣称全部Boss技能触发。独立诊断局数远少于计时矩阵，实际计时中的guard必须另外核验。

## CPU与分配诊断

### CPU 采样（独立于正式计时）

使用 gprofng，固定 CPU 2、单线程、100 ms 时钟采样。expanded 85 案例各 24,000 局；stable 32 案例各 60,000 局，start=2,000,000。四份实验 log.xml 均记录 ptimer=100000 µs；stderr 为空，实验日志未检出 warning/error/timer_create 警告。全量函数表和调用树使用 `-limit 0`，避免把榜单截断误认为函数消失。

JSON 实际行数为 85/32；各案例 group_sizes/runs_per_matchup/total/wins/errors/guard_exhausted 在 baseline/candidate 间逐项完全一致，errors 均为 0。高局数下 guard 并非全零：expanded 的 generated__ffa16__02 双方各 2；stable 的 boss__ffa12__01 各 1、boss__ffa6__02 各 2。

| exclusive CPU 占比 | expanded baseline→candidate | stable baseline→candidate |
|---|---:|---:|
| 默认攻击目标选择 | 6.04%→4.94% | 8.42%→6.38% |
| 行动技能概率扫描 | 8.83%→8.05% | 7.15%→6.86% |
| skill_hook_plan | 6.16%→5.79% | 6.46%→5.56% |
| state_hook_plan（精确函数名） | 1.10%→0.64% | 1.96%→1.54% |
| refill_seed_state（精确函数名） | 4.67%→5.01% | 5.65%→6.50% |
| SkillLoadout::clone | 2.73%→1.98% | 0.58%→0.83% |

调用树中可见 `run_to_completion_prevalidated → select_plain_default_attack_target → RC4`；expanded 目标选择 inclusive 占比为 7.27%→5.86%，stable 为 10.84%→8.27%。另一明确路径是 `run_to_completion_prevalidated → scan_plain_action_skill_probabilities → RC4`。旧 pick_skip_range 独立符号占 expanded 2.79%、stable 3.00%；候选的 next_i32 独立符号占 2.12%、2.84%。候选全表未观察到独立 pick_skip_range/PickSkipRange 符号，但优化/内联会改变归因边界，不能声称其实际工作成本变为零。候选调用树另可见 `drain_action_damage_phase_into → skill_hook_plan`，说明非空钩子仍分派。

仍有明显剩余热点：expanded 候选未解析 libc 静态符号占 12.15%，不得无证据归为 memcpy；初始化 refill 和 clone 成本并未一致下降。WorldArena::next_actor 的 expanded 占比也由 2.14% 升至 3.46%，不能挑选有利指标。

采样 CPU 总量为 expanded 154.1→141.6 s、stable 86.7→84.6 s，约 1541/1416/867/846 个样本。这里只把它们用于解释采样密度，**不能替代重复、配对的正式 E2E 计时或推导独立 speedup**。单次样本少，0.1 s 级量化、内联和符号/栈展开差异限制小热点比较；inclusive 占比重叠，不得相加；套件混合 FFA/团战/Boss，不能把套件函数占比当作单独 FFA profile。

安全派生数据：[profile-functions-safe.csv](core-2026-10-28-recovery-v2/profile-functions-safe.csv)、[profile-calltree-safe.csv](core-2026-10-28-recovery-v2/profile-calltree-safe.csv)、[profile-validation-safe.json](core-2026-10-28-recovery-v2/profile-validation-safe.json)。仅含案例计数/数值/符号，不包含工作区绝对路径或原始实验日志。

复现时在两侧分别运行，使用不同且不存在的输出目录；读取JSON时忽略gprofng附带的非JSON提示行：

```bash
taskset -c 2 gprofng collect app -p 100 -o expanded-a.er "$BINARY_A" \
  -i target/recovery-matrix/expanded -n 24000 --threads 1 --mode win-rate
taskset -c 2 gprofng collect app -p 100 -o stable-a.er "$RANGE_A" \
  -i target/recovery-heldout32/inputs -n 60000 --threads 1 --mode win-rate --start 2000000
# B侧替换为BINARY_B/RANGE_B与不同的expanded-b.er/stable-b.er。
gprofng display text -limit 0 -functions -calltree expanded-a.er
```


本轮分配诊断已完成：保持原mimalloc后端，两侧各运行扩展85与新留出32共117例，每例1,000局。两侧所有案例的total/wins/errors/guard、布局与区间完全一致；共234行结果的失败分配、运行后净活跃字节与错误数均为0。扩展集使用原始round `[0,1000)`，新集使用`[2000000,2001000)`。请求字节不等于RSS、allocator内部页数或realloc复制流量；若FFA分配增加应如实报告，不能仅因端到端变快称为分配优化。

FFA分配请求字节略增，不能将端到端优化预先宣传为FFA分配优化：

| 范围 | 案例 | 基线请求字节合计 | 候选请求字节合计 | 变化 | 单案例峰值差最大值 |
|---|---:|---:|---:|---:|---:|
| 扩展FFA | 41 | 1,427,130,109 | 1,427,152,509 | +0.00156958% | +640 B |
| 新留出FFA | 20 | 147,767,837 | 147,775,517 | +0.00519734% | +480 B |

“峰值差最大值”是逐案例候选减基线后的最大值，不是全部案例同时运行的峰值。完整逐案例alloc/realloc、字节和峰值见[安全分配表](core-2026-10-28-recovery-v2/allocation-safe.csv)。净活跃字节归零不等于所有输入都无泄漏；插桩耗时不进入性能比较。

## 可复现命令

统计脚本使用包含本报告的提交（最终补充仅增加分层汇总与文档，不改计时入口或生产源码）。在对应源码工作树运行；baseline固定到`fcc03f4390b0dff912d9d31ca97b6ed9efcbf0ca`，candidate固定到`de1a5fae39b9727115f75db927757a5829fd7f9d`。给基线仅复制相同examples及工具（或应用只含工具的提交），不得混入候选生产改动；两侧helper应逐字一致。清除继承的PGO/native及profile覆盖环境；`BINARY_A/B`和`RANGE_A/B`指向各自release构建产物。其他机器选择可用且固定CPU，不照搬本机CPU编号。

```bash
python scripts/generate_perf_matrix.py --repo . --out target/recovery-matrix
python scripts/generate_hook_perf_matrix.py --repo . \
  --prior-matrix target/recovery-matrix --out target/recovery-heldout32
mkdir -p target/recovery-ffa9
cp docs/perf/fixed_cases_30/*_ffa_*.txt target/recovery-ffa9/
cargo build --locked --release -p tswn_core --features no_debug \
  --example perf_runtime --example perf_runtime_range --example coverage_hook_rosters

taskset -c 2 python scripts/compare_perf_runtime.py --a "$BINARY_A" --b "$BINARY_A" \
  --input target/recovery-ffa9 --out target/recovery-aa --runs 6000 --rounds 5
taskset -c 2 python scripts/compare_perf_runtime.py --a "$BINARY_A" --b "$BINARY_B" \
  --input target/recovery-matrix/expanded --out target/recovery-expanded85 \
  --runs 6000 --rounds 7
taskset -c 2 python scripts/compare_perf_runtime.py --a "$RANGE_A" --b "$RANGE_B" \
  --input target/recovery-heldout32/inputs --out target/recovery-heldout \
  --runs 12000 --rounds 7 --start 2000000

# 仅汇总已经完整结束且严格验证通过的目录。
python scripts/summarize_perf_comparison.py target/recovery-aa --out target/recovery-aa-tables
python scripts/summarize_perf_comparison.py target/recovery-expanded85 --out target/recovery-expanded85-tables
python scripts/summarize_perf_comparison.py target/recovery-heldout --out target/recovery-heldout-tables
```

诊断独立执行：两侧`coverage_hook_rosters --input target/recovery-heldout32/inputs --runs 32 --start 2000000`，逐basename比较trace和全部数值字段，不将诊断运行加入未插桩耗时样本。分配探针的构建与运行方式见`tools/perf-allocation-probe/README.md`。

安全附件已放在同级`core-2026-10-28-recovery-v2/`数据目录，仅保留公开basename、数值、公开源码符号及白名单编译/二进制/输入绑定信息。不得复制原始环境转储、绝对工作区路径、完整消息日志或构建产物。预登记计时、行为、测试和独立CPU采样均已完成；诊断结果不改变冻结计时结果或追加筛选轮次。

构建及输入绑定见[白名单测量manifest](core-2026-10-28-recovery-v2/measurement-manifest-safe.json)。相同的行为摘要只导出basename、布局与数值：[A/A签名](core-2026-10-28-recovery-v2/aa-control-signature.csv)、[主85签名](core-2026-10-28-recovery-v2/primary85-signature.csv)、[新32签名](core-2026-10-28-recovery-v2/confirm32-signature.csv)。签名每例列一份，表示比较器对全部轮次两侧检查相同；不是某个变体胜率更高的声称。
