# Scripts

仓库根目录下原本分散的 Python 辅助脚本统一收纳到这里。

从仓库根目录运行时，推荐命令形式为 `uv run scripts/<name>.py ...`（Windows 侧使用 `uv` 管理环境），
也可用 `python scripts/<name>.py ...`（需确保已激活虚拟环境）。

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

## gen_test_case.py

生成随机文本用作测试 case 的辅助脚本。

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

## find_bun_tswn_reply_mismatches.py

查找 bun / tswn 胜率不一致的消息，并回查回复的原始消息内容。

从 PostgreSQL 数据库中查询包含 bun 和 tswn 胜率结果的消息，比较两者胜率是否一致，筛出不一致的记录。

典型用法：

- `uv run scripts/find_bun_tswn_reply_mismatches.py --dsn postgresql://...`
- `uv run scripts/find_bun_tswn_reply_mismatches.py --json`

常用参数：

- `--dsn DSN`: PostgreSQL 连接串（默认读取环境变量 `TSWN_PG_DSN`）
- `--schema SCHEMA`: schema 名（默认 `eqq3695888`）
- `--table TABLE`: table 名（默认 `messages`）
- `--sender-id ID`: senderId 过滤值（默认 `45620725`）
- `--content-like PATTERN`: content LIKE 条件（默认 `最终胜率%tswn:%`）
- `--json`: 输出 JSON 格式
- `--output PATH`: 将结果写到指定文件

**--retest 模式**：对每个 mismatch 用当前 `tswn-cli` 重跑胜率，对比是否仍然不一致。

- `--retest`: 启用 retest 模式
- `--tswn-bin PATH`: tswn-cli 可执行文件路径（默认使用 `cargo run --release --bin tswn-cli --`）
- `--retest-rounds N`: 重测场数（默认取各 mismatch 的 bun_buckets 最大轮数）
- `--retest-timeout SEC`: 单次 tswn 调用超时秒数（默认 60）
- `--retest-only-diff`: 只显示重测后依然 diff != 0 的 case
- `--retest-buckets-step N`: 重测时启用分段胜率输出，每 N 场输出一次累积胜率（默认 1000；设为 0 禁用）

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
