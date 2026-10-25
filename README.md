# tswn-core

`tswn-core` 是名字竞技场的 Rust 实现仓库。当前重点是把核心战斗逻辑、名字解析、技能系统、评分/胜率模拟、图标渲染和跨语言绑定集中维护在一个 Cargo workspace 里，并用差分工具持续对齐旧实现行为。

项目起源与原始实现研究见 [历史资料](docs/archive/README.md)；作者的早期随笔继续保存在 [旧 README](docs/archive/legacy-readme.md)。开发约定统一维护在 [AGENTS.md](AGENTS.md)。

## 当前状态

- 核心 crate `tswn_core` 已经是当前主要实现，包含玩家构建、技能系统、战斗 runner、评分/胜率、RC4、图标渲染和 CLI。
- CLI 已覆盖普通对战、JSONL 流式日志、runtime diff 输出、benchmark、图标导出、DIY/OL overlay 导出等常用入口。
- 仓库内同时维护 Python、WASM、C ABI、Openbox GUI/CLI，以及 `ds4` 数据处理与 `tswn-pwp` 胜率数据集/特征编码工具。
- 当前仍处在“行为兼容 + 差分追踪 + 性能整理”的开发状态，不是只提供稳定 SDK 的纯发布仓库。
- `tests/`、`docs/diff/`、`tests/diff/`、`target/ts_diff_cases*` 一类目录主要服务于与旧 JS/TS 行为的差分定位。

## Workspace 结构

```text
crates/
  tswn_core/         核心库和主要 CLI/binary
  tswn_py/           Python 扩展绑定，基于 PyO3
  tswn_wasm/         WebAssembly 绑定和浏览器示例
  tswn_capi/         C ABI 动态库/静态库导出
  tswn_test/         跨 crate 集成测试与 golden/corpus 基线
  tswn_openbox/      图形界面工具与 openbox-cli
  tswn_openbox_backend/  GUI/CLI 共用后端与预设资源
  tswn_lane_ranker/  lane 排名工具
  tswn_ladder/       天梯工具
  tswn_pwp/          胜率数据集、校准与离线训练包（binary `tswn-pwp`）
  tswn_ds4/          ds4 相关数据处理和兼容实验

docs/          架构、DIY、差分、性能、构建和更新记录
scripts/       构建脚本、差分辅助脚本、case 生成脚本
tests/         测试输入、差分记录和样例数据
assets/        资源文件
target/        Cargo 输出和本地差分产物
```

`crates/tswn_name_ranker` 在根 `Cargo.toml` 的 `exclude` 列表里：它依赖独立校准数据，
不参与主 workspace 构建。成员清单与顺序以根 `Cargo.toml` 为准。

## 主要命令

