# Runtime 0.4.0 性能基线

> 日期：2026-07-14
>
> 被测代码：`0c5ac9c444d84abbf112fb057cd3480861fb3aba`
>
> 状态：fixed30 与 CQP/CQD 已达到既定目标；普通 score 正确性通过，但性能仍未达到 legacy 硬目标

这份记录把当前 Runtime 速度固定为后续回归基线，同时把普通 score 的同口径 legacy 速度固定为必须追赶的验收线。机器可读数据见 [`runtime_0.4.0_baseline.json`](../runtime_0.4.0_baseline.json)。

## 1. 环境与计时口径

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器，Windows x86_64-pc-windows-msvc；
- Rust：`rustc 1.99.0-nightly (f10db292a 2026-07-07)`，Cargo `1.99.0-nightly (2f0e7011e 2026-07-05)`；
- 构建：workspace release profile、`no_debug`，默认 feature 保持开启，因此原生使用 mimalloc；
- Rust 别名模型：workspace 默认 `-Z mutable-noalias=yes`；
- fixed30：每个 case 13000 场，单线程取 3 次独立运行中位数，自动线程取 5 次独立运行中位数；
- score：固定单线程，每个输入组 1000 场；仅 `mario` 为了兼容历史口径执行 13000 场。每组 runtime/legacy 共做 5 次独立运行，并交替先跑的 runtime；
- score 裸时间：从 core batch 调用外层计时，排除编译、进程启动、输入读取与报告序列化；
- win-rate：CLI 单线程 13000 场，5 次独立运行中位数；CLI 只输出到毫秒，因此此项保留毫秒精度；
- 自动线程报告中的 init/fight 是各 worker 的累计 CPU 时间，不能相加得到墙钟；自动线程的主指标只看 wall；
- 表中“提升”为 `1 - 当前值 / 参考值`。正数越大越快；score 当前慢于 legacy 时另列出必须继续压缩的比例。

## 2. 总览

| 指标 | 当前 Runtime | 参考值 | 相对变化 | 结论 |
| --- | ---: | ---: | ---: | --- |
| fixed30 单线程 overall | 46.729 µs/场 | 0.3.10：68.952 µs/场 | 快 32.23% | 达标，新回归基线 |
| fixed30 单线程 core 1v1/2v2 | 23.087 µs/场 | 0.3.10：35.354 µs/场 | 快 34.70% | 达标 |
| fixed30 单线程 1v1 | 15.105 µs/场 | 0.3.10：22.520 µs/场 | 快 32.93% | 达标 |
| fixed30 单线程 2v2 | 37.682 µs/场 | 0.3.10：58.883 µs/场 | 快 36.01% | 达标 |
| fixed30 单线程 stress_multi | 97.614 µs/场 | 0.3.10：141.170 µs/场 | 快 30.85% | 达标 |
| fixed30 自动线程 overall | 5.872 µs/场 | 上次 runtime：8.978 µs/场 | 快 34.60% | 新回归基线 |
| win-rate 单线程 13000 场 | 0.184 s | legacy：0.217784 s | 快 15.51% | runtime 已快于 legacy |
| score：mario，13000 场 | 2.325 s | legacy：0.819 s | 慢 183.88% | 未达标，需再降 64.77% |
| score：CQP 单人 20 组 × 1000 场 | 3.608 s | legacy：1.287 s | 慢 180.35% | 未达标，需再降 64.33% |
| score：CQP 双人 32 组 × 1000 场 | 5.613 s | legacy：2.166 s | 慢 159.14% | 未达标，需再降 61.41% |

## 3. fixed30 新基线

### 3.1 单线程

