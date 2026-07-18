# Runtime 0.4.3 `9d3a3b9` 发版基准

> 日期：2026-07-18
>
> 被测代码提交：`9d3a3b919e3df7fb857ec529869632d71766b36c`
>
> 范围：本机 Rust Runtime、默认 mimalloc；legacy 只作为 score 同轮对照，不运行独立 legacy Runtime、Node.js 或 Bun 完整性能套件
>
> 性质：0.4.3 正式多轮中位数发版存档；不重置 0.4.0 确立的半时硬目标

机器可读结果见 [`runtime_0.4.3_9d3a3b9_release_benchmark.json`](runtime_0.4.3_9d3a3b9_release_benchmark.json)。fixed30 与 score 的工具原始 JSON 位于本机 `target/full_bench_0.4.3_9d3a3b9/`，该目录不纳入 Git；OpenBox 与 win-rate 的逐轮值已完整写入机器可读结果。

## 1. 环境与测量口径

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器；
- 系统：Windows 10 专业版 22H2，build 19045，`x86_64-pc-windows-msvc`；
- Rust：`rustc 1.99.0-nightly (3d50c25bc 2026-07-16)`，LLVM 22.1.8；
- Cargo：`cargo 1.99.0-nightly (59800466c 2026-07-07)`；
- 构建：workspace `release` profile，fat LTO、`codegen-units = 1`、`no_debug`、默认 mimalloc；当前配置未使用已被 rustc 移除的 `mutable-noalias`；
- 每类负载先单独预热，正式轮次串行执行，期间不并发运行其他 benchmark；
- fixed30：每个 case 13000 场；单线程 `--thread 1` 独立运行 3 次，自动线程 `--thread 0` 独立运行 5 次，各字段取中位数；
- win-rate：`left@red` 对 `right@blue`，单线程 13000 场，独立运行 5 次；CLI 当前按毫秒输出 wall/init/fight，因此该项中位数精度为 1 ms；
- score：单线程 core batch 裸时间，排除构建、进程启动、输入读取和报告序列化；每个输入运行 5 次，按 `runtime/legacy/runtime/legacy/runtime` 交替首个 runtime，各字段取中位数；
- CQP/CQD：OpenBox 业务入口外层墙钟，包含自动调度、结果回调与既有内存探针；每档运行 5 次，单双人拓扑与场数顺序交替，各档取中位数。

输入按 LF 归一化后的 SHA-256：

| 输入 | 非空行 | SHA-256 |
| --- | ---: | --- |
| `docs/perf/score/mario.txt` | 1 | `9E93C72EEA75345FE27E32AEB9EF86E28B546C4F1A125CA6EFC794D2F7C84246` |
| `docs/perf/cqp/sqp6000_first20.txt` | 20 | `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6` |
| `cqp_double_target.txt` | 32 | `EF42B91EFD02EC33EB8029329C95775A571CE1AA285D54B0A3A4D8D218D8020D` |
| `target1.txt` | 35 | `227C69B2C8C5680B594D26525A1D4CC6383C2615B48AC410F679B06921B7210B` |
| `target2.txt` | 41 | `A285A267AE4438BC254FE0511F9FDD1C39A458532CF872984FFB1C035E81569D` |

## 2. 总体结论

| 指标 | 0.4.3 中位数 | 0.4.2 mimalloc 快照 | 相对变化 | 既定目标 / 状态 |
| --- | ---: | ---: | ---: | --- |
| fixed30 单线程 overall | 28.257 µs/场 | 31.307 | 快 9.74% | ≤ 23.364；还需压缩 17.32% |
| fixed30 自动线程 overall | 3.632 µs/场 | 3.895 | 快 6.76% | ≤ 2.936；还需压缩 19.16% |
| stress_multi 单线程 | 58.209 µs/场 | 65.044 | 快 10.51% | ≤ 48.807；还需压缩 16.15% |
| win-rate 单线程 13000 场 | 0.098 s | 0.107 | 快约 8.41% | ≤ 0.092；还需压缩约 6.12% |
| score mario | 0.428 s | 0.519 | 快 17.59% | legacy/runtime = 1.939x，达标 |
| score CQP 单人 | 0.672 s | 0.765 | 快 12.11% | legacy/runtime = 1.936x，达标 |
| score CQP 双人 | 0.944 s | 1.044 | 快 9.61% | legacy/runtime = 2.307x，达标 |
| CQP 单人 100% | 8.627 s | 10.404 | 快 17.08% | ≤ 7.516；还需压缩 12.88% |
| CQD 双人 100% | 39.575 s | 44.255 | 快 10.57% | ≤ 33.499；还需压缩 15.35% |

