# Agent 工作准则

本文件记录在本仓库工作时必须遵守的约定，供人类与 AI 协作使用。

## 代码格式（rustfmt）

- 格式检查与格式化必须使用 nightly toolchain：`cargo +nightly fmt`（检查用 `cargo +nightly fmt --check`）。
- 原因：`rustfmt.toml` 启用了 `fn_single_line`、`unstable_features` 等 nightly 专属选项；
  仓库不再强制固定 toolchain（默认走 stable），stable 的 rustfmt 不支持这些配置，
  直接跑 `cargo fmt` 会产生大量与配置无关的伪差异，且可能按错误配置改写代码。
- 提交前请确保 `cargo +nightly fmt --check` 无输出。