| 分组 | case 数 | wall | init | fight | 吞吐 | 0.3.10 wall | 提升 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| overall | 30 | 46.729 µs/场 | 7.256 µs/场 | 39.352 µs/场 | 21400.20 场/s | 68.952 µs/场 | 32.23% |
| core_1v1_2v2 | 17 | 23.087 µs/场 | 4.670 µs/场 | 18.292 µs/场 | 43315.11 场/s | 35.354 µs/场 | 34.70% |
| one_v_one | 11 | 15.105 µs/场 | 3.593 µs/场 | 11.398 µs/场 | 66204.33 场/s | 22.520 µs/场 | 32.93% |
| two_v_two | 6 | 37.682 µs/场 | 6.632 µs/场 | 30.908 µs/场 | 26537.97 场/s | 58.883 µs/场 | 36.01% |
| stress_multi | 8 | 97.614 µs/场 | 13.067 µs/场 | 84.470 µs/场 | 10244.46 场/s | 141.170 µs/场 | 30.85% |

三次 overall wall 原始样本为 `46.728540 / 46.682101 / 46.848307 µs/场`，三次赢家总数均为 `150853/390000`。

### 3.2 自动线程

| 分组 | wall 中位数 | 吞吐中位数 | 累计 init | 累计 fight |
| --- | ---: | ---: | ---: | ---: |
| overall | 5.872 µs/场 | 170313.57 场/s | 13.855 µs/场 | 86.866 µs/场 |
| core_1v1_2v2 | 2.967 µs/场 | 337075.89 场/s | 8.495 µs/场 | 37.497 µs/场 |
| one_v_one | 1.978 µs/场 | 505678.81 场/s | 6.039 µs/场 | 23.295 µs/场 |
| two_v_two | 4.777 µs/场 | 209357.19 场/s | 12.806 µs/场 | 63.907 µs/场 |
| stress_multi | 12.141 µs/场 | 82368.69 场/s | 25.718 µs/场 | 193.226 µs/场 |

overall 的五次 wall 原始样本为 `5.851348 / 5.863159 / 5.889122 / 5.885987 / 5.871523 µs/场`。init/fight 是 worker 累计值，只用于定位热点，不是墙钟拆分。

## 4. score 单线程裸时间与 legacy 硬目标

### 4.1 输入

- 兼容历史输入：[`score/mario.txt`](../score/mario.txt)，1 组，13000 场；
- CQP 单人：[`cqp/sqp6000_first20.txt`](../cqp/sqp6000_first20.txt)，取 sqp6000 前 20 个玩家，20 组，每组 1000 场；
- CQP 双人：仓库根目录 `cqp_double_target.txt`，32 组，每组两个玩家，每组 1000 场；
- 1000 场即 CQP 10% 档；全部固定 `thread=1`，测量整组输入连续评分的裸墙钟；
- 单人输入 SHA-256 为 `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6`，双人输入为 `EF42B91EFD02EC33EB8029329C95775A571CE1AA285D54B0A3A4D8D218D8020D`，mario 输入为 `9E93C72EEA75345FE27E32AEB9EF86E28B546C4F1A125CA6EFC794D2F7C84246`。

### 4.2 正式结果

| 输入 | 总场数 | runtime wall | runtime init | runtime fight | legacy wall 目标 | legacy init | legacy fight | runtime 相对 legacy | 还需压缩 | 结果对账 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| mario | 13000 | 2.325268 s | 1.884721 s | 0.436631 s | 0.819091 s | 0.371108 s | 0.430457 s | 慢 183.88% | 64.77% | 4171/13000，0 差异 |
| CQP 单人 20 组 | 20000 | 3.607791 s | 2.914200 s | 0.685937 s | 1.286895 s | 0.586615 s | 0.673293 s | 慢 180.35% | 64.33% | 14513/20000，0 差异 |
| CQP 双人 32 组 | 32000 | 5.612802 s | 4.565780 s | 1.033145 s | 2.165950 s | 0.934405 s | 1.184333 s | 慢 159.14% | 61.41% | 30295/32000，0 差异 |

五次 wall 原始样本：