0.4.3 的正式中位数在全部主要指标上都优于已保存的 0.4.2 mimalloc 单次快照。score 三类输入均明显超过“Runtime 吞吐至少为同轮 legacy 的 1.5 倍”硬线，五轮逐组对账全部为零差异。其余旧 runtime 半时线仍未完成，最接近的是普通 win-rate，按 CLI 毫秒精度还需约 6.12%；这些硬线继续保留到后续版本。

0.4.2 参考值来自 `runtime_0.4.2_1ed1258_allocator_snapshot` 的单次采样，而本页为多轮中位数；“相对变化”用于版本方向判断，不冒充同轮交替 A/B。0.4.3 每个局部优化的严格 A/B 仍以更新日志中对应提交的交替轮次为准。

## 3. fixed30

### 3.1 单线程

| 分组 | wall | init | fight | 吞吐 | 胜场 | 三轮 wall 范围 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| overall | 28.257226 µs/场 | 2.794985 | 25.318136 | 35389.18 场/s | 150858 | 27.837867～28.268518 |
| core 1v1/2v2 | 13.837594 | 1.958578 | 11.750132 | 72266.90 | 112305 | 13.664164～14.042631 |
| 1v1 | 9.054765 | 1.578825 | 7.354471 | 110439.09 | 70035 | 8.796506～9.078883 |
| 2v2 | 22.588204 | 2.667981 | 19.781213 | 44270.89 | 42270 | 22.561899～23.187053 |
| stress_multi | 58.208676 | 4.546714 | 53.485319 | 17179.57 | 23119 | 57.832981～58.593309 |

五个分组相对 0.4.2 快照分别缩短 9.74%、9.45%、6.92%、11.30% 和 10.51%。三轮所有赢家聚合完全一致。

### 3.2 自动线程

自动线程的 init/fight 是 worker 累计 CPU 时间，不能与 wall 相加；吞吐判断只看外层 wall。

| 分组 | wall | worker init | worker fight | 吞吐 | 胜场 | 五轮 wall 范围 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| overall | 3.631740 µs/场 | 4.484947 | 51.809578 | 275350.11 场/s | 150858 | 3.448147～3.791629 |
| core 1v1/2v2 | 1.986392 | 3.146148 | 23.681929 | 503425.34 | 112305 | 1.688476～2.047938 |
| 1v1 | 1.321862 | 2.429817 | 14.442720 | 756508.49 | 70035 | 1.189252～1.442092 |
| 2v2 | 3.081473 | 4.274781 | 40.525396 | 324520.12 | 42270 | 2.603719～3.294810 |
| stress_multi | 7.144792 | 7.971874 | 113.252350 | 139962.08 | 23119 | 6.820109～7.699842 |

五轮所有赢家聚合完全一致。自动线程 overall 相对 0.4.2 快照缩短 6.76%，距离 2.935760 µs/场半时线仍需压缩 19.16%。

## 4. 普通 win-rate

`left@red` 对 `right@blue`，单线程 13000 场：

| wall 中位数 | init | fight | 吞吐中位数 | 胜场 | 五轮 wall |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0.098 s | 0.015 s | 0.081 s | 132803 场/s | 7077/13000 | 0.097 / 0.100 / 0.098 / 0.100 / 0.097 s |

受 CLI 1 ms 输出精度限制，该项只报告到毫秒。五轮胜场完全一致；相对 0.4.2 快照约快 8.41%，距离 0.092 s 半时线约差 6.12%。

## 5. score 单线程裸时间

