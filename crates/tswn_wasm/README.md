# tswn_wasm

`tswn_core` 的浏览器端 WASM 接口（wasm-bindgen）。

## 目标

为网页前端提供 JS 友好的 wasm 封装，不依赖裸指针 / 手动释放 / C ABI 字符串。
可直接支撑类似 `fast-namerena/index.html` 的静态页面。

## 导出

### 顶层函数

| 函数                                                      | 说明                                                                |
| --------------------------------------------------------- | ------------------------------------------------------------------- |
| `version()`                                               | wasm 包装层版本                                                     |
| `core_version()`                                          | tswn_core 版本                                                      |
| `default_eval_rq()`                                       | 普通对局默认 eval_rq                                                |
| `win_rate_eval_rq()`                                      | 胜率语义默认 eval_rq                                                |
| `name_to_png_base64(name)`                                | 名称 → PNG Base64                                                   |
| `name_to_png_bytes(name)`                                 | 名称 → PNG 字节                                                     |
| `name_to_icon_rgba(name)`                                 | 名称 → 16×16 RGBA                                                   |
| `fight(raw_input, options?)`                              | 一次性跑完整局，返回 `FightReplay`                                  |
| `fight_summary(raw_input, options?)`                      | 轻量摘要，返回 `FightSummary`（赢家、玩家列表、最终状态）           |
| `win_rate_sync(raw_input, total_rounds, options?)`        | 同步跑完胜率统计，返回 `WinRateResult`                              |
| `group_win_rate(target, against, total_rounds, options?)` | 批量计算 target 对多个 opponent 的胜率，返回 `GroupWinRateResult[]` |
| `win_rate_summary(...)` / `team_win_rate_summary(...)`    | CLI 对齐的详细胜率摘要，返回 `CliWinRateResult`                     |
| `group_win_rate_summary(...)`                             | CLI 对齐的批量详细胜率摘要，返回 `CliGroupWinRateResult[]`          |
| `score(...)` / `namer_pf(...)`                            | CLI 对齐的评分 / 命配 helper                                        |
| `batch_rate(...)` / `pair_rate(...)`                      | CLI 对齐的批量对抗 / 配对评分 helper                                |
| `to_diy(...)` / `to_diy_batch(...)`                       | CLI 对齐的导出 helper                                               |
| `icon_info(name)` / `parse_group_lines(...)`              | CLI 对齐的图标元信息 / 分组解析 helper                              |

### FightSession

适合逐回合播动画：

```js
const session = new wasm.FightSession(rawInput, {
  eval_rq: 0.5, // 可选，名称评分参数
  include_icons: true, // 可选，是否生成 PNG Base64 头像
  capture_replay: true, // 可选，是否捕获逐帧回放（默认 true）
});
session.players(); // PlayerMeta[] — 玩家元数据列表
session.state(); // PlayerState[] — 当前全量状态快照
session.step(); // RoundFrame — 推进一步
//   { finished, winner_ids, updates, rows, states, total_delay }
session.is_finished(); // bool — 是否已产生胜者
session.winner_ids(); // number[] — 获胜方 ID 列表
session.run_to_end(limit); // FightReplay — 跳过动画直接结算，可限制最大帧数
```

### WinRateSession

增量式胜率统计，不阻塞主线程：

```js
const session = new wasm.WinRateSession(rawInput, 5000, {
  eval_rq: 0.8, // 可选，名称评分参数（默认 win_rate_eval_rq()）
  thread: 1, // 可选，wasm 目标下恒为 1
});
session.eval_rq(); // number — 当前 eval_rq 值
while (!session.is_finished()) {
  session.step(100); // batch_size 可选，默认 100；返回 WinRateProgress
  console.log(session.progress()); // { done, rounds_done, total_rounds, wins, percent }
}
session.result(); // WinRateResult — 含 timing（init_nanos, fight_nanos）
```

### 命名约定

- 顶层函数、类方法和 JSON 字段统一使用 snake_case。
- `options` 也使用 snake_case，例如 `eval_rq`、`include_icons`、`capture_replay`。

### 数据模型

| 类型                 | 说明                                                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `PlayerMeta`         | 玩家元数据：`id`, `team_index`, `id_name`, `display_name`, `icon_png_base64?`                                                                          |
| `PlayerState`        | 玩家状态：`hp`, `max_hp`, `magic_point`, `attack`, `defense`, …, `owner_id?`, `display_index`, `alive`, `status_labels?`（实时状态标签，如 `"聚气"`/`"隐匿"`/`"狂暴"`） |
| `RoundFrame`         | 回合帧：`finished`, `winner_ids`, `updates[]`, `rows[]`, `states[]`, `total_delay`                                                                     |
| `UpdateView`         | 单条更新消息：`caster_id`, `target_id`, `message_rendered`, `message_template`, `tone`, …                                                              |
| `ReplayRow`          | 回放行：`indent`, `clips[]`；一般首行不缩进，后续行缩进                                                                                                 |
| `ReplayClip`         | 回放片段：`delay`, `text_template`, `parts[]`, `color`（`[]` 高亮文字 6 位色号）, `tone`, `player_id`, `show_hp`, `hp_before`, `hp_after`, `death_effect`, `sidebar_states[]` 等 |
| `ReplayTextPart`     | 文本片段：`kind`, `text`, `player_id?`, `show_hp`, `hp_before`, `hp_after`, `death_effect`, `emoji?`；`kind` 为 `text` / `highlight` / `player` / `data` |
| `MessageTone`        | 消息色调：`"normal"` / `"damage"` / `"recover"` / `"knockout"` / `"status_exit"`                                                                        |
| `FightReplay`        | 完整回放：`players`, `frames[]`, `winner_ids`, `final_states`                                                                                          |
| `FightSummary`       | 轻量摘要：`finished`, `players`, `winner_ids`, `final_states`                                                                                          |
| `WinRateProgress`    | 增量进度：`done`, `rounds_done`, `total_rounds`, `wins`, `percent`                                                                                     |
| `WinRateResult`      | 最终结果：`done`, `rounds_done`, `total_rounds`, `wins`, `percent`, `timing?`                                                                          |
| `WinRateTiming`      | 耗时统计：`init_nanos`, `fight_nanos`（wasm32 下均为 0）                                                                                               |
| `GroupWinRateResult` | 批量胜率结果：`opponent`, `result`                                                                                                                     |
| `Cli*Result`         | 与 `tswn-cli` / `tswn_py` 高层 helper 对齐的一组结果类型                                                                                                |

