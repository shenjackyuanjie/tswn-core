# 更新日志

## [0.1.0] - 2026-04-29

### 新增

- 新增 `tswn_wasm` crate，提供面向浏览器前端的 `wasm-bindgen` 封装层，不再沿用 `tswn_capi` 的裸指针 / 手动释放式接口。
- 新增顶层 wasm 导出：
  - `version()`
  - `core_version()`
  - `name_to_png_base64(name)`
  - `fight(raw_input, options?)` — 一次性跑完整局，返回 `FightReplay`
  - `fight_summary(raw_input, options?)` — 轻量摘要，返回 `FightSummary`
  - `win_rate_sync(raw_input, total_rounds, options?)` — 同步跑完胜率统计，返回 `WinRateResult`
- 新增 `FightSession`，支持逐回合推进：
  - `players()` — 返回 `PlayerMeta[]`
  - `state()` — 返回当前全量 `PlayerState[]`
  - `is_finished()` — 是否已产生胜者
  - `winner_ids()` — 获胜方 ID 列表
  - `step()` — 推进一步，返回 `RoundFrame`（含 `updates`、`states`、`total_delay`、`finished`、`winner_ids`）
  - `run_to_end(limit?)` — 推进至结束，可选限制最大帧数，返回 `FightReplay`
- 新增 `WinRateSession`，支持浏览器侧分批推进胜率统计：
  - `is_finished()`
  - `progress()` — 返回 `WinRateProgress`
  - `step(batch_size?)` — 推进指定局数（默认 100），返回 `WinRateProgress`
  - `result()` — 返回 `WinRateResult`（含 `timing` 耗时信息）
  - `eval_rq()` — 返回当前使用的 eval_rq 值
- 新增前端即用的数据模型导出：
  - 玩家元数据 `PlayerMeta`（含 `icon_png_base64`，由 `include_icons` 选项控制）
  - 玩家状态快照 `PlayerState`（含 `owner_id` 追溯召唤单位归属）
  - 渲染后的更新消息 `UpdateView`（含 `message_rendered`、`message_template`、`tone`）
  - 消息色调枚举 `MessageTone`：`Normal` / `Damage` / `Recover` / `Knockout`
  - 更新类型枚举 `UpdateTypeView`：`Win` / `None` / `NextLine`
  - 回合帧 `RoundFrame`（含 `total_delay` 供 JS 正常速度播放）
  - 完整回放 `FightReplay`
  - 轻量摘要 `FightSummary`
  - 增量胜率进度 `WinRateProgress` / 胜率结果 `WinRateResult`（含 `WinRateTiming`）
- 新增 `FightOptions`：
  - `eval_rq` — 名称评分参数
  - `include_icons` — 是否包含 PNG Base64 头像
  - `capture_replay` — 是否捕获逐帧回放数据（默认 true，设为 false 可加速 `run_to_end`）
- 新增 `WinRateOptions`：
  - `eval_rq` — 名称评分参数（默认使用 `WIN_RATE_EVAL_RQ`）
  - `thread` — 线程数（wasm 目标下恒为 1）
- 新增错误对象封装：Rust 侧错误统一转成 `{ code, message }` 风格的 JS 可读对象。错误码包括：
  - `INVALID_INPUT`
  - `INVALID_OPTIONS`
  - `RUNNER_INIT_FAILED`
  - `WIN_RATE_INVALID_GROUPS`
  - `INTERNAL_ERROR`

### 兼容性

- `WinRateSession` 内部对齐 `tswn_core` 当前 wasm 兼容策略：浏览器目标下胜率路径保持单线程，避免进入 `std::thread` 分支。
- 战斗日志在 wasm 层额外做显示名渲染（`message_rendered`），同时保留原始模板（`message_template`），不再直接暴露 `RunUpdate::msg()` 的数字 `PlrId` 替换结果。
- wasm32 目标下不测量耗时（`WinRateTiming` 中各 nanos 字段为 0），非 wasm 目标正常记录。

### 示例

- 新增 `examples/` 目录，提供两套静态页面 demo：
  - `demo.html` / `demo.js` / `demo.css`：快速功能验证（战斗 + 胜率）
  - `show.html` / `show.js` / `show.css`：完整对局动画展示
    - `show-wasm.js`：WASM 模块加载与初始化
    - `show-utils.js`：DOM 渲染工具函数
    - `show-render.js`：玩家状态 / 头像渲染
    - `show-replay.js`：逐帧回放逻辑
  - `examples/README.md`：运行说明
- 示例覆盖：
  - 初始化 wasm 包
  - 创建 `FightSession` 并逐步推进回合
  - 调用 `fight_summary(...)`
  - 创建 `WinRateSession` 并分批显示进度
  - 完整对局动画播放（含头像、状态条、更新消息渲染）
