# FFA 性能剖析与可选 PGO 构建评估

## 结论

**可选 PGO 构建在本机两套留出评估与追加确认中均改善 FFA E2E；默认生产源码没有保留优化改动。** 更紧密的逐案例相邻 A/B 确认中，9 个固定 FFA 案例的配对耗时中位下降 **7.6714%**，MAD **1.7756 个百分点**，5 轮均更快。初次固定 FFA 结果为 -6.4163%，扩展 41 个 FFA 为 -13.5276%。这些是本机、指定编译器与输入分布的测量，不是所有案例、所有硬件都至少提升 5% 的保证。

A–E 源码候选均已撤回，生产 core 与原版 `65ab2ee` 一致。本轮保留的是独立训练与评估工具、可选 PGO recipe 和结果表，不改变默认构建。PGO 未增加源码算法收益或减少分配的证据；训练画像与编译器、源码和配置绑定，升级后必须重训重测。

两套正式矩阵和追加确认行为签名一致。正式 release/runtime/corpus 验收通过；工作区完整测试仍有 5 个换行格式相关失败及 7 个 ignored，不能写作全部测试绿色。最终 CPU 采样对照已完成，但有效样本较少且 PGO 改变符号归因，只能辅助解释编译变化。

最新提交与上次留档的同机对比见[独立基线记录](core-2026-10-27-baseline.md)：该核心路径源码未变，没有确认版本性能差异。

## 测量边界

本轮为 Linux x86_64 云端 / AMD EPYC 9V74；优化、A/A 与确认计时均固定 CPU 2（`taskset -c 2`），单线程，不并行编译或运行其他基准。云端调度仍有波动。没有修改系统频率、安全配置或全局调度；Windows 性能未在本轮验证。

- 内置 `perf_runtime` 的未插桩 E2E 计时涵盖公开胜率 API 的模板准备、每场初始化、战斗和内部释放；不包括进程启动与输入文件读取解析。进程 wall time 另有记录，不混作该 API 指标。
- 探索 A/B/C/D：固定集中的 9 个 FFA 案例，每例 13,000 场、单线程；预热不计入，交替运行基线与候选。
- E 固定 FFA 配置每例 26,000 场、3 轮；扩展集每例 3,000 场、5 轮，与此前实验运行长度不同，不能直接比较跨实验的绝对总秒数。
- 每轮总耗时先累加案例，再计算跨轮中位数。配对变化是同轮 `候选/基线-1` 的中位数；正数表示更慢。MAD 单位为百分点，仅说明离散程度，不是置信区间或统计显著性。
- 分配、占用、回放覆盖以及 CPU profile 均独立于正式计时，不能使用探针运行时间声称加速。

## CPU 采样：具体热点与调用栈

基线采样覆盖 9 个 FFA 案例，每例 260,000 场、单线程。使用 gprofng 时钟采样，记录采样间隔约 10,007 微秒。工具输出总 CPU 指标 10.517 秒，而进程 wall time 约 105 秒，二者尺度不一致，因此以下只解释采样比例，不把工具 CPU 秒数当作精确硬件计数或绝对运行时间。

| 函数/符号 | exclusive 占比 | inclusive 占比 |
|---|---:|---:|
| `RuntimeRunner::run_to_completion_prevalidated` | 19.70% | 84.30% |
| 未解析 libc 静态符号 | 9.32% | 9.32% |
| `scan_plain_action_skill_probabilities` | 8.94% | 11.70% |
| `PhaseScheduler::skill_hook_plan` | 5.33% | 6.76% |
| `select_plain_default_attack_target` | 4.95% | 5.99% |
| `PreparedBattleRoster::refill_seed_state` | 4.47% | 5.80% |
| `SkillLoadout::clone` | 3.62% | 6.47% |
| `RC4::pick_skip_range` | 3.14% | 3.14% |
| `WorldArena::next_actor` | 2.85% | 2.85% |

真实调用树观察到：`run_to_completion_prevalidated → drain_plain_builtin_skill_into → drain_plain_attack_after_dodge_into → drain_kill_hooks_into → apply_plain_merge_into → SkillLoadout::clone → SmallVec::extend`；其下还出现分配器路径。此证据支持检查 Merge clone 和技能缓存维护，而不是仅凭代码外观推断热点。

