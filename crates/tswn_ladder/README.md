# tswn_ladder

内部天梯更新器：从 PostgreSQL 读取玩家，生成对局并运行 core 战斗，更新 ELO、handicap、排名与历史记录。不进入默认聚合分发包。

## 构建（线上口径）

```bash
cargo build --release -p tswn_ladder --features "no_debug,mimalloc_alloc"
```

线上服务使用 `target/release/tswn_ladder`；不要用 debug 或 release-fast 产物部署。

## 运行

先配置环境变量，再从仓库根目录执行 `cargo run --release -p tswn_ladder --features "no_debug,mimalloc_alloc"`。启动会连接指定数据库并执行更新，不是只读查询工具；调试时使用独立测试数据库。

| 环境变量 | 用途 |
| --- | --- |
| `DATABASE_URL` | 必填，PostgreSQL 连接串；不要写入仓库 |
| `LADDER_CRON_SCHEDULE` | 默认每分钟；接受五字段 cron，解析时补秒字段 |
| `LADDER_GROUPS` | 逗号分隔分组，默认 `1a,1b,2a,2b,3a` |
| `LADDER_RUN_ONCE` | 开启后只执行一轮，否则按 cron 持续运行 |
| `LADDER_RANK_LOG_DIR` | rank 日志目录，默认当前工作目录 |

## 内部结构

- `main.rs`：进程入口、共用数据类型与评分常量。
- `config.rs`：环境配置、cron 解析与关闭信号。
- `updator.rs`：单轮/周期更新编排、内存监控和随机辅助。
- `matching.rs`：候选匹配、配额与预期分差。
- `battle.rs`：战斗执行、ELO/handicap 更新与错误记录。
- `storage.rs`：PostgreSQL 读取/回写及排名日志。

评分与匹配策略仍由原有参数控制；这次拆分不改变数据库语句、随机调用顺序或战斗行为。

## 验证

```powershell
cargo test -p tswn_ladder
cargo +nightly fmt -p tswn_ladder --check
```

单元测试覆盖 cron 与 core 胜者映射，不需要数据库。完整更新、事务回写和信号处理仍需在测试数据库中单独验收，不能把单元测试当作数据库端到端验证。
