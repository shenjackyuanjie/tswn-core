# 文档索引

## 当前文档

| 文件 | 内容 |
| --- | --- |
| [`runtime_0.5_migration.md`](runtime_0.5_migration.md) | 主 Runtime 独立化、旧 API 删除与绑定迁移 |
| [`DIY.md`](DIY.md) | DIY / OL overlay 输入与导出格式 |
| [`build_all.md`](build_all.md) | CLI、C、Python、WASM 聚合构建流程 |
| [`howto/1-start.md`](howto/1-start.md) | 项目入门与常用命令 |
| [`howto/capi_cpp_windows.md`](howto/capi_cpp_windows.md) | Windows C/C++ 调用 C ABI |
| [`howto/diy_validation.md`](howto/diy_validation.md) | DIY / OL 验证流程 |
| [`perf/fixed_cases_30_benchmark.md`](perf/fixed_cases_30_benchmark.md) | fixed30 性能回归口径 |
| [`perf/runtime_0.5.0_749fcd1_release_benchmark.md`](perf/runtime_0.5.0_749fcd1_release_benchmark.md) | 0.5.0 主 Runtime 完整发版基准与同机 A/B |
| [`perf/benchmark_tracking.md`](perf/benchmark_tracking.md) | 历史性能追踪索引 |

## 历史与实施记录

- `update/` 保存已发布版本的更新记录。
- `perf/runtime_*.md` / `.json` 保存对应提交与机器环境下的历史基准，不反向改写其中的旧执行器描述。
- `analysis/core_runtime_refactor_plan.md` 与 `analysis/custom_runtime_migration.md` 是主 Runtime 落地过程的实施记录；其中旧路径、双栈和 parity 命令不再存在。
- `analysis/storage_refactor.md` 与 `architecture.md` 分别描述已删除的 Rust 对象模型和更早的 Dart 源实现，仅供考古，不是当前 API 文档。

当前公开入口以各 crate README、0.5 迁移指南和源码类型为准。主 Runtime 独立性与 124-case corpus 使用以下命令验证：

```powershell
python scripts/check_runtime_release.py --corpus
```
