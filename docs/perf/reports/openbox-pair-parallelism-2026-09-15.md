# OpenBox pair 并行修复：同机交替 A/B 复测

> 日期：2026-09-15
> 基线：`9a2bdc1a14f6b5bb10844434e843b94d0bbc7b05`（`9b07a04e` 的父提交）
> 候选：`9b07a04ec51a7869dc409f5c0770fb874dd01318`（本次并行修复）
> 状态：**已在本机实测**。pair 路径达到预期加速，`fixed30` 与共享 CQP/CQD 矩阵都没有回退；
> 复测过程中发现并排除了 `openbox_mem_probe` 周期性 RSS 上报引入的仪表伪影，详见第 5 节。
> 原始样本：`docs/perf/openbox_pair_parallelism_9b07a04e_ab_samples.json`

## 1. 复测环境与构建

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器，64 GB 内存，Windows x86_64-pc-windows-msvc；
- Rust：`rustc 1.98.1 (48a229cea 2026-09-01)`、`cargo 1.98.1 (797e8a9bc 2026-08-05)`；
- 构建：workspace `release` profile（`lto = "fat"`、`codegen-units = 1`、`debug = 1`），
  `tswn_core` 启用 `no_debug + mimalloc_alloc + png_render`；
- 仓库内不存在 `.cargo/config.toml`，本轮没有额外 `rustflags`；
- 不使用 GitHub Actions，全部数字来自本机。

两个隔离 worktree，各自独立 `target/`：

```powershell
git worktree add ..\tswn-core-perf-base 9a2bdc1a   # baseline
git worktree add ..\tswn-core-perf-cand 9b07a04e   # candidate

# 两个 worktree 分别执行：
cargo build --release -p tswn_openbox --bins
cargo build --release -p tswn_core --bin tswn-cli --example perf_runtime --features no_debug
```

线程口径：`openbox_mem_probe --threads 0` 与 CLI 省略 `-t` 都表示自动线程（CLI 的 `-t 0` 是非法参数）；
`perf_runtime --threads 0` 表示自动线程。自动线程档在 16 逻辑处理器上使用 24（短任务）/32（长任务）worker。

## 2. 方法

- **同会话交替 A/B**：每个配置先各自预热 1 轮，然后跑 3 轮；奇数轮 baseline→candidate，
  偶数轮 candidate→baseline，抵消频率/温度漂移；表中数值为 3 轮中位数。
- **两层计时**：外层用 Python `perf_counter` 记录整条进程墙钟（含进程启动与输入解析）；
  同时记录各工具在业务调用外层自报的墙钟（`openbox_mem_probe` 的 `t=`、`openbox_pair_probe` 的
  `elapsed_s`）。pair 相关 workload 用墙钟中位数，CQP/CQD 短档用内部计时（进程启动与收尾
  约有 0.4 s 的固定开销，会稀释秒级以下的差异）。
- **仪表干扰**：`openbox_mem_probe` 默认每 2 秒在进度回调里上报一次 RSS，而 Windows 上的 RSS
  查询要 spawn 一个 powershell（约 0.3 s）。这个停顿落在被测量的区间里，并且会因为候选侧的有界
  完成事件队列反过来阻塞 worker，属于仪表伪影：第一批带默认上报的样本让候选看起来在 100% 精度档
  慢了 13%，关掉周期性上报后同一配置是 ±1%。所有 CQP/CQD 复测因此都加 `--report-ms 3600000`，
  第一批样本已作废（细节见第 5 节）。
- 正确性门禁：每个配置对两侧输出取 SHA-256 并逐字节比较。25 个配置全部分别只有 1 组哈希，
  即新旧输出完全一致（见第 4.4 节）。
- 固定输入及其 LF 归一化 SHA-256 见随附 JSON 的 `input_sha256_lf`；其中
  `cqp_double_target.txt` 已不在仓库内（`d9dcba0d` 删除且被 `.gitignore` 忽略），
  本次从 `d9dcba0d^` 原样恢复，哈希 `EF42B91E…` 与历史 `cqp-runtime-baseline.md` 记载一致。