| 输入 | Runtime | legacy |
| --- | --- | --- |
| mario | 2.331528 / 2.326205 / 2.316470 / 2.325268 / 2.316307 s | 0.818369 / 0.816872 / 0.822479 / 0.827905 / 0.819091 s |
| CQP 单人 | 3.604636 / 3.602382 / 3.647778 / 3.607791 / 3.666674 s | 1.279524 / 1.290375 / 1.286895 / 1.286418 / 1.292669 s |
| CQP 双人 | 5.613838 / 5.630758 / 5.610015 / 5.610190 / 5.612802 s | 2.141044 / 2.151737 / 2.166061 / 2.165950 / 2.175110 s |

score 的验收规则固定为：相同输入、相同场数、单线程且结果完全一致时，Runtime 的整批 wall 必须不高于同一工具测得的 legacy wall。当前 runtime 数值只作为回归基线，不代表性能任务完成。历史 CLI/probe 的 mario legacy `1.113 s` 属于不同计时边界，仅保留作历史参考，不能替代本轮更严格的裸时间目标 `0.819091 s`。

从分项可见，三组 score 的 fight 已接近 legacy，双人 fight 甚至快约 12.8%；主要差距集中在每轮动态 profile/roster 初始化。后续优化应优先减少完整冷身份复位、profile 派生数据重建和临时分配，同时继续保持逐组结果对账为 0 差异。

本轮 CQP 单人第 13 组还发现了“聚气触发时没有刷新待生效疾走倍率”的真实正确性问题，已由提交 `5067a97` 修复并增加精确回归。修复后上述三种输入各重复 5 次，所有 runtime/legacy 结果均一致。

## 5. win-rate 与 CQP/CQD 自动调度

win-rate 使用 `left@red` 对 `right@blue`，单线程 13000 场。Runtime 五次 wall 为 `0.183 / 0.184 / 0.185 / 0.183 / 0.184 s`，中位数 `0.184 s`；init/fight 中位数分别为 `0.040 / 0.142 s`，结果始终为 `7077/13000`。历史同口径 legacy 中位数为 wall `0.2177835 s`、init `0.046990 s`、fight `0.152587 s`，当前 runtime wall 快约 15.51%。

CQP/CQD 自动调度六档沿用已正式留档的同机结果，完整输入、目标号库、线程策略和逐轮口径见 [`cqp-runtime-baseline.md`](cqp-runtime-baseline.md)：

| 输入 | 精度 | legacy Runtime | Runtime | 提升 |
| --- | ---: | ---: | ---: | ---: |
| 单人 20 × target1 | 1% | 0.345417 s | 0.173947 s | 49.64% |
| 单人 20 × target1 | 10% | 3.145753 s | 1.531991 s | 51.30% |
| 单人 20 × target1 | 100% | 25.381207 s | 15.031414 s | 40.78% |
| 双人 32 × target2 | 1% | 1.287154 s | 0.714121 s | 44.52% |
| 双人 32 × target2 | 10% | 11.965849 s | 6.773550 s | 43.39% |
| 双人 32 × target2 | 100% | 111.456257 s | 66.997727 s | 39.89% |

## 6. 复测命令

```powershell
# fixed30 单线程；自动线程把 --thread 改为 0
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_perf_cases -- `
  --case-dir docs/perf/fixed_cases_30 `
  --out-dir target/perf_cases_runtime_0.4.0_t1 `
  --bench-runs 13000 `
  --thread 1 -q

# score：CQP 10% 单人；双人替换输入为 cqp_double_target.txt
cargo run -p tswn_core --release --features "no_debug aux_bins" --bin track_score_perf -- `
  --input docs/perf/cqp/sqp6000_first20.txt `
  --label cqp-single-runtime-0.4.0 `
  --count 1000 `
  --engine both `
  --first main `
  --mode normal `
  --out target/score_perf_cqp_single_runtime_0.4.0.json

# win-rate 单线程
cargo run -p tswn_core --release --features no_debug --bin tswn-cli -- `
  bench win-rate -r "left@red\nright@blue" -n 13000 -s --perf
```

正式复测 score 时应在不同进程中运行 5 次，并交替使用 `--first main` / `--first legacy`，避免固定先后顺序把热机或频率波动偏向某一方。
