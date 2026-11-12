# 2026-10-24 维护前后性能与正确性报告

> 完成状态：34个配置完成五轮同机交替A/B，重点路径另做三组七轮独立确认；17项正确性/质量门禁最终通过。PWP真实校准的端到端编码准备未完成，明确保留为验证边界。

## 1. 结论先行

**本轮主要成果是正确性、依赖和维护性，不是全局大幅提速。** fixed30四档总体变化都在±1.5%内；编码器与图标路径没有证明跨场景、稳定的大幅加速。独立确认中，无图标1v1/2v2会话在本机固定输入下有小幅改善，不能归因于图标缓存，也不推广到全项目。

- 初测CQP score hot的+8.52%、带图标3v3v3的+3.91%、CLI pair回退信号，都没有在七轮独立确认中复现。
- PWP generate初测+3.64%，同参数独立确认−1.54%，未复现生成回退；validate确认有小幅偏慢，保留观察，不宣称所有路径均无回退。
- Python典型整场耗时大体持平，但icons压力场景有明显p95长尾；WASM控制组也存在独立轮次长尾。没有GC/调度trace，不能归因PyO3，也不能当作噪声删掉。
- **所有稳定摘要比对一致，完整workspace、release/no_debug、124项冻结corpus及四端精确payload验证通过。** 这不替代缺失的训练校准、原生GUI/GPU或外部数据库验收。

### 留档产物

- [机器可读结果与逐轮数字](../maintenance_2026-10-24_benchmark.json)：初测、三组独立确认、合并延迟分布、输入/产物哈希及正确性状态。
- [原始证据ZIP](../archives/maintenance_2026-10-24_raw.zip)：正式/预热样本、失败现场、污染隔离、逐调用延迟、监测日志、执行脚本及逐文件索引，纳入Git。
- [本机二进制快照ZIP](../../../dist/perf/maintenance-2026-10-24-cfee708d/binaries.zip)：仅供本机复测，位于既有忽略的dist目录，不纳入Git、不是发布包。

**上述两份ZIP都在target之外，后续cargo clean不会删掉它们。** 可确定性再生的Parquet大数据、Cargo缓存和PDB不打包；生成参数、分片SHA和验证日志完整保留。

## 2. 比较对象与环境

- 维护前产品：`704a1ac90885e73a1cc913f2b9e8dc2fc6aa0066`。
- 维护后产品：`cfee708dddbaf7770d8a70fbbd448b259f77f917`。
- 统一[串行驱动](../../../scripts/benchmark_maintenance.py)及[公开API微基准](../../../crates/tswn_core/examples/perf_maintenance.rs)：`5ec9a29b`；网页计时外截图开关补丁：`80a6452b`。后续提交仅含基准/报告，不改变被测产品运行逻辑。
- AMD Ryzen 7 5800X，8核16逻辑处理器，约64 GiB内存；Windows 10专业版22H2，build19045；电源计划保持“平衡”，不改系统频率或全局配置。
- Rust/Cargo 1.99.0、LLVM23.1.1、`x86_64-pc-windows-msvc`；wasm-bindgen CLI0.2.129与锁内版本匹配。
- 各版本各自锁文件、`--locked`、普通release、fat LTO、单codegen unit、`no_debug`，原生mimalloc；Openbox探针不启用GUI。命令级清空wrapper，关闭incremental和调试信息，6个job串行构建两版；构建不进入计时。

基线worktree可能由Git检出CRLF，已核对LF规范文本一致；实际两版共读当前输入。48项输入/派生输入记录哈希。未重生成golden/corpus，也未修改像素基线。

## 3. 方法、数量与边界

每个配置双方各预热一次，随后五轮交替：奇数轮旧→新、偶数轮新→旧。独立确认另开新目录、各七轮，不与初测合并。耗时变化为`median(新)/median(旧)-1`，负值更快；另列逐轮配对变化中位数与MAD。

小于1.5%的差异不宣称收益；临界变化、短任务、尾分位数需结合离散度和绝对时间。不能以297条指标“投票”：score的case/overall及固定集分组存在重叠，并非独立实验。

