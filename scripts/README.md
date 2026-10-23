# 脚本与验证入口

本目录维护组件构建、数据集分析、差分辅助与跨语言验证脚本。下面的命令默认从仓库根目录执行；各脚本的额外依赖和输入分别准备。

从仓库根目录运行时，推荐命令形式为 `uv run scripts/<name>.py ...`（Windows 侧使用 `uv` 管理环境），
也可用 `python scripts/<name>.py ...`（需确保已激活虚拟环境）。

## read_winprob_dataset.py

使用 `pyarrow` 读取战斗状态 Parquet，只向调用方提供 `state` 和 `winner_team_index`，默认排除空标签。
先用 `tswn-pwp validate` 校验完整性，再运行 `python scripts/read_winprob_dataset.py target/winprob-demo`。
生成命令和数据契约见 [生成器说明](../crates/tswn_pwp/README.md)。

## read_encoded_batches.py

校验 `tswn-pwp encode` 生成的版本 2 训练包，包括 manifest、载荷长度/SHA-256、批布局、label/mask 语义与审计载荷；命令行检查仅用 Python 标准库：

```powershell
python scripts/read_encoded_batches.py target/encoded-train
```

训练代码调用 `iter_encoded_batches(root)` 时需 NumPy，逐批返回只读 memmap 的 `features` 和 `labels`。`extra_bits`、`order_key` 只在显式指定 `include_exact=True` 时进入 `exact`；审计数据不混入模型数组。流式迭代须消费到结尾才完成整包总数核对，全面验收使用 `load_export(root)`。契约、空间预算及失败恢复说明见 [PWP README](../crates/tswn_pwp/README.md#离线张量编码)。

## measure_encoder_capacity.py

按 [FeatureEncoder 规格](../docs/design/feature-encoder-spec.md) 第 4 节的计费公式，统计数据集每样本的容量峰值
（模板数、lane 总数、五类 lane list 条目、世界列表、状态、保护链、槽条目与 X 记录数），
用于冻结 profile 的 `H_max` / `L_max` / `Q_max` / `V_max` / `X_max`。只读，不修改数据集。

```powershell
python scripts/measure_encoder_capacity.py --dataset target/winprob-100k --out target/caps-100k.json
```

## winprob_hp_baseline.py

HP-only 胜率基线：只读 `samples.parquet`，把每支输入队伍的血量比例
（`sum(存活成员 hp) / sum(该队全部成员的 max_hp)`，分母含已阵亡成员）softmax 成概率，
按 split 与 progress 分桶报告 Log Loss／Brier／Top-1。不训练任何参数，用来确认数据集与标签可用，
并作为后续模型的对照下界；`progress` 只参与分桶，不参与打分。
基线定位见 [battle-analyze.md](../docs/design/battle-analyze.md)。

```powershell
python scripts/winprob_hp_baseline.py --dataset target/winprob-100k --json-out target/hp-baseline.json
```

`--limit N` 可只统计前 N 个样本，便于快速冒烟。

## check_runtime_release.py

运行主 Runtime 的 release 行为回归：

- 运行 release 与 `no_debug` 的主 Runtime 定向测试；
- 运行 release CLI 的 Runtime 测试；
- 传入 `--corpus` 时执行完整 corpus，否则只编译 corpus 测试目标。

已删除入口与架构约定见 [Runtime 迁移指南](../docs/guides/runtime-0.5-migration.md)；脚本不检查源码路径、禁用字符串或固定测试数量。

```powershell
python scripts/check_runtime_release.py
python scripts/check_runtime_release.py --corpus
```

## build_all.py

一次性聚合打包以下内容，并生成最终 zip：

- `capi`: 现场构建并整理分发目录
- `cli`: 现场构建并整理可执行文件
- `openbox`: 现场构建 GUI 与 `openbox-cli`，两份可执行文件均收录到聚合 ZIP
- `py`: 只收集当前 `tswn_py` 版本的现有 wheel / sdist 及其他包文件，不现场构建
- `wasm`: 现场构建 `tswn_wasm`，并整理浏览器可直接消费的 `wasm-bindgen` 包

典型用法：

- `uv run scripts/build_all.py --release`
- `uv run scripts/build_all.py --release --clean`
- `uv run scripts/build_all.py --bundle-name my_custom_bundle`

常用参数：

- `-o DIR` / `--output-dir DIR`: 输出目录（默认 `dist/all`）
- `--bundle-name NAME`: 自定义 bundle 目录名与 zip 基名（默认按 core/capi/py/wasm/openbox 版本自动生成）
- `--release`: 对 capi/cli/openbox/wasm 使用 release 构建
- `--clean`: 构建前清空 bundle 目录与最终 zip
- `--target TRIPLE`: 指定 capi/cli/openbox 的 Cargo target triple；WASM 保持自己的 target
- `--skip-capi` / `--skip-cli` / `--skip-openbox` / `--skip-py` / `--skip-wasm`: 跳过对应组件
- `--capi-with-example-build`: 传给 `build_capi.py`，额外尝试编译 C examples
- `--cli-features FEATURES`: CLI 追加 features，逗号分隔（默认 `no_debug,mimalloc_alloc`；传空字符串表示不追加）
- `--openbox-features FEATURES`: Openbox 追加 features（默认 `no_debug,mimalloc_alloc`）
- `--include-ohos-cli`: 额外构建未签名 OHOS CLI；SDK 与 target 准备见 [OpenHarmony 指南](../docs/guides/openharmony.md)
- `--pgo`: 对 CLI 与 Openbox 同时启用 PGO；当前 Openbox 训练路径待适配，参见下方 [PGO 状态](#pgo_buildpy)
- `--cargo ...`: 追加到 cargo/build_capi 的额外参数（放在最后）

默认输出位置：

- bundle 目录：`dist/all/<bundle_name>/`
- zip 文件：`dist/all/<bundle_name>.zip`

其中 bundle 名默认会按版本自动生成，类似：

- `tswn_core_x_y_z_capi_a_b_c_py_m_n_k_wasm_p_q_r_openbox_u_v_w_bundle`

打包结果中通常包含：

- `capi/`: 头文件、动态库、C examples
- `cli/`: 带版本号的 `tswn-cli` 可执行文件
- `openbox/`: 带版本号的 GUI 与 `openbox-cli` 可执行文件、包说明与 changelog
- `py/`: 当前版本的现有 wheel/sdist、其他包文件与 Python examples
- `wasm/`: `pkg/`、原始 `.wasm`、静态页面 examples 与 changelog

跨平台流程、两种 `--clean` 的范围和聚合验收见 [全量构建指南](../docs/guides/build-all.md)。

## pgo_build.py

CLI 的插桩、单线程训练、合并与 `profile-use` 重建入口：

```powershell
python scripts/pgo_build.py --kind cli
python scripts/pgo_build.py --kind cli --train-runs 8000
```

默认训练覆盖 fixed30 与 score；默认 CLI 工作目录为 `target/pgo/cli/`，合并结果为其中的 `merged.profdata`。`--skip-train` 用于复用已有 profile，不替代新输入/新编译配置的训练。`llvm-profdata` 的 LLVM 大版本必须与 rustc 一致，脚本会检查。

**Openbox 状态**：脚本的训练靶子仍指向迁移前的 `crates/tswn_openbox/assets/targets/`，实际资源在 `crates/tswn_openbox_backend/assets/targets/`。适配前不要把 `--kind openbox` 或包含 Openbox 的 `build_all.py --pgo` 当作可直接使用的发布流程；普通 `--release` 构建不受此限制。历史收益与当前 A/B 验证要求见 [性能准则](../docs/perf/guides/performance.md#pgo-构建)。

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

构建 `tswn_py` 的 wheel，需要 Python 3.12+；使用的解释器决定 wheel 的 Python/平台标签。详细依赖见 [Python 构建说明](../crates/tswn_py/README_BUILD.md)。

典型用法：

- `uv run scripts/build_py.py`
- `uv run scripts/build_py.py --clean`
- `uv run scripts/build_py.py --verify`
- `uv run scripts/build_py.py --no-isolation`

常用参数：

- `-o DIR` / `--output-dir DIR`: wheel 输出目录（默认 `crates/tswn_py/dist`）
- `--clean`: 构建前清空输出目录
- `--no-isolation`: 跳过 PEP 517 隔离环境，直接使用当前 Python 环境（更快；需已安装 setuptools、setuptools-rust、wheel）
- `--verify`: 构建完成后向当前 Python 环境强制安装 wheel 并验证 `import tswn_py`；使用独立虚拟环境执行

默认输出位置：

- `crates/tswn_py/dist/`

说明：

- 该脚本用于构建 Python wheel
- 多平台 / 多环境产物可以共同放在 `crates/tswn_py/dist/` 下；`--clean` 会清空整个目录，通常仅在第一轮执行
- 聚合脚本仅收集其中与当前 `tswn_py` 版本匹配的 wheel/sdist；同版本的旧 core/源码产物仍需重新构建或主动核验
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
- `to_diy(..., minions=True)` 的 BattleSession 往返初始状态一致性
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

三个子命令都需要 `psycopg`（即使只查看帮助也会导入），通过 `--dsn` 或环境变量 `TSWN_PG_DSN` 连接 PostgreSQL；默认 schema、table 和 senderId 保持原来的值。使用具备所需表读取权限的账号，连接信息仅在本地环境提供；不要把 DSN 写入命令示例或报告。

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

## 其他差分与测量脚本

- `benchmark_md5_runtime.cjs`：使用外部 `md5.js` 测量 fixed/win-rate/score/matrix；参数由脚本解析，不提供 `--help`。固定输入、线程和 `eval_rq` 后再比较，历史口径见 [四方报告](../docs/perf/reports/runtime-four-way-2026-07-15.md)。
- `bun_score_trace.js`：与 `bun_profile_trace.js` 一样接收 `--input-file`、`--rounds`、`--md5`，但采集 score 的逐轮结果；补丁依赖外部 JS 产物的源码形态，不是通用 JS 调试器。
- `ohos-env.ps1` / `ohos-env.sh`：由当前 shell 点导入以设置交叉编译环境；需要已有 SDK，不是自动安装器，见 [OpenHarmony 指南](../docs/guides/openharmony.md)。
- `test_build_all.py` / `test_read_encoded_batches.py`：脚本单元测试，覆盖打包内容与真实编码文件读取；NumPy 数组测试需要 NumPy。运行方式见 [开发指南](../docs/guides/development.md#验证怎么选)。
- `_tswn_diff/` 是 `tswn_diff.py` 的内部实现，统一通过顶层入口调用，不直接执行包内模块。

## 关于 uv

仓库使用 `uv` 管理 Python 虚拟环境：

- `.venv`（Windows 侧）由 `uv` 创建，`uv run` 会自动使用该环境
- `.venv-wsl`（WSL 侧）是独立的 Linux 虚拟环境
- 所有 `python scripts/...` 命令均可替换为 `uv run scripts/...`（Windows 侧推荐）

## BattleSession 验收

- `verify_py_cli_api.py`：构建 Python 扩展并检查 TypedDict、迭代器、错误码与 session/replay 一致性。
- `verify_wasm_battle.test.mjs`：真实 Node WASM 包的 canonical DTO、错误与旧 FightSession 兼容测试。
- Rust 集成测试 [`cli_battle.rs`](../crates/tswn_core/tests/cli_battle.rs)：CLI JSONL、stdin、人类输出与输入错误路径，运行 `cargo test -p tswn_core --test cli_battle`。
- `verify_battle_cross_binding.py`：真实 Rust CLI / Python / C / WASM 的完整 payload 精确对比；调用 `dump_battle_wasm.mjs` 读取 Node WASM 输出。
- `verify_web_playback.mjs`：真实页面模块与 DOM 的延迟 source 测试，需 `--experimental-vm-modules` 和 target/web-test-tools 下的 linkedom。
- `benchmark_web_streaming.mjs`：独立桌面浏览器四组各 20 次性能测试，生成 timing JSON、表格和截图。
- `benchmark_py_battle_session.py`：构建本地 release Python 扩展，测量 `BattleSession.next_frame()` 的端到端 DTO 开销；传入 Node WASM 包时同时按相同输入比较两端结果。基线与复现口径见 [Python BattleSession DTO 转换基线](../docs/perf/reports/python-battle-session-baseline.md)。
- `verify_battle_docs.py`：从公共 API 文档提取 Rust / Python / WASM 示例并实际运行。

这些验证脚本不统一提供命令行帮助：`verify_battle_docs.py`、`verify_battle_cross_binding.py` 直接执行验证，不接受 `--help`；`dump_battle_wasm.mjs` 与 `benchmark_wasm_battle_session.mjs` 从 stdin 接收 JSON，供上游脚本调用。Node 包默认位于 `target/battle_wasm_node/`，跨绑定桥接还支持 `TSWN_WASM_NODE_PACKAGE` 指定包文件。跨绑定脚本当前只自动定位 Windows DLL 和 Linux SO，macOS 的动态库路径需要适配。

浏览器性能脚本还需要 Playwright 和可用的 Edge；DOM 验证需要 linkedom，均安装到独立的 `target/web-test-tools/`。完整构建与复现步骤见 [Web streaming 基线](../docs/perf/reports/web-streaming-baseline.md)；其中测量日期、旧工具版本和性能数字按历史记录阅读，重新构建 WASM 时始终匹配当前 Cargo.lock。
