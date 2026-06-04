# 更新日志

## [0.1.0] - unreleased

### 新增

- 新增 `tswn_ladder` 天梯更新器，用于按 cron 周期从 PostgreSQL 读取活跃玩家、生成对局、执行 `tswn_core` 战斗并回写分数、排名、handicap 和历史记录。
- 新增 `DATABASE_URL` 必填环境变量，用于连接 PostgreSQL。
- 新增 `LADDER_CRON_SCHEDULE`、`LADDER_GROUPS`、`LADDER_RUN_ONCE`、`LADDER_RANK_LOG_DIR` 环境变量，分别控制调度周期、分组列表、单轮运行模式和 rank 日志输出目录。
- 新增 handicap 参与的匹配生成与 ELO 更新逻辑，支持扰动、衰减、稀疏持久化和对局质量统计。
- 新增运行时内存监控，超过内部限制时主动退出，方便由外部 supervisor 拉起。
- 新增 rank 日志输出，按分组追加记录每轮排序后的玩家 ID 列表。

### 说明

- 该 crate 当前作为内部运维工具加入 workspace，版本仍处于 `0.1.0` 初始阶段。