- 主矩阵：34个完成配置、340次正式进程调用、297个指标行；另有双方预热、8次官方JS参考。PWP端到端encode准备失败，没有性能样本，不计入完成配置。
- session确认：14次正式进程调用，8个工作负载，每项每轮2000场。
- score/CLI确认：4个独立配置、56次正式进程调用；CQP每组1000→3000，mario13000→50000，不能横比两批绝对秒数或合成十二轮。
- PWP确认：同参数七轮、14次正式进程调用，实际每轮2700局、20285样本。
- Python/WASM：严格r1..r5，共16000场、524000次next_frame调用；Web200场、6350次pull、14950次render。分位数直接合并原始样本，以nearest-rank计算，不平均各轮p95、不删除慢样本。

最初encoder20000次/session100次的143.592秒预热仅用于估量工作量，单独归档；正式均使用encoder2000次/session2000次。

### 外部编译污染

01:40后另一Bevy/bevy_egui项目进行release构建；快照观察到rustc约416%单核CPU（约4个核心）。本任务没有终止它。按保守01:40边界将跨过该时段的整项配置归档：8配置、287数据文件，另有说明和runner日志。

等待外部Cargo自然结束后，两种Python/WASM和6种Openbox CQP/CQD配置都从预热开始重跑全部五轮；此前已完成的core、score与CLI保留。ZIP中的`contaminated-from-0140/`只作审计，不入最终统计。不是仅挑走慢样本，也不因此保证机器已消除全部背景噪声。

## 4. fixed30：核心战斗整体持平

全部30输入，每case13000场，每轮390000场；单线程/自动、hot/timed分开。hot不插入逐场计时，timed仅作init/fight诊断；自动线程worker累计时间不与外层墙钟相加。

| 档位 | 旧中位数 s | 新中位数 s | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| 单线程 hot | 10.864941 | 10.998843 | +1.23% | +1.38% |
| 单线程 timed | 11.197744 | 11.084474 | -1.01% | -2.60% |
| 自动线程 hot | 1.382768 | 1.391988 | +0.67% | +0.20% |
| 自动线程 timed | 1.386737 | 1.376928 | -0.71% | +0.82% |

四档总体按预设门槛均不作收益或确定回退结论。逐case、1v1/2v2/多人分组、wins与guard都在JSON/ZIP保留；total、wins、errors与guard摘要跨版本一致。guard存在正常保护终止，不强求为零。

## 5. 编码器：减少重复工作，未证明普遍明显提速

四个固定stress输入，预推进4个可见帧后反复编码同一未决状态。紧凑合法profile与BASELINE_64并列，B=1预分配后调用encode_into；准备状态/manifest/缓冲、预热、全张量checksum在计时外。固定合成归一化常数只供这项版本对照，不是训练工件。

| 输入 / 容量档 | 旧 µs/次 | 新 µs/次 | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| 1v1 / maintenance-case-sized | 160.895 | 158.861 | -1.26% | +1.23% |
| 1v1 / baseline-64 | 823.748 | 829.078 | +0.65% | +2.28% |
| 2v2 / maintenance-case-sized | 321.958 | 316.828 | -1.59% | -3.05% |
| 2v2 / baseline-64 | 959.101 | 947.182 | -1.24% | -0.02% |
| ffa8 / maintenance-case-sized | 640.298 | 624.170 | -2.52% | +0.64% |
| ffa8 / baseline-64 | 1270.437 | 1271.468 | +0.08% | +0.83% |
| 3v3v3 / maintenance-case-sized | 736.146 | 723.403 | -1.73% | -1.69% |
| 3v3v3 / baseline-64 | 1409.817 | 1370.970 | -2.76% | -2.76% |

本轮源级上确实少一次容量遍历，但编码还包含验证、padding清空、索引和完整张量写入。标准容量与紧凑容量的不同耗时不能当版本收益；当前五轮数据没有证明跨输入、跨容量的稳定大幅加速。不能把局部−1%～−3%推广到整个战斗引擎。

## 6. 会话与初测信号的独立确认

下表为七轮确认，每项每轮2000场；包含new→逐帧消费→result→释放，不含JSON转换。只对同输入/同选项比较checksum，不能跨图标选项比较。

