# docs/ 文档目录状态评估

本文档对 `docs/` 下所有文件及子目录进行状态分类。

---

## 一、当前有用且不过时的文档 ✅

| 文件                           | 内容                            | 说明                                                                           |
| ------------------------------ | ------------------------------- | ------------------------------------------------------------------------------ |
| `build_all.md`                 | 聚合构建流程                    | 当前构建流程的完整指南（Windows wheel、WSL、CLI、WASM、聚合包）                |
| `perf/benchmark_tracking.md`   | 性能追踪表                      | 持续更新的基准测试结果，含各版本横向对比                                       |
| `perf/opt_target_selection.md` | 目标选择优化移植报告            | 已完成并合入主线的优化报告                                                     |
| `perf/ub_fix_no_debug.md`      | UB 修复 + no_debug 泄漏修复报告 | 已完成并合入主线的修复报告                                                     |
| `storage_refactor_analysis.md` | Storage 内部可变性方案分析      | 当前架构的内部可变性方案分析，与实际代码一致                                   |
| `DIY.md`                       | DIY / OL overlay 使用说明       | 当前代码通过 `PlayerOverlay` 支持玩家与召唤物的 `diy[...]` / `ol:{...}` 覆盖   |
| `howto/README.md`              | 项目概况                        | 简要说明项目目标和起源                                                         |
| `howto/capi_cpp_windows.md`    | C API C++ Windows 使用指南      | C++ 编译/链接指南，与当前 bundle 产物一致                                      |

---

## 二、历史/已完成/已删除的文档 ❌

| 文件                             | 处理      | 原因                                                              |
| -------------------------------- | --------- | ----------------------------------------------------------------- |
| `rust_design.md`                 | ❌ 已删除 | 早期 Rust 设计文档，描述 `Rc<RefCell>` 等方案，与当前架构严重不符 |
| `plr.md`                         | ❌ 已删除 | 基于 Dart 源码的 Plr 设计文档，当前架构已完全不同                 |
| `proc_registration_locations.md` | ❌ 已删除 | 基于 Dart 源码的注册点清单，无法映射到当前 Rust 实现              |
| `verify_checklist.md`            | ❌ 已删除 | 重写验证清单，大部分检查项已被更现代的方法覆盖                    |
| `00_summary.md`                  | ❌ 已删除 | 时序图汇总，描述的是 Dart API 和 Dart 架构                        |
| `mermaid/`                       | ❌ 已删除 | 5 个时序图均基于 Dart 源码，函数名/签名/架构与 Rust 实现不符      |
| `diff/`                          | ❌ 已删除 | 空目录                                                            |

---

## 三、计划中但尚未实现的文档 📋

| 文件 | 内容 | 现状 |
| ---- | ---- | ---- |
| 无   | -    | -    |

---

## 四、统计摘要

| 类别                | 数量                       |
| ------------------- | -------------------------- |
| ✅ 当前有用且不过时 | **8** 个文件               |
| ❌ 已删除的过时文档 | **10** 个文件 + 1 个空目录 |
| 📋 计划中未实现     | **0** 个文件               |

---

## 五、现状

```text
docs/
├── README.md                       # ← 本文档
├── build_all.md                    # 构建流程
├── DIY.md                          # DIY / OL overlay 使用说明
├── storage_refactor_analysis.md    # 内部可变性分析
├── howto/
│   ├── README.md                   # 项目概况
│   └── capi_cpp_windows.md         # C API 使用指南
├── perf/
│   ├── benchmark_tracking.md       # 性能追踪表
│   ├── opt_target_selection.md     # 目标选择优化报告
│   └── ub_fix_no_debug.md          # UB 修复报告
└── update/                         # 27 个版本日志（历史存档）
```
