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
print(tswn_py.wrapper_version_str(), tswn_py.core_version_str())

# 图标渲染
b64 = tswn_py.name_to_png_base64("某个玩家名")

# 正式增量对局接口
raw_input = "alpha\n\nbeta"
session = tswn_py.BattleSession(raw_input, max_rounds=20_000)
for frame in session:
    print(frame["frame_index"], frame["round_index"])
result = session.result()

# PreparedRunner 复用胜率
groups, _ = tswn_py.Runner.split_namerena_into_groups(raw_input)
prepared = tswn_py.Runner.prepare_groups(groups)
rate = prepared.win_rate(1000)
```

## BattleSession

`BattleSession(raw, eval_rq=None, include_icons=False, max_rounds=None)` 支持迭代器和 `next_frame()`；初始/当前快照用 `initial_states()` / `current_states()` 获取。`status()`、`stop_reason()`、完成标记、轮次/帧计数和 `result()` 与 [公共 API](../../docs/public_api.md) 一致。终止后 `next_frame()` 返回 None；运行中没有 result。

`max_rounds` 默认 20,000，统计真实 Runtime round，包括空轮；frame_index / round_index 均从零开始。正式 `BattleReplay` / `BattleResult` 等 TypedDict 位于 `_types_battle.pyi`。错误异常携带 core 的稳定 `.code`。

## 主要 API

### 顶层函数

| 函数                                                  | 说明                           |
| ----------------------------------------------------- | ------------------------------ |
| `core_version_str()`                                  | tswn_core 版本                 |
| `wrapper_version_str()`                               | tswn_py 版本                   |
| `name_to_png_base64(name)`                            | 名称 → PNG Base64              |
| `name_to_png_bytes(name)`                             | 名称 → PNG 字节                |
| `name_to_icon_rgba(name)`                             | 名称 → 16×16 RGBA              |
| `win_rate(raw, n, eval_rq, thread)`                   | 胜率统计                       |
| `group_win_rate(target, against, n, eval_rq, thread)` | 分组胜率                       |
| `prepared_win_rate(prepared, n, eval_rq, thread)`     | 基于 PreparedRunner 的胜率统计 |
| `win_rate_summary(raw, n, eval_rq, thread)`           | 胜率统计，返回胜场/总场次/耗时 |
| `team_win_rate_summary(team1, team2, n, ...)`         | 对齐 `bench win-rate` 的两队胜率 |
| `group_win_rate_summary(target, against, n, ...)`     | 对齐 `bench group-win-rate` 的多对手胜率 |
| `score(raw, n, mode, eval_rq, thread)`                | 对齐 score benchmark 的普通/`!` 评分 |
| `namer_pf(raw, n, modes, keep_rq, thread)`            | 对齐 `namer-pf` 的 pp/pd/qp/qd 评分 |
| `batch_rate(target_groups, player_groups, n, ...)`    | 对齐 `bench batch-rate` 的批量平均胜率 |
| `pair_rate(target_groups, players, teammates, ...)`   | 对齐 `bench pair` 的配队评分 |
| `to_diy(name, old, minions)`                          | 对齐 `to-diy` 的 DIY/OL overlay 导出 |
| `to_diy_batch(names, old, minions)`                   | 批量导出 DIY/OL overlay |
| `icon_info(name)`                                     | 对齐 `icon show` 的图标结构信息 |
| `parse_group_lines(content, double_plus)`             | 对齐 batch 列表文件的组解析 |
| `battle_replay(raw, ...)`                              | 推荐的一次性完整 UI 回放     |
| `compute_show_timeline(updates, player_count, scale)` | 按 show.html 语义计算事件播放延迟 |

CLI 对齐 helper 返回结构化对象，方便脚本继续处理：

- `WinRateResult`: `wins`、`total`、`win_rate`、`init_nanos`、`fight_nanos`
- `ScoreResult`: `score`、`wins`、`total`、`init_nanos`、`fight_nanos`
- `NamerPfResult`: `group`、`modes`、`scores`、`total_score`，以及 `as_line(precision)`
- `BatchRateResult`: `label`、`avg_win_rate`、`aggregate_win_rate`、`wins`、`total`、`valid_matchups`、`skipped_matchups`
- `PairRateResult`: `label`、`final_score`、`head`、`selected`、`top_pairs`、`aggregate_win_rate`、`wins`、`total`
- `IconInfo`: `border_style`、`shapes`、`bg_color`、`fg_colors` 等图标生成信息

示例：

```python
import tswn_py

wr = tswn_py.team_win_rate_summary("mario", "luigi", 1000, thread=1)
print(wr.win_rate, wr.wins, wr.total)