| 输入 / icons | 旧 µs/场 | 新 µs/场 | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| 1v1 / false | 195.716 | 186.930 | -4.49% | -2.37% |
| 1v1 / true | 264.908 | 262.542 | -0.89% | -0.55% |
| 2v2 / false | 404.889 | 393.394 | -2.84% | -3.35% |
| 2v2 / true | 580.298 | 575.654 | -0.80% | -0.09% |
| ffa8 / false | 1478.324 | 1490.044 | +0.79% | +0.85% |
| ffa8 / true | 2167.852 | 2152.869 | -0.69% | -1.03% |
| 3v3v3 / false | 1977.501 | 1992.528 | +0.76% | +0.40% |
| 3v3v3 / true | 2564.998 | 2568.334 | +0.13% | +0.11% |

无图标1v1/2v2在本机固定工作负载下观察到小幅改善；带图标四项确认都在±1.5%内，**不能写成图标缓存已确认加速**。其余场景也不作收益。

| 信号 | 初测中位数比 / 配对中位 | 独立确认中位数比 / 配对中位 | 结论 |
| --- | ---: | ---: | --- |
| CQP score hot | +8.52% / +8.05% | +0.03% / -0.43% | 初测回退未复现；确认工作量扩大 |
| 带图标3v3v3 | +3.91% / +3.79% | +0.13% / +0.11% | 初测回退未复现 |
| CLI pair | +1.91% / +6.13% | -0.21% / -1.04% | 初测回退信号未复现，不宣称收益 |
| mario score hot | +2.41% / +4.39% | -2.28% / -0.90% | 两种确认口径不一致，不挑有利数字宣布收益 |
| PWP generate | +3.64% / +3.64% | -1.54% / -1.54% | 生成回退未复现，确认值接近门槛，不推广为稳定收益 |

## 7. CLI 与 Openbox 主矩阵

CLI胜率使用百万场简单/复杂输入的单线程与自动线程；auto、group-win-rate、batch-rate、pair另列。CLI外层时间包含进程启动、输入及输出，业务签名剔除动态用时字段。CLI的线程0无效，省略表示自动，不与probe的0=auto混淆。

| CLI配置 | 旧中位数 s | 新中位数 s | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| cli-win-simple-single | 5.579376 | 5.649877 | +1.26% | +1.26% |
| cli-win-simple-auto | 0.719332 | 0.705256 | -1.96% | -3.11% |
| cli-win-complex-single | 9.009553 | 8.963747 | -0.51% | +1.59% |
| cli-win-complex-auto | 1.179297 | 1.187191 | +0.67% | -0.99% |
| cli-auto | 1.038004 | 1.072789 | +3.35% | +3.76% |
| cli-group-win-rate | 0.222797 | 0.227647 | +2.18% | +2.63% |
| cli-batch-rate | 0.512116 | 0.520803 | +1.70% | +2.80% |
| cli-pair | 5.355511 | 5.458032 | +1.91% | +6.13% |

Openbox单人20×35，派生双人32×41，各count100/1000/10000；report-ms=3600000避免周期RSS子进程污染。表中是内部计算墙钟，完整进程墙钟另存raw，不能混用。

pair：count100为20×16×41候选矩阵，1000/10000为8×4×41；另有带权及live档，head固定5。普通输出按稳定业务字节核对，live按无序业务行核对，不把合法到达顺序差异当语义变化。

历史双人32组文件未跟踪，按固定20选手索引组合顺序派生前32对，另记哈希，不冒充历史输入。

| Openbox配置 | 旧中位数 s | 新中位数 s | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| openbox-cqp-100 | 0.105434 | 0.112853 | +7.04% | +2.82% |
| openbox-cqp-1000 | 1.049531 | 1.048171 | -0.13% | +4.05% |
| openbox-cqp-10000 | 10.326523 | 10.313302 | -0.13% | -2.07% |
| openbox-derived-cqd32-100 | 0.356378 | 0.363505 | +2.00% | +1.95% |
| openbox-derived-cqd32-1000 | 3.754078 | 3.749817 | -0.11% | -0.19% |
| openbox-derived-cqd32-10000 | 35.978358 | 35.318060 | -1.84% | -4.11% |
| openbox-pair-100 | 5.872580 | 6.055373 | +3.11% | +2.45% |
| openbox-pair-1000 | 5.616683 | 5.696827 | +1.43% | +1.43% |
| openbox-pair-10000 | 45.178683 | 46.063676 | +1.96% | +1.65% |
| openbox-pair-live | 4.644918 | 4.685276 | +0.87% | -2.82% |
| openbox-pair-weighted | 2.039318 | 2.058614 | +0.95% | +0.05% |

