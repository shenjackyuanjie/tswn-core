# Runtime 0.4.2 `1ed1258` 完整性能快照

> 日期：2026-07-16
>
> 被测提交：`1ed1258b26f9939398b0e8a6f6c988b70781448f`
>
> 范围：仅测试本机 Rust Runtime；分别使用 mimalloc 与 Windows 系统分配器，不运行 legacy Runtime、Node.js 或 Bun 性能测试
>
> 性质：每种 allocator 各一次完整阶段采样；用于修复后留档，不替代正式多轮中位数验收，也不重置既定半时硬线

机器可读结果见 [`runtime_0.4.2_1ed1258_allocator_snapshot.json`](../runtime_0.4.2_1ed1258_allocator_snapshot.json)。工具生成的原始 fixed30 JSON、score JSON 和 OpenBox 日志保存在本机 `target/full_bench_1ed1258_20260716/`，不纳入 Git。

## 1. 环境与口径

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器；
- 系统：Windows 10 专业版 22H2，build 19045；
- Rust：`rustc 1.99.0-nightly (da80ed070 2026-07-14)`；
- Cargo：`cargo 1.99.0-nightly (59800466c 2026-07-07)`；
- 构建：workspace release profile、`no_debug`；当前 rustc 已移除 `mutable-noalias`，本轮未使用该参数；
- mimalloc 组：保留 `tswn_core` / `tswn_openbox` 默认 `mimalloc_alloc` feature；
- 系统分配器组：core 使用 `--no-default-features --features "no_debug aux_bins png_render"`，OpenBox 使用 `--no-default-features`，两组产物位于独立 target 目录；
- 每套二进制在正式计时前分别预热 fixed、score、win-rate 和 OpenBox CQP；
- fixed30：每个 case 13000 场，分别运行单线程 `--thread 1` 和自动线程 `--thread 0`；
- win-rate：`left@red` 对 `right@blue`，单线程 13000 场；
- score：单线程 core batch 裸时间；mario 13000 场，CQP 单人/双人每组 1000 场；
- CQP/CQD：OpenBox 业务入口外层墙钟，自动线程；1%/10%/100% 分别为每 matchup 100/1000/10000 场；
- 表中“mimalloc 节省”按 `1 - mimalloc / 系统分配器` 计算；
- 与 `d813e5f` 的变化只比较已保存的 Runtime 数据；正数表示当前更快。

输入按 LF 归一化后的 SHA-256 与既有基线一致：

| 输入 | SHA-256 |
| --- | --- |
| `docs/perf/score/mario.txt` | `9E93C72EEA75345FE27E32AEB9EF86E28B546C4F1A125CA6EFC794D2F7C84246` |
| `docs/perf/cqp/sqp6000_first20.txt` | `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6` |
| `cqp_double_target.txt` | `EF42B91EFD02EC33EB8029329C95775A571CE1AA285D54B0A3A4D8D218D8020D` |
| `target1.txt` | `227C69B2C8C5680B594D26525A1D4CC6383C2615B48AC410F679B06921B7210B` |
| `target2.txt` | `A285A267AE4438BC254FE0511F9FDD1C39A458532CF872984FFB1C035E81569D` |

## 2. 总览

| 指标 | mimalloc | 系统分配器 | mimalloc 节省 | 相对 `d813e5f` | 距半时目标还需压缩 |
| --- | ---: | ---: | ---: | ---: | ---: |
| fixed30 单线程 overall | 31.307 µs/场 | 35.484 µs/场 | 11.77% | 快 2.48% | 25.37% |
| fixed30 自动线程 overall | 3.895 µs/场 | 4.813 µs/场 | 19.07% | 快 16.12% | 24.63% |
| stress_multi 单线程 | 65.044 µs/场 | 73.195 µs/场 | 11.14% | 快 1.95% | 24.96% |
| win-rate 单线程 13000 场 | 0.107 s | 0.117 s | 8.55% | 快 8.55% | 14.02% |
| score mario | 0.519 s | 0.609 s | 14.66% | 慢 10.72% | 已达历史 `1.5x legacy` 线 |
| score CQP 单人 | 0.765 s | 0.907 s | 15.67% | 慢 7.97% | 已达历史 `1.5x legacy` 线 |
| score CQP 双人 | 1.044 s | 1.189 s | 12.13% | 慢 1.00% | 已达历史 `1.5x legacy` 线 |
| CQP 单人 100% | 10.404 s | 12.173 s | 14.53% | 快 5.04% | 27.76% |
| CQD 双人 100% | 44.255 s | 55.333 s | 20.02% | 快 7.70% | 24.30% |

