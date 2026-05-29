# Build All

本文记录当前仓库实际可用的一套聚合构建流程，用于同时准备：

- Windows `tswn_py` wheel
- WSL / Linux `tswn_py` wheel
- WSL / Linux `tswn-cli`
- Windows `capi` / `cli`
- Windows `tswn_openbox` GUI
- 浏览器侧 `tswn_wasm` 包
- 最终 `build_all` 聚合包

## 前提

- Windows 侧可用：`uv`、`cargo`
- Windows 侧建议额外安装：`wasm-bindgen-cli`
- WSL 侧可用：`cargo`
- WSL Python 构建请使用仓库根目录下的 `.venv-wsl`

说明：

- 仓库使用 `uv` 管理 Python 环境（`.venv` 由 `uv` 创建）
- Windows 侧的 Python 构建/聚合脚本推荐用 `uv run scripts/...` 替代 `python scripts/...`
- `scripts/build_all.py` 不会现场构建 Python wheel，只会收集 `crates/tswn_py/dist/` 中已经存在的产物
- `scripts/build_all.py` 会现场构建当前平台的 `capi` / `cli` / `tswn_openbox`，并构建 `tswn_wasm` 浏览器包
- `scripts/build_all.py` 的 Windows CLI 默认 feature 为 `no_debug,mimalloc_alloc`；如需覆盖，可使用 `--cli-features`
- `scripts/build_all.py` 的 Windows Openbox 默认 feature 为 `no_debug,mimalloc_alloc`；如需覆盖，可使用 `--openbox-features`
- 若仓库 `target/release/` 下已经存在 WSL 构建出的 Linux `tswn-cli` / `tswn_openbox` / `libtswn_capi.so`，聚合脚本也会一并收集
- `tswn_wasm` 打包默认依赖 `wasm-bindgen-cli`，可通过 `cargo install wasm-bindgen-cli` 安装

## 推荐顺序

建议在仓库根目录 `D:\githubs\namer\tswn-core` 下，按下面顺序执行。

### 1. 构建 Windows Python wheel

```powershell
uv run scripts/build_py.py --clean
```

说明：

- 这一步会清空 `crates/tswn_py/dist/`
- 所以通常只在第一步做一次 `--clean`
- 使用 `uv run` 会自动使用 `.venv` 中的 Python 环境，无需手动激活

### 2. 构建 WSL Python wheel

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && python scripts/build_py.py"
```

说明：

- WSL 下不要直接用系统 Python 跑这个脚本
- 需要先激活 `.venv-wsl`
- 否则脚本内部可能会误走别的环境，导致 `uv` / `build` 检测失败
- WSL 侧如果也安装了 `uv`，可以考虑改用 `uv run scripts/build_py.py`（但需要确保 `.venv-wsl` 也是 `uv` 管理的）

### 3. 构建 WSL CLI

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_core --bin tswn-cli --release --features no_debug,mimalloc_alloc"
```

说明：

- 这里激活 `.venv-wsl` 主要是为了统一 WSL 环境入口
- 真正参与 CLI 构建的是 WSL 里的 Rust 工具链
- 最终发布 CLI 默认启用 `mimalloc_alloc`；benchmark 口径仍不要带这个 feature