CQP100仅约0.1秒，+7%左右对应几毫秒；较长档的中位数比近零，但配对方向有分歧。正确结论是“短任务变化未形成随规模稳定的回退证据，本轮不作确定回退结论”，不是证明全是噪声。也不把pair高档约2%的观察当全局收益/回退。

## 8. PWP：真实数据、确认结果与校准缺口

固定30输入经规范化去重后为27阵容，每阵容100局，**实际2700局、20285样本**；8样本/局上限、250局/分片、自动线程、采样间隔0.02秒。两版Parquet字节签名一致。generate包含两遍战斗、Parquet写入及分片回读；validate另计。

| 七轮同参数确认 | 旧中位数 s | 新中位数 s | 中位数比变化 | 配对中位变化 |
| --- | ---: | ---: | ---: | ---: |
| pwp/generate | 2.517415 | 2.478697 | -1.54% | -1.54% |
| pwp/validate | 0.923047 | 0.942754 | +2.13% | +3.92% |

生成初测+3.64%未复现。校验确认中位数比+2.13%、配对中位+3.92%，保留小幅偏慢观察；其中I/O和采样线程收尾等待均计入墙钟，一周期约20ms，不据此宣布整个管线无回退。CPU字段是进程累计值，validate CPU不能当阶段独立增量；结束RSS与采样峰值亦不混淆。

端到端encode单独准备了8局、最多64样本的小数据源，避免把全量稠密padding物化到近百GiB。但真实校准报告没有观察到`state.payload.shield_value`，build-manifest按完整性门禁返回`MissingCalibration`，**没有产生端到端编码性能样本**。

失败日志及校准现场完整留档，不填默认常数、不绕过契约。第5节的合成尺度operator微基准不能替代真实训练契约/导出验收。后续应补真实字段覆盖或明确未观测字段的契约策略，再重跑校准与encode。

## 9. Python / Node WASM 与浏览器

每后端、模式、版本、输入各500场；正常GC，不强制收集。Python/Node的next_frame含末次None/null，total不含result转换，创建/逐帧/总场分别统计；不是纯DTO转换成本，也不与Rust会话的计时范围混算。

独立统计复核256个p50/p95和30个源文件SHA通过。下表均是合并r1..r5后的整场耗时，单位ms。

| 后端 / 图标 / 输入 | p50 旧→新 ms | p95 旧→新 ms |
| --- | ---: | ---: |
| python / plain / 1v1 | 1.628 → 1.625 | 2.039 → 2.029 |
| python / plain / 2v2 | 4.326 → 4.357 | 5.044 → 5.886 |
| python / plain / ffa8 | 22.617 → 23.250 | 27.733 → 29.042 |
| python / plain / 3v3v3 | 26.213 → 26.249 | 31.104 → 31.404 |
| python / icons / 1v1 | 1.907 → 1.851 | 5.671 → 3.253 |
| python / icons / 2v2 | 4.925 → 4.956 | 10.200 → 6.857 |
| python / icons / ffa8 | 27.198 → 27.126 | 34.947 → 54.353 |
| python / icons / 3v3v3 | 30.728 → 30.705 | 35.670 → 44.548 |
| wasm / plain / 1v1 | 1.523 → 1.488 | 2.305 → 2.195 |
| wasm / plain / 2v2 | 3.505 → 3.413 | 4.800 → 4.321 |
| wasm / plain / ffa8 | 16.023 → 16.139 | 22.634 → 20.376 |
| wasm / plain / 3v3v3 | 18.272 → 18.854 | 21.662 → 24.421 |
| wasm / icons / 1v1 | 1.682 → 1.648 | 2.356 → 2.733 |
| wasm / icons / 2v2 | 3.788 → 3.835 | 4.692 → 4.978 |
| wasm / icons / ffa8 | 17.814 → 17.831 | 21.551 → 21.156 |
| wasm / icons / 3v3v3 | 20.998 → 20.620 | 25.359 → 38.058 |

Python整场p50约−2.95%～+2.80%，未见统一明显退化；**仍保留icons压力长尾风险**：ffa8 p95约+19.406ms/+55.53%，3v3v3约+8.878ms/+24.89%，主要由第二轮抬高。WASM icons 3v3v3也有独立第四轮长尾，p95约+50.08%而p50下降。