本轮覆盖的入口与各自的职责：

| workload 前缀 | 入口 | 覆盖的改动 |
| --- | --- | --- |
| `fixed30-*` | `crates/tswn_core/examples/perf_runtime.rs` | 纯战斗热路径回归门禁（与调度无关） |
| `cqp-*` / `cqd-*` | `openbox_mem_probe` → `tswn_openbox::backend::run_batch_rate` | 共享 CQP/CQD 矩阵 `runtime_cqp_matchups` |
| `pair-probe-*` | 本地临时探针 → `tswn_openbox::backend::run_pair` | OpenBox GUI 的 pair 后端（窗口化矩阵 + 共享调度器） |
| `cli-pair-*` | `tswn-cli bench pair` | CLI pair 外层并行（`bench_sched` + `bench/batch.rs`） |
| `check-cli-batch-*` | `tswn-cli bench batch-rate --log` | 输出文件对账（剔除计时字段后哈希） |

`fixed30` 在本轮里同时充当**控制组**：它不经过本次改动的调度代码，两侧编译产物的
差异只有源码补丁本身。它的 ±0.1% 结果说明两个 worktree 的工具链与运行环境可比，
因此其余 workload 的差异可以归因到补丁。

### 2.1 OpenBox pair 后端探针

OpenBox 的 `pair` 只在 GUI 里被调用，仓库没有现成的 headless 入口。本轮在两侧
**使用同一份未提交探针源码**（附录 A）直接驱动 `tswn_openbox::backend::run_pair`，
在业务调用外层计时并把该路径产生的日志行按原顺序打印，再做哈希比较。
两侧的 `PairInput` / `CommonBenchOptions` 字段一致，探针可以逐字复用。

### 2.2 派生输入

为控制单轮时长，部分配置使用固定输入的确定性子集（hash 见随附 JSON，两侧字节相同）：

| 文件 | 派生方式 |
| --- | --- |
| `docs/perf/cqp/sqp6000_first8.txt` | `sqp6000_first20.txt` 前 8 行 |
| `docs/perf/cqp/sqp6000_first2.txt` | `sqp6000_first20.txt` 前 2 行 |
| `crates/tswn_openbox/assets/targets/target1_first1.txt` | `target1.txt` 第 1 行 |
| `crates/tswn_openbox/assets/teammates/teammate_fz4.txt` | `teammate_fz.txt` 前 4 行 |
| `crates/tswn_openbox/assets/teammates/teammate_fz4.toml` | `teammate_fz.toml` 前 4 个 `[[targets]]` 块 |

## 3. 结果：pair 路径

### 3.1 OpenBox pair 后端（`run_pair`）

小网格：8 选手 × 4 队友 × 41 靶子 = 1312 matchup，`count=100`（131.2k 场）：

| 线程 | baseline 墙钟 (s) | candidate 墙钟 (s) | 加速比 | 正确性 |
| --- | ---: | ---: | ---: | --- |
| `-t 1` | 4.264083 | 4.233226 | 1.01× | 一致 |
| `-t 2` | 2.834028 | 2.219727 | 1.28× | 一致 |
| `-t 4` | 1.970250 | 1.235187 | 1.59× | 一致 |
| 自动 | 2.653761 | 0.546161 | **4.86×** | 一致 |

全网格：20 选手 × 16 队友 × 41 靶子 = 13120 matchup：

| 精度档 | baseline 墙钟 (s) | candidate 墙钟 (s) | 加速比 | 正确性 |
| --- | ---: | ---: | ---: | --- |
| `count=100`（1.31M 场） | 24.134032 | 4.741989 | **5.09×** | 一致 |
| `count=1000`（13.1M 场） | 60.320419 | 42.049740 | 1.44× | 一致 |

