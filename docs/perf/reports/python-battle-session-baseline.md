# Python BattleSession DTO 转换性能基线

2026-09-08，代码基于 `4d8fd62a`，测量脚本随本报告提交。四类 frozen fixture 各测量两轮，每轮先预热 10 场，再测量 100 场；Python 与 WASM 各计入 800 场。两轮均未触发可测的优化门槛 A、B，保留 `serde_json::to_string() -> json.loads()`。

当前不是优先瓶颈，暂不优化。这个结论限于本机、当前输入规模和调用方式；ffa_8 接近 1 ms 门槛，应在下一阶段真实采集任务中继续观察。门槛 C 所需的实际任务 CPU profiling 尚未进行，不能据此推断 DTO 的 CPU 占比低于 25%。

## 环境与口径

- Windows 10 22H2（10.0.19045），AMD Ryzen 7 5800X，CPython 3.12.10 x64，rustc 1.98.0，tswn_py 0.5.2 / core 0.5.3。
- Python：`cargo build -p tswn_py --release`，沿用 workspace 的 release 配置（fat LTO、1 个 codegen unit、保留调试信息）。脚本每次构建后从独立目录导入本地扩展，不使用已安装 wheel。
- WASM：同一代码、同机 `wasm32-unknown-unknown` release，wasm-bindgen 0.2.127 的 Node 包，Node v24.19.0。Python 测量完成后顺序运行 WASM；没有并行运行两个性能进程。此比较不包含浏览器、DOM 或页面播放开销。
- 固定 `max_rounds=20_000`、`include_icons=false`；`eval_rq` 使用当前默认值 4.0。模块和 fixture 文件加载不计时；每场重新创建会话，保留正常 GC。
- `session_create_ms` 只计构造函数。`next_frame_wall_ms` 包含 Runtime 推进和完整 DTO 转换，按该类全部调用合并计算 nearest-rank p50/p95，包含最后一次返回 None/null 的调用。Python 每帧对象在该次调用计时之后释放；WASM 对象按正常 JS GC 回收。
- `total_session_ms` 从构造前到最后一次空返回，包含循环、计时和逐帧处理/释放的开销；不调用 `initial_states()`，不保留完整回放，也不将 `result()` 转换、结果断言或会话释放计入总耗时。每场都在计时后检查正常胜者、帧数和轮数。
- 因首轮 ffa_8 接近 1 ms，完整重复测量一次。下表报告复测结果，后表同时保留首轮 p95；未筛除较慢样本或异常峰值。两輪使用相同 Python/WASM 二进制，文件 SHA-256、输入 SHA-256、环境和全部逐次样本见[原始数据](../python_battle_session_samples.json)。

本基线测量的是 Python 用户看到的 `next_frame()` 总耗时，无法单独归因于 DTO 转换；没有修改转换实现，也没有手工 PyDict/PyList 的 A/B 结果。

## Python 实测结果

单位为 ms；每类 100 场，不包含该轮的 10 场预热。

| fixture | create p50 / p95 | next_frame p50 / p95 / max | total p50 / p95 | 每场帧数 | 调用样本数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1v1 | 0.1126 / 0.1325 | 0.1207 / 0.1870 / 0.3464 | 1.4818 / 1.7015 | 10 | 1100 |
| 2v2 | 0.1933 / 0.2611 | 0.2356 / 0.3855 / 0.7968 | 3.8452 / 5.3406 | 15 | 1600 |
| ffa_8 | 0.3694 / 0.4184 | 0.4086 / 0.9580 / 2.0667 | 20.9514 / 22.3857 | 45 | 4600 |
| 3v3v3 | 0.4245 / 0.4620 | 0.4142 / 0.7577 / 1.4366 | 24.4028 / 25.5529 | 57 | 5800 |

## 复测与优化门槛

| fixture | 首轮 Python p95 | 复测 Python p95 | 同轮 WASM p95 | Python / WASM |
| --- | ---: | ---: | ---: | ---: |
| 1v1 | 0.1981 | 0.1870 | 0.2111 | 0.886 |
| 2v2 | 0.3850 | 0.3855 | 0.3062 | 1.259 |
| ffa_8 | 0.9706 | 0.9580 | 0.6642 | 1.442 |
| 3v3v3 | 0.7580 | 0.7577 | 0.5435 | 1.394 |

- A（Python next_frame p95 >= 1.0 ms）：两轮均未触发。单次 max 超过 1 ms 不等于 p95 达标；ffa_8 距门槛较近，机器负载或输入变化可能改变结论。
- B（Python p95 >= 同环境 WASM p95 的 4 倍）：两轮均未触发。首轮比值 0.901–1.431，复测 0.886–1.442。本次采用同机 Node WASM 的相同构造/逐帧循环口径，不套用旧浏览器报告的时间。
- C（实际下一阶段采集任务中 DTO conversion >= 总 CPU 时间的 25%）：尚无对应采集任务的 profiling，记录为未测，不能视为已通过或已失败。

本轮不重写 DTO conversion，也不增加转换依赖。以后若 A/B 实测越过门槛，或真实任务触发 C，另开独立优化任务：保持 dict/list shape、TypedDict 和跨绑定 parity，在同 fixture 上比较优化前后的数据。

## 固定输入

输入均取自 `crates/tswn_test/cases/runtime_stress/`，不修改 frozen corpus：

- `1v1-0f92cb76cc37fdc5.txt`
- `2v2-554f4128af707167.txt`
- `ffa_8-16d11de1ebe1df41.txt`
- `3v3v3-0ace5df17b84e26a.txt`

## 复现

在仓库根目录运行 Python 基线，脚本自动构建 release 扩展：

```powershell
python scripts/benchmark_py_battle_session.py
```

同时复现同机 WASM 比较：

```powershell
cargo build -p tswn_wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/tswn_wasm.wasm --target nodejs --out-dir target/battle_wasm_release_node
python scripts/benchmark_py_battle_session.py --wasm-package target/battle_wasm_release_node/tswn_wasm.js
```

默认原始输出为 `target/python_battle_session/baseline.json`。可用 `--output` 保留每轮结果，`--sessions` 增加样本（不得低于 100），`--warmup` 调整预热（不得低于 1）。不传 WASM 包时，B 记录为未测；脚本始终将 C 记录为未测。比较脚本只校验两端帧数、轮数和胜者，完整 payload 一致性仍由 `scripts/verify_battle_cross_binding.py` 单独验证。