结论：

- mimalloc 在本轮所有正式指标上都更快，墙钟优势为 8.55%～25.41%；继续作为原生默认 allocator 是合理的；
- mimalloc 的 OpenBox CQP/CQD 结束 RSS 为约 13.1～29.2 MiB，系统分配器约 7.8～9.8 MiB。系统分配器更省常驻内存，但吞吐明显较低；
- fixed30、自动线程、win-rate 与 CQP/CQD 的中长档较 `d813e5f` 继续提升；CQP 单人 1% 受短任务固定开销影响，本轮单样本慢 11.45%；
- score 三组较 `d813e5f` 回退 1.00%～10.72%，但相对已保存 legacy 墙钟仍达到 `1.566x / 1.680x / 1.998x`，没有跌破 `1.5x legacy` 硬线；
- 所有“旧 runtime 半时线”仍未完成。当前最接近的是 win-rate，还需压缩约 14.02%；其余主要指标仍需约 22%～36%。

## 3. fixed30

### 3.1 单线程

| 分组 | mimalloc wall | 系统 wall | mimalloc init/fight | 系统 init/fight | mimalloc 节省 | 相对 `d813e5f` | 半时目标差距 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| overall | 31.307 µs/场 | 35.484 | 2.943 / 28.201 | 3.595 / 31.712 | 11.77% | 快 2.48% | 25.37% |
| core 1v1/2v2 | 15.282 | 17.404 | 2.017 / 13.125 | 2.409 / 14.843 | 12.19% | 快 3.79% | 24.47% |
| 1v1 | 9.728 | 11.040 | 1.591 / 8.010 | 1.824 / 9.076 | 11.88% | 快 6.14% | 22.36% |
| 2v2 | 25.465 | 29.072 | 2.798 / 22.501 | 3.481 / 25.417 | 12.41% | 快 2.07% | 26.01% |
| stress_multi | 65.044 | 73.195 | 4.922 / 59.909 | 6.039 / 66.927 | 11.14% | 快 1.95% | 24.96% |

两套 allocator 的 overall 赢家均为 `150858/390000`。

### 3.2 自动线程

自动线程的 init/fight 是 worker 累计 CPU 时间，不能与 wall 相加。

| 分组 | mimalloc wall | 系统 wall | mimalloc 节省 | mimalloc 吞吐 |
| --- | ---: | ---: | ---: | ---: |
| overall | 3.895 µs/场 | 4.813 | 19.07% | 256726 场/s |
| core 1v1/2v2 | 2.000 | 2.499 | 19.97% | 499974 场/s |
| 1v1 | 1.403 | 1.600 | 12.29% | 712600 场/s |
| 2v2 | 3.094 | 4.148 | 25.41% | 323183 场/s |
| stress_multi | 7.918 | 9.700 | 18.37% | 126302 场/s |

## 4. 普通 win-rate

| allocator | wall | init | fight | 胜场 | 吞吐 |
| --- | ---: | ---: | ---: | ---: | ---: |
| mimalloc | 0.107 s | 0.015 s | 0.089 s | 7077/13000 | 121802 场/s |
| 系统分配器 | 0.117 s | 0.016 s | 0.098 s | 7077/13000 | 111416 场/s |

mimalloc 墙钟快 8.55%。相对 `d813e5f` 的 0.117 s 也快 8.55%，但距离 0.092 s 半时线仍需压缩 14.02%。

## 5. score 单线程裸时间

| 输入 | allocator | wall | init | fight | 结果 |
| --- | --- | ---: | ---: | ---: | ---: |
| mario | mimalloc | 0.519494 s | 0.172981 | 0.342903 | 4171/13000 |
| mario | 系统 | 0.608746 s | 0.196984 | 0.407393 | 4171/13000 |
| CQP 单人 20 组 | mimalloc | 0.764700 s | 0.258846 | 0.494838 | 14513/20000 |
| CQP 单人 20 组 | 系统 | 0.906804 s | 0.296379 | 0.592244 | 14513/20000 |
| CQP 双人 32 组 | mimalloc | 1.044492 s | 0.327424 | 0.700945 | 30295/32000 |
| CQP 双人 32 组 | 系统 | 1.188704 s | 0.359820 | 0.804673 | 30295/32000 |

