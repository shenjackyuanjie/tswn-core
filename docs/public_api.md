# 对外 API 对齐约定

`BattleSession` 是 Rust、Python、WASM、C 的正式增量对局 API；`battle_replay` 收集同一个 session 的完整结果。`Runner`、`PreparedRunner`、WASM `FightSession` 和 `WinRateSession` 属于 Advanced / Compatibility API。

## 会话与结果

```rust
use tswn_core::cli_api::{BattleOptions, BattleSession};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = BattleSession::new("alpha\n\nbeta", BattleOptions::default())?;
    println!("{} players", session.initial_states().len());
    while let Some(frame) = session.next_frame()? {
        println!("frame={} round={}", frame.frame_index, frame.round_index);
    }
    let result = session.result().expect("terminal session");
    assert_eq!(result.frames_emitted, session.frames_emitted());
    Ok(())
}
```

三端方法保持一致：`initial_states()`、`current_states()`、`next_frame()`、`status()`、`stop_reason()`、`is_done()`、`is_finished()`、`is_truncated()`、`rounds_advanced()`、`frames_emitted()`、`result()`。C 通过对应 JSON / enum 输出参数提供这些状态；计数和完成标记包含在 result JSON 中。

| 参数 | 默认值 | 语义 |
| --- | --- | --- |
| `eval_rq` | `DEFAULT_EVAL_RQ` | 普通对局评分参数，必须有限 |
| `include_icons` | `false` | 是否在状态中带 PNG Base64；关闭时字段为 null |
| `max_rounds` | `20_000` | Runtime `main_round()` 的调用次数上限，必须大于零 |

`next_frame()` 自动吸收空 round，返回下一可见 frame，或在正常终止后返回 `None` / `null`。空 round 也消耗 `max_rounds`。`frame_index` 与 `round_index` 都从 0 开始：前者连续编号可见帧，后者记录真实推进轮次，允许跳号。空 updates 的获胜轮仍生成包含 winner row 的末帧。

| status | stop_reason | finished | truncated | winner |
| --- | --- | --- | --- | --- |
| `running` | null | false | false | result 尚不存在 |
| `finished` | `winner` | true | false | 由 Runtime 给出 |
| `truncated` | `max_rounds` / `no_progress` | false | true | 空数组 |

优先级为 Runtime 错误、winner、max_rounds、no_progress。连续无可见进展的轮次达到 `max(1, 当前实体数) * 16` 时停止；该策略只在 core 实现。终止后重复 `next_frame()` 总是返回空；运行中 `result()` 为 null，终止后返回稳定的 `BattleResult`。Runtime 错误以异常/错误码返回，不伪装为截断结果。

## 规范 DTO

字段统一 snake_case；Python 返回精确 TypedDict 对应的 dict，WASM 返回 plain JS object / array / null，C 返回 UTF-8 JSON。定义见 [Rust DTO](../crates/tswn_core/src/cli_api/battle/dto.rs)、[Python TypedDict](../crates/tswn_py/tswn_py/_types_battle.pyi)、[TypeScript DTO](../crates/tswn_wasm/src/battle_types.d.ts)。

- `BattlePlayerState`：完整身份、归属、队伍、图标 key、属性、HP/MP/体力和状态标签；动态 clone/summon/shadow/zombie 也使用相同 schema。
- `BattleReplayFrame`：`frame_index`、`round_index`、`finished`、`winner_ids`、`updates`、`rows`、`states`、`total_delay`。
- `rows[].clips[].parts[]`：文本、玩家、高亮、数值、HP before/after 和 death effect，展示语义由 core 提供。消费者不应再从消息模板或伤害值推断展示效果。
- `BattleResult`：`status`、`stop_reason`、`finished`、`truncated`、`rounds_advanced`、`frames_emitted`、`winner_ids`、`winner_team_indices`、`final_states`。
- `BattleReplay`：保留 `initial_states`、`frames`、全部 result 字段和固定为 `round` 的 `state_granularity`。`collect(BattleSession)` 与 `battle_replay` 的帧和结果相同。Rust 的 `BattleReplayOptions` 是已弃用兼容别名。

调用方应保持 canonical snapshots 不变。昵称、图标 CSS 类、播放计划、检查点都是显示层数据。Python/WASM 的快照脱离会话持有的 Rust 内存，修改返回对象不会修改 Runtime。

## Python

