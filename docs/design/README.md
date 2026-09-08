# 设计与实施规格

[返回文档中心](../README.md)

| 文档 | 定位 |
| --- | --- |
| [BattleSession 与 Web Streaming 重构计划](battle-session-plan.md) | 已锁定的实施规格，正文包含实施与验收记录 |
| [BattleSession API 冻结前加固要求](battle-session-hardening.md) | 在上述工作基础上的加固要求，保留原文 FINAL / 可直接执行状态；不是已完成声明 |
| [战斗状态导出与胜率数据生成](battle-analyze.md) | Rust 状态接口、两遍分层抽样、Parquet 与续跑契约 |
| [BattleModelState Runtime 审计](battle-model-state-audit.md) | 状态字段来源、名字机制派生和缓存排除理由 |

BattleSession 的前两篇规格按表中顺序阅读；当前调用方式见 [公共 API](../reference/public-api.md)。状态导出和数据生成采用后两篇契约。已完成的旧 Runtime 重构过程见 [历史归档](../archive/README.md)。
