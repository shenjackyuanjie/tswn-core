---
name: fixed30-benchmark
description: 在 tswn-core 仓库跑 fixed30 性能基准并留档：用 perf_runtime 多轮采样、汇总分组中位数与离散度、生成趋势对比表、写 docs/perf 报告与 JSON。当用户要求跑基准、出性能对比表、留档性能快照或追踪 fixed30 回归时使用。
---

# fixed30 基准采样与留档

固定口径：`docs/perf/fixed_cases_30` 的 30 个输入，每 case 13000 场，单线程（`--threads 1`）与自动线程（`--threads 0`）各采样多轮，取分组中位数。热路径采样**不带** `--timed`；`--timed` 只用于 init/fight 分解。

## 1. 准备

```powershell
# 负载与机器确认（突发后台负载会直接放大离散度，读数写进报告）
(Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average

# 构建（release + no_debug + 默认 mimalloc）
cargo build -p tswn_core --release --features no_debug --example perf_runtime
```

本机 sccache 0.16 包装 `libmimalloc-sys` 的 C 编译时会把 `/WX-` 传给 `c1xx` 并报 `C1083`；遇到时追加 `--config build.rustc-wrapper=""` 关闭 sccache 再构建。该开关只影响构建过程，不改变产物。

## 2. 采样

```powershell
pwsh .agents/skills/fixed30-benchmark/scripts/run_rounds.ps1 -Rounds 10
```

默认行为：线程配置 `1,0` 各 10 轮、每 case 13000 场、轮间 3 秒，输出到 `target/fixed30_<时间戳>/`：

- `t<threads>_r<round>.jsonl`：每轮 30 行 JSON（`elapsed_seconds`、`total`、`wins`、`errors`、`guard_exhausted`、`group_sizes` 等）；
- `runs.log`：每轮 wall、起止时间、前后 CPU 负载、退出码。

常用覆盖：`-Rounds 3`、`-Threads 0`、`-RunsPerCase 1000`（冒烟）、`-TimedRound`（额外一轮写 `t1_r0_timed.jsonl`，不参与统计）。任何一轮非零退出会立即停止。

## 3. 汇总与出表

```powershell
# 控制台看分组表 + 趋势表 + 一致性
node .agents/skills/fixed30-benchmark/scripts/summarize.mjs --dir target/fixed30_<时间戳>

# 写机器可读 JSON（留档用）
node .agents/skills/fixed30-benchmark/scripts/summarize.mjs --dir target/fixed30_<时间戳> --out docs/perf/fixed30_<日期>_<commit>_runs.json --label "0.8.0 <commit>" --version 0.8.0

# 只要 Markdown（直接喂给 noticer --render）
node .agents/skills/fixed30-benchmark/scripts/summarize.mjs --dir ... --markdown
```

脚本按 `sum(elapsed)/sum(total)` 算 µs/场，跨轮取中位数，并用 `3 × 1.4826 × MAD` 判定离群轮后给出 `kept_median`。分组固定为 overall、core_1v1_2v2、one_v_one、two_v_two、stress_multi（ffa_6/ffa_8/3v3v3）、ffa_4。

历史基线在 [`history.json`](history.json)，趋势表由它加上本次结果生成；与最后一条数值接近（overall/core 差 < 0.05 µs/场）时不重复追加。新增已发布版本时，往 `entries` 末尾追加一条（label、version、date、commit、tool、single、auto、report）。

## 4. 留档

1. 报告写到 `docs/perf/reports/runtime-<version>-<commit>-fixed30-benchmark.md`，最少包含：环境与口径（CPU/OS/rustc/profile/命令/轮数/负载范围）、结论表（分组中位数与 CV）、单线程与自动线程明细、逐 case 中位数、一致性与 `--timed` 分解、复测命令；
2. 机器可读结果写到 `docs/perf/fixed30_<日期>_<commit>_runs.json`；
3. 更新三处索引：[docs/perf/README.md](../../../docs/perf/README.md)（「Runtime 基准与快照」与「固定输入与原始结果」表）、[docs/perf/benchmark-history.md](../../../docs/perf/benchmark-history.md)（说明列表）、[docs/perf/guides/fixed30-benchmark.md](../../../docs/perf/guides/fixed30-benchmark.md)（当前复测入口）；
4. 原始 `target/fixed30_*` 不纳入 Git，`cargo clean` 会清掉；JSON 里已保留逐轮数值。

## 5. 上报（可选）

用户要求上报时，加载 `noticer-progress` skill，把 summarize 的 Markdown 表格作为 `--message` 用 `--render` 发出。只看本次结果就够时不必带整张趋势表。

## 6. 判读规则

- **跨时段绝对值不作门禁**：不同日期、不同后台负载之间的 ±2% 属噪声；要判定回退必须做同会话奇偶轮交替 A/B；
- 参考本次实测 CV：单线程 overall 约 2.5%，自动线程 3%–5%，case 级可达 9%；变化小于对应 CV 时不要宣称收益或回退；
- 每次采样核对 `wins`、`errors`、`guard_exhausted` 是否逐轮一致；不一致先查输入与语义变化，再谈性能；
- 口径经历过 `track_perf_cases`（0.3.x–0.4.x）→ 临时 harness（0.5.0）→ `perf_runtime`（0.7.0 起）三次变更，跨代只看趋势；
- `guard` / `wins` 在不同版本间可能变化（0.5.0 的 case 17 每轮 1 次 guard、wins 150858；0.7.0 为 0 与 150853），属运行语义差异，不当作性能噪声；
- 输入目录是相对路径，必须在仓库根执行；否则报「输入目录没有 txt 文件」。
