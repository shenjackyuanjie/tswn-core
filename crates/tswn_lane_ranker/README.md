# tswn_lane_ranker

内部 lane 排名与管理服务。使用 SQLite 保存名字组合、两两胜率、排名、校准结果和任务状态，通过 Axum 提供 Web 管理界面；不进入默认聚合分发包。

## 运行

从仓库根目录执行：

```powershell
cargo run -p tswn_lane_ranker
```

默认监听 `127.0.0.1:3000`，数据库为当前目录的 `lane_ranker.sqlite3`。可通过 `LANE_RANKER_BIND` 和 `LANE_RANKER_DB` 修改；启动会初始化数据库结构。

排名参数从 `LANE_WARMUP_ROUNDS`、`LANE_TOTAL_ROUNDS`、`LANE_WIN_RATE_SAMPLES`、`LANE_STICKINESS`、`LANE_OUTER_WORKERS` 和 `LANE_SKIP_ARCHIVED` 读取，默认值与处理方式见 [RankerConfig](src/ranker.rs)。内层胜率 worker 固定为 1；外层为 0 时使用动态分配，否则按指定 worker 数静态分块。

## 校准与靶子生成

Rust 侧负责读取已保存结果、补算缺失胜率、调用 Python 求解器并保存审计结果。嵌入的 [严格校准器](tools/strict_python_calibrator.py) 和 [靶子求解器](tools/target_milp_solver.py) 在独立工作目录执行，需要可用的 Python 环境与 NumPy、pandas、SciPy、scikit-learn；普通 Rust 单元测试不代表这两条完整数值流程已验收。

当前默认 Raw 校准池阈值为：单人 lane `48.0`，多人 lane `48.7`；Correct 假设替换槽位为：单人 `10`，多人 `5`。这些属于可调整的默认策略，维护时同步实际调用方和页面说明，不用只比较返回常量的测试冻结配置。

## 内部结构

- `db.rs` 保留连接与任务入口；`db/schema.rs` 管理迁移，`groups.rs` 管理组合/胜率/归档，`results.rs` 管理进度、排名与 Correct 靶子轨迹。
- `ranker.rs` 负责配置和重新计算编排；`ranker/algorithm.rs` 负责 Raw/Golden 迭代和重建，`archive.rs` 负责归档规则，`rates.rs` 负责胜率矩阵和补算。
- `pairwise.rs` 编排校准；`pairwise/` 分离 Rust Correct 计算、Python 结果解析和胜率准备。
- `service.rs` 保留服务对象和队列入口；`service/` 分离组合管理、后台任务、靶子生成与压缩协议。HTTP 路由和页面仍在 `web.rs`、`static/`。

## 验证

```powershell
cargo test -p tswn_lane_ranker
node --check crates/tswn_lane_ranker/static/app.js
cargo +nightly fmt -p tswn_lane_ranker --check
```

单元测试验证组合解析、SQLite 结果/归档管理、Raw/Golden 重建和胜率补算范围。接入真实数据库、Python 数值工具和服务页面时还需对应的端到端验证。
