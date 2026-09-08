# 性能文档与数据

[返回文档中心](../README.md)

报告按采样时的版本、提交和机器环境解读，不将不同口径直接视为当前性能。旧执行器与临时 harness 的命令可能已不可用；当前发布约束见 [rule.md](../../rule.md)。

## 方法与追踪

- [性能追踪与历史比较](benchmark-history.md)：含当前 `perf_runtime` 复测入口。
- [AMD uProf 采样指南](guides/amd-uprof.md)。
- [fixed30 固定输入与历史工具口径](guides/fixed30-benchmark.md)。

## 调查报告

- [Web Streaming 基线](reports/web-streaming-baseline.md)。
- [Python BattleSession DTO 转换基线](reports/python-battle-session-baseline.md)。
- [RC4 热点与优化调查（2026-09-05）](reports/rc4-profile-2026-09-05.md)。
- [0.5.3 优化机会](reports/optimization-opportunities-0.5.3.md)。
- [目标选择优化](reports/target-selection-optimization.md)。
- [UB 与 no_debug 修复性能记录](reports/ub-fix-no-debug.md)。

## Runtime 基准与快照

- [0.5.0 发版基准 · 749fcd1](reports/runtime-0.5.0-749fcd1-release-benchmark.md)。
- [0.4.3 发版基准 · 9d3a3b9](reports/runtime-0.4.3-9d3a3b9-release-benchmark.md)。
- [0.4.2 分配器快照 · 1ed1258](reports/runtime-0.4.2-1ed1258-allocator-snapshot.md)。
- [0.4.2 完整快照 · d813e5f](reports/runtime-0.4.2-d813e5f-snapshot.md)。
- [Node / Bun / legacy / Runtime 四方比较（2026-07-15）](reports/runtime-four-way-2026-07-15.md)。
- [0.4.0 基线](reports/runtime-0.4.0-baseline.md)。
- [CQP / CQD Runtime 基线](reports/cqp-runtime-baseline.md)。

## 固定输入与原始结果

下列文件保留原命名和路径，供现有脚本及历史复现使用。`reports/` 中的手写报告通过相对链接关联原始数据，不要求与 JSON 同名。

| 路径 | 内容 |
| --- | --- |
| [fixed_cases_30/](fixed_cases_30/) | 30 个固定输入，文件顺序与内容保持不变 |
| [fixed_cases_30_results/](fixed_cases_30_results/) | 工具生成的 [初始](fixed_cases_30_results/perf_cases.md)、[0.3.8](fixed_cases_30_results/perf_cases_0.3.8.md)、[0.3.10](fixed_cases_30_results/perf_cases_0.3.10.md) 报告与同名 JSON |
| [score/](score/)、[cqp/](cqp/)、[win_rate/](win_rate/)、[pgo_training/](pgo_training/) | score、CQP、胜率与 PGO 输入 |
| `runtime_*.json` | 对应 Runtime 报告的原始结果；从报告内链接进入 |
| [rc4_20260905_profile.json](rc4_20260905_profile.json) | RC4 调查数据 |
| [python_battle_session_samples.json](python_battle_session_samples.json) | Python BattleSession DTO 转换基线的逐次样本 |
| [web_streaming_result.png](web_streaming_result.png) | Web Streaming 页面截图 |