带权组合（小网格，自动线程）：

| 配置 | baseline 墙钟 (s) | candidate 墙钟 (s) | 加速比 | 正确性 |
| --- | ---: | ---: | ---: | --- |
| 带权靶子（75 个 factored 靶子） | 0.921218 | 0.107690 | **8.55×** | 一致 |
| 带权队友（4 个 factored 队友） | 2.080324 | 0.374606 | **5.55×** | 一致 |

`count=100` 档的加速来自两侧都只有一次外层任务切分：baseline 按“选手 → 队友”串行 320 次，
每次只并行 41 个 matchup；candidate 把 320×41 个 matchup 放进窗口化矩阵统一调度，
自动线程下 16 个逻辑核基本吃满。

`count=1000` 档加速回落到 1.44×，是固定开销摊薄后的正常结果：`count=100` 时 baseline 要为
320 个外层作业各自重建 41 个 matchup 的 roster，固定开销占主导；`count=1000` 后战斗本身占主导，
而 baseline 的单作业内层本来就有 41 路 matchup 并行，所以差距收窄。两条曲线都没有退化点。

### 3.2 CLI `bench pair`

同样的 1312 matchup 小网格，`count=100`，输出 JSONL 后剔除计时字段做哈希：

| 线程 | baseline 墙钟 (s) | candidate 墙钟 (s) | 加速比 | 正确性 |
| --- | ---: | ---: | ---: | --- |
| `-t 1` | 3.414280 | 3.378539 | 1.01× | 一致 |
| `-t 2` | 2.487591 | 1.853314 | 1.34× | 一致 |
| `-t 4` | 1.670372 | 1.031879 | 1.62× | 一致 |
| 自动 | 2.085838 | 0.522616 | **3.99×** | 一致 |
| 自动（20×16×41 全网格） | 20.894420 | 4.613927 | **4.53×** | 一致 |

## 4. 结果：其余路径

### 4.1 fixed30（回归门禁，390k 场）

| 线程 | baseline (s) | candidate (s) | 变化 | 内部计时 |
| --- | ---: | ---: | ---: | --- |
| `--threads 1` | 11.594644 | 11.581970 | -0.11% | 11.562023 → 11.553467 |
| `--threads 0`（自动） | 1.561401 | 1.556185 | -0.33% | 1.519839 → 1.515075 |

结论：单线程与自动线程都没有可测回退，说明战斗热路径、`PreparedRunner` 复位与纯单场
计时路径不受本次改动影响。`wins/total/errors/guard_exhausted` 两侧逐 case 相同。

### 4.2 OpenBox 共享 CQP/CQD 矩阵（自动线程）

单人 20 × target1（35 组，700 matchup），关闭周期性 RSS 上报：

| 精度档 | baseline 内部 (s) | candidate 内部 (s) | 变化 | 墙钟 旧 → 新 (s) |
| --- | ---: | ---: | ---: | --- |
| 1%（100 场） | 0.105831 | 0.098438 | -6.98% | 0.623587 → 0.600142 |
| 10%（1000 场） | 0.858643 | 0.907030 | +5.63% | 1.334261 → 1.390754 |
| 100%（10000 场） | 8.414051 | 8.515004 | +1.20% | 8.820802 → 8.924196 |

双人 32 × target2（41 组，1312 matchup），关闭周期性 RSS 上报：

| 精度档 | baseline 内部 (s) | candidate 内部 (s) | 变化 | 墙钟 旧 → 新 (s) |
| --- | ---: | ---: | ---: | --- |
| 1%（100 场） | 0.410521 | 0.423539 | +3.17% | 0.817296 → 0.831632 |
| 10%（1000 场） | 3.917640 | 3.965888 | +1.23% | 4.334035 → 4.370344 |
| 100%（10000 场） | 38.781067 | 38.619037 | -0.42% | 39.206453 → 39.059812 |

