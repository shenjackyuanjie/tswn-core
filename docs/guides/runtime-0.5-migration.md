# 主 Runtime 0.5 迁移指南

`tswn_core 0.5.0` 完成主 Runtime 独立化，并删除仓库内的旧执行器与对象模型。本次是有意的破坏性升级：不提供 Rust 兼容壳，也不再允许 CLI 或绑定层选择旧执行器。

## 版本边界

| 组件 | 新版本 | 兼容性说明 |
| --- | --- | --- |
| `tswn_core` | `0.5.0` | 删除旧 Rust 对象模型与 legacy API |
| `tswn_py` | `0.5.0` | 删除旧对象 getter、`round_tick*` 与 parity API |
| `tswn_wasm` | `0.5.0` | 会话改用主 Runtime，JS/JSON shape 保持不变 |
| `tswn_capi` | `0.6.0` | C ABI 版本由 `3` 升为 `4` |

这张表记录 0.5 迁移时的历史版本边界，不是当前各 crate 的版本清单；当前公开入口与稳定性分层见 [公共 API](../reference/public-api.md)。

## Rust

根级 `Runner` 与 `PreparedRunner` 是主 Runtime 的进阶执行入口。普通战斗展示与增量消费优先使用 `cli_api::BattleSession`；完整收集使用 `battle_replay`，当前示例见 [公共 API](../reference/public-api.md)。迁移旧执行代码可使用：

```rust
use tswn_core::Runner;

let mut runner = Runner::new_from_namerena_raw(raw_input)?;
let summary = runner.run_to_completion(20_000);
let winner = runner.winner_team_index();
```

批量运行应先构造 `PreparedRunner`，再按 seed 复用。名字解析、名字属性、团队升级、武器、DIY/OL overlay、技能等级、clone 与 minion blueprint 位于 `tswn_core::namerena` 的数据准备链；运行期实体、调度、会话与更新位于 `tswn_core::runtime`，`RunUpdate` / `RunUpdates` 也可继续从 crate 根导入。

以下入口已删除，调用方必须迁移，不能改名后继续依赖旧对象：

- `tswn_core::legacy`、`LegacyRunner`、`LegacyPreparedRunner`；
- `tswn_core::engine`、`tswn_core::player` 及其 `Player` / `Storage` / `WorldState` 类型；
- 旧 `Skill` trait、`SkillStorage`、`PlayerStateStore`；
- legacy normalizer 与 parity report。

## CLI

当前 `fight`、`runtime diff`、`runtime normalized-run`、`bench` 与 `namer-pf` 都固定使用主 Runtime：

```powershell
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt --jsonl --max-rounds 20000
cargo run -p tswn_core --bin tswn-cli -- runtime diff -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime normalized-run -f input.txt --max-rounds 20000
```

删除的命令面包括 `--runtime legacy`、其他执行器选择值、`runtime parity`、顶层 `diff` / `raw` 和 `fight --out-raw`。结构化增量输出使用 `fight --jsonl`；诊断文本使用 `runtime diff`，normalized-run JSON 使用 `runtime normalized-run`。参数依据见 [CLI 定义](../../crates/tswn_core/src/bin/tswn_cli/args/cli.rs)。需要验证行为时运行冻结 corpus。

## Python

0.5 迁移保留 `Runner`、`PreparedRunner`、`RunUpdate(s)`、RC4、snapshot、replay、胜率和评分接口；当前普通战斗入口已统一为 `BattleSession` / `battle_replay`，runner 对象查询属于进阶接口。常见旧调用迁移如下：

| 已删除 | 替代方式 |
| --- | --- |
| `Runner.round_tick*()` | `Runner.main_round()` |
| `runner.world_state.winner` | `runner.winner_team_index()` / `winner_team_indices()` |
| `runner.storage`、`Storage`、`WorldState`、`Player` getter | `runner.snapshot_players()`、`alives*()`、`all_plrs()`、`input_groups` |
| parity helper | 运行仓库冻结 corpus |

`run_to_completion()` 与 `Runner.build_replay(limit=None)` 都受 20,000 主回合保护。后者的显式 `limit` 另按可见事件数计量，不能当作帧数或替换主回合预算；完整回合 frame 与截断的扁平 events 可能覆盖不同长度。普通对局优先使用规范 `BattleSession` / `battle_replay`。

## C

0.5 迁移中 C 侧保留 opaque runner、prepared、updates、snapshot、score 与 win-rate 符号，删除 `tswn_default_custom_runtime_parity_json`，既有函数签名和结构体布局保持不变，ABI 查询值升为 4。当前还提供 `tswn_battle_session_*` 与 `tswn_battle_replay_json`；战斗选项 V1 prefix 和句柄生命周期见 [公共 API](../reference/public-api.md)。ABI 检查示例：

```c
if (tswn_capi_abi_version() != 4) {
    /* 拒绝加载不匹配的动态库 */
}
```

从 `tswn_capi 0.5.x` 升级时必须重新编译并重新链接调用方。

## WASM 与 replay

0.5 迁移保留 `FightSession`、`WinRateSession` 与原有 replay shape。当前普通战斗使用 `BattleSession` 的 canonical DTO，旧 `FightSession` / `WinRateSession` 属于兼容或进阶入口；定义以 [WASM battle 绑定](../../crates/tswn_wasm/src/battle.rs) 与 [共享 battle DTO](../../crates/tswn_core/src/cli_api/battle/dto.rs) 为准。正文渲染只读取 clip 的 `parts[]`：

- HP swap 的双方 part 各自携带正确血条；
- 百分比伤害的血条使用真实 score；
- 机制死亡把 HP 同步为 `0` 并显示死亡特效；
- 带死亡特效的 part 不同时显示血条。

不要恢复已删除的 clip 顶层文本、HP 或死亡效果字段。

## 回归门禁

```powershell
python scripts/check_runtime_release.py --corpus
```

该入口运行 release/no_debug 主 Runtime、release CLI 和完整冻结 corpus。压力 golden 验证有效输入 SHA-256、`eval_rq`、胜者、回合数、总分、最终 RC4 和逐回合 canonical digest。0.5 迁移时的基线包含 87 个 JS exact trace 与 37 个压力 golden；实际执行范围由 Cargo 测试目标决定，不在脚本中重复冻结数量。

旧路径、已删除符号和执行器选择器的维护约束以本指南为准，不再扫描源码来禁止重新出现某个路径或字符串。
