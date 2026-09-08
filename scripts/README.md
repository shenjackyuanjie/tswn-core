# Scripts

仓库根目录下原本分散的 Python 辅助脚本统一收纳到这里。

从仓库根目录运行时，推荐命令形式为 `uv run scripts/<name>.py ...`（Windows 侧使用 `uv` 管理环境），
也可用 `python scripts/<name>.py ...`（需确保已激活虚拟环境）。

## read_winprob_dataset.py

使用 `pyarrow` 读取战斗状态 Parquet，只向调用方提供 `state` 和 `winner_team_index`，默认排除空标签。
先用 `tswn-winprob-dataset validate` 校验完整性，再运行 `python scripts/read_winprob_dataset.py target/winprob-demo`。
生成命令和数据契约见 [生成器说明](../crates/tswn_winprob_dataset/README.md)。

## check_runtime_release.py

验证主 Runtime 的 release 独立性与行为回归：

- 检查已删除的 `engine` / `player` 源码路径和旧 Rust/CLI API 没有回流；
- 校验 corpus 清单固定为 87 个 JS exact trace 与 37 个压力 golden；
- 运行 release 与 `no_debug` 的主 Runtime 定向测试；
- 传入 `--corpus` 时实际执行全部 124 项 corpus，否则只编译 corpus 测试目标。

```powershell
python scripts/check_runtime_release.py
python scripts/check_runtime_release.py --corpus
```

## build_all.py

一次性聚合打包以下内容，并生成最终 zip：

- `capi`: 现场构建并整理分发目录
- `cli`: 现场构建并整理可执行文件
- `py`: 只收集当前已经存在的 Python wheel / 产物，不现场构建
- `wasm`: 现场构建 `tswn_wasm`，并整理浏览器可直接消费的 `wasm-bindgen` 包

典型用法：

- `uv run scripts/build_all.py --release`
- `uv run scripts/build_all.py --release --clean`
- `uv run scripts/build_all.py --bundle-name my_custom_bundle`

常用参数：

- `-o DIR` / `--output-dir DIR`: 输出目录（默认 `dist/all`）
- `--bundle-name NAME`: 自定义 bundle 目录名与 zip 基名（默认按 core/capi/py/wasm 版本自动生成）
- `--release`: 对 capi/cli 使用 release 构建
- `--clean`: 构建前清空 bundle 目录与最终 zip
- `--target TRIPLE`: 指定 cargo target triple
- `--skip-capi` / `--skip-cli` / `--skip-py` / `--skip-wasm`: 跳过对应组件
- `--capi-with-example-build`: 传给 `build_capi.py`，额外尝试编译 C examples
- `--cli-features FEATURES`: CLI 构建 features，逗号分隔（默认 `no_debug`；传空字符串表示不追加）
- `--cargo ...`: 追加到 cargo/build_capi 的额外参数（放在最后）

默认输出位置：

- bundle 目录：`dist/all/<bundle_name>/`
- zip 文件：`dist/all/<bundle_name>.zip`

其中 bundle 名默认会按版本自动生成，类似：

- `tswn_core_x_y_z_capi_a_b_c_py_m_n_k_wasm_p_q_r_bundle`

打包结果中通常包含：

- `capi/`: 头文件、动态库、C examples
- `cli/`: 带版本号的 `tswn-cli` 可执行文件
- `py/`: 当前已有 wheel 与 Python examples
- `wasm/`: `pkg/`、原始 `.wasm`、静态页面 examples 与 changelog

## build_wasm.py

构建 `tswn_wasm`，并整理出浏览器可直接消费的分发目录。

典型用法：

- `uv run scripts/build_wasm.py`
- `uv run scripts/build_wasm.py --release`
- `uv run scripts/build_wasm.py --release --clean`

常用参数：

- `-o DIR` / `--output-dir DIR`: 输出目录（默认 `crates/tswn_wasm/dist/wasm`）
- `--release`: 使用 release 构建
- `--clean`: 构建前清空输出目录
- `--target TRIPLE`: cargo target triple（默认 `wasm32-unknown-unknown`）
- `--bindgen-target {web,bundler,no-modules}`: wasm-bindgen 生成目标（默认 `web`）
- `--out-name NAME`: wasm-bindgen 输出包名（默认 `tswn_wasm`）
- `--features FEATURES`: 传给 cargo 的 features（逗号分隔）
- `--no-default-features`: 传给 cargo 的 `--no-default-features`
- `--cargo ...`: 追加 cargo build 参数（放在最后）

默认输出位置：

- `crates/tswn_wasm/dist/wasm/`

结果目录通常包含：

