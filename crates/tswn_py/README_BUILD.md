# tswn_py 构建说明（Wheel / 多 Python 版本）

本目录是 `tswn_core` 的 Python 绑定（PyO3 扩展模块）。

本文件只描述“对外分发”的构建方式：**生成 wheel**。wheel 内的扩展模块文件名会带上 Python/ABI/平台 tag（例如 `tswn_py.cp38-win_amd64.pyd`），从而天然区分 cp38/cp39/cp310 等多版本环境，避免你遇到的“产物永远叫 `tswn_py.pyd`”的问题。

---

## 你最终会得到什么

在 `crates/tswn_py/dist/` 下生成 wheel，例如：

- Windows: `tswn_py-0.1.0-cp311-cp311-win_amd64.whl`
- Linux: `tswn_py-0.1.0-cp311-cp311-manylinux_2_28_x86_64.whl`（tag 取决于构建环境）
- macOS: `tswn_py-0.1.0-cp311-cp311-macosx_*.whl`

wheel 内部大致结构类似：

```text
tswn_py/
  __init__.py
  tswn_py.cp311-win_amd64.pyd     # Windows 示例
  tswn_py.cpython-311-x86_64-linux-gnu.so  # Linux 示例（具体命名由构建系统决定）
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
需要安装 Rust（包含 `cargo`）。

### 2) Python（多版本）
你需要哪些 wheel，就用对应版本的 Python 来构建哪些 wheel（同一份 Rust 代码需要分别为 cp38/cp39/... 构建）。

### 3) 构建工具（PEP 517）
建议准备 `pip` 与 `build`：

```bash
python -m pip install -U pip build setuptools setuptools-rust wheel
```

> 本项目使用 `pyproject.toml + setuptools-rust` 走 PEP 517 构建（而不是手写 `setup.py` 然后 `python setup.py bdist_wheel` 那套旧流程）。

---

## 构建 wheel（单 Python 版本）

在 `crates/tswn_py` 目录下执行：

```bash
python -m build --wheel -o dist
```

生成的 wheel 会在 `crates/tswn_py/dist/` 下。

安装测试（可选）：

```bash
python -m pip install --force-reinstall dist/tswn_py-*.whl
python -c "import tswn_py; print(tswn_py.wrapper_version_str()); print(tswn_py.core_version_str())"
```

---

## 构建多 Python 版本 wheel（同一台机器）

你需要为每个 Python 版本分别运行一次构建。常见做法是：

### Windows（py launcher）
```powershell
py -3.8 -m pip install -U pip build setuptools setuptools-rust wheel
py -3.8 -m build --wheel -o dist

py -3.11 -m pip install -U pip build setuptools setuptools-rust wheel
py -3.11 -m build --wheel -o dist
```

### Linux/macOS（多解释器）
用 pyenv/conda/系统多 Python 都可以；关键是“用哪个 python 执行 `python -m build`，就产出哪个 ABI 的 wheel”。

示例（伪代码）：
```bash
python3.8 -m pip install -U pip build setuptools setuptools-rust wheel
python3.8 -m build --wheel -o dist

python3.11 -m pip install -U pip build setuptools setuptools-rust wheel
python3.11 -m build --wheel -o dist
```

---

## 说明：为什么不再推荐“手动 rename 成 tswn_py.pyd”

把构建产物硬改名为 `tswn_py.pyd/tswn_py.so` 的方式，只能满足“本机 import 能跑”，但它丢失了 ABI 信息：

- cp38 / cp39 / cp310 的产物会互相覆盖
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