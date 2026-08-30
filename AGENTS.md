# Repository Guidelines

## 项目结构与模块

这是一个 Rust workspace。核心战斗与 CLI 位于 `crates/tswn_core/`；跨语言与界面适配分别在 `tswn_capi`、`tswn_py`、`tswn_wasm`、`tswn_openbox`，排名与对战工具在 `tswn_*ranker`、`tswn_ladder`。共享或集成测试主要在 `crates/tswn_test/`，输入样例在 `tests/`，设计与构建说明在 `docs/`，自动化脚本在 `scripts/`。修改功能时优先将测试放在所属 crate 的模块内；跨 crate 行为放入 `tswn_test`。

## 构建、测试与开发

- `cargo build --workspace`：构建全部 Rust crate。
- `cargo test`：执行工作区测试；日常核心改动可用 `cargo test -p tswn_core`。
- `cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt`：以文本输入运行战斗 CLI。
- `cargo clippy --workspace --all-targets`：检查常见 Rust 问题。
- `cargo +nightly fmt --check`：提交前验证格式。

## 代码风格与命名

遵循 Rust 默认命名：模块、函数和文件用 `snake_case`，类型与 trait 用 `PascalCase`，常量用 `SCREAMING_SNAKE_CASE`。避免无关重排或大规模格式差异。格式检查与格式化**必须**使用 nightly：`cargo +nightly fmt`（检查用 `cargo +nightly fmt --check`）。`rustfmt.toml` 启用了 `fn_single_line`、`unstable_features` 等 nightly 专属选项；stable 的 `cargo fmt` 会产生伪差异且可能按错误配置改写代码。提交前确保该检查无输出。

## 测试要求

使用 Rust 内置测试框架（`#[test]`）。修复缺陷应先添加可复现测试；涉及 runtime 或解析语义时，覆盖边界条件与已有 case。提交前运行受影响 crate 的测试；影响核心逻辑时运行 `cargo test -p tswn_core`，跨模块变更再运行 `cargo test`。除非有意更新语义，不要重生成 `crates/tswn_test/cases/` 中的 golden/corpus 基线。

## 提交与 PR

近期历史采用 Conventional Commit 风格，如 `feat(openbox): 添加版本信息弹窗`、`docs(repo): 添加 Agent 工作约定`。使用 `feat`、`fix`、`docs`、`chore` 等类型和简短 scope。PR 应说明动机、关键实现与验证命令；关联 issue；若更改 Openbox 或 WASM 可见界面，附截图或录屏。避免混入无关格式化、构建产物或本地配置。
