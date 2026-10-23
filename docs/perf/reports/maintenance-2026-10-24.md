# 2026-10-24 维护前后性能与正确性报告

> 状态：采集中。当前只记录测量口径、已完成样本及环境事件；最终结论待独立复测与正确性门禁完成后更新。

## 1. 比较对象与环境

- 维护前：`704a1ac90885e73a1cc913f2b9e8dc2fc6aa0066`。
- 维护后产品代码：`cfee708dddbaf7770d8a70fbbd448b259f77f917`。
- 统一基准驱动：`5ec9a29b75f9051803ce53aa5e9178fc56f07be5`。它只补充基准，不修改产品运行逻辑。
- CPU：AMD Ryzen 7 5800X，8 核 16 逻辑处理器，约 64 GiB 内存。
- Windows 10 专业版 22H2，build 19045；电源计划保持“平衡”，不调整频率或系统设置。
- Rust / Cargo：1.99.0，LLVM 23.1.1，`x86_64-pc-windows-msvc`。
- 构建：各自锁文件、`--locked`、普通 release、fat LTO、单 codegen unit、`no_debug`，原生 mimalloc；Openbox 探针不启用 GUI。
- 清空 wrapper 仅限命令环境，关闭 incremental，以 6 个构建 job 串行准备两版原生与 WASM 产物；测量期间不启动本任务的编译或安装。

环境及二进制哈希保存在[原始运行目录](../../../target/maintenance-bench-20261024/)。构建时间不计入性能结果。

## 2. 固定方法与覆盖矩阵

[串行 A/B 驱动](../../../scripts/benchmark_maintenance.py)对每个配置各预热一次，然后执行五轮交替顺序：奇数轮旧→新，偶数轮新→旧。全部原始 stdout、stderr、JSON 和每轮元数据保留。

耗时变化为 `median(新) / median(旧) - 1`；负值更快。同步保留配对变化、MAD 和范围，不删除慢样本，不把小于 1.5% 的差异宣称为收益。可疑差异另开独立轮次确认，原始批与确认批分开记录。

| 范围 | 固定配置 | 结果边界 |
| --- | --- | --- |
| fixed30 | 全部 30 个输入，每 case 13000 场；单线程/自动线程，hot/timed 分开 | 记录 overall、1v1、2v2、多人与逐 case；自动线程 worker 累计 timing 不等于墙钟 |
| score | mario 每组 13000；固定20组与派生双人32组每组1000；单线程、hot/timed分开 | example 默认 modifier 为 `!`，不混作普通评分 |
| CLI | 百万场简单/复杂胜率，单线程/自动；auto、group、batch-rate、pair | 外部进程墙钟包含启动与输出；签名剔除用时字段 |
| encoder | 四个固定 stress 输入，预推进4可见帧；紧凑档/BASELINE_64，B=1，预分配 `encode_into`，各2000次 | 状态、契约和缓冲准备在计时外；合成固定校准仅供对照，不是训练工件 |
| Rust会话 | 四个固定 stress 输入，icons=false/true，各2000场，max_rounds=20000 | 含构造、逐帧消费、结果及释放；不含JSON转换 |
| Python/Node WASM | 四输入×icons两档；每轮每类100场，预热10场，五轮 | 独立进程/隔离产物；保留正常GC及原始逐调用样本 |
| Openbox CQP/CQD | 20×35单人；派生32×41双人；count=100/1000/10000，自动线程 | `--report-ms 3600000` 排除周期RSS子进程干扰；内部墙钟与全进程墙钟分列 |
| Openbox pair | count100全20×16×41；1000/10000用8×4×41；另有带权和live档 | 核对完成数；普通输出按字节，live按无序业务行校验，不把到达顺序当语义 |
| PWP | fixed30，每case100局，8样本/局，250局/分片，自动线程，采样间隔0.02秒 | generate/validate分开；CPU为进程累计值，不冒充阶段增量 |
| PWP encode | 单独8局小数据集、最多64样本；共用数据与真实校准manifest，B=8 | 避免全量稠密编码占近百GiB；端到端包含I/O、哈希与校验，不等于纯encoder |
| Web | 四输入，每轮每类5场，五轮合计25；1440×1000、headless Edge、turbo | TTIS/TTFE是DOM指标，不是GPU像素呈现；合并原始样本求分位数 |
| 官方JS参考 | Node/Bun，各fixed/win-rate/score/matrix | JS未随本轮Rust变化，不纳入维护前后收益；fixed只覆盖17个两队case |

历史双人32组输入未受版本控制。本轮从受跟踪20选手按索引组合顺序确定性取前32对，分别生成后端行式与Runtime空行分组形式，记录哈希；不冒充历史输入或与历史绝对值比较。基线worktree可能由Git检出CRLF，已核对LF规范文本一致；实际两版共读当前工作区输入。

最初一次encoder 20000次、session100次的预热耗时143.592秒，仅用于估量工作量，已单独归档。正式两版都固定encoder2000次、session2000次，不混入试跑样本。

## 3. 外部编译污染与隔离

01:40之后出现另一个使用Bevy/bevy_egui的release构建，非本workspace依赖。本机快照记录到rustc约416%单核占用（约4个核心），与Openbox短任务及并行档明显波动重合。

处理方式：

1. 不终止、不修改其他项目的进程。
2. 停止本任务采样，按保守边界01:40对整项配置归档，包含预热和所有快慢样本，不挑选快样本。
3. [污染划分记录](../../../target/maintenance-bench-20261024/contamination-plan.json)保存边界、受影响配置与观察依据；[污染样本目录](../../../target/maintenance-bench-20261024/contaminated-from-0140/)保存原始数据。
4. 外部Cargo结束后，从各配置预热开始重跑全部五轮。此前完成的core、score与CLI配置保留，并对可疑变化独立确认。

因此，污染时段的波动不作为本轮代码提升或回退的证据。最终报告仍需给出有效样本的离散度，不承诺消除机器的全部背景噪声。

## 4. 当前已完成的正确性比对

- 首批16项encoder/会话五轮：同档位张量checksum、帧数、终局一致。
- fixed30与score有效样本：总场数、胜场、errors及guard与基线一致。
- CLI有效样本：胜场/总场数或去计时后的业务输出一致。
- 完整workspace、无默认feature、release/no_debug、冻结corpus、四端逐字段payload等独立门禁已安排在性能采样结束后串行执行，当前不记为已通过。

## 5. 最终结果与留档

待主矩阵、独立确认和正确性门禁全部完成后补充：主结果表、逐路径回退/收益判定、合并延迟分位数、完整正确性结果、原始数据索引与提交记录。

本轮不测原生GUI/GPU帧耗时；按用户要求不继续排查RDP访问冲突。DS4、排名及数据库相关业务没有现成统一基准，亦不连接生产服务。本报告的“完整”指上述可执行基准入口与明确配置，不表示所有业务、所有规模、所有线程数的笛卡尔积。
