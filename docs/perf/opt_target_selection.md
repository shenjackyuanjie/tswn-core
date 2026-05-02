# 性能报告：将目标选择热路径优化移植回 main 分支

## 背景

`tswn_core_opencode` 上有一批性能优化并不依赖 stable 别名修复本身，主要集中在 `fight` 热路径的目标选择与主动技扫描开销上。

这一轮把其中可独立移植的部分回移到 `tswn-core` 主线，并验证它们是否能在不引入行为回归的前提下直接改善 `main` 分支性能。

## 本次移植的优化

涉及文件：

- `crates/tswn_core/src/player/impl_runtime.rs`
- `crates/tswn_core/src/engine/tick.rs`
- `crates/tswn_core/src/player/action_targets.rs`
- `crates/tswn_core/src/rc4.rs`
- `crates/tswn_core/src/player/boss/covid.rs`
- `crates/tswn_core/src/player/boss/lazy.rs`

具体包括：

1. `action()` 不再每次主动技扫描都 `clone()` 一份 `self.skills.skill`
2. `ActionTargets` 新增 `enemy_skip_indices`，把敌方抽样所需的 skip 列表提前算好
3. `tick::select_targets()` 把 `all_alive` / `enemy_alive` / `enemy_skip_indices` 的构造合并为一轮循环
4. `RC4::pick_skip_range()` 改为接收切片，避免调用点为传参额外构造 `Vec`
5. `boss/covid` 与 `boss/lazy` 同步改为复用已有 skip 列表

这些改动都不触碰 stable 修复时引入的 owner-centric / extracted path 行为边界，因此可以独立回移到主线。

## 验证

- `cargo test -p tswn_core`
- `python ./track_case_miner.py -q --modes 1v1,2v2,3v3v3,ffa --ffa-sizes 4,6,8 --case-offset-per-mode 0 --max-cases-per-mode 2000 --keep-going`

结果：

- `cargo test -p tswn_core` 全绿
- `12000 case` 全过
- `ts_failures = 0`
- `rust_failures = 0`
- `diff_failures = 0`

## 基准口径

- 编译参数：`cargo run --release --features no_debug -q --bin tswn-cli -- bench win-rate ... -n 100000 --single-thread --perf`
- 基线版本：`6dd7efe`（移植前的 `main`）
- 对照版本：当前工作区（移植后，未提交）

## 基准结果

| 样本 | 移植前（6dd7efe） | 移植后 | 总耗时变化 | fight 变化 | 结论 |
|---|---:|---:|---:|---:|---|
| `aaa` vs `bbb` | `2.195s` | `1.981s` | `-9.7%` | `1.641s -> 1.423s`（`-13.3%`） | 明显改善 |
| `喘际瞬爆@昀澤` vs `蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall` | `3.412s` | `3.014s` | `-11.7%` | `2.683s -> 2.313s`（`-13.8%`） | 明显改善 |

## 结论

这批优化可以独立存在于 `main` 分支，不依赖 stable/UB 修复链路。

从结果看，收益几乎全部来自 `fight`：

- `init` 基本持平
- `fight` 在两条样本上都下降了约 `13%`

这说明主收益点确实是：

- 减少每 tick 的目标集合重复扫描
- 减少敌方抽样时的临时分配与重复构造
- 去掉主动技筛选过程中的固定 `clone` 成本

换句话说，这是一组可以安全回移到主线、而且收益明确的热路径优化。