构建前准备 Rust/Cargo、Python 3.12+ 和对应平台工具链。当前全功能 workspace 的依赖要求 Rust 至少 1.95；本轮使用 Rust 1.99，未单独验证 1.95 的完整构建。Python wheel、WASM CLI 版本匹配和 GUI 系统依赖见 [全量构建指南](docs/guides/build-all.md#1-构建范围与前提)。

构建和检查：

```powershell
cargo check --workspace
cargo test -p tswn_core
cargo clippy --workspace --all-targets
cargo +nightly fmt --check
# 线上/发布口径（唯一允许部署的构建）
cargo build -p tswn_core --bin tswn-cli --release --features "no_debug,mimalloc_alloc"
```

说明：`debug` / `release-fast` 只用于本地开发与测试，**不要部署**；线上统一
`--release` 且启用 `no_debug,mimalloc_alloc`。`tswn_core` 默认 feature 已包含
`mimalloc_alloc`，但命令里仍显式写出，避免默认 feature 变化时口径漂移。

## 编译口径（部署/线上）

所有部署到服务器的 tswn-core 产物必须用 `--release`，并启用
`no_debug,mimalloc_alloc`：

| 产物 | 命令 |
| --- | --- |
| `tswn-cli` | `cargo build --release -p tswn_core --bin tswn-cli --features "no_debug,mimalloc_alloc"` |
| `tswn_openbox` GUI | `cargo build --release -p tswn_openbox --bin tswn_openbox --features "no_debug,mimalloc_alloc"` |
| `openbox-cli` | `cargo build --release -p tswn_openbox --no-default-features --features "no_debug,mimalloc_alloc" --bin openbox-cli` |
| `tswn_ladder` | `cargo build --release -p tswn_ladder --features "no_debug,mimalloc_alloc"` |
| `tswn_py` | `cargo build --release -p tswn_py --features "no_debug,mimalloc_alloc"` |

`tswn_py` 的依赖已固定启用 `tswn_core` 的这两个 feature，同时提供同名转发
feature，所以统一命令可以直接用于它。不要用 `target/debug` / `target/release-fast`
的二进制部署；部署前检查 `target/release` 的时间戳，并确认对应 fingerprint 的
features 含 `no_debug` 与 `mimalloc_alloc`。

运行主 CLI：

```powershell
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- fight --jsonl -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime diff -f input.txt
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire"
cargo run -p tswn_core --bin tswn-cli -- bench win-rate -f teams.txt -n 10000
```

普通战斗与绑定复用 `BattleSession`，诊断和评分使用主 Runtime；CLI 不提供执行器选择器。DIY/OL、带权靶子、批量筛选、pair 和技能榜的详细参数只在 [core README](crates/tswn_core/README.md) 维护。

主 Runtime release 回归：

```powershell
python scripts/check_runtime_release.py --corpus
```

该入口运行 release/no_debug 主 Runtime 与 release CLI 测试；`--corpus` 会执行完整冻结 corpus。旧路径、已删除 API 与源码写法的维护约束见 [迁移指南](docs/guides/runtime-0.5-migration.md)，不通过源码扫描或固定测试数量冻结实现。

## 重要入口

- `tswn-cli`: 日常调试和用户入口。
- Openbox GUI：`cargo run -p tswn_openbox --bin tswn_openbox`。
- Openbox 无头 CLI：`cargo run -p tswn_openbox --no-default-features --features mimalloc_alloc --bin openbox-cli -- --help`。
- `python track.py test`: 主 Runtime corpus 跟踪器的短命令转发入口。
- `track_test.py`: release corpus 的测试失败集 checkpoint 工具，默认运行完整 corpus；实际范围由当前测试目标决定。

## 文档入口

- [全仓库更新日志](CHANGELOG.md)：跨组件更新汇总，各 crate 继续维护自身历史。
- [2026-10 维护报告](docs/maintenance-2026-10.md)：本轮审查覆盖、实现与统一验收；[文档专项全量清单](docs/reviews/2026-10-maintenance-doc-audit.md) 列出 148 份原 Markdown 和脚本覆盖。

- [项目与 core 原理指南](docs/core-guide/README.md)：面向熟悉 namerena 的读者，图文介绍角色构建、Runtime、hook、状态、语言接口及全项目工具链。
- [Cargo doc 阅读入口](docs/core-guide/README.md#在本机看-api-文档)：按类型、方法和字段浏览 API，并运行文档示例。
- [开发入口与模块边界](docs/guides/development.md)：代码归属、测试和提交范围。
- [文档中心](docs/README.md)：按用途浏览指南、接口参考、设计规格、机制分析、性能、版本记录与历史归档。
- [公共 API 与跨语言契约](docs/reference/public-api.md)：BattleSession、快照、回放与错误模型。
- [脚本与验证入口](scripts/README.md)：组件构建、聚合打包与跨语言验证。
- [`docs/guides/runtime-0.5-migration.md`](docs/guides/runtime-0.5-migration.md): 0.5.0 主 Runtime 迁移指南。
- [`docs/archive/dart-architecture.md`](docs/archive/dart-architecture.md): 原始 Dart 实现的历史架构说明。
- [`docs/reference/diy-overlay.md`](docs/reference/diy-overlay.md): DIY/OL overlay 相关说明。
- [`docs/archive/project-origins.md`](docs/archive/project-origins.md): 项目起源与重写背景。
- [`docs/guides/diy-validation.md`](docs/guides/diy-validation.md): DIY 验证流程。
- [`docs/guides/build-all.md`](docs/guides/build-all.md): 多产物构建说明。
- [`docs/perf/benchmark-history.md`](docs/perf/benchmark-history.md): 性能追踪。
- [`docs/perf/guides/fixed30-benchmark.md`](docs/perf/guides/fixed30-benchmark.md): 固定 30-case 性能回归口径。
- [`docs/perf/guides/amd-uprof.md`](docs/perf/guides/amd-uprof.md): Windows / Zen CPU 函数级采样与 agent 报告读取。
- [`crates/tswn_core/README.md`](crates/tswn_core/README.md): core crate 说明。
- [`crates/tswn_py/README.md`](crates/tswn_py/README.md): Python 绑定说明。
- [`crates/tswn_wasm/README.md`](crates/tswn_wasm/README.md): WASM 绑定说明。
- [`crates/tswn_capi/README.md`](crates/tswn_capi/README.md): C API 说明。
- [Openbox 使用说明](crates/tswn_openbox/README.md)：GUI、无头 CLI 与共享后端。
- [lane 排名服务](crates/tswn_lane_ranker/README.md) 与 [天梯更新器](crates/tswn_ladder/README.md)：数据库、数值工具和验证边界。
- [独立单号排名服务](crates/tswn_name_ranker/README.md)：校准资源、独立构建与 HTTP 配置。
- [DS4 批处理](crates/tswn_ds4/README.md)：单人/配对评分、历史缓存与外部 ABCP5。
- [胜率数据集与离线特征编码](crates/tswn_pwp/README.md)：`tswn-pwp` 生成、校验、校准和编码入口。

## 开发注意事项

- Rust edition 使用 2024；格式化必须使用 `cargo +nightly fmt`，以支持仓库的 nightly rustfmt 配置。
- `no_debug` feature 用于 release/绑定场景，避免调试路径影响性能和输出。
- `png_render` 是 `tswn_core` 默认 feature，用于图标 PNG/base64 输出。
- 差分工具会大量写入 `target/`，这些产物通常不应提交。
- 修改 Markdown 文档后，需要检查相对链接与 `git diff --check`。
- 主 Runtime 的 release 行为回归统一由 `scripts/check_runtime_release.py` 执行；`track test` 仅用于维护测试失败集 checkpoint。
- 当前工作区可能有本地调试文件和未提交产物，提交前需要用 `git status` 明确区分源码改动与生成输出。
