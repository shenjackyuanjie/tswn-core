# tswn_name_ranker

内部单号排名与管理服务。它使用独立 SQLite 保存任务和排名，通过 Axum 提供 HTTP 接口，并使用专用 ABCP 校准资源。

整体调用关系见 [项目地图](../../docs/core-guide/project-map.md)，与 core 和其他工具的数据交互见 [交互与工具](../../docs/core-guide/interfaces-and-tools.md)。

## 构建资源与入口

本 crate 由根 workspace 的 `exclude` 单独管理。构建时需要在本目录准备两份经过校准的资源：

- `abcp_directional_calibrator.json`
- `abcp_calibration_features.tsv`

它们由 [abcp_calibration.rs](src/abcp_calibration.rs) 在编译期读取。备齐资源后，从仓库根目录执行：

```powershell
cargo build --manifest-path crates/tswn_name_ranker/Cargo.toml
cargo test --manifest-path crates/tswn_name_ranker/Cargo.toml
cargo doc --manifest-path crates/tswn_name_ranker/Cargo.toml --no-deps
cargo run --manifest-path crates/tswn_name_ranker/Cargo.toml
```

## 服务配置

| 环境变量 | 默认值 | 用途 |
| --- | --- | --- |
| `NAME_RANKER_DB` | `name_ranker.sqlite3` | 独立 SQLite 文件 |
| `NAME_RANKER_BIND` | `127.0.0.1:3001` | HTTP 监听地址 |

启动和后台任务会初始化/更新数据库。使用独立测试库可观察排名和校准的完整过程；数据库文件与校准资源分别记录，便于复现同一口径。

## 内部结构

- `main.rs`：配置、数据库、服务对象和监听入口。
- `web.rs` / `service.rs`：HTTP 接口与业务编排。
- `ranker.rs` / `name_profile.rs`：排名和角色/profile 计算。
- `db.rs` / `model.rs`：持久化和业务数据。
- `abcp_calibration.rs`：专用校准资料的读取与数值转换。

此工具作为内部服务独立构建和部署；根 workspace 的文档、测试与聚合打包维护各自成员范围。