libc 符号未完整解析，不能把 9.32% 全部称作 memcpy、memset 或分配开销。最终 PGO 的符号归因变化见后文。采样比例受内联、符号和有限样本影响，exclusive 大的包装函数也可能包含被内联的子逻辑。

### 扩展集基线采样的交叉检查

扩展 85 案例每例 5,000 场，共 425,000 场、单线程。逐案例 API 时间合计 35.007795 秒，错误为 0、guard 耗尽合计 1；此运行场数不同于分配和机制覆盖探针，其 guard 数不能由后者的 0 代替。高频采样的名义间隔约 997 微秒，但约 35 秒运行仅获得约 349 个样本，工具汇总 CPU 时间为 0.349 秒；名义间隔不等于实际有效采样频率。因此这里只使用粗粒度比例，不能将其解释为精确 CPU 秒数，也不能凭小幅比例差异推导热点迁移或优化收益。

| 函数/符号 | exclusive 占比 | inclusive 占比 |
|---|---:|---:|
| `RuntimeRunner::run_to_completion_prevalidated` | 17.14% | 86.29% |
| `scan_plain_action_skill_probabilities` | 11.43% | 13.43% |
| 未解析 libc 静态符号 | 8.86% | 8.86% |
| `select_plain_default_attack_target` | 6.86% | 7.43% |
| `PhaseScheduler::skill_hook_plan` | 6.00% | 8.00% |
| `PreparedBattleRoster::refill_seed_state` | 2.86% | 3.71% |
| `SkillLoadout::clone` | 2.00% | 4.57% |

扩展集仍出现战斗推进、技能概率扫描、目标选择与 hook 计划等热点；它与固定 FFA 的组成、运行长度和有效样本量不同，不宜直接比较两个表的百分点作为性能变化。

## 分配计数：115 个案例、115,000 场

独立 `GlobalAlloc` 包装器委托给同版本 mimalloc，保留实际分配器；禁止默认 allocator feature 以免重复定义。输入解析后取起始快照，公开胜率 API 返回后取结束快照。固定 30 与扩展 85 个案例各 1,000 场、单线程。

| 集合 | 场数 | alloc 次数 | realloc 次数 | 累计成功请求字节 | 单案例最大额外存活请求字节 |
|---|---:|---:|---:|---:|---:|
| fixed30 | 30,000 | 2,284,726 | 166,969 | 436,365,745 | 483,438 |
| expanded85 | 85,000 | 15,540,538 | 1,137,767 | 2,555,039,933 | 758,236 |

所有案例错误、分配失败和 guard 耗尽均为 0，结束与开始的存活请求字节差均为 0。后者不是一般性“无内存泄漏”证明，只是该运行范围内的计数结果。

请求字节不是 RSS，也不含 mimalloc 元数据、保留页或绕过 Rust 全局分配器的 C 分配。realloc 按新请求完整尺寸累计，不代表实际复制字节；探针看不到 realloc 内部瞬时新旧双缓冲。原子计数会扰动运行，不用于吞吐测量。

## 状态占用：115 × 16 = 1,840 场

独立公开 API 探针以标准 batch seed 和标准 WIN_RATE eval，逐次调用 `run_to_completion_prevalidated(1)`。初始及每回合结束统计所有现存实体的 `states.entries().len()`，不启用 replay capture；逐案例胜数与标准 batch API 一致。

实测 1,281,117 个实体快照：0 项 1,169,814 次，1 项 105,098 次，2 项 5,982 次，3 项 223 次。20,998 个实体-场次的最大占用中，只有 28 个超过 2 项（0.1333%），没有观察到超过 3 项。

这些只是回合边界快照，可能遗漏回合内新增后删除和内部 tick 的瞬时峰值；不包含被压缩为专用字段的 legacy 状态，也不是所有可能输入的上界。任何缩容必须保留 SmallVec 的自动堆溢出。

本机结构尺寸：`StateEntry` 136 B、`StatePayload` 104 B、`StateStore` 1208 B、`EntityRecord` 2768 B、`PlayerTemplate` 1232 B、`SkillLoadout` 872 B、`PlayerRuntime` 272 B。D 把状态条目与注册序内联容量同时从 8 改为 4 后，StateStore 降至 632 B、EntityRecord 降至 2192 B（-20.81%），但 E2E 反而回退，因此不能以体积优化代替性能实测。

