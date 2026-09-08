# 性能报告：post_kill 别名 UB 修复 + no_debug 环境变量泄漏修复

## 测试环境

- OS：Windows 11
- CPU：（当前机器）
- Rust：1.94.0 (stable)
- 编译选项：`--release --features no_debug` (`opt-level=3`, `lto="fat"`, `codegen-units=1`)
- 测试命令：`tswn-cli --win_rate_st aaa bbb 100000`（单线程，10 万场）

## 修改内容

### 1. 修复 post_kill 回调中 `&mut Player` 别名 UB（commit 330cd46）

- 新增 `run_post_kill()` 函数，通过 `take_skill_type()` / `put_skill_type()` 临时取出技能实现
- 确保 kill 回调（如吞噬）执行时不存在重叠的 `&mut Player` 引用
- 消除了 release 构建中 LLVM noalias 优化导致的 27 个测试失败

### 2. 修复 `no_debug` feature 环境变量泄漏

- 将所有 23 处直接调用 `std::env::var("TSWN_DEBUG_*")` 的代码替换为 `crate::debug::*` 函数
- `no_debug` feature 启用时，这些函数被内联为常量 `false` / `None`，编译期完全消除
- 新增 `debug_damage()` 到 debug 模块（实际模块和 no_debug 存根）
- `TSWN_PROBE_PROTECT` 包裹在 `#[cfg(not(feature = "no_debug"))]` 中

## 基准测试结果

### 单线程 (`--win_rate_st aaa bbb 100000`)

| 版本                      | Run 1   | Run 2   | Run 3   | 平均        | 场/秒  |
| ------------------------- | ------- | ------- | ------- | ----------- | ------ |
| 修复前（2dd7203）         | 2692 ms | 2655 ms | 2676 ms | **2674 ms** | 37,401 |
| 仅 UB 修复（330cd46）     | 2626 ms | 2601 ms | 2604 ms | **2610 ms** | 38,314 |
| 完整修复（UB + no_debug） | 1968 ms | 1977 ms | 1929 ms | **1958 ms** | 51,073 |

### 多线程 (`--win_rate aaa bbb 100000`)

| 版本                      | 耗时    | 场/秒   |
| ------------------------- | ------- | ------- |
| 修复前（2dd7203）         | 3076 ms | 32,511  |
| 完整修复（UB + no_debug） | 430 ms  | 232,433 |

## 分析

### UB 修复本身的开销

UB 修复引入了 `take_skill_type()` / `put_skill_type()` 的 Box swap 操作。  
单线程对比：2674 ms → 2610 ms（**快了 2.4%**）。

这看上去不可思议——修复 UB 反而变快了。原因是修复前的代码存在未定义行为，LLVM 基于 noalias 假设所做的优化实际上是错误的（读取了过期的缓存值），当这些 "优化" 被消除后，编译器可能生成了更规律、更容易被 CPU 分支预测器和缓存命中的代码。差异在测量噪声范围内，可以认为 **UB 修复本身无可测量的性能退化**。

### no_debug 环境变量泄漏修复的收益

泄漏修复是主要的性能提升来源：

- **单线程**：2610 ms → 1958 ms（**快 25%**）
- **多线程**：3076 ms → 430 ms（**快 7.2 倍**）

原因：

1. **单线程**：`std::env::var()` 每次调用需要查询进程环境块、分配字符串、处理 UTF-8 转换。在热路径中（伤害计算每次 get_at 调 2 次、pre_defend 调 2 次、defned 调 1 次），100,000 场 × ~50 回合 × ~5 次/回合 ≈ 数千万次无用的系统调用。
2. **多线程**：Windows 进程环境块有全局锁，多线程同时调用 `std::env::var()` 产生严重的锁竞争，成为性能瓶颈。修复后这些调用在编译期消除，完全没有锁竞争。

### 正确性验证

两个版本在测试输入 "aaa vs bbb" 上产出完全一致的结果：

- 胜率：53.97%（53972/100000）
- 该对局不涉及吞噬技能（MergeSkill），UB 不影响结果

SBY 大样本测试（1800 case）确认：

- 修复前（release）：41 个 diff_failures（其中 27 个为 release-only UB 导致）
- 修复后（release）：14 个 diff_failures（与 debug 一致，UB 导致的失败全部消除）