rows = tswn_py.namer_pf("mario+luigi\npeach", 1000, modes=["pp", "qd"], thread=1)
for row in rows:
    print(row.group, row.as_line(0))

targets = ["peach", "bowser"]
players = ["mario", "luigi"]
for result in tswn_py.batch_rate(targets, players, 1000, thread=1):
    print(result.label, result.avg_win_rate, result.skipped_matchups)

overlay = tswn_py.to_diy("mario@red+fire", minions=True)
```

注意：`to_diy(old=True, minions=True)` 与 CLI 的 `--old` / `--minions` 一样互斥，会抛出 `ValueError`。

`battle_replay(raw, eval_rq=None, include_icons=False, max_rounds=None)` 返回跨语言统一的 dict：包含初始/最终状态、`frames[].updates`、`frames[].rows[].clips[]`、赢家和完成状态；达到 `max_rounds` 或 `no_progress` 时返回 `truncated=True`，原因见 `stop_reason`。

`Runner`、`PreparedRunner`、RC4 与基础 `win_rate` 是 Advanced API。`PreparedRunner.eval_rq` 在创建时固定；`win_rate(..., eval_rq=...)` 只能省略或传入该相同值。

### 直播回放辅助

`Runner` 提供面向服务端直播/回放的高层接口：

```python
runner = tswn_py.Runner.new_from_namerena_raw(raw_input)

# 核心侧给出的胜利输入队伍，不需要从 winner player ids 反推。
winner_team = runner.winner_team_index()

# 标准玩家快照，包含原始玩家以及分身、召唤物等运行时实体。
states = runner.snapshot_players()

# 跑完整局并生成可直接广播给前端的 replay timeline。
replay = runner.build_replay()
for item in replay["events"]:
    event = item["event"]
    print(item["delay_ms"], event["tone"], event["message_rendered"])

# 新的 frames[].rows 可直接用于前端 replay view 渲染。
for frame in replay["frames"]:
    for row in frame["rows"]:
        for clip in row["clips"]:
            print(clip["delay"], clip["parts"])

# 已有 RunUpdates 也可以单独计算 show.html 风格的延迟。
timeline = tswn_py.compute_show_timeline(updates, player_count=len(states))
```

`RunUpdate.to_dict(rendered=True)` 会返回结构化事件字段，包括 `type`、`tone`、
`message_template`、`message_rendered`、`caster_id`、`target_id`、`target_ids`、
`param`、`score`、`delay0`、`delay1`、`is_win` 和 `is_next_line`。

`build_replay()` 返回 `initial_states`、`events`、`frames`、`final_states`、`winner_team_index`、
`winner_team_indices`、`winner_ids` 和 `winner_names`。`events` 保留兼容旧 timeline 消费方式；
`frames[].rows[].clips[]` 是与 WASM 共用的 replay view 结构，clip 包含 `delay`、`parts`、`color`
（`[]` 高亮文字色号）、`tone`、关联玩家 id、侧栏状态快照和胜利片段标记；文本、玩家、数值、血条前后值、死亡特效标记和 emoji 占位字段都在 `parts[]` 内。调用方可以直接渲染这些结构化字段，
无需再根据事件文本模拟扣血、召唤、复活或状态变化。事件快照仍使用每个 tick 前后的真实引擎状态
（`state_granularity == "tick"`）。

玩家快照中的 `display_index` 用于展示同名分身序号：本体为 `0`，后续分身为 `1`、`2`……。
replay view 中的玩家文本会使用该序号，而唯一对象编号仍保留在 `id` 字段中。

### 类

| 类               | 说明                                          |
| ---------------- | --------------------------------------------- |
| `Runner`         | 主 Runtime 对战会话，支持逐回合推进或一次跑完 |
| `PreparedRunner` | 预处理后的复用模板，支持 `win_rate(...)`      |
| `RunUpdates`     | 回合更新容器                                  |
| `RunUpdate`      | 单条结构化更新                                |
| `RC4`            | RC4 状态与算法                                |

0.5.0 不再暴露 `Storage`、`WorldState`、`Player` 或 `Runner.round_tick*`。状态查询改用
`snapshot_players()`、`winner_team_index()` / `winner_team_indices()`、`alives*()` 与
`rc4`；逐回合推进统一使用 `main_round()`。

## 构建

详细构建说明见 [README_BUILD.md](./README_BUILD.md)。

```powershell
# 构建 wheel
uv run scripts/build_py.py

# 构建并验证
uv run scripts/build_py.py --clean --verify

# 验证 CLI 对齐的 Python helper
python scripts/verify_py_cli_api.py
```

## 要求

- Python ≥ 3.12
- Windows / Linux / macOS

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