`display_index` 是底层分配的展示序号：普通本体为 `0`；同名分身在名字中显示为 `#1`、`#2`……。
左侧仍会单独显示对象 `#playerId`，用于区分唯一对象编号。`ReplayClip.delay` 按句子级规则给出：
frame 首句 `900ms`，雷击/地裂行首句 `150ms`，展示血条的句子 `600ms`，其他句子 `500ms`，按该顺序优先匹配。

### 错误

所有可能失败的函数返回 `WasmResult<T>`（即 `Result<T, JsValue>`），错误对象结构为 `{ code: string, message: string }`。

错误码：

- `INVALID_INPUT` — 输入为空或格式错误
- `INVALID_OPTIONS` — options 解析失败
- `RUNNER_INIT_FAILED` — Runner 初始化失败
- `WIN_RATE_INVALID_GROUPS` — 胜率统计要求至少两个非空分组
- `INTERNAL_ERROR` — 内部异常

### 实时状态标签

`PlayerState.statusLabels` 会在每个回合帧中收集玩家当前生效的技能运行时态和状态效果，前端可直接用于渲染状态提示 pill。

正面状态（绿色）：`聚气`、`蓄力`、`隐匿`、`潜行`、`狂暴`、`疾走`、`铁壁`、`守护`

负面状态（红色）：`魅惑`、`诅咒`、`冰冻`、`中毒`、`迟缓`、`垂死`

`show.html` 演示页面已内置状态标签渲染，在玩家面板中显示彩色 pill。

### 头像 Sprite 渲染

`show.html` 演示页面使用 CSS Sprite 方式渲染玩家头像，不再生成大量 `<img src="data:...">` DOM 节点：

- `show-utils.js` 提供 `iconClassName()` / `buildIconClassCss()` 工具函数，将玩家的 PNG Base64 头像编码为 `.icon_N { background-image: url(...) }` 样式规则。
- `show.js` 在回放开始时调用 `normalizeReplayPlayers()` 为玩家补齐 `iconClassId`（同队统一使用队首个玩家头像编号），并通过 `syncIconStyles()` 动态注入 `<style>` 标签。
- 渲染层（`show-render.js` `/` `show-replay.js`）统一使用 `renderIconSprite(iconId, className)` 生成 `<span class="icon-sprite icon_N">` 节点，头像图片由 CSS background-image 加载。

#### 同队头像统一

多对多对局中，`withTeamIconClassIds()` 确保同队所有玩家共用该队第一个玩家的头像编号，保证整队头像视觉一致性。

### 示例

`examples/` 目录包含两套静态页面：

| 文件                                 | 说明                                                                                                                               |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| `demo.html` / `demo.js` / `demo.css` | 快速功能验证（战斗 + 胜率）                                                                                                        |
| `show.html` / `show.js` / `show.css` | 完整对局动画展示                                                                                                                   |
| `show-wasm.js`                       | WASM 模块加载与初始化                                                                                                              |
| `show-utils.js`                      | DOM 渲染工具函数（含头像 Sprite 工具 `iconClassName()` / `buildIconClassCss()` / `withTeamIconClassIds()` / `renderIconSprite()`） |
| `show-render.js`                     | 玩家状态 / 头像渲染（CSS Sprite 方式，`renderIconSprite()`）                                                                       |
| `show-replay.js`                     | 逐帧回放逻辑 + 结算表格（支持 HP 条显示）                                                                                          |

```bash
# 构建 wasm 分发目录后启动静态服务器
cd crates/tswn_wasm/dist/wasm
python -m http.server 8000
# 打开 http://127.0.0.1:8000/examples/demo.html
# 或   http://127.0.0.1:8000/examples/show.html
```

详细运行说明见 `examples/README.md`。

## 构建

```powershell
# 构建 wasm + wasm-bindgen 打包
uv run scripts/build_wasm.py --release
```

前置依赖：`wasm-bindgen-cli`

```powershell
cargo install wasm-bindgen-cli
```

## 设计

详见 [docs/tswn_wasm_design.md](../../docs/tswn_wasm_design.md)。

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
