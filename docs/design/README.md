# 设计与实施规格

[返回文档中心](../README.md)

本目录保留设计推导、实施规格和验收记录。阅读时先看各篇开头的**当前落地状态**：正文中的历史备选方案、阶段计划和当时的待办，不等于当前 API。现行调用契约以 [公共 API](../reference/public-api.md) 与源码为准。

胜率预测链路统一归属于 **tswn-pwp（player winchance predictor）**。当前已实现机制状态导出、两遍分层采样、Parquet / 续跑基础设施、FeatureEncoder 全八族核心张量编码与 `tswn-pwp encode` 基础离线导出；完整训练包、模型训练及推理接口仍需后续实现。状态、数据集与 encoder 契约均属于 Experimental，与正式 BattleSession / DTO 的稳定性边界分开。

| 文档 | 当前定位 |
| --- | --- |
| [BattleSession 与 Web Streaming 重构计划](battle-session-plan.md) | 核心会话、跨语言 DTO、CLI JSONL 与网页流式架构已落地；保留原始设计、历史备选与验收过程 |
| [BattleSession API 冻结前加固要求](battle-session-hardening.md) | 关键加固已落地，包括 C options V1 prefix、sticky failure、绑定类型与释放约定；历史 FINAL / 可执行清单按开篇对照阅读 |
| [战斗状态导出与胜率数据生成](battle-analyze.md) | 已实现 Rust 状态接口、两遍分层抽样、Parquet 与续跑；保留生成方案和数据契约依据 |
| [BattleModelState Runtime 审计](battle-model-state-audit.md) | 已实现状态投影的字段来源、名字机制派生和缓存排除理由；不是 Runtime 存档格式 |
| [FeatureEncoder 设计规格](feature-encoder-spec.md) | 已实现八族、69 个张量的核心编码及基础离线包；完整训练包与跨绑定接口等后续验收项另列 |

## 阅读关系

BattleSession 的前两篇按表中顺序阅读：先看 [重构计划](battle-session-plan.md)，再看 [API 加固要求](battle-session-hardening.md)；当前调用方式见 [公共 API](../reference/public-api.md)。

`tswn-pwp` 数据链路先看 [状态导出与数据生成](battle-analyze.md) 确认状态边界和 Parquet 契约，再看 [Runtime 状态审计](battle-model-state-audit.md) 追溯字段来源，最后看 [FeatureEncoder 规格](feature-encoder-spec.md) 了解固定张量映射与导出边界。生成器命令与文件格式补充说明见 [tswn-pwp README](../../crates/tswn_pwp/README.md)。

## 当前实现依据

- 会话推进与失败状态：[BattleDriver](../../crates/tswn_core/src/cli_api/battle/driver.rs)、[BattleSession](../../crates/tswn_core/src/cli_api/battle/session.rs)。
- 实验状态接口与数据链路：[BattleModelSession](../../crates/tswn_core/src/cli_api/battle/model_session.rs)、[Runtime 状态投影](../../crates/tswn_core/src/runtime/model_state.rs)、[生成器](../../crates/tswn_pwp/src/generate.rs)。
- 核心张量与离线导出：[张量注册表](../../crates/tswn_core/src/encoder/batch.rs)、[FeatureEncoder](../../crates/tswn_core/src/encoder/encode.rs)、[encode 命令](../../crates/tswn_pwp/src/encode.rs)。

已完成的旧 Runtime 重构过程见 [历史归档](../archive/README.md)。
