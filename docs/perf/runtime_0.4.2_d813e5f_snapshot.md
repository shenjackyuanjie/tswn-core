# Runtime 0.4.2 `d813e5f` 完整性能快照

> 日期：2026-07-15
>
> 被测提交：`d813e5f88dcccb7313dee5dc5acb7e4564935327`
>
> 性质：单次全套阶段采样；用于留档和决定后续优化方向，不替代正式 3/5 次中位数基线，也不重置既定半时硬线

机器可读汇总见 [`runtime_0.4.2_d813e5f_snapshot.json`](runtime_0.4.2_d813e5f_snapshot.json)。三组 score 的工具原始 JSON 与 fixed30 原始报告保存在本机 `target/full_bench_d813e5f/`，该目录不纳入 Git。

## 1. 环境与口径

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器，Windows x86_64-pc-windows-msvc；
- Rust：`rustc 1.99.0-nightly (f10db292a 2026-07-07)`，Cargo `1.99.0-nightly (2f0e7011e 2026-07-05)`；
- 构建：workspace release profile、`no_debug`，原生默认 mimalloc；
- Rust 别名模型：workspace 默认 `-Z mutable-noalias=yes`；
- fixed30：每 case 13000 场；分别运行一次单线程 `--thread 1` 和自动线程 `--thread 0`；
- win-rate：CLI 单线程 13000 场；
- score：单线程 core batch 裸时间，排除编译、进程启动、输入读取和报告序列化；mario 为 13000 场，CQP 单人/双人每组 1000 场；
- CQP/CQD：OpenBox 业务调用外层墙钟，自动线程，1%/10%/100% 分别为每 matchup 100/1000/10000 场；
- 本次只跑一轮，因此微小差异应视为阶段信号，不能替代正式多轮中位数结论。

输入按 LF 归一化后的 SHA-256 与既有基线一致：

| 输入 | SHA-256 |
| --- | --- |
| `docs/perf/cqp/sqp6000_first20.txt` | `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6` |
| `cqp_double_target.txt` | `EF42B91EFD02EC33EB8029329C95775A571CE1AA285D54B0A3A4D8D218D8020D` |
| `target1.txt` | `227C69B2C8C5680B594D26525A1D4CC6383C2615B48AC410F679B06921B7210B` |
| `target2.txt` | `A285A267AE4438BC254FE0511F9FDD1C39A458532CF872984FFB1C035E81569D` |

## 2. 总体结论

| 指标 | 本次值 | 既定目标 | 距目标还需压缩 | 状态 |
| --- | ---: | ---: | ---: | --- |
| fixed30 单线程 overall | 32.105 µs/场 | ≤ 23.364 µs/场 | 27.22% | 未达半时线 |
| fixed30 自动线程 overall | 4.644 µs/场 | ≤ 2.936 µs/场 | 36.78% | 未达半时线 |
| stress_multi 单线程 | 66.340 µs/场 | ≤ 48.807 µs/场 | 26.43% | 未达半时线 |
| win-rate 单线程 13000 场 | 0.117 s | ≤ 0.092 s | 21.37% | 未达半时线 |
| score mario | 0.469 / 0.814 s（runtime/legacy） | legacy 吞吐的 ≥ 1.5 倍 | 已达 1.734 倍 | 达标，0 差异 |
| score CQP 单人 | 0.708 / 1.285 s | legacy 吞吐的 ≥ 1.5 倍 | 已达 1.814 倍 | 达标，0 差异 |
| score CQP 双人 | 1.034 / 2.087 s | legacy 吞吐的 ≥ 1.5 倍 | 已达 2.018 倍 | 达标，0 差异 |
| CQP 单人 1%/10%/100% | 0.123 / 1.119 / 10.956 s | 旧 runtime 半时线 | 还需 29.20%～31.56% | 三档未达 |
| CQD 双人 1%/10%/100% | 0.530 / 5.006 / 47.945 s | 旧 runtime 半时线 | 还需 30.13%～32.64% | 三档未达 |

总体上，score 三类输入已经稳定超过 legacy 1.5 倍吞吐硬线；其余指标相对 0.4.0 或旧 runtime 基线均继续前进，但“在已保存速度上再快 50%”仍未完成。最远的是 fixed30 自动线程，当前还需压缩 36.78%；单线程共同战斗路径约还需压缩 26%～28%。

## 3. fixed30

### 3.1 单线程

| 分组 | wall | init | fight | 吞吐 | 胜场 | 半时目标 | 还需压缩 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| overall | 32.104545 µs/场 | 2.875688 | 29.090406 | 31148.24 场/s | 150858 | 23.364270 | 27.22% |
| core_1v1_2v2 | 15.883761 µs/场 | 2.001405 | 13.758437 | 62957.38 场/s | 112305 | 11.543320 | 27.33% |
| one_v_one | 10.363990 µs/场 | 1.627855 | 8.620210 | 96487.94 场/s | 70035 | 7.552380 | 27.13% |
| two_v_two | 26.003341 µs/场 | 2.686247 | 23.178521 | 38456.60 场/s | 42270 | 18.840930 | 27.54% |
| stress_multi | 66.340118 µs/场 | 4.757907 | 61.411286 | 15073.84 场/s | 23119 | 48.806870 | 26.43% |