结论：共享矩阵层没有方向性退化。100% 精度档两侧样本都很紧（单人 8.414/8.404/8.421 对
8.515/8.552/8.432），单人档 +1.2% 已接近本机同配置重复运行的波动上限；1%~10% 精度档绝对时间短
（0.1 s~4 s），单次样本离散度与效应量同阶，只能判定为持平。原先“100% 档退化 13%”的结论来自
带周期性 RSS 上报的第一批样本，已在第 5 节证伪。

### 4.3 少 matchup × 长轮次（`count=10000`，自动线程）

| 矩阵 | baseline 内部 (s) | candidate 内部 (s) | 加速比 |
| --- | ---: | ---: | ---: |
| 2 matchup（2 选手 × 1 靶子） | 0.110519 | 0.029179 | **3.79×** |
| 8 matchup（8 选手 × 1 靶子） | 0.326466 | 0.135078 | **2.42×** |

这正是本次修复想要解决的“少量长任务被 matchup 数限制并行度”：baseline 只能用
`min(worker, matchup 数)` 个 worker，2/8 个 matchup 时分别只有 2/8 个 worker 在跑；
candidate 把轮次区间拆开喂满线程池，于是 2 个 matchup 也能吃满 16 个逻辑核。

### 4.4 正确性对账

- 25 个配置全部两侧哈希一致（含全网格 pair、带权 pair、CQP/CQD 全档）；
- `tswn-cli bench batch-rate -l target1 -p sqp6000_first20 -n 100 --log -o out.jsonl`
  单人输出哈希 `FDAC7C75…`、双人输出哈希 `D9BA616E…`，两侧一致；
- 侧信道：CLI JSONL 的 `wins/total/valid_matchups/skipped_matchups/avg_win_rate/aggregate_win_rate`
  与 pair 日志行逐字节相同，说明权重、镜像 50%、重复名跳过和 Top-K 汇总没有因并行完成顺序改变。
- `cargo test -p tswn_core -p tswn_openbox`：592 通过 / 0 失败
  （core lib 513 + 2 ignored、CLI 46、`cli_battle` 集成 3、openbox lib 24、openbox bin 5、doctest 1）。

## 5. 仪表伪影：`openbox_mem_probe` 的周期性 RSS 上报

第一批 CQP/CQD 样本（`openbox_mem_probe` 默认 `--report-ms 2000`）给出的结论是
“candidate 在 10000 场档慢 13%”，且 6 个样本两组互不重叠。该结论**不成立**，原因是仪表本身：

- `openbox_mem_probe` 的进度回调里每 2 秒执行一次 `report()`，而 Windows 上的
  `current_rss_kb()` 会 spawn 一个 powershell 去查 `WorkingSet64`，单次约 0.3 s；
- 这个停顿发生在**被计时的区间内**，并且发生在主线程上；
- candidate 的 `runtime_cqp_matchups` 用容量为 `workers * 2` 的有界完成事件队列，
  主线程停顿期间 worker 会因为队列写满而阻塞；baseline 用的是无界 `mpsc`，worker 不受影响。

于是同一段停顿在两侧的代价不对称：对 9 s 左右的一轮，4 次上报就能让 candidate 少掉约 1 s 的
worker 时间，正好是观察到的 +13%。

关闭周期性上报（`--report-ms 3600000`）后，同一组配置、同一对二进制给出：

| workload | 默认上报（作废） | 关闭上报（本报告采用） |
| --- | ---: | ---: |
| cqp-single-10000 内部计时 | 9.189202 → 10.436557（+13.6%） | 8.414051 → 8.515004（+1.2%） |
| cqd-double-10000 内部计时 | 42.370917 → 48.164371（+13.7%） | 38.781067 → 38.619037（-0.4%） |

教训（对后续复测同样适用）：`openbox_mem_probe` 的 `t=` 包含回调里的上报开销，做调度类 A/B 时
必须显式加大 `--report-ms`；也说明有界事件队列会把调用方的停顿反向传播成 worker 阻塞，
在 GUI 场景下这是设计取舍，但在纯 CLI 批量场景下值得复核队列容量。

