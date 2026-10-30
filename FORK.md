# 本 fork 的改动（名竞尖塔 · 卡牌对决）

上游：[shenjackyuanjie/tswn-core](https://github.com/shenjackyuanjie/tswn-core) 的本地副本，
只为一件事服务：让卡牌前端能把「这个单位这一次行动用哪个技能」交给引擎执行，
并且保证玩家角色（本体 + 分身）的每个动作都由玩家发出。

构建方式（Windows，GNU 工具链 + MinGW）：

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Strawberry\c\bin;$env:PATH"
cargo build -p tswn_wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir dist target\wasm32-unknown-unknown\release\tswn_wasm.wasm
```

产物（`dist/`）复制到游戏仓库的 `vendor/tswn_wasm_duel/`。

## 改了哪些文件

| 文件 | 改动 |
| --- | --- |
| `crates/tswn_core/src/runtime/forced_action.rs` | **新增**：线程内的强制行动队列、受控队伍、暂停标记 |
| `crates/tswn_core/src/runtime/mod.rs` | 注册 `forced_action` 模块 |
| `crates/tswn_core/src/runtime/combat/round.rs` | 核心改动：消费强制行动、按抽牌时定好的 MP 扣费、受控单位暂停、潜行→背刺窗口、狂暴接管、待发背刺的 pre-action 槽位补回 |
| `crates/tswn_core/src/cli_api/battle/dto.rs` | `BattlePlayerState.skills`、`BattleReplay.paused_actor`、`BattleOptions.controlled_team`、`BattleStopReason::Paused` |
| `crates/tswn_core/src/cli_api/battle/session.rs` | 暂停时截断回放、填充技能熟练度 |
| `crates/tswn_core/src/cli_api/battle/replay.rs` | 暂停时提前返回回放，设置/清理受控队伍 |
| `crates/tswn_wasm/src/lib.rs` | 导出 `battle_replay_forced(raw, actions, options)`，`actions` 是 `[actor, skill, mp]` 三元组 |
| `crates/tswn_wasm/src/model.rs` | wasm 侧选项映射（`controlled_team`） |
| `Cargo.toml` | release profile 加 `opt-level = "z"` 和 `strip = "symbols"`，wasm 从 1.52 MB 降到 942 KB |

## 接口语义

```js
const replay = wasm.battle_replay_forced(input, [actor, skill, mp, ...], {
  max_rounds: 20000,
  controlled_team: playerTeam,   // 玩家队伍编号（从 initial_states 取）
});
// replay.paused_actor === 实体 id → 这个单位在等玩家下指令，回放到此为止
// replay.paused_actor === null   → 战斗已结束，看 winner_team_indices
```

- `skill` 用 overlay 的公开编号 `0..34`，`255` 表示普通攻击；
- `mp` 是抽牌时就掷好的消耗（`8 + (byte & 15)`），传 `0` 表示让引擎现场掷；
- 受控队伍里只有**本体**和**分身**会暂停等指令；幻影 / 使魔 / 丧尸仍由引擎自动行动；
- 引擎自身的强制动作优先于玩家指令：潜行→背刺的第二段、狂暴期间的强制攻击，
  都不接受玩家插入技能，也不会停下来要牌。

## 注意

- 上游的 `.gitignore` 会忽略 `target`、`vendor`、`dist`、`*.txt` 等，构建产物不会进仓库；
- 改动最初来自手工副本。本分支以对应的上游提交 `5237f2b3` 为父提交重新整理，只带入卡牌引擎接口、构建设置和说明，保留上游历史及测试样例。原副本版本为 `f20f053`。
