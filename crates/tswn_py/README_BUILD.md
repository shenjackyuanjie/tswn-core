# tswn_py 构建说明（Wheel / 多 Python 版本）

本目录是 `tswn_core` 的 Python 绑定（PyO3 扩展模块）。

本文件描述对外分发的 **wheel** 构建方式。当前最低要求是 Python 3.12（见 [pyproject.toml](pyproject.toml)）；wheel 内的扩展模块文件名带有 Python/ABI/平台 tag（例如 `tswn_py.cp312-win_amd64.pyd`），可区分 cp312、cp313 等解释器环境。

> **线上/部署口径**：wheel 必须是 release 构建，且 `tswn_core` 启用
> `no_debug,mimalloc_alloc`。`Cargo.toml` 已固定依赖的这两个 feature；
> `build_dist.py` 现在默认 `--release --features "no_debug,mimalloc_alloc"`，
> 只有本地开发才显式加 `--debug`。

---

## 你最终会得到什么

在 `crates/tswn_py/dist/` 下生成 wheel，例如：

以下以 Python 3.12 和包装层版本 0.5.4 为例；实际版本由 [Cargo.toml](Cargo.toml) 提供，平台 tag 由构建环境决定：

- Windows：`tswn_py-0.5.4-cp312-cp312-win_amd64.whl`
- Linux：`tswn_py-0.5.4-cp312-cp312-linux_x86_64.whl`（发布 manylinux wheel 时另需相应构建和修复流程）
- macOS：`tswn_py-0.5.4-cp312-cp312-macosx_*.whl`

wheel 内部大致结构类似：

```text
tswn_py/
  __init__.py
  tswn_py.cp312-win_amd64.pyd     # Windows 示例
  tswn_py.cpython-312-x86_64-linux-gnu.so  # Linux 示例（具体命名由构建系统决定）
  *.pyi
  py.typed
```

安装后即可直接：

```python
import tswn_py
print(tswn_py.wrapper_version_str())
print(tswn_py.core_version_str())
```

---

## 前置依赖

### 1) Rust 工具链

需要安装 Rust（包含 `cargo`）。当前 workspace 的全功能依赖要求 Rust 至少 1.95；此要求来自依赖声明，本轮实际工具链为 1.99，未单独完成最低版本构建验收。

### 2) Python（多版本）

使用 Python 3.12 或更新版本。需要哪些 wheel，就用对应版本的 Python 分别构建，例如 cp312、cp313；当前配置按解释器 ABI 分发，不是通用 abi3 wheel。

### 3) 构建工具（PEP 517）

PEP 517 隔离环境按本 crate 的 [pyproject.toml](pyproject.toml) 安装构建依赖：`setuptools>=84.2.1`、`setuptools-rust>=1.13.0`、`wheel>=0.48.0`。`--no-isolation` 使用当前环境，因此也需满足这些下限；它们不改变项目的 Python 3.12 最低要求。

建议准备 `pip` 与 `build`：

```bash
python -m pip install -U pip build "setuptools>=84.2.1" "setuptools-rust>=1.13.0" "wheel>=0.48.0"
```

> 本项目使用 `pyproject.toml + setuptools-rust` 走 PEP 517 构建（而不是手写 `setup.py` 然后 `python setup.py bdist_wheel` 那套旧流程）。

---

## 构建 wheel（单 Python 版本）

推荐从仓库根目录使用构建脚本：

```powershell
uv run scripts/build_py.py
uv run scripts/build_py.py --clean --verify
```

默认输出到 `crates/tswn_py/dist/`。`--clean` 会清空该输出目录；`--verify` 会把最新生成的 wheel 安装到脚本所用的当前 Python 环境，再导入并检查版本。多解释器环境应使用目标解释器运行脚本或下面的 PEP 517 命令。

也可在 `crates/tswn_py` 目录下直接执行：

```bash
python -m build --wheel -o dist
```

生成的 wheel 会在 `crates/tswn_py/dist/` 下。

安装测试（可选；选择与当前解释器和平台匹配的具体 wheel）：

```bash
python -m pip install --force-reinstall dist/<匹配的-wheel-文件名>.whl
python -c "import tswn_py; print(tswn_py.wrapper_version_str()); print(tswn_py.core_version_str())"
```

---

## 构建多 Python 版本 wheel（同一台机器）

你需要为每个 Python 版本分别运行一次构建。常见做法是：

### Windows（py launcher）

```powershell
py -3.12 -m pip install -U pip build setuptools setuptools-rust wheel
py -3.12 -m build --wheel -o dist

py -3.13 -m pip install -U pip build setuptools setuptools-rust wheel
py -3.13 -m build --wheel -o dist
```

### Linux/macOS（多解释器）

用 pyenv/conda/系统多 Python 都可以；关键是“用哪个 python 执行 `python -m build`，就产出哪个 ABI 的 wheel”。

示例（伪代码）：

```bash
python3.12 -m pip install -U pip build setuptools setuptools-rust wheel
python3.12 -m build --wheel -o dist

python3.13 -m pip install -U pip build setuptools setuptools-rust wheel
python3.13 -m build --wheel -o dist
```

---

## 说明：为什么不再推荐“手动 rename 成 tswn_py.pyd”

把构建产物硬改名为 `tswn_py.pyd/tswn_py.so` 的方式，只能满足“本机 import 能跑”，但它丢失了 ABI 信息：

- cp312、cp313 等不同 ABI 的产物会互相覆盖
- 你很难同时保留多版本并让 pip 正确选择
- 分发时也无法表达兼容性（wheel tag 才是标准做法）

因此对外分发建议以 wheel 为准；你仍然可以保留脚本来做本地开发快速验证，但不建议把它当发布产物。

---

## 常见问题

### 1) `ImportError: DLL load failed` / 找不到依赖库

通常与：

- 构建时 Python 与运行时 Python 不一致（版本/架构/ABI 不匹配）
- Windows 缺少 MSVC 运行库
- 依赖动态库不在系统可搜索路径

wheel tag 只能保证“版本匹配”，不保证系统运行库齐全。

### 2) Debug/Release

wheel 的构建默认应是 release 语义（以构建后端为准）。对外分发建议使用 release 产物。

---