## 6. `RangePlan` 分片公式的敏感性实验（未提交）

第 5 节排除了退化之后，`RangePlan` 的分片公式仍值得记录：`run_range_jobs` 里每个 worker 只缓存
**当前** matchup 的准备结果，因此同一个 matchup 被切给多个 worker 时，`PreparedRuntimeRunner`
会被重复构建。

```rust
// crates/tswn_core/src/runtime/cqp.rs（候选实现）
let parts = requested
    .saturating_mul(4)
    .div_ceil(matchups)
    .min(n.div_ceil(64).max(1))
    .max(n.div_ceil(256))
    .max(1)
    .min(usize::MAX / matchups);
```

`count=10000` 时 700 / 1312 个 matchup 的矩阵会得到 `parts = 40`，即每个 matchup 最多被
约 32 个 worker 各准备一次。为了量化这部分开销，本机在候选树上做了一次**未提交**的对照实验
（只改 `parts`，让它只在 matchup 数喂不满 worker 时才分片）：

```rust
let parts = requested.div_ceil(matchups).clamp(1, 8).min(usize::MAX / matchups);
```

两次会话各自内部交替 A/B（绝对值跨会话不可比，只看会话内比值）：

| workload | 候选（分片公式原样） | 候选（粗分片实验） |
| --- | ---: | ---: |
| cqp-single-10000 内部计时 | 8.414051 → 8.515004（+1.2%） | 8.435208 → 8.447152（±0%） |
| cqd-double-10000 内部计时 | 38.781067 → 38.619037（-0.4%） | 38.564939 → 38.715257（-0.4%） |
| cqp-few-long-2 内部加速比 | **3.79×** | 3.49× |
| cqp-few-long-8 内部加速比 | **2.42×** | 2.08× |
| pair-probe-full-auto-1000 墙钟加速比 | **1.44×** | 1.36× |

结论：在本机可测范围内，重复准备 roster 的开销不足以抵消细分片带来的负载均衡收益——
粗分片既没有换到可测的吞吐提升，还让“少 matchup 长轮次”和 pair 全网格的加速比各掉
6%~14%。因此**不建议**改这个公式；本报告把它记为一次证伪实验，不包含任何代码改动。

如果将来 `parts` 需要继续变大（更长轮次、更多 worker、或更长尾的矩阵），重复准备会线性放大，
届时更合适的方向是给 worker 加一个按 matchup 索引的持久准备缓存（代价是内存），
而不是简单减小 `parts`。

## 7. 复测命令

