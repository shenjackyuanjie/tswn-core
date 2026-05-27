# tswn-core

`tswn-core` 是名字竞技场的 Rust 实现仓库。当前重点是把核心战斗逻辑、名字解析、技能系统、评分/胜率模拟、图标渲染和跨语言绑定集中维护在一个 Cargo workspace 里，并用差分工具持续对齐旧实现行为。

旧 README 已保留为 [`README.old.md`](README.old.md)。

## 当前状态

- 核心 crate `tswn_core` 已经是当前主要实现，包含玩家构建、技能系统、战斗 runner、评分/胜率、RC4、图标渲染和 CLI。
- CLI 已覆盖普通对战、raw 对战日志、diff 输出、benchmark、图标导出、DIY/OL overlay 导出等常用入口。
- 仓库内同时维护 Python、WASM、C ABI、以及 `ds3` 相关实验/工具 crate。
- 当前仍处在“行为兼容 + 差分追踪 + 性能整理”的开发状态，不是只提供稳定 SDK 的纯发布仓库。
- `tests/`、`docs/diff/`、`tests/diff/`、`target/ts_diff_cases*` 一类目录主要服务于与旧 JS/TS 行为的差分定位。

## Workspace 结构

```text
crates/
  tswn_core/   核心库和主要 CLI/binary
  tswn_py/     Python 扩展绑定，基于 PyO3
  tswn_wasm/   WebAssembly 绑定和浏览器示例
  tswn_capi/   C ABI 动态库/静态库导出
  tswn_ds3/    ds3 相关数据处理和兼容实验

docs/          架构、DIY、差分、性能、构建和更新记录
scripts/       构建脚本、差分辅助脚本、case 生成脚本
tests/         测试输入、差分记录和样例数据
assets/        资源文件
target/        Cargo 输出和本地差分产物
```

## 主要命令

构建和检查：

```powershell
cargo check
cargo build
cargo test -p tswn_core
cargo build --release --features no_debug,mimalloc_alloc
cargo build --profile release-fast --features no_debug
```

说明：`--release` 是正式 benchmark/发版口径；日常需要更快的优化构建时可以用 `--profile release-fast`。benchmark 不启用 `mimalloc_alloc`，最终 release 构建再启用。

运行主 CLI：

```powershell
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- fight --out-raw -f input.txt
cargo run -p tswn_core --bin tswn-cli -- raw -f input.txt
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire"
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire" --old
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt -o diy.txt
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt --minions -o diy.txt
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt --min-screen 60
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o out.txt --min-file 65
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o out.jsonl --log
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o names.txt --pure
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt --wr-precision 5
cargo run -p tswn_core --bin tswn-cli -- bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3
cargo run -p tswn_core --bin tswn-cli -- bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o pair.txt --min-file 250
```

`to-diy --minions` 会在 `+ol` 输出中附带可生成的 shadow / summon / zombie 模板，用于更接近原始名字的评分与对战行为。使魔模板的 `skills` 使用普通 JSON object 格式；两个火球固定命名为 `sklfire1`、`sklfire2`，自爆命名为 `sklexplode`，字段顺序就是行动顺序，不再使用旧数组格式或 `skill_order` 字段。

`bench pair` 会先把 `player-list` 中非 DIY/OL 的名字转换为默认 `+ol` 格式，再与 `teammate-list` 中每个队友组成二人组。它会对每个二人组计算一次 batch rate，并把最高的 `--head <N>` 个 batch rate 求和作为该选手的最终分数；`player-list` 和 `teammate-list` 都是每行一个名字。

常用差分 case miner：

```powershell
cargo run --release --features no_debug --bin tswn_case_miner -- `
  --library .\tests\sqp6000.txt `
  --md5-tool .\md5.js `
  --out-dir .\target\ts_diff_cases `
  --modes 1v1,2v2,3v3v3,ffa `
  --ffa-sizes 4,6,8 `
  --case-offset-per-mode 0 `
  --max-cases-per-mode 4000 `
  --keep-going
```

DIY 往返验证：

```powershell
cargo run --release --features no_debug --bin track_diy_roundtrip -- `
  --library .\tests\sqp6000.txt `
  --out-dir .\target\diy_roundtrip `
  --modes 1v1,2v2,3v3v3,ffa `
  --ffa-sizes 4,6,8 `
  --case-offset-per-mode 0 `
  --max-cases-per-mode 4000 `
  --keep-going
```

## 重要 binary

- `tswn-cli`: 日常调试和用户入口。
- `tswn_case_miner`: 批量生成 case，运行 TS/JS 基准输出并与 Rust trace 对比。
- `track_diy_roundtrip`: 生成或读取 case，把玩家转成 DIY/OL overlay，再验证初始状态和可选战斗日志一致性。

## 文档入口

- [`docs/architecture.md`](docs/architecture.md): 当前架构说明。
- [`docs/DIY.md`](docs/DIY.md): DIY/OL overlay 相关说明。
- [`docs/howto/1-start.md`](docs/howto/1-start.md): 入门操作记录。
- [`docs/howto/diy_validation.md`](docs/howto/diy_validation.md): DIY 验证流程。
- [`docs/build_all.md`](docs/build_all.md): 多产物构建说明。
- [`docs/perf/benchmark_tracking.md`](docs/perf/benchmark_tracking.md): 性能追踪。
- [`docs/perf/fixed_cases_30_benchmark.md`](docs/perf/fixed_cases_30_benchmark.md): 固定 30-case 性能回归口径。
- [`crates/tswn_core/README.md`](crates/tswn_core/README.md): core crate 说明。
- [`crates/tswn_py/README.md`](crates/tswn_py/README.md): Python 绑定说明。
- [`crates/tswn_wasm/README.md`](crates/tswn_wasm/README.md): WASM 绑定说明。
- [`crates/tswn_capi/README.md`](crates/tswn_capi/README.md): C API 说明。

## 开发注意事项

- Rust edition 使用 2024，工具链由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定。
- `no_debug` feature 用于 release/绑定场景，避免调试路径影响性能和输出。
- `png_render` 是 `tswn_core` 默认 feature，用于图标 PNG/base64 输出。
- 差分工具会大量写入 `target/`，这些产物通常不应提交。
- 修改 Markdown 文档后，记得使用 `oxfmt docs` 格式化文档。
- 回归追踪入口已迁移到 Rust bin：`track`、`track_test`、`track_case_miner`、`track_diy_roundtrip`。
- 当前工作区可能有本地调试文件和未提交产物，提交前需要用 `git status` 明确区分源码改动与生成输出。
