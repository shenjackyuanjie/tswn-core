# tswn-core

`tswn-core` 是名字竞技场的 Rust 实现仓库。当前重点是把核心战斗逻辑、名字解析、技能系统、评分/胜率模拟、图标渲染和跨语言绑定集中维护在一个 Cargo workspace 里，并用差分工具持续对齐旧实现行为。

旧 README 已保留为 [`README.old.md`](README.old.md)。

## 当前状态

- 核心 crate `tswn_core` 已经是当前主要实现，包含玩家构建、技能系统、战斗 runner、评分/胜率、RC4、图标渲染和 CLI。
- CLI 已覆盖普通对战、JSONL 流式日志、runtime diff 输出、benchmark、图标导出、DIY/OL overlay 导出等常用入口。
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

说明：`--release` 是正式 benchmark/发版口径；日常需要更快的优化构建时可以用 `--profile release-fast`。`tswn_core 0.5.0` 的原生默认 feature 已包含 `mimalloc_alloc`，正式 benchmark 与发布构建都保持默认 allocator；显式关闭默认 feature 的结果必须作为独立口径记录。

运行主 CLI：

```powershell
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- fight --jsonl -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime diff -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime normalized-run -f input.txt --max-rounds 20000
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire"
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire" --old
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt -o diy.txt
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt --minions -o diy.txt
cargo run -p tswn_core --bin tswn-cli -- bench win-rate -r "mario+luigi\npeach+bowser" -n 10000
cargo run -p tswn_core --bin tswn-cli -- bench win-rate -f teams.txt --double-plus --keep-rq
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt --min-screen 60
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o out.txt --min-file 65
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o out.jsonl --log
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt -o names.txt --pure
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l targets.txt -p players.txt --wr-precision 5
cargo run -p tswn_core --bin tswn-cli -- bench batch-rate -l weighted-targets.toml -p players.txt --target-factored
cargo run -p tswn_core --bin tswn-cli -- bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3
cargo run -p tswn_core --bin tswn-cli -- bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o pair.txt --min-file 250
cargo run -p tswn_core --bin tswn-cli -- bench pair -l weighted-targets.toml -p players.txt --teammate-list teammates.txt --head 3 --target-factored
```

`fight` / `fight --jsonl` 使用正式 `BattleSession`；诊断使用 `runtime diff` / `runtime normalized-run`，评分和胜率使用明确的 `bench` 子命令。CLI 不再提供执行器选择器或 parity 子命令；C、Python 与 WASM 绑定也都从同一主 Runtime 会话读取完成态、快照、RC4、胜者与回放。

`to-diy --minions` 会在 `+ol` 输出中附带可生成的 shadow / summon / zombie 模板，用于更接近原始名字的评分与对战行为。OL/DIY 的 `attrs` 都使用前七围 +36、HP 原样的编码；使魔模板的 `skills` 使用普通 JSON object 格式，两个火球固定命名为 `sklfire1`、`sklfire2`，自爆命名为 `sklexplode`，字段顺序就是行动顺序。0 熟练度技能会省略输出；解析时未带前缀的 `summon.skills` 只接受这三个 `skl` 槽位名，不再支持旧数组格式、`skill_order` 字段或旧的 `sklfire` 别名。

OL 召唤物模板可以继续嵌套 `shadow` / `summon` / `zombie` 子模板，用来配置“召唤物的召唤物”。如果要给使魔模板配置普通玩家技能，需要写 `normal:` 前缀，例如 `{"normal:sklsummon":255,"sklfire1":9}`；普通玩家技能、使魔固定技能和幻影附体会分别保留独立编号通道，吞噬时不会互相串槽。使魔召唤出的子使魔会按直接来源链路传导伤害；使魔分身仍按 root owner 命名/随主人清理，但伤害分摊会直接传到主名字。

`bench win-rate` 使用两行文本输入两队：两队之间用 `\n` 分隔，队内默认用 `+` 分隔；传入 `--double-plus` 时队内分隔符改为 `++`，方便保留名字里的 `+diy[...]` / `+ol:...`。`--keep-rq` 只切换玩家构造用的 rq，胜率模拟的 seed 仍固定使用 JS ProfileWinChance 口径：第 0 场无 seed，后续为 `seed:(33554431 + i)@!`。

`bench pair` 会先把 `player-list` 中非 DIY/OL 的名字转换为默认 `+ol` 格式，再与 `teammate-list` 中每个队友组合组成对局。两个文件均按每行一个组合处理：选手默认使用单个 `+` 分隔成员，可用 `--player-list-double-plus` 改为 `++`；队友默认使用 `++`，可用 `--teammate-list-single-plus` 改为单个 `+`。带权靶子使用 `--target-factored` 读取 `[[targets]]` TOML，并按 `sum(胜率 * factor) / sum(factor)` 计算每个队友组合的平均值。

例如，`players.txt`（默认 `+`）可以写成：

```text
a@team+b@team
```

`teammates.txt`（默认 `++`）可以写成：

```text
c@team++d@team
```

`bench batch-rate` 也支持 `--target-factored`；不加该选项时保持普通文本靶子的等权平均行为。

主 Runtime release 回归：

```powershell
python scripts/check_runtime_release.py --corpus
```

该门禁先检查旧对象路径、Rust/CLI 禁用符号和 corpus 清单，再运行 release/no_debug 主 Runtime 测试；`--corpus` 会执行 87 个 JS exact trace 与 37 个冻结压力 golden，共 124 项。

## 重要入口

- `tswn-cli`: 日常调试和用户入口。
- `python track.py test`: 主 Runtime corpus 跟踪器的短命令转发入口。
- `track_test.py`: release corpus 的测试失败集 checkpoint 工具，默认运行完整 124 项。

## 文档入口

- [文档中心](docs/README.md)：按用途浏览指南、接口参考、设计规格、机制分析、性能、版本记录与历史归档。
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

## 开发注意事项

- Rust edition 使用 2024；格式化必须使用 `cargo +nightly fmt`，以支持仓库的 nightly rustfmt 配置。
- `no_debug` feature 用于 release/绑定场景，避免调试路径影响性能和输出。
- `png_render` 是 `tswn_core` 默认 feature，用于图标 PNG/base64 输出。
- 差分工具会大量写入 `target/`，这些产物通常不应提交。
- 修改 Markdown 文档后，需要检查相对链接与 `git diff --check`。
- 主 Runtime 独立性与 124-case release 回归统一由 `scripts/check_runtime_release.py` 执行；`track test` 仅用于维护测试失败集 checkpoint。
- 当前工作区可能有本地调试文件和未提交产物，提交前需要用 `git status` 明确区分源码改动与生成输出。
