# 全量构建与聚合打包

[返回指南索引](README.md)

本文说明从仓库根目录构建组件并生成聚合 ZIP 的流程。版本由各 crate 的 `Cargo.toml` 和 Python 的 `pyproject.toml` 读取，不在本文维护另一份版本表。脚本参数全集见 [脚本索引](../../scripts/README.md) 或各脚本的 `--help`。

## 1. 构建范围与前提

| 组件 | `build_all.py` 的行为 | 主要产物 |
| --- | --- | --- |
| `cli` | 构建当前主机的 `tswn-cli` | 带 core 版本号的可执行文件 |
| `capi` | 调用 `build_capi.py` 构建当前平台分发包 | 头文件、库和 C examples |
| `openbox` | 构建当前主机的 GUI 与 `openbox-cli`，两者都必须存在 | 两份带版本号的可执行文件 |
| `py` | 收集现有产物，不构建 wheel | 当前 `tswn_py` 版本的 wheel / sdist、包文件和 examples |
| `wasm` | 调用 `build_wasm.py` 构建浏览器包 | JS glue、WASM、静态页面 examples |

需要 Rust/Cargo、`uv` 和 Python **3.12 或更高版本**。当前全功能 workspace 的依赖声明要求 Rust 至少 **1.95**（包括 GUI 和 `sysinfo`）；本轮实际使用 1.99，最低版本仍需独立构建验收。Windows 的 MSVC 构建还需要 Visual Studio 2022 Build Tools 的 C++ 工作负载与 Windows SDK，建议在 Developer PowerShell 中运行，环境准备见 [Windows C/C++ 指南](c-api-cpp-windows.md)。Openbox GUI 还需要平台的图形库与链接依赖，见 [Openbox README](../../crates/tswn_openbox/README.md)。

WASM 需要安装 `wasm32-unknown-unknown` target，并让 `wasm-bindgen-cli` 的版本与 `Cargo.lock` 中的 `wasm-bindgen` **完全一致**；脚本会检查版本。安装与独立构建见 [WASM README](../../crates/tswn_wasm/README.md)。

```powershell
rustup target add wasm32-unknown-unknown
```

正式发布使用 `--release`；原生 CLI/Openbox 默认追加 `no_debug,mimalloc_alloc`。默认 Cargo feature 仍开启，传空 feature 字符串只表示不追加 feature，不表示关闭默认 feature。

## 2. 当前主机的最短流程

以下 PowerShell 命令适用于 Windows。首次准备 Python 环境时执行第一行；已有 Python 3.12+ 环境时直接使用。

```powershell
uv venv --python 3.12 .venv
uv run --python 3.12 scripts/build_py.py --clean
uv run --python 3.12 scripts/build_all.py --release --clean
```

Linux 上可在仓库根目录使用独立的 Linux 虚拟环境，然后运行同样的脚本：

```bash
uv venv --python 3.12 .venv
. .venv/bin/activate
python scripts/build_py.py --clean
python scripts/build_all.py --release --clean
```

聚合脚本会现场构建当前平台的 C API、CLI、Openbox GUI/CLI 和 WASM，无需提前重复构建这些组件。仅 Python wheel 需要提前构建；详细依赖与 wheel 验证见 [Python 构建说明](../../crates/tswn_py/README_BUILD.md)。

两种 `--clean` 的范围不同：

- `build_py.py --clean` 清空整个 `crates/tswn_py/dist/`。多平台 wheel 共用目录时，只在第一轮执行，后续平台不加此选项。
- `build_all.py --clean` 清空本次 bundle 目录与同名 ZIP，不清空 Python wheel 源目录或 Cargo 构建缓存。

## 3. Windows 包额外收录 WSL/Linux 产物

如需同时分发 Windows 与 Linux 产物，先构建 Windows wheel，再在 **WSL 终端**切换到同一份仓库的 Linux 挂载路径。以下命令从该仓库根目录执行，不要求特定开发机路径。

```bash
# 首次创建；与 Windows 的 .venv 分开保存
uv venv --python 3.12 .venv-wsl
. .venv-wsl/bin/activate

# 保留先前的 Windows wheel，不加 --clean
python scripts/build_py.py

# 按需准备额外的 Linux 产物
cargo build -p tswn_core --bin tswn-cli --release --features no_debug,mimalloc_alloc
cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc
cargo build -p tswn_capi --release
```

再回到 Windows 的仓库根目录执行：

```powershell
uv run --python 3.12 scripts/build_all.py --release --clean
```

Windows 聚合脚本会额外收集已存在的 `target/release/tswn-cli`、`target/release/tswn_openbox` 和 `target/release/libtswn_capi.so`。这只是收集，不会自动调用 WSL 重建，也不会核验额外 Linux 二进制的版本；请先按待发布提交重新构建。

当前主机的 Openbox GUI 与 `openbox-cli` 都会进入 ZIP；**Windows 侧额外收集 Linux Openbox 的逻辑目前只收 GUI，不收 Linux `openbox-cli`**。需要完整 Linux GUI/CLI 包时，在 Linux 上运行聚合脚本。