新增超过 8 条状态的行为回归覆盖溢出、删除重加、注册序、hook 排序、clone、大/小模板 reset、clear 后重用；`state_store` 前缀 10 项测试通过。容量改动及新增回归测试均已撤回，避免留下与最终方案无关的 core 改动；上述测试结果只属于已撤销实验。

## 机制覆盖：115 × 32 = 3,680 场

另一独立诊断通过公开回放 API 统计实际发出的消息模板，确认语料触发死亡、状态、召唤、防御、Merge、分身及复活等机制。固定集 960 场、扩展集 2,720 场；guard 耗尽均为 0。每份 JSONL 最后一行是集合汇总，不能再次与逐案例行求和，否则会重复计数。

这些计数是回放消息族，不是内部 hook 调用次数或代码覆盖率；状态类别包括施加与解除。此诊断使用 capture 模式和独立预定 seed，不能混同 no-capture 的计时，也不能称作穷尽覆盖。

## 候选探索结果

A/B/C/D 各自都对照原 current 二进制，并非相邻候选相互对比。不同实验期间机器状态不同，不能用跨实验 baseline 秒数推导候选排名。

| 候选 | 改动 | 正式轮数 | 基线/候选中位秒 | 配对耗时变化中位数 | 配对 MAD | 中位耗时比对应吞吐变化 |
|---|---|---:|---:|---:|---:|---:|
| A | 判胜扫描早退 + lazy replay 构建 | 3 | 5.410408 / 5.507023 | +1.7857% | 0.6225 pp | -1.7544% |
| B | 原版 + Merge 新增正技能后重建缓存；不含 A | 5 | 5.098412 / 5.151282 | +0.2896% | 1.2364 pp | -1.0263% |
| C | B + Merge no-clone | 3 | 5.354333 / 5.278551 | -0.3000% | 1.9700 pp | +1.4357% |
| D | C + StateStore 两个内联容量 8→4 | 3 | 4.461278 / 4.986578 | +11.7747% | 1.8177 pp | -10.5343% |
| E | C + SkillLoadout bulk-copy Clone（不含 D） | 3 | 10.314610 / 10.272010 | +0.3454% | 0.6699 pp | +0.4147% |

A 已撤回。B/C 的观测效应小于或接近噪声，尚不证明达到目标。D 明显不利，生产改动已撤回；最初一组 D 测量与尺寸构建预热重叠而中断，表格只取重新独占运行的 `explore-d-clean-t1`，不包含被中止结果。

### E 扩展集复核：85 案例，5 轮，每例 3,000 场

| 范围 | 基线/候选中位秒 | 中位耗时比变化 | 配对耗时变化中位数 | 配对 MAD | 中位耗时比对应吞吐变化 |
|---|---:|---:|---:|---:|---:|
| 全部 | 19.830473 / 20.747833 | +4.6260% | -0.8945% | 3.7641 pp | -4.4215% |
| FFA | 11.066338 / 11.839022 | +6.9823% | +1.5600% | 4.0446 pp | -6.5266% |
| 队伍战 | 8.731364 / 8.874713 | +1.6418% | -2.0113% | 2.5231 pp | -1.6153% |
| generated | 12.014272 / 12.695448 | +5.6697% | +4.5370% | 5.3680 pp | -5.3655% |
| stress | 7.852579 / 8.045171 | +2.4526% | -1.2421% | 4.8612 pp | -2.3939% |

必须区分两种统计：`median(B)/median(A)-1` 与 `median(B_i/A_i-1)` 并非同一量。E 固定 FFA 中位耗时比显示 -0.4130%，配对中位却是 +0.3454%；扩展全组中位耗时比 +4.6260%，配对中位 -0.8945%。排序后的中位值可能来自不同轮次；这里基线也随轮次波动，不能挑更有利的口径宣布提升。扩展 FFA 的两种统计都未支持提速，且配对效应小于 MAD。

