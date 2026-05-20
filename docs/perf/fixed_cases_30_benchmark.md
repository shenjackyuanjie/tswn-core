# Fixed 30-case Performance Benchmark

这份文档记录固定 30 个 case 的长期性能口径。它是当前仓库唯一维护的固定性能回归集合：覆盖多人、队伍、FFA 和复杂战斗路径，并追加强化 1v1 / 2v2 核心场景。

## 1. 固定输入

固定 case 存放在：

- `docs/perf/fixed_cases_30/*.txt`

约束：

- `01` 到 `20` 是原阶梯覆盖输入，继续保留在 30-case 集合中；
- `21` 到 `30` 是新增核心 1v1 / 2v2 case；
- 文件名中的 `mode-hash` 来自固定输入内容，输入文本稳定时 id 稳定；
- benchmark 默认使用 `prepared_win_rate` 的固定 seed 调度，不依赖外部随机源。

## 2. 新增 case 覆盖

| # | id | 类型 | 主要覆盖热点 |
| -: | -- | -- | -- |
| 21 | `1v1-2cf20e674201187f` | 1v1 短局爆发 | 极轻量 1v1，低回合基础开销；使用 `a` vs `b`，方便长期复现。 |
| 22 | `1v1-e6497125749fa0a3` | 1v1 中等长度常规 | 普通 1v1 baseline，攻防相对均衡。 |
| 23 | `1v1-d985a03666ef702e` | 1v1 长回合消耗 | 生存/回复/拖回合路径，观察长期回合循环与状态更新成本。 |
| 24 | `1v1-fb453d38e6c4cf02` | 1v1 高频技能触发 | 技能 act/fire、追加行动和条件触发路径。 |
| 25 | `1v1-d9ead8dd53251df2` | 1v1 高频 buff/debuff | 状态添加、刷新、过期、检查和叠层相关路径。 |
| 26 | `1v1-7822d33f277cc830` | 1v1 高频随机/目标选择 | RNG、命中/概率触发、目标/条件筛选路径。 |
| 27 | `1v1-e4d73d1d7ae3ebe9` | 1v1 极端边界 | 特殊规则/边界机制，防止非普通路径退化。 |
| 28 | `2v2-f5743e32254e64e5` | 2v2 常规 baseline | 普通 2v2 主指标，基础队友交互。 |
| 29 | `2v2-b1e08a4b5faf6712` | 2v2 群体技能/多目标 | AoE、全体效果、队伍遍历和多目标 effect application。 |
| 30 | `2v2-a2fa863c153307e2` | 2v2 队友交互/连锁触发 | 保护、协作、反击、死亡/连锁触发等 2v2 额外交互路径。 |

## 3. 运行口径

- 编译参数：`--release --features no_debug`
- 工具：`track_perf_cases`
- 输入：`--case-dir docs/perf/fixed_cases_30`
- 单线程：`--thread 1`
- 每个 case：`13000` 场
- 总量：`30 * 13000 = 390000` 场
- 当前机器实测目标耗时：约 `40s`（`0.3.7` / 单线程 / `--features no_debug` 下为 `40.507s`）

推荐命令：

```powershell
cargo run --release --features no_debug --bin track_perf_cases -- `
  --case-dir docs/perf/fixed_cases_30 `
  --out-dir docs/perf/fixed_cases_30_results `
  --bench-runs 13000 `
  --thread 1
```

工具会输出：

- `perf_cases.md/json`：最近一次运行结果；
- `perf_cases_<version>.md/json`：按 crate 版本命名的结果文件，适合每个版本留档。

## 4. 报告分组

`track_perf_cases` 会为固定 case 报告输出以下汇总：

- `overall`：所有 case 总耗时、平均 `µs/场`、总吞吐；
- `core_1v1_2v2`：只统计 1v1 和 2v2，当前最重要的核心分数；
- `one_v_one`：只统计 1v1；
- `two_v_two`：只统计 2v2；
- `stress_multi`：统计 `ffa_6`、`ffa_8`、`3v3v3` 等重型多人局。

每个分组包含：case 数、runs、win rate、elapsed、`us/场`、`场/s`、`init us/场`、`fight us/场`。

## 5. 判读

- 看当前主路径是否退化，优先比较 `core_1v1_2v2`；
- 只改战斗循环、技能、状态、目标选择时，同时看 `one_v_one`、`two_v_two` 和各 case 的 `fight us/场`；
- 只改初始化、解析、模板构建时，重点看各分组和各 case 的 `init us/场`；
- 多人压力路径仍用 `stress_multi` 观察，避免核心 1v1/2v2 变快时掩盖多人局退化。