```powershell
# 两个 worktree 各自构建
cargo build --release -p tswn_openbox --bins
cargo build --release -p tswn_core --bin tswn-cli --example perf_runtime --features no_debug

# fixed30
.\target\release\examples\perf_runtime.exe --input docs\perf\fixed_cases_30 --runs 13000 --threads 1
.\target\release\examples\perf_runtime.exe --input docs\perf\fixed_cases_30 --runs 13000 --threads 0

# CQP 单人 20 × target1（count 依次 100 / 1000 / 10000；--report-ms 关掉周期性 RSS 上报）
.\target\release\openbox_mem_probe.exe --players .\docs\perf\cqp\sqp6000_first20.txt `
  --targets .\crates\tswn_openbox\assets\targets\target1.txt --limit all --target-limit all `
  --count 100 --threads 0 --report-ms 3600000

# CQD 双人 32 × target2（count 依次 100 / 1000 / 10000）
.\target\release\openbox_mem_probe.exe --players .\cqp_double_target.txt `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt --limit all --target-limit all `
  --count 100 --threads 0 --report-ms 3600000

# 少 matchup × 长轮次：--players 换成 sqp6000_first8/first2、--targets 换成 target1_first1.txt
.\target\release\openbox_mem_probe.exe --players .\docs\perf\cqp\sqp6000_first2.txt `
  --targets .\crates\tswn_openbox\assets\targets\target1_first1.txt --limit all --target-limit all `
  --count 10000 --threads 0 --report-ms 3600000

# OpenBox pair 后端（探针源码见附录 A；-t 依次 1 / 2 / 4 / auto）
.\target\release\openbox_pair_probe.exe --players .\docs\perf\cqp\sqp6000_first8.txt `
  --teammates .\crates\tswn_openbox\assets\teammates\teammate_fz4.txt `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt --count 100 --threads auto --head 5

# CLI pair
.\target\release\tswn-cli.exe bench pair `
  -l .\crates\tswn_openbox\assets\targets\target2.txt `
  -p .\docs\perf\cqp\sqp6000_first8.txt `
  --teammate-list .\crates\tswn_openbox\assets\teammates\teammate_fz4.txt `
  --head 5 -n 100 -f -o .\target\cli-pair.jsonl --log

# 输出对账（单人 / 双人各一次，比较剔除计时字段后的 JSONL）
.\target\release\tswn-cli.exe bench batch-rate `
  -l .\crates\tswn_openbox\assets\targets\target1.txt `
  -p .\docs\perf\cqp\sqp6000_first20.txt -n 100 -f -o .\target\check.jsonl --log
```

A/B 驱动脚本（本机临时工具，未提交）：预热 1 轮 + 3 轮交替，奇偶轮换顺序，取中位数，
原始样本与汇总写入随附 JSON。

## 8. 结论

- `fixed30` 单线程/自动线程都在 ±0.35% 内，纯战斗路径无回退；
- OpenBox pair 后端自动线程加速 4.86×（小网格）/5.09×（全网格 `count=100`），
  线程数越大收益越明显，Windows 上原本被串行外层浪费的核心被吃满；
- CLI pair 自动线程加速 3.99×（小网格）/4.53×（全网格）；
- 带权靶子/带权队友分别加速 8.55×/5.55×，输出与 baseline 逐字节一致；
- 少 matchup × 长轮次从 0.111 s / 0.326 s 降到 0.029 s / 0.135 s（内部计时，3.79× / 2.42×）；
- 共享 CQP/CQD 矩阵没有方向性退化：100% 精度档 -0.42%（双人）~ +1.20%（单人），
  10% 档 +1.2%~+5.6%（绝对时间短、离散度同阶），1% 档两侧互有快慢；
- 正确性：25 个配置的新旧输出哈希全部一致，没有发现语义变化；
- 本轮唯一“发现的问题”是复测仪表本身（`openbox_mem_probe` 周期性 RSS 上报 + 候选侧有界
  完成队列的相互作用，见第 5 节），已通过复测参数避免；`RangePlan` 分片公式经对照实验
  确认利大于弊，无需改动（见第 6 节）。

## 附录 A：OpenBox pair 探针源码（本机临时工具，未提交）

放在 `crates/tswn_openbox/src/bin/openbox_pair_probe.rs`，两侧使用同一份源码：

