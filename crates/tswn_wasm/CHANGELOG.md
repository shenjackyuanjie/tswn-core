# 更新日志

## [0.1.0] - 2026-04-29

### 新增

- 新增 `tswn_wasm` crate，提供面向浏览器前端的 `wasm-bindgen` 封装层，不再沿用 `tswn_capi` 的裸指针 / 手动释放式接口。
- 新增顶层 wasm 导出：
  - `version()`
  - `core_version()`
  - `name_to_png_base64(name)`
  - `fight(raw_input, options)`
  - `fight_summary(raw_input, options)`
  - `win_rate_sync(raw_input, total_rounds, options)`
- 新增 `FightSession`，支持：
  - `players()`
  - `state()`
  - `is_finished()`
  - `winner_ids()`
  - `step()`
  - `run_to_end(limit)`
- 新增 `WinRateSession`，支持浏览器侧分批推进胜率统计：
  - `is_finished()`
  - `progress()`
  - `step(batch_size)`
  - `result()`
- 新增前端即用的数据模型导出语义：
  - 玩家元数据 `PlayerMeta`
  - 玩家状态快照 `PlayerState`
  - 渲染后的更新消息 `UpdateView.messageRendered`
  - 完整回放 `FightReplay`
  - 增量胜率结果 `WinRateProgress` / `WinRateResult`
- 新增错误对象封装：Rust 侧错误统一转成 `{ code, message }` 风格的 JS 可读对象。

### 兼容性

- `WinRateSession` 内部对齐 `tswn_core` 当前 wasm 兼容策略：浏览器目标下胜率路径保持单线程，避免进入 `std::thread` 分支。
- 战斗日志在 wasm 层额外做显示名渲染，不再直接暴露 `RunUpdate::msg()` 的数字 `PlrId` 替换结果。

### 示例

- 新增 `examples/` 目录，提供最小静态页面 demo：
  - `examples/demo.html`
  - `examples/demo.js`
  - `examples/README.md`
- 示例覆盖：
  - 初始化 wasm 包
  - 创建 `FightSession` 并逐步推进回合
  - 调用 `fight_summary(...)`
  - 创建 `WinRateSession` 并分批显示进度