- `pkg/`: `wasm-bindgen` 生成的 JS glue 与 `.wasm`
- `raw/`: cargo 原始 `.wasm` 产物
- `examples/`: demo 页面与 README
- `README.txt` / `MANIFEST.txt`

说明：

- 该脚本依赖本机已安装 `wasm-bindgen-cli`，且会检查版本与 `Cargo.lock` 中的 `wasm-bindgen` 是否一致
- 默认使用 `--target web`，方便直接服务静态页面 demo

## build_capi.py

构建 `tswn_capi`，并整理出可分发目录。

典型用法：

- `uv run scripts/build_capi.py --release`
- `uv run scripts/build_capi.py --release --clean`
- `uv run scripts/build_capi.py --release --with-example-build`

常用参数：

- `-o DIR` / `--output-dir DIR`: 输出目录（默认 `crates/tswn_capi/dist/capi`）
- `--release`: 使用 release 构建
- `--clean`: 构建前清空输出目录
- `--target TRIPLE`: cargo target triple（可选；不指定则使用默认 target）
- `--features FEATURES`: 传给 cargo 的 features（逗号分隔）
- `--no-default-features`: 传给 cargo 的 `--no-default-features`
- `--with-example-build`: 额外尝试编译 C examples 到 `output/examples/bin`
- `--cargo ...`: 追加 cargo build 参数（放在最后）

默认输出位置：

- `crates/tswn_capi/dist/capi/`

结果目录通常包含：

- `include/tswn_capi.h`
- `lib/` 下的动态库及伴生产物
- `examples/` 下的 C 示例源码
- `README.txt`
- `MANIFEST.txt`

## build_py.py

构建 `tswn_py` 的 wheel。

典型用法：

- `uv run scripts/build_py.py`
- `uv run scripts/build_py.py --clean`
- `uv run scripts/build_py.py --verify`
- `uv run scripts/build_py.py --no-isolation`

常用参数：

- `-o DIR` / `--output-dir DIR`: wheel 输出目录（默认 `crates/tswn_py/dist`）
- `--clean`: 构建前清空输出目录
- `--no-isolation`: 跳过 PEP 517 隔离环境，直接使用当前 Python 环境（更快；需已安装 setuptools、setuptools-rust、wheel）
- `--verify`: 构建完成后安装 wheel 并验证 `import tswn_py`

默认输出位置：

- `crates/tswn_py/dist/`

说明：

- 该脚本用于构建 Python wheel
- 多平台 / 多环境产物可以共同放在 `crates/tswn_py/dist/` 下
- 聚合打包脚本会直接收集这里已有的内容
- 脚本会自动检测并安装 `build` 包（通过 `uv pip install build`）

## verify_py_cli_api.py

验证 `tswn_py` 中与 `tswn-cli` 对齐的 Python helper。

典型用法：

- `python scripts/verify_py_cli_api.py`
- `python scripts/verify_py_cli_api.py --release`
- `python scripts/verify_py_cli_api.py --skip-build`

脚本会先构建本地 `tswn_py` 扩展，把扩展模块和 Python 包文件复制到 `target/py_cli_api_verify/import/`，再从该临时目录导入并运行验证，不会安装 wheel，也不会修改当前 Python 环境。

覆盖内容：

- `win_rate_summary` / `team_win_rate_summary` / `group_win_rate_summary` 与旧 `win_rate` / `group_win_rate` 的一致性
- `score`、`namer_pf`、`batch_rate`、`pair_rate` 的可复算关系、重复名跳过和结果字段
- `to_diy(..., minions=True)` 的 Runner roundtrip 初始状态一致性
- `icon_info` 与 PNG/RGBA helper 的基础结构一致性
- `parse_group_lines` 的 `+` / `++` 组解析行为

参数：

- `--release`: 构建并导入 release 产物
- `--skip-build`: 复用上一次生成的 `target/py_cli_api_verify/import/`，用于快速重跑断言

## tswn_diff.py

统一查询和复核历史 Bun / tswn 差异。旧的 `find_bun_tswn_*_mismatches.py` 脚本已删除；从仓库根目录运行：

```powershell
uv run scripts/tswn_diff.py rate --json
uv run scripts/tswn_diff.py pf --tswn-version 0.3.12
uv run scripts/tswn_diff.py round --case-id MESSAGE_ID
```

三个子命令均通过 `--dsn` 或环境变量 `TSWN_PG_DSN` 连接 PostgreSQL；默认 schema、table 和 senderId 保持原来的值。

