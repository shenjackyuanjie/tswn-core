# 更新日志

## [Unreleased] - 0.0.0

### 新增

- 新增跨引擎共享测试 harness：通过 `EngineAdapter` 和 `test_engine_suite!` 复用小型、完整大型及多队混战 fixture，统一校验事件、胜者、分数和 RC4 状态。
- 新增 `runtime-corpus` 可选测试目标，接入 Runtime 压力输入与冻结 golden 基线；基线记录输入摘要、`eval_rq`、胜者、回合数、总分、最终 RC4 和逐回合 canonical digest。
- 新增 `BattleSession` 与 `battle_replay()` 回放一致性测试，覆盖逐帧状态、终局结果、回合预算和图标选项。

### 变更

- 将 Runtime v2 corpus 迁移为正式 `runtime` corpus，保留 87 项共享 exact trace，并冻结 37 项 legacy Runtime golden 作为删除旧对象模型后的对账基线。
- 将大型 fight fixture 拆分为独立文件和多片测试模块；压力输入按来源、模式和首个差异归档，修复后持续保留并接入回归。
- 统一 golden 输入换行与 `\x02` 控制字符还原规则，确保跨平台加载和摘要校验稳定。

### 说明

- 该 crate 仅供 workspace 内部测试使用，版本为 `0.0.0` 且不发布；普通测试无需额外 feature，Runtime corpus 通过 `--features runtime-corpus` 启用。