### 4. 构建 WSL Openbox（如需 Linux GUI 产物）

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc"
```

说明：

- `scripts/build_all.py` 会现场构建 Windows `tswn_openbox`
- 这一步只用于提前准备 Linux `tswn_openbox`，存在时聚合脚本会一并收集
- 如果只需要 Windows Openbox，可以跳过这一步

### 5. 执行聚合打包

```powershell
uv run scripts/build_all.py --release --clean
```

这一步会：

- 现场构建 Windows `tswn_capi`
- 现场构建 Windows `tswn-cli`
- 现场构建 Windows `tswn_openbox`
- 现场构建 `tswn_wasm` 浏览器包
- 收集 `crates/tswn_py/dist/` 下已有的 wheel
- 若存在 Linux `target/release/tswn-cli` / `target/release/tswn_openbox`，也会一起打进 bundle

## 产物位置

### Python wheel

输出目录：

```text
crates/tswn_py/dist/
```

当前通常会看到：

- `tswn_py-...-win_amd64.whl`
- `tswn_py-...-linux_x86_64.whl`

### 聚合包

默认输出目录：

```text
dist/all/
```

当前 bundle 命名规则示例：

```text
dist/all/tswn_core_0_3_3_capi_0_3_0_py_0_2_0_wasm_0_2_3_openbox_0_3_0_bundle/
dist/all/tswn_core_0_3_3_capi_0_3_0_py_0_2_0_wasm_0_2_3_openbox_0_3_0_bundle.zip
```

### 聚合包内容

- `capi/`
  - Windows `dll` / `lib`
  - `include/tswn_capi.h`
  - `examples/`
- `cli/`
  - Windows `tswn-cli_alpha_*.exe`
  - 若已存在，也会额外收集 Linux `tswn-cli_alpha_*.bin`
- `openbox/`
  - Windows `tswn_openbox_alpha_*.exe`
  - 若已存在，也会额外收集 Linux `tswn_openbox_alpha_*.bin`
  - `changelog/`
  - `tswn_openbox` 首次启动会在当前工作目录自动生成 `setting/` 默认预设目录
- `py/`
  - 已有的 Windows / Linux wheel
  - `examples/`
  - `CHANGELOG`
- `wasm/`
  - `pkg/tswn_wasm.js`
  - `pkg/tswn_wasm_bg.wasm`
  - `raw/tswn_wasm.wasm`
  - `examples/demo.html`
  - `CHANGELOG`

## 一次跑完的命令清单（完整版）

如果只是按当前推荐流程完整跑一遍，可以直接依次执行：

### Windows 平台（使用 uv）

```powershell
uv run scripts/build_py.py --clean
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && python scripts/build_py.py"
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_core --bin tswn-cli --release --features no_debug,mimalloc_alloc"
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc"
uv run scripts/build_all.py --release --clean
```

如果不需要 Linux Openbox GUI 产物，可以跳过第 4 条 `cargo build -p tswn_openbox` 的 WSL 命令。

### WSL / Linux 纯环境（仅构建 Linux 产物）

如果只在 WSL/Linux 下构建，不需要 Windows 侧的产物：

```bash
# 激活 WSL venv
. .venv-wsl/bin/activate

# 构建 Linux Python wheel
python scripts/build_py.py --clean

# 构建 Linux CLI
cargo build -p tswn_core --bin tswn-cli --release --features no_debug,mimalloc_alloc

# 构建 Linux Openbox
cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc

# 构建 Linux capi
cargo build -p tswn_capi --release

# 聚合打包（Linux 下会构建当前平台 cli/openbox/wasm，并收集 py wheel；capi 需提前构建好）
python scripts/build_all.py --release --clean
```

### uv 用户的 WSL 增强方案

如果 WSL 侧也安装了 `uv`，可以用 `uv` 统一管理 WSL 环境：

```powershell
# 在 WSL 中安装 uv（如果未安装）
wsl sh -lc "curl -LsSf https://astral.sh/uv/install.sh | sh"

# 然后用 uv run 替代直接 python 调用
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && uv run scripts/build_py.py"
```

## 备选：单个步骤的 uv 快捷命令

### 仅构建 Windows Python wheel

```powershell
uv run scripts/build_py.py
```

### 仅构建（或更新）聚合包

如果 wheel 没变、只需要重新打包 capi + cli + openbox + wasm：

```powershell
uv run scripts/build_all.py --release --clean
```

### 仅构建 Windows capi

```powershell
uv run scripts/build_capi.py --release
```

### 仅构建 Windows Openbox

```powershell
cargo build -p tswn_openbox --release --features no_debug,mimalloc_alloc
```

## 备注

- 如果只想更新聚合包，但 `py` wheel 没变，可以跳过前两步
- 如果只想更新 Linux `cli` 被聚合收集的版本，可以只重跑第 3 步和第 5 步
- 如果只想更新 Linux `openbox` 被聚合收集的版本，可以只重跑第 4 步和第 5 步
- 若后续还需要把 Linux `capi` 一并放进聚合包，可以先在 WSL 中额外执行：

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && cargo build -p tswn_capi --release"
```

## 版本对照

当前构建产出版本：

| 组件         | 版本  |
| ------------ | ----- |
| tswn_core    | 0.3.3 |
| tswn_capi    | 0.3.0 |
| tswn_py      | 0.2.0 |
| tswn_wasm    | 0.2.3 |
| tswn_openbox | 0.3.1 |
