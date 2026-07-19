# 主 Runtime 0.5 迁移指南

`tswn_core 0.5.0` 完成主 Runtime 独立化，并删除仓库内的旧执行器与对象模型。本次是有意的破坏性升级：不提供 Rust 兼容壳，也不再允许 CLI 或绑定层选择旧执行器。

## 版本边界

| 组件 | 新版本 | 兼容性说明 |
| --- | --- | --- |
| `tswn_core` | `0.5.0` | 删除旧 Rust 对象模型与 legacy API |
| `tswn_py` | `0.5.0` | 删除旧对象 getter、`round_tick*` 与 parity API |
| `tswn_wasm` | `0.5.0` | 会话改用主 Runtime，JS/JSON shape 保持不变 |
| `tswn_capi` | `0.6.0` | C ABI 版本由 `3` 升为 `4` |

上述版本已经按 2026-07-18 的发布边界封板；release commit、tag 与推送由仓库发布流程分别执行。

## Rust

根级 `Runner` 与 `PreparedRunner` 是唯一正式执行入口：

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

`fight`、`diff`、`raw`、`bench` 和 `runtime normalized-run` 都固定使用主 Runtime：

```powershell
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime normalized-run -f input.txt --max-rounds 20000
```

删除的命令面包括 `--runtime legacy`、其他执行器选择值和 `runtime parity`。需要验证行为时应运行冻结 corpus，而不是在生产 CLI 内切换执行器。

## Python

保留 `Runner`、`PreparedRunner`、`RunUpdate(s)`、RC4、snapshot、replay、胜率和评分接口。常见迁移如下：

| 已删除 | 替代方式 |
| --- | --- |
| `Runner.round_tick*()` | `Runner.main_round()` |
| `runner.world_state.winner` | `runner.winner_team_index()` / `winner_team_indices()` |
| `runner.storage`、`Storage`、`WorldState`、`Player` getter | `runner.snapshot_players()`、`alives*()`、`all_plrs()`、`input_groups` |
| parity helper | 运行仓库冻结 corpus |

`run_to_completion()` 与未显式传入 limit 的 `build_replay()` 使用 20,000 主回合保护上限。

## C

C 侧继续保留 opaque runner、prepared、updates、snapshot、score 与 win-rate 符号，只删除 `tswn_default_custom_runtime_parity_json`。公开函数签名和结构体布局不变，但 ABI 查询值已升级：

```c
if (tswn_capi_abi_version() != 4) {
    /* 拒绝加载不匹配的动态库 */
}
```

从 `tswn_capi 0.5.x` 升级时必须重新编译并重新链接调用方。

## WASM 与 replay

`FightSession`、`WinRateSession`、`ReplayClip` 和 `ReplayTextPart` 的 JS/JSON shape 保持 GitHub main 的定义。正文渲染只读取 `ReplayClip.parts[]`：

- HP swap 的双方 part 各自携带正确血条；
- 百分比伤害的血条使用真实 score；
- 机制死亡把 HP 同步为 `0` 并显示死亡特效；
- 带死亡特效的 part 不同时显示血条。

不要恢复已删除的 clip 顶层文本、HP 或死亡效果字段。

## 回归门禁

```powershell
python scripts/check_runtime_release.py --corpus
```

门禁会验证旧源码路径和禁用 API 未回流，并执行 87 个 JS exact trace 与 37 个冻结压力 golden，共 124 项。压力 golden 固定有效输入 SHA-256、`eval_rq`、胜者、回合数、总分、最终 RC4 和逐回合 canonical digest。