逐案例与 group_sizes 分组表 [候选 E 扩展数值](expanded-e-timing.csv) 保留全部 5 轮 a/b 时间、每轮变化、中位数、MAD、吞吐与回退分类；分类本身不等于统计显著性。E 已撤回，A/B/C/D/E 全部未作为生产源码优化保留。A/A 相同二进制对照已完成，见下节；PGO 为独立构建特化实验，其正式结果见后文。

## A/A 相同二进制噪声对照

A/A 对照的 A、B 指向同一个二进制，SHA-256 均为 `46ba7eac5fc4e59fcaff7d36851d5e77fd293a04ecae4588248a07b2031780a9`。固定 9 个 FFA 案例，每例 13,000 场、单线程，预热之外完成 3 轮，每轮每侧 117,000 场。A/B 标签不代表任何实现差别。

| 轮次 | A 合计秒 | B 合计秒 | 配对耗时变化 |
|---|---:|---:|---:|
| 1 | 5.475842771 | 5.521636559 | +0.836287% |
| 2 | 5.309479330 | 5.611499841 | +5.688326% |
| 3 | 5.950150157 | 5.684801349 | -4.459531% |

A/B 总耗时中位数为 5.475842771 / 5.611499841 秒，中位耗时比变化 +2.477373%，对应吞吐变化 -2.417483%；配对耗时变化中位数为 +0.836287%，MAD 为 4.852039 个百分点。逐案例配对中位变化范围为 -5.972428% 至 +8.245690%，即使没有代码变化也会出现超过 5% 的个例表观变化。

这 3 轮说明本环境有明显的顺序与运行波动，不能视作稳定的噪声上界或显著性检验，也不能把 A/A 的偏差直接从 A/B 结果中相减。A/B/C/E 的小幅结果仍不足以支持稳定收益。已离线生成 [A/A 逐案例数值](control-aa-timing.csv)，保留逐案例、布局分组与逐轮数值；其中阈值分类仅是描述性标签，不是有效优化或回退判定。

## PGO 构建特化与训练一致性

训练集包含 48 个新阵容，固定 30 加扩展 85 的全部 115 个案例保留作 held-out 评估。训练与评估无相同文件内容、无相同无序角色名多重集合；单个角色名仍可能重叠，不能宣称角色完全隔离。训练 seed/round 区间为 `[1,000,000, 1,010,000)`，独立于评估区间。阵容按预先声明的布局规则生成，不依据性能结果筛选。

训练案例 `train__ffa8__02.txt` 出现 guard 耗尽 3 场、错误 0、胜场 956；原版 control 的复核得到相同结果。非零 guard 本身不是候选回归，也不能把这些场次删除后宣称行为一致。训练 helper 保留 guard 结果，wrapper 要求 control 与插桩版本的完整行为摘要一致；不筛除该案例，不调整 battle limit。已完成的 `training_comparison` 为 exact match：48 案例、480,000 场，control 与插桩版本的完整摘要一致，错误合计 0，guard 合计 3 且仅来自该案例。此验证不等同于最终 profile-use 二进制的 held-out 验证。

PGO profile-use 构建已完成，无构建错误，源码 hash 一致性检查通过，生产 core 源码未变。两个 LTO 二进制分别报告 181 个（`perf_runtime`）和 714 个（CLI）缺失函数 profile 的警告。观察到未训练的 wrapper 路径，但未逐一分类全部符号，不能把所有警告都归为 wrapper；已检查的主要采样热点名称未出现在警告中，也不能据此宣称训练覆盖完整。无 profile 路径需回退到编译器常规优化，性能覆盖仍不完整；此类警告本身不是行为错误证据，也不能代替行为测试。

固定与扩展 held-out 矩阵及更紧密配对确认均已完成，结果见下节；最终 profile 也已完成，不宣称所有输入普遍至少改善 5%。这些收益属于训练驱动的构建特化，不能写成已保留的默认 core 源码优化。

## 测试结果

工作区 `--no-fail-fast` 测试已执行至结束，汇总为 **935 通过、5 失败、7 ignored**。不能写成全部通过，也不能省略既有 ignored 项。5 项失败集中在 `tswn_ds4` 的 3 个集成测试 target：

