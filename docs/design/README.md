# 设计与实施规格

[返回文档中心](../README.md)

| 文档 | 定位 |
| --- | --- |
| [BattleSession 与 Web Streaming 重构计划](battle-session-plan.md) | 已锁定的实施规格，正文包含实施与验收记录 |
| [BattleSession API 冻结前加固要求](battle-session-hardening.md) | 在上述工作基础上的加固要求，保留原文 FINAL / 可直接执行状态；不是已完成声明 |
| [战斗状态导出与胜率数据生成](battle-analyze.md) | Rust 状态接口、两遍分层抽样、Parquet 与续跑契约；依赖已完成的 BattleSession 推进语义 |
| [BattleModelState Runtime 审计](battle-model-state-audit.md) | 状态字段来源、名字机制派生和缓存排除理由；为状态导出和 FeatureEncoder 提供字段依据 |
| [FeatureEncoder 设计规格](feature-encoder-spec.md) | 原始机制状态到固定张量的字段映射、数值与引用契约、Rust 导出及验收计划；消费 BattleModelState 与 Parquet 契约 |

## 阅读关系

BattleSession 的前两篇规格按表中顺序阅读：先看 [重构计划](battle-session-plan.md)，再看其后的 [API 加固要求](battle-session-hardening.md)；当前调用方式见 [公共 API](../reference/public-api.md)。

胜率数据链路按以下顺序阅读：先看 [状态导出与数据生成](battle-analyze.md) 确认状态边界和 Parquet 契约，再看 [Runtime 状态审计](battle-model-state-audit.md) 追溯字段来源，最后看 [FeatureEncoder 规格](feature-encoder-spec.md) 了解固定张量映射。三篇共同约束生成器、训练和未来推理实现；生成器命令与文件格式补充说明见 [winprob 数据集 README](../../crates/tswn_winprob_dataset/README.md)。

已完成的旧 Runtime 重构过程见 [历史归档](../archive/README.md)。
