# 更新日志

## [0.1.1] - 2026-10-13

### 构建

- 线上构建统一 `--release --features "no_debug,mimalloc_alloc"`；部署使用
  `target/release/tswn_ladder`，不要用 debug / release-fast 产物。

### 修复（部署特化）

- seed 生成的时间戳固定使用 `Asia/Shanghai`，不再受部署主机本地时区影响。
- PostgreSQL 连接关闭时自动重连：每轮/每组开始前检查连接，处理异常时若确为连接关闭也重连一次，
  避免长驻服务在数据库连接中断后持续空转失败。

### 内部整理

- 将环境配置、更新编排、匹配、战斗及数据库存储从单一入口文件拆分；共用类型和常量保留在入口，不改数据库语句、随机调用顺序与评分公式。
- 2026-10-13 的 `cargo test -p tswn_ladder` 三项回归通过；使用正常 mimalloc 配置，未连接或修改真实数据库。

### 依赖更新

- `cron` 升级到 `0.17`；`Schedule::from_str` 与 `upcoming` 用法无需调整。

### 验证

- `cargo check -p tswn_ladder --all-targets` 通过（本机以临时关闭 mimalloc 的配置绕过
  `libmimalloc-sys` 的 `D8050` 环境故障）。

## [0.1.0] - 2026-09-30

### 新增

- 新增 `tswn_ladder` 天梯更新器，用于按 cron 周期从 PostgreSQL 读取活跃玩家、生成对局、执行 `tswn_core` 战斗并回写分数、排名、handicap 和历史记录。
- 新增 `DATABASE_URL` 必填环境变量，用于连接 PostgreSQL。
- 新增 `LADDER_CRON_SCHEDULE`、`LADDER_GROUPS`、`LADDER_RUN_ONCE`、`LADDER_RANK_LOG_DIR` 环境变量，分别控制调度周期、分组列表、单轮运行模式和 rank 日志输出目录。
- 新增 handicap 参与的匹配生成与 ELO 更新逻辑，支持扰动、衰减、稀疏持久化和对局质量统计。
- 新增运行时内存监控，超过内部限制时主动退出，方便由外部 supervisor 拉起。
- 新增 rank 日志输出，按分组追加记录每轮排序后的玩家 ID 列表。

### 变更

- 战斗执行迁移至 core 的主 Runtime `Runner`，适配旧执行器与对象模型删除后的初始化和判胜接口。

### 说明

- 该 crate 作为内部运维工具加入 workspace，本轮首次封板为 `0.1.0`，不进入默认聚合包。