| target | 失败测试数 |
|---|---:|
| `golden_pairing` | 1 |
| `golden_pipeline` | 3 |
| `golden_single` | 1 |

对 5 项实际输出与 fixture 的核验显示：实际输出为 CRLF、fixture 为 LF，统一换行后内容均相同。这解释了已观察到的比较失败，但原始测试仍为失败，不能将手工归一化核验重写为测试通过；未修改或重生成 golden/corpus 基线。

首次使用默认 Python 3.12 时缺少可链接的库；随后使用已有系统 Python 3.13 并在临时工作区设置链接，编译得以完成。此项是测试环境处理，不是生产代码修复，也不是性能优化。

正式 release 验收脚本已完成，退出码为 0：

| 验证项 | 通过 | 失败 | ignored | filtered out |
|---|---:|---:|---:|---:|
| release runtime 库测试 | 450 | 0 | 0 | 165 |
| release no_debug runtime 库测试 | 446 | 0 | 0 | 164 |
| release CLI runtime 测试 | 6 | 0 | 0 | 85 |
| release runtime-corpus | 124 | 0 | 0 | 0 |
| PGO profile-use no_debug runtime-corpus | 124 | 0 | 0 | 0 |

各行是独立测试调用，存在重复覆盖，不能简单求和为唯一测试数；filtered out 来自命令筛选，也不等于 ignored。PGO corpus 构建另报告 208 条缺失函数 profile 警告，测试仍为 124 通过、0 失败；训练未覆盖的测试入口不应被误读为完整 profile 覆盖。

新增训练 example 的 clippy 检查退出码为 0，库仍报告 30 条既有警告，不宣称零警告。性能工具新增 5 项单元测试全部通过；最终 nightly fmt 检查退出码为 0、无差异输出。3 个 Python 脚本的 `py_compile` 检查也以 0 退出。所有相关编译进程均已退出，避免与正式计时并行争用资源。

## PGO 固定集已完成的初步性能结果

固定 held-out 30 案例每例 13,000 场、单线程，预热之外完成 5 轮交替运行。FFA 9 案例的配对耗时变化中位数为 **-6.4163%**，MAD **0.5112 个百分点**，5 轮均更快。这是已完成固定子集上的正向结果，不是跨语料、跨机器的普遍 5% 保证；其中一轮仅改善 3.0352%，另一轮达 14.9081%，仍需结合环境噪声和后续确认看待。

| 范围 | 案例数 | 配对耗时变化中位数 | 配对 MAD |
|---|---:|---:|---:|
| 固定集全部 | 30 | -7.4638% | 1.7321 pp |
| FFA | 9 | -6.4163% | 0.5112 pp |
| duel | 11 | -17.9972% | 2.2780 pp |
| 队伍战 | 10 | -5.9529% | 0.6977 pp |

FFA 五轮配对变化依次为 -3.0352%、-5.9051%、-6.4163%、-14.9081%、-6.8167%。其 A/B 总耗时中位数为 5.086332114 / 4.621560430 秒，中位耗时比为 -9.1377%，对应吞吐 +10.0566%；这不是配对中位数，不取较大的数字作为首要收益结论，也不将耗时下降百分比直接写成吞吐提升百分比。

全部 10 次正式运行（5 轮 × 两侧）的 input、group_sizes、total、wins、errors、guard_exhausted 签名均与固定签名相同；每侧每轮共 390,000 场，错误与 guard 耗尽均为 0。已离线生成 [固定集逐案例数值](pgo-fixed30-timing.csv)，包含逐案例、真实布局分组与逐轮数值；表中 5% 分类和 2×MAD 标签是描述性规则，不是统计显著性认证。

扩展 85 案例与追加逐案例相邻 A/B 确认均已完成，见下节。训练规则已冻结，不依据 held-out 结果调整训练集或筛除不利案例。PGO recipe 保持 opt-in，不改为默认构建；训练数据与生成的 PGO profile 不发布，只交付审查后的安全源码和派生结果。

## PGO 扩展集结果与回退项

扩展 held-out 85 案例每例 3,000 场、单线程，完成 5 轮交替运行，每侧每轮 255,000 场。FFA 41 案例的配对耗时中位变化为 **-13.5276%**，MAD **1.2144 pp**；全部 85 案例为 **-10.8216%**，MAD **3.1485 pp**。两项聚合的五轮均更快，但不能掩盖个别回退或跨轮波动。

