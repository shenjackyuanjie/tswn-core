# 更新日志

## [Unreleased] - 0.1.1

## [0.1.0] - 2026-09-30

### 变更

- 重写 lane ranker 的 SQLite、pairwise、排名和 Web service 实现，保留原有导入、归档、胜率采样、排名计算及管理页面工作流。
- 校准流程改为以已保存的 Raw lane 结果驱动，按 lane 大小选择阈值和 Correct 替换槽位；Rust 端嵌入严格 Python 校准器，支持缺失胜率动态补算、Correct target trace 持久化和结果审计。
- 清理废弃的 Top50 压缩、拟合搜索及未使用数据库辅助接口，保留现行评分、Python 校验端点和兼容数据结构。
- 将胜率评估常量迁移至 `tswn_core::namerena::eval_name`，同步中文化排名器源码与工具注释。

### 验证

- 当前 lane ranker 回归测试覆盖 9 项；校验命令为 `cargo test -p tswn_lane_ranker`、`cargo +nightly fmt --check` 和前端 `node --check`。

### 新增

- 新增 `tswn_lane_ranker` 服务端工具，用于维护 lane 组合、两两对战数据、排名计算和 Web 管理界面。
- 新增 SQLite 存储层，支持组合导入、归档、重新计算、胜率样本记录和排名结果持久化。
- 新增基于 `axum` 的 HTTP 服务与静态前端页面，默认监听 `127.0.0.1:3000`，可通过 `LANE_RANKER_BIND` 配置。
- 新增 `LANE_RANKER_DB` 环境变量，用于指定 SQLite 数据库路径，默认使用 `lane_ranker.sqlite3`。
- 新增 ranking、pairwise、skill equivalence 和 team parsing 模块，用于批量比较 lane 组合、归并等价技能并计算评分排序。
- 新增与 `tswn_core` 胜率计算集成的采样路径，将实际对战胜率纳入 lane ranker 排序流程。
- 新增严格 Python 校准器 `strict_python_calibrator.py`、缺失胜率动态补算、手动胜率录入和校准结果 Web 面板。
- 新增靶子生成工具 `target_milp_solver.py`，支持靶子池阈值、固定主榜数量、约束求解、参考胜率审计和导出。
- 新增文字技能类型、简化主技能类型、胜率分布类型和残差类型等结果字段，用于前端展示、导出和校准质量判断。

### 调整

- ranker 配置支持从环境变量读取预热轮数、总轮数、胜率样本数、并发 worker 和归档组合跳过策略等运行参数。
- pairwise 校准入口按 lane size 使用默认 Raw Score 校准池阈值；lane 结果补充校准分、约束选择、胜率类型、残差类型、置信度、稳定性和诊断指标。
- Web 管理界面增加校准等待、靶子预览与导出、胜率录入、结果质量徽标和更完整的结果表展示。

### 说明

- 该 crate 作为内部 Web 工具加入 workspace，本轮首次封板为 `0.1.0`，不进入默认聚合包。