### 3.2 自动线程

自动线程的 init/fight 是 worker 累计 CPU 时间，不能相加得到墙钟；判断吞吐只看 wall。

| 分组 | wall | 累计 init | 累计 fight | 吞吐 | 胜场 |
| --- | ---: | ---: | ---: | ---: | ---: |
| overall | 4.643520 µs/场 | 5.014779 | 61.911469 | 215353.87 场/s | 150858 |
| core_1v1_2v2 | 2.183333 µs/场 | 3.223226 | 27.717166 | 458015.33 场/s | 112305 |
| one_v_one | 1.423277 µs/场 | 2.503509 | 16.333845 | 702603.96 场/s | 70035 |
| two_v_two | 3.576769 µs/场 | 4.542708 | 48.586588 | 279581.92 场/s | 42270 |
| stress_multi | 9.903467 µs/场 | 8.784559 | 135.076859 | 100974.74 场/s | 23119 |

## 4. win-rate

`left@red` 对 `right@blue`，单线程 13000 场：

| wall | init | fight | 胜场 | 吞吐 | 半时目标 | 还需压缩 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0.117 s | 0.015 s | 0.100 s | 7077/13000 | 110928 场/s | ≤ 0.092 s | 21.37% |

## 5. score 单线程裸时间

| 输入 | 总场数 | runtime wall | runtime init | runtime fight | legacy wall | legacy init | legacy fight | legacy/runtime 吞吐 | runtime wall 缩短 | 对账 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| mario | 13000 | 0.469200 s | 0.131109 s | 0.334786 s | 0.813613 s | 0.361093 s | 0.434652 s | 1.734x | 42.33% | 4171/13000，0 差异 |
| CQP 单人 20 组 | 20000 | 0.708266 s | 0.201917 s | 0.497350 s | 1.284716 s | 0.569854 s | 0.686899 s | 1.814x | 44.87% | 14513/20000，0 差异 |
| CQP 双人 32 组 | 32000 | 1.034110 s | 0.275535 s | 0.744642 s | 2.086516 s | 0.876001 s | 1.167389 s | 2.018x | 50.44% | 30295/32000，0 差异 |

三组均满足“Runtime 吞吐至少为同轮 legacy 的 1.5 倍”，且逐组结果差异为零。继续优化共同 runtime 时，仍需用这三组输入防止 score 回退。

## 6. CQP/CQD 自动调度

| 输入 | 精度 | matchup 场数 | 本次 wall | 旧 runtime | 相对旧 runtime | 旧 runtime 半时线 | 还需压缩 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 单人 20 × target1 | 1% | 100 | 0.122841 s | 0.173947 s | 快 29.38% | 0.086974 s | 29.20% |
| 单人 20 × target1 | 10% | 1000 | 1.119174 s | 1.531991 s | 快 26.95% | 0.765996 s | 31.56% |
| 单人 20 × target1 | 100% | 10000 | 10.955759 s | 15.031414 s | 快 27.11% | 7.515707 s | 31.40% |
| 双人 32 × target2 | 1% | 100 | 0.530053 s | 0.714121 s | 快 25.78% | 0.357061 s | 32.64% |
| 双人 32 × target2 | 10% | 1000 | 5.006091 s | 6.773550 s | 快 26.09% | 3.386775 s | 32.35% |
| 双人 32 × target2 | 100% | 10000 | 47.945151 s | 66.997727 s | 快 28.44% | 33.498864 s | 30.13% |

六档均确认最近的公共 Runtime 优化已经传递到 OpenBox 自动调度路径；当前主要差距仍是每场共同 init/fight 成本，而不是某个精度档独有的调度退化。

## 7. 本次命令

```powershell
# fixed30；自动线程把 --thread 改为 0
target\release\track_perf_cases.exe --case-dir docs\perf\fixed_cases_30 `
  --out-dir target\full_bench_d813e5f\fixed30_t1 --bench-runs 13000 --thread 1 -q

# score；其余输入替换为 sqp6000_first20.txt / cqp_double_target.txt
target\release\track_score_perf.exe --input docs\perf\score\mario.txt `
  --label full-d813e5f-mario --count 13000 --engine both --first main --mode normal `
  --out target\full_bench_d813e5f\score_mario.json

# CQP；双人替换 players/targets，各档把 count 改为 100、1000、10000
target\release\openbox_mem_probe.exe `
  --players docs\perf\cqp\sqp6000_first20.txt `
  --targets crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 100 --threads 0
```
