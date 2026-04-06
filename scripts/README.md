# Scripts

仓库根目录下原本分散的 Python 辅助脚本统一收纳到这里。

从仓库根目录运行时，命令形式统一为 `python scripts/<name>.py ...`。

## build_all.py

一次性聚合打包以下内容，并生成最终 zip：

- `capi`: 现场构建并整理分发目录
- `cli`: 现场构建并整理可执行文件
- `py`: 只收集当前已经存在的 Python wheel / 产物，不现场构建

典型用法：

- `python scripts/build_all.py --release`
- `python scripts/build_all.py --release --clean`
- `python scripts/build_all.py --bundle-name tswn_core_x_y_z_capi_a_b_c_py_m_n_k_bundle`

默认输出位置：

- bundle 目录：`dist/all/<bundle_name>/`
- zip 文件：`dist/all/<bundle_name>.zip`

其中 bundle 名默认会按版本自动生成，类似：

- `tswn_core_x_y_z_capi_a_b_c_py_m_n_k_bundle`

打包结果中通常包含：

- `capi/`: 头文件、动态库、C examples
- `cli/`: 带版本号的 `tswn-cli` 可执行文件
- `py/`: 当前已有 wheel 与 Python examples

## build_capi.py

构建 `tswn_capi`，并整理出可分发目录。

典型用法：

- `python scripts/build_capi.py --release`
- `python scripts/build_capi.py --release --clean`
- `python scripts/build_capi.py --release --with-example-build`

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

- `python scripts/build_py.py`
- `python scripts/build_py.py --clean`
- `python scripts/build_py.py --verify`

默认输出位置：

- `crates/tswn_py/dist/`

说明：

- 该脚本用于构建 Python wheel
- 多平台 / 多环境产物可以共同放在 `crates/tswn_py/dist/` 下
- 聚合打包脚本会直接收集这里已有的内容

## gen_test_case.py

生成测试 case 的辅助脚本。