| 范围 | 案例数 | 配对耗时变化中位数 | 配对 MAD | 五轮全部更快 |
|---|---:|---:|---:|---|
| overall | 85 | -10.8216% | 3.1485 pp | 是 |
| ffa | 41 | -13.5276% | 1.2144 pp | 是 |
| team | 43 | -10.8168% | 3.6065 pp | 是 |
| duel | 1 | -22.4108% | 8.9455 pp | 否 |
| kind:generated | 48 | -13.8437% | 1.9281 pp | 是 |
| kind:stress | 37 | -10.7216% | 3.3739 pp | 是 |
| layout:ffa4 | 9 | -7.5110% | 4.4399 pp | 否 |
| layout:ffa6 | 8 | -8.8633% | 4.2575 pp | 否 |
| layout:ffa8 | 12 | -12.1430% | 5.2920 pp | 是 |
| layout:ffa12 | 6 | -12.1413% | 0.3506 pp | 是 |
| layout:ffa16 | 6 | -9.1633% | 7.3686 pp | 否 |
| layout:2v2 | 10 | -12.7446% | 4.2924 pp | 否 |
| layout:3v3v3 | 22 | -10.8466% | 4.5682 pp | 是 |
| layout:4v4 | 6 | -12.4003% | 2.4361 pp | 是 |
| layout:2v3 | 5 | -9.3439% | 2.1276 pp | 是 |

分组以实际 `group_sizes` 为准，FFA 人数分布为 4 人 9 例、6 人 8 例、8 人 12 例、12 人 6 例、16 人 6 例；队伍战 43 例，duel 1 例。generated 48 例、stress 37 例是另一交叉分类，不可与布局行叠加求总数。

逐案例配对中位数共 **82 例更快、3 例更慢、0 例相同**；FFA 为 41 快 / 0 慢，队伍战 40 快 / 3 慢，duel 1 快 / 0 慢。85 例中 46 例的效应落在 ±2×MAD 描述性范围内；这不是显著性检验，亦不能将这些案例一律判断为无效。以下按配对耗时变化从差到好列出最差 8 例，完整 85 例及逐轮值保存在 [扩展集逐案例数值](pgo-expanded85-timing.csv)。

| 案例 | 配对耗时变化中位数 | MAD | 效应在 ±2×MAD 内 | 五轮同方向 |
|---|---:|---:|---|---|
| `stress__3v3v3-711d7e09df7b7da3.txt` | +6.2936% | 12.0971 pp | 是 | 否 |
| `generated__2v2__06.txt` | +3.6129% | 22.3711 pp | 是 | 否 |
| `stress__3v3v3-42186ce5db28b886.txt` | +1.0033% | 5.5879 pp | 是 | 否 |
| `stress__3v3v3-7dfb2a54a1cd12d7.txt` | -0.4342% | 10.6887 pp | 是 | 否 |
| `stress__3v3v3-35070658d93637e1.txt` | -1.5928% | 2.6383 pp | 是 | 否 |
| `stress__ffa_4-b9ba9b639670ceb3.txt` | -1.9635% | 4.6102 pp | 是 | 否 |
| `stress__score_round_7.txt` | -2.8189% | 22.4812 pp | 是 | 否 |
| `generated__ffa4__04.txt` | -2.8391% | 4.1337 pp | 是 | 否 |

唯一配对中位耗时回退超过 5% 的案例是 `stress__3v3v3-711d7e09df7b7da3.txt`（+6.2936%）；其 MAD 为 12.0971 pp，轮间方向不一致。3 个回退项全部在 ±2×MAD 内且不是五轮均回退，但仍完整列出，不用噪声解释删除不利结果。

正式 10 次运行的 input、group_sizes、total、wins、errors、guard_exhausted 签名逐字段一致。每侧每轮错误 0、guard 耗尽 1；非零 guard 为 control 与 PGO 共有，不筛除或修改 battle limit。追加确认与最终 profile 均已完成；未依据此矩阵调整训练。

## 逐案例相邻 A/B 确认

