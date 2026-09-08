# AMD uProf CPU 性能分析

本文规定 Windows 上 `tswn_core` 的管理员态函数级 CPU profiling 流程。目标是生成可由
自动化 agent 直接读取的 `report.csv`，而不是只生成供人手浏览的火焰图。

## 适用范围

- CPU：AMD Zen 系列；当前开发机为 Ryzen 7 5800X（Zen 3）。
- 工具：AMD uProf 5.3+ 的 `AMDuProfCLI.exe`。
- 权限：PMC / IBS 采样应从已提权的终端或已提权的 agent 会话启动。
- 构建：使用 `--release --features no_debug`。workspace 的 release profile 已保留
  `debug = 1`，uProf 可通过同目录 PDB 归因到 Rust 符号与源码。

不要把 uProf 数据作为吞吐 benchmark 的最终数字：采样和调用栈展开都有开销。性能是否成立
仍遵循 [`../../rule.md`](../../../rule.md) 中同会话交替 A/B、取中位数且忽略小于 1.5% 差异的规则。

## 标准热点采样

TBP（Time-Based Profile）用于先找 CPU 热函数；`-g` 要求收集用户态调用栈。选择能运行至少
约 15 秒的固定输入，避免样本太少。下面的 case 在当前机器很快，因此将次数调到 600 万
（2026-09-05 实测 300 万场约 13 秒，600 万场约 27 秒）：

```powershell
$uProf = (Get-Command AMDuProfCLI.exe -ErrorAction Stop).Source
cargo build -p tswn_core --release --features no_debug --bin tswn-cli
& $uProf collect --config tbp -g `
  -o target\amduprof\before `
  target\release\tswn-cli.exe bench win-rate `
  -f docs\perf\fixed_cases_30\01_1v1-6a2ace7473581042.txt -n 6000000 -s
& $uProf report -i target\amduprof\before --detail -g
```

uProf 5.3 将结果自动写至 `target\amduprof\before\report.csv`；`report` 子命令不接受旧版本
教程中的 `-o`。每次采样使用新的输出目录，例如 `before`、`after`，避免混合原始数据。

`collect -g` 负责采集调用栈，`report -g` 才会把调用栈写入 CSV；两处都需要。
默认只输出前 10 个函数，需要扩大范围时追加 `--cutoff 40`（`--cutoff 0` 输出全部）。
查看内联热路径的机器指令可追加 `--disasm-full`，并用
`--report-output target\amduprof\before\disasm.csv` 单独保存；检查报告是否完整，避免将
生成失败的汇编报告当作完整热点数据。

采样时不加 `tswn-cli --perf`：该选项会为每场比赛额外进行四次 QPC 计时，适合 init/fight
粗分，不适合测正常热路径。

## Agent 判读

`report.csv` 包含元数据、`HOTTEST FUNCTIONS`、`HOTTEST PROCESSES`、模块及调用栈区段。
agent 应按以下顺序工作：

1. 确认 `Target Path`、输入、commit、feature 与线程模式是预期口径；确认 `Call Stack Sampling` 为 `True`。
2. 从 `HOTTEST FUNCTIONS` 按 `CPU_TIME` 选择位于 `tswn-cli.exe` 的候选函数；再从调用栈区段确认其调用者，不要只根据函数名做局部优化。
3. 修改后以同一输入、次数、线程模式重采样到 `after` 目录；比较热点排序和 CPU_TIME 仅用于解释变化。
4. 最后运行正常的 CLI A/B benchmark 作为收益结论；记录 commit、原始输出、uProf 的 collect command 和两份 CSV 路径。

本机已在 2026-09-04 验证过此链路：uProf 5.3.521 的 TBP + `-g` 对
`tswn-cli bench win-rate` 成功采样并生成 `report.csv`，其中能识别
`PreparedBattleRoster::refill_seed_state`、`CombatRuntime::scan_plain_action_skill_probabilities`、
`RuntimeRunner::run_to_completion_prevalidated` 等 Rust 符号。

## 深入诊断

TBP 只回答“CPU 时间主要花在哪里”。确认热点后可对同一负载另开一次采样：

- `--config assess`：观察整体微架构瓶颈；
- `--config ibs -g`：对 Zen 的指令级 / 内存路径做更精确归因；
- 自定义 PMC event：仅在有明确假设（如 branch 或 L1/L2 refill）时使用。

不要把多类硬件事件、IBS 与 TBP 混进同一份基准结果；每次只回答一个性能问题，并保留原始
输出目录。

## 无管理员权限时

AMD uProf 的硬件采样不可用时，不要试图用 `samply` 或 `wpr` 绕过 Windows 的内核采样权限。
继续使用 CLI benchmark / `--perf` 做回归定位，并运行 `scripts/pgo_build.py` 取得 LLVM
instrumentation 的函数执行热度；需要可视化时间线时，待有管理员会话后再运行 `samply`。
