# 设计与机制规格

[返回文档中心](../README.md)

本目录说明现行架构取舍、机制字段来源和固定张量契约。使用方式见 [原理指南](../core-guide/README.md)，调用契约见 [公共 API](../reference/public-api.md)，各工具命令见所属 crate README。

胜率预测数据链路归属于 **tswn-pwp（player winchance predictor）**，交付状态与完整流程见 [PWP README](../../crates/tswn_pwp/README.md)。状态、数据集与 encoder 按 Experimental 契约演进；各篇将已交付能力和后续模型/绑定目标分别说明。

| 文档 | 主要解决的问题 |
| --- | --- |
| [BattleSession 与网页增量播放](battle-session.md) | 驱动与两种会话如何分层，源端/历史/播放如何独立，关键兼容决策为何这样选择 |
| [战斗状态导出与胜率数据生成](battle-analyze.md) | 两遍采样、标签、切分、Parquet、恢复和后续模型接入如何组织 |
| [BattleModelState Runtime 审计](battle-model-state-audit.md) | 每类机制字段从哪里导出，身份如何派生，机制状态与存档/缓存各有什么用途 |
| [FeatureEncoder 设计规格](feature-encoder-spec.md) | 状态如何映射到固定张量，容量、引用、数值、标签与审计通道如何约定 |
| [Runtime 架构评估（未实施提案）](runtime-redesign-assessment.md) | 基于文中提交的执行内核评估与候选重构路线；不是当前 API 或已经完成的实施说明 |

## 阅读关系

会话接入先看 [公共 API](../reference/public-api.md)，理解内部职责时再看 [会话设计](battle-session.md)。

`tswn-pwp` 数据链路先看 [状态导出与数据生成](battle-analyze.md) 确认状态边界和 Parquet 契约，再看 [Runtime 状态审计](battle-model-state-audit.md) 追溯字段来源，最后看 [FeatureEncoder 规格](feature-encoder-spec.md) 了解固定张量映射与导出边界。生成器命令与文件格式补充说明见 [tswn-pwp README](../../crates/tswn_pwp/README.md)。

## 当前实现依据

- 会话推进与失败状态：[BattleDriver](../../crates/tswn_core/src/cli_api/battle/driver.rs)、[BattleSession](../../crates/tswn_core/src/cli_api/battle/session.rs)。
- 实验状态接口与数据链路：[BattleModelSession](../../crates/tswn_core/src/cli_api/battle/model_session.rs)、[Runtime 状态投影](../../crates/tswn_core/src/runtime/model_state.rs)、[生成器](../../crates/tswn_pwp/src/generate.rs)。
- 核心张量与离线导出：[张量注册表](../../crates/tswn_core/src/encoder/batch.rs)、[FeatureEncoder](../../crates/tswn_core/src/encoder/encode.rs)、[manifest 命令](../../crates/tswn_pwp/src/manifest.rs)、[encode 命令](../../crates/tswn_pwp/src/encode.rs)、[Python loader](../../scripts/read_encoded_batches.py)。

旧版本升级见 [Runtime 迁移指南](../guides/runtime-0.5-migration.md)；已完成的长篇实施计划可从对应文档的 Git 历史查阅。