固定 9 个 FFA 案例每例 26,000 场，预热外 5 轮。每个案例立即运行 A/B，按轮次与案例序号交替先后顺序；这比初次整矩阵逐侧交替更能缩短同案例时间间隔，但仍不能消除所有系统波动。每侧每轮 234,000 场。

| 指标 | 结果 |
|---|---:|
| 配对耗时变化中位数 | -7.6714% |
| 配对 MAD | 1.7756 pp |
| A / B 合计耗时中位数 | 10.618927413 / 9.804306942 秒 |
| 中位耗时比对应吞吐变化 | +8.3088% |

五轮配对耗时变化为 -9.4470%、-5.8438%、-7.6714%、-8.6410%、-3.2666%，全部更快，但并非每轮都超过 5%。9 个案例的配对中位均为负，范围为 -12.4082% 至 -1.5048%，也并非每个案例都超过 5%。全部 90 份正式逐案例输出的 total、wins、errors、guard_exhausted 与布局签名精确一致，错误与 guard 均为 0。

详见[确认实验逐案例数值](pgo-ffa-confirm-timing.csv)。这支持将 PGO 作为当前配置下有效的可选构建方式，不支持把全部收益归于某一个函数、对所有工作负载保证最低收益，或省略重新训练与验证。

## 最终 PGO CPU profile 对照

control 与 PGO 都使用扩展 85 案例、每例 5,000 场、相同名义 997 微秒采样设置。工具汇总 CPU 指标分别为 0.345 / 0.300 秒，对应约 346 / 301 个有效样本；逐案例 API 时间合计却为 34.6238 / 30.1846 秒。该尺度不一致再次说明名义采样率没有可靠实现，不能用 profile 的 CPU 秒数或探针运行时间作为正式加速证据。85 例行为签名完全一致，错误 0、guard 耗尽 1。

| 函数/符号 | control exclusive / inclusive | PGO exclusive / inclusive |
|---|---:|---:|
| `RuntimeRunner::run_to_completion_prevalidated` | 15.32% / 86.99% | 44.85% / 77.74% |
| `scan_plain_action_skill_probabilities` | 9.54% / 11.27% | 不再保留同名独立函数符号 |
| `select_plain_default_attack_target` | 6.94% / 8.38% | 未在本次独立函数表中观测到可比条目 |
| `PhaseScheduler::skill_hook_plan` | 3.18% / 6.36% | 符号仍存在，不据低采样归因推算成本 |
| `drain_plain_post_damage_skill_chain_into` | 2.89% / 6.07% | 4.98% / 4.98% |
| `PlayerTemplate::clone` | 0.29% / 3.18% | 4.65% / 5.32% |
| `SkillLoadout::clone` | 2.60% / 6.07% | 1.99% / 2.99% |

PGO 的 `drain_plain_attack_with_atp_into` 另占 exclusive 10.30%、inclusive 18.27%。`nm -C` 确认 control 保留独立 `scan_plain_action_skill_probabilities` 符号而 PGO 不再保留，相关 closure 仍存在；`skill_hook_plan` 在两者均存在。结合主 runner 的 exclusive 归因增加，这些证据符合编译器内联、代码布局和符号归因改变的解释，但不是具体优化机制的穷尽证明。

不能把扫描函数独立符号消失写成扫描成本归零，也不能用主 runner 的比例增大断言该函数变慢。两个分母不同，内联边界不同，且样本均少；这些比例不能直接相减为单函数绝对成本差。最终收益以未插桩的配对 E2E 表为准，不由采样表推导。基线分配探针不适合作为 PGO 分配变化证据，本报告不声称减少分配次数或字节。

## 复现步骤与构建绑定

以下为 Linux Bash 复现命令，从仓库根目录运行；Windows 的 shell、可执行文件后缀和亲和性工具需另作适配，不能据此承诺同样收益。需要 Python 3、Rust/Cargo、当前工具链的 `llvm-tools-preview`；本轮为 rustc 1.99.1（58cf8f9c3）、LLVM 23.1.1、`x86_64-unknown-linux-gnu`，release + no_debug，未启用 `target-cpu=native`。不同机器或编译器应重新建立 A/A 与对照，不直接拼接绝对秒数。