```rust
//! OpenBox pair 后端 A/B 探针（本地复测临时工具，不随仓库提交）。
//!
//! 直接驱动 `tswn_openbox::backend::run_pair`，在业务调用外层记录整批墙钟，
//! 并把该路径产生的日志行按原顺序打印到 stdout，便于新旧输出逐字节比较。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tswn_openbox::backend::{CommonBenchOptions, OutputMode, PairDetailMode, PairInput, ProgressEvent, run_pair};

struct Args {
    players: PathBuf,
    teammates: PathBuf,
    targets: PathBuf,
    count: usize,
    threads: Option<usize>,
    head: usize,
    detail_mode: PairDetailMode,
    teammate_factored: bool,
    target_factored: bool,
}

impl Args {
    fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut parsed = Self {
            players: PathBuf::from("docs/perf/cqp/sqp6000_first20.txt"),
            teammates: PathBuf::from("crates/tswn_openbox/assets/teammates/teammate_fz.txt"),
            targets: PathBuf::from("crates/tswn_openbox/assets/targets/target2.txt"),
            count: 100,
            threads: Some(1),
            head: 5,
            detail_mode: PairDetailMode::Top,
            teammate_factored: false,
            target_factored: false,
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--players" => parsed.players = PathBuf::from(args.next().expect("--players needs a path")),
                "--teammates" => parsed.teammates = PathBuf::from(args.next().expect("--teammates needs a path")),
                "--targets" => parsed.targets = PathBuf::from(args.next().expect("--targets needs a path")),
                "--count" => parsed.count = args.next().expect("--count needs a value").parse().expect("invalid --count"),
                "--threads" => {
                    let raw = args.next().expect("--threads needs a value");
                    parsed.threads = match raw.as_str() {
                        "auto" | "0" => None,
                        _ => Some(raw.parse().expect("invalid --threads")),
                    };
                }
                "--head" => parsed.head = args.next().expect("--head needs a value").parse().expect("invalid --head"),
                "--detail" => {
                    parsed.detail_mode = match args.next().expect("--detail needs a value").as_str() {
                        "none" => PairDetailMode::None,
                        "top" => PairDetailMode::Top,
                        "every" => PairDetailMode::Every,
                        other => panic!("invalid --detail: {other}"),
                    }
                }
                "--teammate-factored" => parsed.teammate_factored = true,
                "--target-factored" => parsed.target_factored = true,
                other => panic!("unknown arg: {other}"),
            }
        }
        parsed
    }
}

fn read_all(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read failed {}: {err}", path.display()))
}

fn main() {
    let args = Args::parse();
    let cancel = Arc::new(AtomicBool::new(false));
    let input = PairInput {
        target_text: read_all(&args.targets),
        target_factor_enabled: args.target_factored,
        player_text: read_all(&args.players),
        player_double_plus: false,
        teammate_text: read_all(&args.teammates),
        teammate_double_plus: false,
        teammate_factor_enabled: args.teammate_factored,
        head: args.head,
        detail_mode: args.detail_mode,
        detail_min: None,
        highlight_delta: None,
        output_mode: OutputMode::Log,
        output_file: None,
        options: CommonBenchOptions {
            count: args.count,
            threads: args.threads,
            keep_rq: true,
            verbose: false,
            min_screen: None,
            min_file: None,
            wr_precision: 3,
        },
        cancel: Arc::clone(&cancel),
    };

    let lines = RefCell::new(Vec::<String>::new());
    let ticks = Cell::new(0usize);
    let last = Cell::new(0usize);
    let done = RefCell::new(None::<Result<String, String>>);
    let start = Instant::now();
    run_pair(input, |event| match event {
        ProgressEvent::Log(line) | ProgressEvent::HighlightLog(line) | ProgressEvent::SkillBoardLog(line) => {
            lines.borrow_mut().push(line);
        }
        ProgressEvent::Progress { done, .. } => {
            ticks.set(ticks.get() + 1);
            last.set(done);
        }
        ProgressEvent::Done(result) => *done.borrow_mut() = Some(result),
    });
    let elapsed = start.elapsed().as_secs_f64();

    let lines = lines.into_inner();
    for line in &lines {
        println!("{line}");
    }
    let status = match done.into_inner() {
        Some(Ok(message)) => message,
        Some(Err(err)) => {
            eprintln!("probe done=Err err={err}");
            std::process::exit(2);
        }
        None => {
            eprintln!("probe done=None");
            std::process::exit(3);
        }
    };
    eprintln!(
        "probe elapsed_s={elapsed:.6} lines={} progress_ticks={} progress_last={} done={status}",
        lines.len(),
        ticks.get(),
        last.get()
    );
}
```

探针把该路径的全部日志行按原顺序写到 stdout，两侧输出逐字节比较（第 4.4 节）。
