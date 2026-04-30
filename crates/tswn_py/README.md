# tswn_py

`tswn_core` 的 Python 绑定（PyO3）。

## 安装

```bash
pip install tswn_py
```

或从本地 wheel 安装：

```bash
pip install crates/tswn_py/dist/tswn_py-*.whl
```

## 快速开始

```python
import tswn_py

# 版本查询
print(tswn_py.core_version_str())

# 图标渲染
b64 = tswn_py.name_to_png_base64("某个玩家名")

# 创建对局
runner = tswn_py.Runner.new_from_namerena_raw(raw_input, eval_rq=0.0)
runner.run_to_completion()
winner = runner.winner()
```

## 主要 API

### 顶层函数

| 函数 | 说明 |
|------|------|
| `core_version_str()` | tswn_core 版本 |
| `wrapper_version_str()` | tswn_py 版本 |
| `name_to_png_base64(name)` | 名称 → PNG Base64 |
| `name_to_png_bytes(name)` | 名称 → PNG 字节 |
| `name_to_icon_rgba(name)` | 名称 → 16×16 RGBA |
| `win_rate(raw, n, eval_rq, thread)` | 胜率统计 |
| `group_win_rate(target, against, n, eval_rq, thread)` | 分组胜率 |

### 类

| 类 | 说明 |
|----|------|
| `Runner` | 对战运行器，支持逐步推进或一次跑完 |
| `PreparedRunner` | 预处理后的复用模板 |
| `RunUpdates` | 回合更新容器 |
| `Storage` | 玩家数据存储 |
| `WorldState` | 世界状态 |
| `Player` | 玩家状态只读视图 |
| `RC4` | RC4 加密算法 |

## 构建

详细构建说明见 [README_BUILD.md](./README_BUILD.md)。

```powershell
# 构建 wheel
uv run scripts/build_py.py

# 构建并验证
uv run scripts/build_py.py --clean --verify
```

## 要求

- Python ≥ 3.12
- Windows / Linux / macOS

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
