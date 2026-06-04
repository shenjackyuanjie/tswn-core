# 更新日志

## [0.1.0] - unreleased

### 新增

- 新增 `tswn_lane_ranker` 服务端工具，用于维护 lane 组合、两两对战数据、排名计算和 Web 管理界面。
- 新增 SQLite 存储层，支持组合导入、归档、重新计算、胜率样本记录和排名结果持久化。
- 新增基于 `axum` 的 HTTP 服务与静态前端页面，默认监听 `127.0.0.1:3000`，可通过 `LANE_RANKER_BIND` 配置。
- 新增 `LANE_RANKER_DB` 环境变量，用于指定 SQLite 数据库路径，默认使用 `lane_ranker.sqlite3`。
- 新增 ranking / pairwise / skill equivalence / team parsing 等模块，用于对 lane 组合做批量比较、等价技能归并和评分排序。
- 新增与 `tswn_core` 胜率计算集成的采样路径，用于把实际对战胜率纳入 lane ranker 的排序流程。

### 调整

- ranker 配置支持从环境变量读取预热轮数、总轮数、胜率样本数、并发 worker、归档组合跳过策略等运行参数。

### 说明

- 该 crate 当前作为内部 Web 工具加入 workspace，版本仍处于 `0.1.0` 初始阶段。