| 子命令 | 用途 | 常用参数 |
| --- | --- | --- |
| `rate` | 查找 Bun / tswn 胜率不一致的消息并回查回复原文。 | `--json`、`--output PATH`、`--retest`、`--retest-rounds N` |
| `pf` | 查找 `/namer-pf` 的 pp/pd/qp/qd 评分差异。保留 `--mode new/old/all`，可处理旧版 bun-only 记录。 | `--mode all`、`--tswn-version VERSION`、`--dedup`、`--retest` |
| `round` | 调用 Bun trace 与当前 tswn，定位胜负发生分叉的具体 round。 | `--case-id ID`、`--rounds N`、`--md5-path PATH`、`--md5-fallback PATH` |

所有子命令的完整参数由 `python scripts/tswn_diff.py <rate|pf|round> --help` 查看。`round` 还需要 Bun 和可用的 `md5.js`；主 md5 路径失败时会使用 `--md5-fallback`。

## md5_winner_probe.cjs

Node 脚本，用于核对 legacy `md5.js` 的判胜语义：只在内存中对源码打补丁，在
`Grp.dj`（移出存活）与 `Grp.aZ`（复活 / 加入存活）里记录真实存活队伍数、`Q`
（`Engine.y.a.Q`，即 Rust 侧 `alive_group_count`）以及判胜时使用的比较值。

典型用法：

- `node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --case-dir crates/tswn_test/cases/runtime_stress`
- `node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --names tests/sqp5900.txt --battles 10000`

参数：

- 第一个位置参数：`md5.js` 路径（本仓库根目录的 `md5.js` 与 `fast-namerena/md5.js` 代码相同，只差版本号常量）
- `--case-dir DIR`：逐个跑目录下的 `*.txt` 对局输入
- `--names PATH`：从名字池按 2v2v2 抽样，配合 `--battles N` 指定局数（默认 2000）

输出为 JSON，包含清空次数、复活次数、`q_stale_after_wipe`、`q_eq_1_but_two_teams_alive`、
`winner_with_mismatched_counts` 与残留样本。结论与解读见 [判胜语义](../docs/mechanics/winner.md)。

## bun_profile_trace.js

Bun 脚本，用于对 tswn-md5 模块进行 profile trace。

向指定的 md5 模块传入一组名称列表，批量运行胜率回调并收集每轮结果。

典型用法：

- `bun scripts/bun_profile_trace.js --input-file names.txt --rounds 100 --md5 path/to/md5.js`

参数：

- `--input-file PATH`: 包含名称列表的文本文件（每行一个）
- `--rounds N`: 运行轮数
- `--md5 PATH`: tswn-md5 模块路径（脚本会对其做 patch 后加载）

输出为 JSON，包含 `win_count` 和 `raw_data`（每轮的 round/wins）。

## 关于 uv

仓库使用 `uv` 管理 Python 虚拟环境：

- `.venv`（Windows 侧）由 `uv` 创建，`uv run` 会自动使用该环境
- `.venv-wsl`（WSL 侧）是独立的 Linux 虚拟环境
- 所有 `python scripts/...` 命令均可替换为 `uv run scripts/...`（Windows 侧推荐）

## BattleSession 验收

- `verify_py_cli_api.py`：构建 Python 扩展并检查 TypedDict、迭代器、错误码与 session/replay 一致性。
- `verify_wasm_battle.test.mjs`：真实 Node WASM 包的 canonical DTO、错误与旧 FightSession 兼容测试。
- `verify_cli_battle.py`：CLI JSONL、stdin、人类输出与删除命令的错误路径。已迁移为 Rust 集成测试 `crates/tswn_core/tests/cli_battle.rs`，运行 `cargo test -p tswn_core --test cli_battle`。
- `verify_battle_cross_binding.py`：真实 Rust CLI / Python / C / WASM 的完整 payload 精确对比；调用 `dump_battle_wasm.mjs` 读取 Node WASM 输出。
- `verify_web_playback.mjs`：真实页面模块与 DOM 的延迟 source 测试，需 `--experimental-vm-modules` 和 target/web-test-tools 下的 linkedom。
- `benchmark_web_streaming.mjs`：独立桌面浏览器四组各 20 次性能测试，生成 timing JSON、表格和截图。
- `benchmark_py_battle_session.py`：构建本地 release Python 扩展，测量 `BattleSession.next_frame()` 的端到端 DTO 开销；传入 Node WASM 包时同时按相同输入比较两端结果。基线与复现口径见 [Python BattleSession DTO 转换基线](../docs/perf/reports/python-battle-session-baseline.md)。
- `verify_battle_docs.py`：从公共 API 文档提取 Rust / Python / WASM 示例并实际运行。

完整构建与复现步骤见 [Web streaming 基线](../docs/perf/reports/web-streaming-baseline.md)。