这些说明轮次波动存在，不证明Python尾部全是环境/GC，也不构成PyO3单独因果实验。原始max/p99/逐轮分布保留，后续若关注交互尾延迟，应加GC/调度trace并增加独立轮次，而不是抹掉长尾或立刻重写DTO。

### Web：DOM计时，非视觉截图验收

两版同Edge、1440×1000、headless、turbo，各四输入×25场。每轮保留首场初始化/JIT。原计时外截图在当前环境返回“Unable to capture screenshot”，已保留失败；两版统一`TSWN_BENCH_WEB_SCREENSHOT=0`重跑，未改被测前端、计时区间或GC。

| 输入 | TTIS p50 旧→新 ms | TTIS p95 旧→新 ms | TTFE p95 旧→新 ms | pull p95 旧→新 ms | 帧/chunks |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1v1 | 2.6 → 2.5 | 25.3 → 27.1 | 38.0 → 36.8 | 0.8 → 0.9 | 10/28 |
| 2v2 | 3.1 → 2.9 | 4.7 → 4.5 | 6.8 → 6.6 | 0.5 → 0.5 | 15/43 |
| ffa8 | 5.8 → 6.0 | 7.6 → 8.0 | 9.9 → 10.0 | 0.9 → 0.8 | 45/109 |
| 3v3v3 | 5.4 → 5.5 | 8.6 → 8.0 | 10.2 → 9.6 | 0.6 → 0.6 | 57/119 |

TTIS/TTFE中位数多数只变约0.1～0.2ms，未见统一持续回退。pull/render约0.1ms量化，一格变化不包装成高精度百分比。1v1 TTIS p95受每轮首场初始化支配。Web计时JSON只验证帧/chunk/pull和无错误，不含winner/rounds；完整DTO由第11节四端脚本另验。

## 10. 官方JS横向参考（不是维护收益）

Node/Bun各执行fixed、win-rate、score、matrix一次，模块和预热在内部计时外。JS fixed实际只覆盖17个两队case，跳过13个多队，不能与Rust full30 overall直接计算“加速比”。本轮Rust升级未改变JS，不纳入维护A/B结论。

| 运行时 | 版本 | 模式 | 实际场数 | 内部墙钟 s | errors |
| --- | --- | --- | ---: | ---: | ---: |
| bun | 1.4.4 | fixed30-two-team-single-thread | 221000 | 49.990918 | 0 |
| bun | 1.4.4 | cqp-matrix-auto | 280000 | 24.061055 | 0 |
| bun | 1.4.4 | score-single-thread | 13000 | 3.762014 | 0 |
| bun | 1.4.4 | win-rate-single-thread | 100000 | 10.074608 | 0 |
| nodejs | 24.19.0 | fixed30-two-team-single-thread | 221000 | 80.615387 | 0 |
| nodejs | 24.19.0 | cqp-matrix-auto | 280000 | 15.075835 | 0 |
| nodejs | 24.19.0 | score-single-thread | 13000 | 5.082586 | 0 |
| nodejs | 24.19.0 | win-rate-single-thread | 100000 | 19.749991 | 0 |

## 11. 正确性监测：最终17项通过

全部性能测量和复测都与构建/测试串行，避免本任务内部CPU竞争。PWP边缘复测在测试全部结束、确认无cargo/rustc后执行。

| 检查 | 结果 |
| --- | --- |
| 完整workspace/all-features，含其他crate、集成与doctest | 通过；核心610、CLI91等 |
| 核心无默认feature lib | 614项通过 |
| release Runtime、release/no_debug Runtime、release CLI Runtime | 全部通过 |
| release冻结runtime corpus | 124项通过，不重生成 |
| 真实CLI/CAPI构建、Python公开API | 通过 |
| WASM接口契约 | 8项通过 |
| Rust CLI / Python / CAPI（ctypes）/ WASM（Node）完整payload | include_icons=false、max_rounds=1/20000，四输入共8场initial/frames/result逐字段一致 |
| 前端纯模块 | 43项通过 |
| DOM播放/主题/盾条/中止/历史/释放等集成断言 | 通过 |
| Python脚本单测（含NumPy编码读取） | 6项通过 |
| 公共API文档Rust/Python/WASM示例 | 实际执行通过 |
| nightly fmt、全workspace/all-targets/all-features Clippy | 通过；Clippy仍有非阻断风格警告，不宣称零告警 |
| 严格Rustdoc，`RUSTDOCFLAGS=-D warnings` | 通过 |

