# Fixed 20-case Performance Benchmark

这份文档记录固定 20 个 case 的长期性能口径。它补充 `benchmark_tracking.md` 中的两个历史样本，目标是覆盖更多多人、队伍、FFA 和复杂战斗路径。

## 1. 固定输入

固定 case 存放在：

- `docs/perf/fixed_cases_20/*.txt`

这些输入已经固化到仓库中，后续 benchmark 不再重新从号库生成或重新抽样。文件名前缀 `01` 到 `20` 是固定顺序，整体从较简单到较复杂排列。

## 2. 运行口径

- 编译参数：`--release --features no_debug`
- 工具：`track_perf_cases`
- 输入：`--case-dir docs/perf/fixed_cases_20`
- 单线程：`--thread 1`
- 每个 case：`8000` 场
- 总量：`20 * 8000 = 160000` 场
- 目标耗时：约 `30s`

推荐命令：

```powershell
cargo run --release --features no_debug --bin track_perf_cases -- `
  --case-dir docs/perf/fixed_cases_20 `
  --out-dir docs/perf/fixed_cases_20_results `
  --bench-runs 8000 `
  --thread 1
```

工具会输出：

- `perf_cases.md/json`：最近一次运行结果；
- `perf_cases_<version>.md/json`：按 crate 版本命名的结果文件，适合每个版本留档。

## 3. 当前结果

| 版本 | 结果文件 | case 数 | 每 case 场数 | 总场数 | 合计耗时 | 加权 `µs/场` |
| -- | -- | -: | -: | -: | -: | -: |
| `0.3.7` | `docs/perf/fixed_cases_20_results/perf_cases_0.3.7.md` | 20 | 8000 | 160000 | `29.890s` | `186.81` |

结构化结果：

- `docs/perf/fixed_cases_20_results/perf_cases_0.3.7.json`

## 4. 判读

- 看版本整体回退时，优先比较合计耗时和加权 `µs/场`；
- 看具体热路径回退时，打开对应版本的 `perf_cases_<version>.md`，按 case 行比较 `init us/场` 和 `fight us/场`；
- 如果只修改初始化、解析、构建模板相关逻辑，重点看 `init us/场`；
- 如果修改战斗循环、状态、技能、召唤、死亡清理、选择目标等逻辑，重点看 `fight us/场`。