不计入性能结果的 legacy/runtime 正确性对账已对三组各执行一次，逐组差异均为 0。按 `d813e5f` 保存的 legacy 墙钟 `0.813613 / 1.284716 / 2.086516 s` 计算，当前 mimalloc runtime 吞吐分别为 legacy 的 `1.566x / 1.680x / 1.998x`，仍满足至少 `1.5x legacy`。

## 6. CQP/CQD 自动调度

| 输入 | 精度 | mimalloc wall | 系统 wall | mimalloc 节省 | 相对 `d813e5f` | 半时目标差距 | 结束 RSS（mimalloc/系统） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 单人 20 × target1 | 1% | 0.136911 s | 0.165999 s | 17.52% | 慢 11.45% | 36.47% | 15.5 / 8.9 MiB |
| 单人 20 × target1 | 10% | 1.039850 s | 1.193674 s | 12.89% | 快 7.09% | 26.34% | 13.1 / 8.5 MiB |
| 单人 20 × target1 | 100% | 10.403798 s | 12.173085 s | 14.53% | 快 5.04% | 27.76% | 13.4 / 7.8 MiB |
| 双人 32 × target2 | 1% | 0.511023 s | 0.664032 s | 23.04% | 快 3.59% | 30.13% | 24.9 / 9.8 MiB |
| 双人 32 × target2 | 10% | 4.602381 s | 5.528828 s | 16.76% | 快 8.06% | 26.41% | 29.2 / 8.6 MiB |
| 双人 32 × target2 | 100% | 44.254562 s | 55.332844 s | 20.02% | 快 7.70% | 24.30% | 23.4 / 9.8 MiB |

所有运行均完成 `700/700` 或 `1312/1312` matchup，没有任务失败或保护上限异常。

## 7. 正确性插曲与最终门禁

第一次 benchmark 在提交 `6ae9c86` 上发现 mario score 变为 `4170/13000`。定点对账定位到 round 11350：首次施加冰冻没有复刻 legacy `set_state → update_states`，导致待生效强化疾走仍按旧倍率递减冰冻步数。`out_md5.ts` 与 legacy 一致。

该问题由提交 `1ed1258` 修复，并将原始输入以可逆 `\x02` 转义归档到 `crates/tswn_test/cases/runtime_stress/score-mario-r11350.txt`。最终门禁：

- `cargo test -p tswn_core`：核心库 596 通过、2 忽略；CLI 59、runtime trace 3、engine 集成 29 均通过；
- release `no_debug` Runtime 库测试：429 通过、2 忽略；
- release Runtime corpus：124/124；
- mario、CQP 单人、CQP 双人完整 score legacy/runtime 对账：0 差异；
- 最终正式 benchmark 的赢家数恢复为 `4171/13000`、`14513/20000`、`30295/32000`。

## 8. 复测命令

```powershell
# mimalloc：默认 feature 保持开启
cargo build -p tswn_core --release --features "no_debug aux_bins" `
  --bin track_perf_cases --bin track_score_perf --bin tswn-cli
cargo build -p tswn_openbox --release --bin openbox_mem_probe

# 系统分配器：使用独立 target，保留 png_render 以只移除 mimalloc_alloc
$env:CARGO_TARGET_DIR = "$PWD\target\bench_no_mimalloc"
cargo build -p tswn_core --release --no-default-features `
  --features "no_debug aux_bins png_render" `
  --bin track_perf_cases --bin track_score_perf --bin tswn-cli
cargo build -p tswn_openbox --release --no-default-features --bin openbox_mem_probe

# fixed30；自动线程把 --thread 改为 0
target\release\track_perf_cases.exe `
  --case-dir docs\perf\fixed_cases_30 `
  --out-dir target\full_bench_1ed1258_20260716\mimalloc\fixed30_t1 `
  --bench-runs 13000 --thread 1 --engine main -q

# score；其余输入替换为 sqp6000_first20.txt / cqp_double_target.txt
target\release\track_score_perf.exe `
  --input docs\perf\score\mario.txt `
  --label 1ed1258-mimalloc-mario `
  --count 13000 --engine main --mode normal `
  --out target\full_bench_1ed1258_20260716\mimalloc\score_mario.json

# CQP；双人替换 players/targets，各档把 count 改为 100、1000、10000
target\release\openbox_mem_probe.exe `
  --players docs\perf\cqp\sqp6000_first20.txt `
  --targets crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 100 --threads 0
```