```python
import tswn_py

session = tswn_py.BattleSession("alpha\n\nbeta", include_icons=False, max_rounds=20_000)
initial = session.initial_states()
frames = list(session)  # 也可 for frame in session 逐帧消费
result = session.result()
assert result["frames_emitted"] == len(frames)
assert session.next_frame() is None
```

`battle_replay(raw, eval_rq=None, include_icons=False, max_rounds=None)` 返回 `BattleReplay` TypedDict。旧 Advanced tick 类型单独保留，不用于正式 battle DTO。

## WASM

初始化生成的 wasm-bindgen 模块后：

```js
const session = new wasm.BattleSession("alpha\n\nbeta", {
  include_icons: false,
  max_rounds: 20_000,
});
try {
  const initial = session.initial_states();
  const frames = [];
  for (let frame; (frame = session.next_frame()) !== null;) frames.push(frame);
  const result = session.result();
  console.assert(result.frames_emitted === frames.length);
} finally {
  session.free();
}
```

WASM 对象 handle 必须显式 `free()`。网页 source 在复制 terminal result、abort、重开或错误时释放，不依赖 GC。旧 `FightSession.run_to_end(limit)` 的 limit 只限制本次收集的帧数；未终止时可以继续调用，终止判断使用 `is_done()`。

## C ABI 4

使用 `tswn_battle_options_default(&options)` 初始化版本化 `tswn_battle_options_t` 后修改参数；传 NULL 使用默认值。`struct_size` 小于当前结构尺寸会拒绝，较大的未来尾部会忽略，`include_icons` 只接受 0 / 1。

`tswn_battle_session_new()` 创建 opaque handle；`tswn_battle_session_initial_states_json()` / `current_states_json()` 返回快照；`next_frame_json()` / `result_json()` 通过 `has` 标志区分有无数据。没有值时输出 `{NULL, 0}`，终止后的重复读取也如此。状态和原因另有 `status()` / `stop_reason()` enum 查询。

所有成功返回的 JSON 用 `tswn_str_free()` 释放，handle 用 `tswn_battle_session_free()` 释放。调用方不得并发操作同一 handle。完整可编译示例：[battle_session.c](../crates/tswn_capi/examples/battle_session.c)。

## CLI JSONL 输出

```sh
cargo run -p tswn_core --bin tswn-cli -- fight --jsonl -f input.txt --max-rounds 20000
cargo run -p tswn_core --bin tswn-cli -- runtime diff -f input.txt
cargo run -p tswn_core --bin tswn-cli -- runtime normalized-run -f input.txt --max-rounds 20000
```

`fight` 提供人类可读输出，`fight --jsonl` 每行输出 `{ "type": "initial" | "frame" | "result", "data": payload }` 并立即 flush。stdout 没有 banner 或 error event；失败写 stderr 并非零退出。旧 `raw`、顶层 `diff`、`--out-raw` 与 `!test!` 自动路由已删除，评分/胜率使用明确的 `bench` 子命令。

## 网页流式播放

`createBattleStreamSource` 先返回 initial states，`BattleStreamController` 按需拉帧并保留历史，最多预取 2 帧。暂停停止补帧；回退/重播只消费历史；返回实时尾部后继续同一个 session。observer 提供 initial/frame/result/error/disposed，异步 source 接口可替换，当前不使用 Worker。

`?perf=1` 在完整播放结束后打印 TTIS、TTFE、拉帧和渲染统计，不采集原始输入。参见 [浏览器性能基线](perf/web_streaming_baseline.md)。

## 错误与其他高层 API

core 定义六个稳定错误码：`INVALID_INPUT`、`INVALID_ARGUMENT`、`UNSUPPORTED_OPTION`、`RUNNER_INIT_FAILED`、`RUNTIME_FAILED`、`INTERNAL_ERROR`。Python 异常提供 `.code`，WASM 抛出 `{ code, message }`，C 返回 status 并提供 `tswn_last_error_code()` / `tswn_last_error_message()`。

胜率摘要（`win_rate_summary` / `team_win_rate_summary` / `group_win_rate_summary`）、评分（`score` / `namer_pf` / `batch_rate` / `pair_rate`）、工具（`to_diy` / `to_diy_batch` / `icon_info` / `parse_group_lines`）仍按高层 API 对齐；`default_custom_runtime_normalized_run` 是诊断接口。PreparedRunner 的 `eval_rq` 在创建时固定。

验证入口：`scripts/verify_battle_cross_binding.py` 使用真实 Rust CLI、Python 扩展、C 动态库和 Node WASM，逐字段精确比较四组输入在完整/限轮两种设置下的 initial、frames、result。