```bash
rustup component add llvm-tools-preview
python scripts/generate_perf_matrix.py --repo . --out target/perf-matrix
python scripts/build_pgo.py --repo . --work-dir target/perf-pgo \
  --input target/perf-matrix/training \
  --holdout docs/perf/fixed_cases_30 --holdout target/perf-matrix/expanded \
  --start 1000000 --runs 10000 --eval-max-rounds 1000000 --jobs 3

HOST=$(rustc -vV | sed -n 's/^host: //p')
A="target/perf-pgo/control-target/$HOST/release/examples/perf_runtime"
B="target/perf-pgo/use-target/$HOST/release/examples/perf_runtime"
# 输出目录必须不存在；先完成所有构建和测试，再独占运行计时。
taskset -c 2 python scripts/compare_perf_runtime.py --a "$A" --b "$B" \
  --input docs/perf/fixed_cases_30 --out target/perf-fixed30 --runs 13000 --rounds 5
taskset -c 2 python scripts/compare_perf_runtime.py --a "$A" --b "$B" \
  --input target/perf-matrix/expanded --out target/perf-expanded85 --runs 3000 --rounds 5
mkdir target/perf-ffa9
cp docs/perf/fixed_cases_30/*_ffa_*.txt target/perf-ffa9/
taskset -c 2 python scripts/compare_perf_runtime.py --a "$A" --b "$B" \
  --input target/perf-ffa9 --out target/perf-ffa-confirm --runs 26000 --rounds 5
taskset -c 2 python scripts/compare_perf_runtime.py --a "$A" --b "$A" \
  --input target/perf-ffa9 --out target/perf-aa --runs 13000 --rounds 3
python -m unittest discover -s scripts -p test_perf_tools.py
cargo +nightly fmt --check
```

公开比较脚本采用追加确认所用的逐案例相邻 A/B 顺序；前两张已记录矩阵最初采用整矩阵逐侧交替。上述命令重现相同输入、局数与构建比较，但顺序更紧密，不承诺复现同一小数结果。所有计时单线程，预热不计，统计口径均为先对案例求和、再计算轮间配对变化。按需要重新生成派生统计，不上传原始构建目录。

三个阶段都使用 `--release --features no_debug --locked --target <host>`，共享默认通用 CPU、fat LTO、单 codegen unit 和 mimalloc。control 编译参数为空；插桩阶段仅增加 `-Cprofile-generate=<work>/raw`；最终构建增加 `-Cprofile-use=<work>/merged.profdata -Cllvm-args=-pgo-warn-missing-function`。脚本通过 `CARGO_ENCODED_RUSTFLAGS` 传递，未使用 `target-cpu=native`。`LLVM_PROFILE_FILE=<work>/raw/%m_%p.profraw` 只用于训练；控制组没有 profile-use 参数。

构建脚本为 control、插桩训练、profile-use 使用独立 target 目录；记录编译器、Cargo、LLVM、源码指纹、Cargo.lock、训练输入和二进制散列，并在每次构建前后校验源码未变。训练 48 个新阵容与 115 个评估阵容隔离；不允许因 guard 或性能结果换样本。训练 profile 必须与产生它的源码、工具链、目标架构、features 和编译配置一起验证，不应跨版本盲目复用。缺失函数 profile 警告仍需保留和审查。

不提交二进制、profraw、profdata、训练产物、环境转储或原始运行日志。默认构建无需 PGO；选择使用者应在自己的目标场景重训、检查完整行为摘要，并重新运行所需测试及相同口径的性能矩阵。

## 验收范围与限制

本报告分别保留固定集、扩展集、追加确认和 A/A 噪声数据，不跨运行拼接最佳结果。性能结论仅覆盖实测输入、seed 区间、单线程公开胜率 API 与该构建；MAD 和 ±2×MAD 标签不是置信区间或统计显著性检验。扩展 3 个回退项全部保留，不以聚合改善掩盖单项退步。

默认生产源码没有优化改动。状态容量、clone 和缓存候选的负面或不确定结果仍有价值，但不能计入 PGO 收益。PGO 最终热点对照仅作粗粒度归因解释；分配探针只报告基线，不声称 PGO 减少了分配。源码或编译器变更后必须重新训练与验收。