| 输入 | 总场数 | runtime wall | runtime init/fight | legacy wall | legacy init/fight | legacy/runtime | 相对 0.4.2 | 对账 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| mario | 13000 | 0.428136 s | 0.147312 / 0.277332 | 0.830057 s | 0.372725 / 0.438456 | 1.939x | 快 17.59% | 4171/13000，5 轮均 0 差异 |
| CQP 单人 20 组 | 20000 | 0.672131 s | 0.234221 / 0.429179 | 1.301449 s | 0.581869 / 0.688663 | 1.936x | 快 12.11% | 14513/20000，5 轮均 0 差异 |
| CQP 双人 32 组 | 32000 | 0.944080 s | 0.302716 / 0.628017 | 2.177672 s | 0.922634 / 1.208237 | 2.307x | 快 9.61% | 30295/32000，5 轮均 0 差异 |

三组 runtime wall 的五轮范围分别为 `0.419747～0.431080 s`、`0.658135～0.704039 s` 和 `0.936965～0.956452 s`。首个 runtime 交替后仍没有观察到结果或吞吐硬线回退。

## 6. CQP/CQD OpenBox 自动调度

| 输入 | 精度 | matchup 场数 | 五轮中位数 | 0.4.2 快照 | 相对变化 | 半时目标 | 还需压缩 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 单人 20 × target1 | 1% | 100 | 0.101573 s | 0.136911 | 快 25.81% | 0.086974 | 14.37% |
| 单人 20 × target1 | 10% | 1000 | 0.867426 s | 1.039850 | 快 16.58% | 0.765996 | 11.69% |
| 单人 20 × target1 | 100% | 10000 | 8.627235 s | 10.403798 | 快 17.08% | 7.515707 | 12.88% |
| 双人 32 × target2 | 1% | 100 | 0.414332 s | 0.511023 | 快 18.92% | 0.357061 | 13.82% |
| 双人 32 × target2 | 10% | 1000 | 4.095233 s | 4.602381 | 快 11.02% | 3.386775 | 17.30% |
| 双人 32 × target2 | 100% | 10000 | 39.575480 s | 44.254562 | 快 10.57% | 33.498864 | 15.35% |

30 次正式运行均完成 `700/700` 或 `1312/1312` matchup，没有任务失败。短档固有抖动较大，因此只用五轮中位数判断；完整逐轮 wall 与 RSS 见机器可读结果。

## 7. 正确性与发版门禁

被测代码提交 `9d3a3b9` 在 benchmark 前已通过：

- `cargo test -p tswn_core`：核心库 596 通过、2 忽略；CLI 59、runtime trace 3、engine 集成 29 均通过；
- `no_debug` 核心库：589 通过、2 忽略；其余 CLI、runtime trace 与 engine 集成门禁通过；
- release Runtime corpus：124/124；
- 固定 SBY 12000-case：TS/Rust 执行失败、空输出与 diff 均为 0；
- 本轮 fixed30 所有正式轮次赢家聚合一致，三组 score 共 15 次 legacy/runtime 完整对账均为 0 差异，OpenBox 30 次正式运行全部完成。

## 8. 复测命令

```powershell
cargo build -p tswn_core --release --features "no_debug aux_bins" `
  --bin track_perf_cases --bin track_score_perf --bin tswn-cli
cargo build -p tswn_openbox --release --bin openbox_mem_probe

# fixed30；单线程正式跑 3 次，自动线程把 --thread 改为 0 并跑 5 次
target\release\track_perf_cases.exe `
  --case-dir docs\perf\fixed_cases_30 `
  --out-dir target\full_bench_0.4.3_9d3a3b9\fixed_t1_r1 `
  --bench-runs 13000 --thread 1 --engine main -q

# 普通 win-rate，正式跑 5 次
target\release\tswn-cli.exe bench win-rate `
  -r "left@red`nright@blue" -n 13000 -s --perf

# score；三个输入各跑 5 次，并在相邻轮次切换 --first main/legacy
target\release\track_score_perf.exe `
  --input docs\perf\score\mario.txt `
  --label release-0.4.3-mario-r1 --count 13000 `
  --engine both --first main --mode normal `
  --out target\full_bench_0.4.3_9d3a3b9\score_mario_r1.json

# OpenBox；单双人、100/1000/10000 六档各跑 5 次
target\release\openbox_mem_probe.exe `
  --players docs\perf\cqp\sqp6000_first20.txt `
  --targets crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 100 --threads 0
```
