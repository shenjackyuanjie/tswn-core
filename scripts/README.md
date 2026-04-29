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
- `uv run scripts/build_all.py --bundle-name tswn_core_x_y_z_capi_a_b_c_py_m_n_k_wasm_p_q_r_bundle`

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

默认输出位置：

- `crates/tswn_wasm/dist/wasm/`

结果目录通常包含：

- `pkg/`: `wasm-bindgen` 生成的 JS glue 与 `.wasm`
- `raw/`: cargo 原始 `.wasm` 产物
- `examples/`: demo 页面与 README

说明：

- 该脚本依赖本机已安装 `wasm-bindgen-cli`
- 默认使用 `--target web`，方便直接服务静态页面 demo

## build_capi.py

构建 `tswn_capi`，并整理出可分发目录。

典型用法：

- `uv run scripts/build_capi.py --release`
- `uv run scripts/build_capi.py --release --clean`
- `uv run scripts/build_capi.py --release --with-example-build`

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

默认输出位置：

- `crates/tswn_py/dist/`

说明：

- 该脚本用于构建 Python wheel
- 多平台 / 多环境产物可以共同放在 `crates/tswn_py/dist/` 下
- 聚合打包脚本会直接收集这里已有的内容
- 脚本本身已内建 `uv` 支持（自动安装 `build` 包等）

## gen_test_case.py

生成测试 case 的辅助脚本。

## 关于 uv

仓库使用 `uv` 管理 Python 虚拟环境：

- `.venv`（Windows 侧）由 `uv` 创建，`uv run` 会自动使用该环境
- `.venv-wsl`（WSL 侧）是独立的 Linux 虚拟环境
- 所有 `python scripts/...` 命令均可替换为 `uv run scripts/...`（Windows 侧推荐）