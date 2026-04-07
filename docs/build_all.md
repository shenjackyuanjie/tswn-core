# Build All

本文记录当前仓库实际可用的一套聚合构建流程，用于同时准备：

- Windows `tswn_py` wheel
- WSL / Linux `tswn_py` wheel
- WSL / Linux `tswn-cli`
- Windows `capi` / `cli`
- 最终 `build_all` 聚合包

## 前提

- Windows 侧可用：`python`、`cargo`
- WSL 侧可用：`cargo`
- WSL Python 构建请使用仓库根目录下的 `.venv-wsl`

说明：

- `scripts/build_all.py` 不会现场构建 Python wheel，只会收集 `crates/tswn_py/dist/` 中已经存在的产物
- `scripts/build_all.py` 会现场构建当前平台的 `capi` / `cli`
- 若仓库 `target/release/` 下已经存在 WSL 构建出的 Linux `tswn-cli` / `libtswn_capi.so`，聚合脚本也会一并收集

## 推荐顺序

建议在仓库根目录 `D:\githubs\namer\tswn-core` 下，按下面顺序执行。

### 1. 构建 Windows Python wheel

```powershell
python scripts/build_py.py --clean
```

说明：

- 这一步会清空 `crates/tswn_py/dist/`
- 所以通常只在第一步做一次 `--clean`

### 2. 构建 WSL Python wheel

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && python scripts/build_py.py"
```

说明：

- WSL 下不要直接用系统 Python 跑这个脚本
- 需要先激活 `.venv-wsl`
- 否则脚本内部可能会误走别的环境，导致 `uv` / `build` 检测失败

### 3. 构建 WSL CLI

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_core --bin tswn-cli --release --features no_debug"
```

说明：

- 这里激活 `.venv-wsl` 主要是为了统一 WSL 环境入口
- 真正参与 CLI 构建的是 WSL 里的 Rust 工具链

### 4. 执行聚合打包

```powershell
python scripts/build_all.py --release --clean
```

这一步会：

- 现场构建 Windows `tswn_capi`
- 现场构建 Windows `tswn-cli`
- 收集 `crates/tswn_py/dist/` 下已有的 wheel
- 若存在 Linux `target/release/tswn-cli`，也会一起打进 bundle

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
dist/all/tswn_core_0_2_13_capi_0_1_2_py_0_1_10_bundle/
dist/all/tswn_core_0_2_13_capi_0_1_2_py_0_1_10_bundle.zip
```

### 聚合包内容

- `capi/`
  - Windows `dll` / `lib`
  - `include/tswn_capi.h`
  - `examples/`
- `cli/`
  - Windows `tswn-cli_alpha_*.exe`
  - 若已存在，也会额外收集 Linux `tswn-cli_alpha_*.bin`
- `py/`
  - 已有的 Windows / Linux wheel
  - `examples/`
  - `CHANGELOG`

## 一次跑完的命令清单

如果只是按当前推荐流程完整跑一遍，可以直接依次执行：

```powershell
python scripts/build_py.py --clean
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && python scripts/build_py.py"
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && . .venv-wsl/bin/activate && cargo build -p tswn_core --bin tswn-cli --release --features no_debug"
python scripts/build_all.py --release --clean
```

## 备注

- 如果只想更新聚合包，但 `py` wheel 没变，可以跳过前两步
- 如果只想更新 Linux `cli` 被聚合收集的版本，可以只重跑第 3 步和第 4 步
- 若后续还需要把 Linux `capi` 一并放进聚合包，可以先在 WSL 中额外执行：

```powershell
wsl sh -lc "cd /mnt/d/githubs/namer/tswn-core && cargo build -p tswn_capi --release"
```