Windows 和 WSL 的 Python 环境分别使用 `.venv` / `.venv-wsl`；在共享仓库中保持二者独立，WSL 构建时先激活 `.venv-wsl`，再使用其中的 `python`。

## 4. 常用选项与独立构建

```powershell
# 自定义名称和输出目录
uv run scripts/build_all.py --release --bundle-name my_bundle -o dist/all

# 不收录 Python，或跳过不需要的组件
uv run scripts/build_all.py --release --skip-py
uv run scripts/build_all.py --release --skip-openbox

# 单独构建与打包
uv run scripts/build_py.py
uv run scripts/build_capi.py --release --with-example-build
uv run scripts/build_wasm.py --release --clean
cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc
```

`--target TRIPLE` 作用于 C API、CLI 和 Openbox 的 Cargo 构建；WASM 使用自己的 `wasm32-unknown-unknown` target。CLI/Openbox 的追加 feature 分别由 `--cli-features` / `--openbox-features` 覆盖。附加 Cargo 参数通过 `--cargo ...` 传递，放在命令最后。

### PGO 状态

CLI 的独立 PGO 流程可使用 `python scripts/pgo_build.py --kind cli`，要求匹配 rustc 的 LLVM 大版本，且训练需在本机单线程运行。性能结论需重新按同机 A/B 测量，历史收益不作为当前保证。

Openbox 资源已迁到 `crates/tswn_openbox_backend/assets/`，但 `pgo_build.py` 的 Openbox 训练靶子仍指向旧的 `crates/tswn_openbox/assets/targets/`。因此在脚本适配前，全量打包推荐普通 `--release`；`build_all.py --pgo` 会同时启用 CLI 和 Openbox 的 PGO，不作为当前完整打包的推荐命令。约束见 [性能准则](../perf/guides/performance.md#pgo-构建)。

### 本机 sccache / C++ 编译器误判排查

本轮重新编译时，在本机 `sccache` 与 C/C++ 编译器检测的组合下，观察到 mimalloc 的 MSVC 编译器被误判为 GNU，出现把 `/WX-` 当成文件名等报错。`cc` 会使用 Cargo 传入的 `RUSTC_WRAPPER`，即使没有显式配置 `CC/CXX` 也可能经过 wrapper；现有证据只确认本机环境组合问题，不判定为新版 `cc` 的普遍回归。遇到相同症状时，在单次命令中关闭 wrapper，保留默认 allocator、依赖和全局配置：

```powershell
cargo --config 'build.rustc-wrapper=""' check -p tswn_core --lib --locked
```

该命令已在本轮维护环境验证通过；没有配置此 wrapper 的环境无需添加。若检查成功而完整 release 构建仍失败，沿用同一 `--config` 覆盖重跑目标构建并保存编译器诊断，不把这次检查等同于完整发布验收。

### 可选 OHOS CLI

`--include-ohos-cli` 可额外构建并收录 OHOS CLI 的未签名 ELF；SDK、target、部署与签名流程见 [OpenHarmony 交叉编译指南](openharmony.md)。聚合脚本不执行设备部署或签名。

## 5. 输出与验收

默认输出：

```text
dist/all/<自动生成的 bundle 名>/
dist/all/<自动生成的 bundle 名>.zip
```

自动名称依次包含 core、capi、py、wasm、openbox 的版本，版本中的点替换为下划线。也可用 `--bundle-name` 固定名称。典型目录结构为：

```text
<bundle>/
  README.txt / MANIFEST.txt
  capi/
    include/tswn_capi.h
    lib/
    examples/
    changelog/
  cli/
    bin/tswn-cli_alpha_<core版本>.*
    changelog/
  openbox/
    bin/tswn_openbox_alpha_<openbox版本>.*
    bin/openbox-cli_alpha_<openbox版本>.*
    changelog/
  py/
    dist/
    examples/
    changelog/
  wasm/
    pkg/tswn_wasm.js
    pkg/tswn_wasm_bg.wasm
    raw/tswn_wasm.wasm
    examples/
    changelog/
```

各组件另有 `README.txt` 和 `MANIFEST.txt`。Openbox 首次启动会在当前工作目录生成 `setting/` 默认预设，部署时选择可写目录。

打包完成后核对根和组件清单，以及 ZIP 中的实际文件：

- CLI、Openbox GUI 与 `openbox-cli` 的平台、版本与待发布提交一致。
- C API 包含头文件、目标平台的库与 examples。
- Python wheel 的 Python/平台标签符合部署环境；聚合脚本按 `tswn_py` 版本过滤 wheel/sdist，但同版本旧 wheel 的 core 或源码状态仍需主动核验，依赖或实现变动后重新构建。
- WASM 的 JS glue 与 `.wasm` 来自同一次匹配版本的构建。

当前发布行为门禁见 [Runtime 0.5 迁移指南](runtime-0.5-migration.md) 和 [性能准则](../perf/guides/performance.md)；历史发布包快照见 [版本记录](../releases/README.md)。