首次DOM监测命令漏传Node `--experimental-vm-modules`，在启动时失败；补参数实际重跑通过，未修改页面。原始失败日志与修正命令/重试日志一起保留，不覆盖失败事实。17项是最终检查项目数，不是17个单元测试。

主Cargo锁SHA保持`2eefc1388e76ced45d43d171a7ba2e1be16b98711e4504553cb5ed5ea5d12f52`。测试生成的crate内默认setting目录，已按测试前不存在、17文件均在测试时段创建/写入及精确路径核验后清理；未触碰用户根setting。

不连接生产数据库；独立单号排名工具的外部校准资源、完整wheel分发与原生GUI/GPU不在这组门禁内。按用户要求不继续排查RDP访问冲突。本报告的“完整”指明确列出的可执行入口/配置，不是所有规模和线程数的笛卡尔积。

## 12. 复现与归档校验

两版先按各自锁文件构建并复制产物，后运行[串行驱动](../../../scripts/benchmark_maintenance.py)。驱动源码、所有阶段PowerShell/Python脚本及命令元数据都在原始ZIP。

```powershell
# 构建阶段统一环境；不修改全局Cargo配置
$env:RUSTC_WRAPPER = ''
$env:CARGO_BUILD_RUSTC_WRAPPER = ''
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_BUILD_JOBS = '6'
# 已准备baseline/current隔离产物后，各阶段串行执行
python scripts/benchmark_maintenance.py --phase all --resume
# 当前真实校准会在pwp_encode阶段按契约拒绝：保留失败后继续独立阶段
$env:TSWN_BENCH_WEB_SCREENSHOT = '0'
python scripts/benchmark_maintenance.py --phase web --resume
python scripts/benchmark_maintenance.py --phase reference --resume
python scripts/benchmark_maintenance.py --phase summary
# 精确的三组七轮确认命令/脚本在原始ZIP的confirm_*.py与元数据内
# 二进制快照解压后，从工作区根手动复测微基准，显式提供输入避免依赖编译期worktree
./baseline/native/perf_maintenance.exe --input-dir crates/tswn_test/cases/runtime_stress --encoder-iters 2000 --session-iters 2000 --max-rounds 20000
# DOM正确性脚本必须打开VM Modules；纯模块单独跑
node --experimental-vm-modules --test scripts/verify_web_playback.mjs
node --test crates/tswn_wasm/tests/*.test.mjs
```

`--resume`逐次核对驱动hash、命令、输入清单hash、产物hash和环境，身份变化拒绝复用；失败PWP使用全新attempt目录，不把续跑的剩余/零工作量当性能样本。


- 原始证据ZIP：8,097,397字节；SHA-256：`d650c5fa01caa2d8543bab6e19fbb40c946bd66ba911b16f8d9c8b05eb31ce4e`。
- 本机二进制快照ZIP：24,468,097字节；SHA-256：`b37bdaa5e3ec5e0d68615970bc931befd44ab059323136b2d474dab28bc74dc1`。
- 原始证据收录2398个文件，另含ZIP内索引与说明；ZIP CRC完整性已检验。


## 13. 后续优先级

1. **真实训练校准覆盖**：补齐未观察字段或定义明确的未观测策略，解决MissingCalibration后再验收PWP端到端encode；不以合成尺度冒充训练工件。
2. **绑定长尾定位**：对Python/WASM icons压力场景追加GC/调度trace，区分分配、转换与机器时序；保留现有原始长尾，不凭一次p95改架构。
3. **encoder实际瓶颈**：标准profile与紧凑profile差距明显，后续应独立采样padding/张量写入/验证成本，再决定是否改清槽与预算，保证失败清理契约。
4. **回归基准维护**：正式归档双人派生输入或其生成配方、继续分离短任务/大批量及内部/全进程时间；不能因某项初测好看就报全局加速。

本次没有为追求好看的数字回滚依赖、删除慢样本、重生成golden或绕过校准。性能结论按具体工作负载和证据范围给出，正确性与未完成项分别列明。
