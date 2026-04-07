# 更新日志

## [0.1.2] - 2026-04-07

### 新增

- 为高层胜率接口新增 `thread: u32` 参数，统一支持 `0=自动线程数`、`1=单线程`、`n=指定多线程数量`：
  - `tswn_win_rate(...)`
  - `tswn_win_rate_with_eval_rq(...)`
  - `tswn_group_win_rate(...)`
  - `tswn_group_win_rate_with_eval_rq(...)`
  - `tswn_prepared_win_rate(...)`
  - `tswn_prepared_win_rate_with_eval_rq(...)`
- `tswn_capi` 胜率统计内部改为支持基于 `PreparedRunner` 的多线程执行，线程分发策略参考 `tswn_cli`：自动线程模式下优先使用 `available_parallelism()`，并按总局数自动收敛。
- 补充了头文件注释、README、examples README 与 C 示例中的 `thread` 参数使用说明。

### 修复

- 修复 `tswn_prepared_win_rate(...)` / `tswn_prepared_win_rate_with_eval_rq(...)` 的 JS profile seed 调度偏移：首局保持无 seed，后续局数改为从 `seed:33554431@! + i` 开始递增，避免 prepared 胜率结果整体错位一局。

## [0.1.1] - 2026-04-06

### 新增

- 新增输入队伍查询辅助接口：可根据 `player_id` 直接查询其对应的原始输入 `group_index`，便于调用方按输入队伍归属判断胜者属于哪一组，而不必再自行扫描 `input_groups` roster 构造映射。
- 新增基于 `PreparedRunner` 的高层胜率接口：
  - `tswn_prepared_win_rate(...)`
  - `tswn_prepared_win_rate_with_eval_rq(...)`
  便于调用方在已经持有 prepared 模板时，直接复用同一份预处理结果批量统计第一组对其余组的胜率，而不必每次重新从 raw 输入计算。
- Windows 构建现在同时产出 `tswn_capi.lib`（`staticlib`），便于 C / C++ 调用方在需要时将 `tswn_capi` 静态链接进最终可执行文件，而不只依赖 `tswn_capi.dll` / `tswn_capi.dll.lib` 的动态链接形式。
- 补充了 `PreparedRunner` 的 seed 使用说明：`tswn_runner_new_from_prepared(...)` 的 seed 参数应传入与 raw 文本一致的完整 `seed:...` 行，而不是只传裸 seed 值。
- 补充了 Windows C++ 编译与链接说明文档，覆盖 `clang++` 动态/静态链接、`g++` 动态链接兼容性、以及运行时 DLL 查找方式等常见问题。
- 新增 `examples/prepared_win_rate.c`，演示如何在 C 层先构造 `PreparedRunner`，再基于它重复构造 runner 并统计胜率。

### 说明

- 该接口返回的是原始输入顺序对应的队伍索引语义，可与 `tswn_runner_input_group_count` / `tswn_runner_input_group_copy` 保持一致理解。
- 对 `PreparedRunner` 而言：
  - 不传 seed 时，应传 `NULL`
  - 传 seed 时，应传完整字符串，例如 `seed:33554431@!`
  - 不建议只传 `33554431@!` 这类裸 seed 值，否则可能与 raw 路径结果不一致
- `tswn_prepared_win_rate(...)` / `tswn_prepared_win_rate_with_eval_rq(...)` 的定位是“高性能复用版胜率接口”：
  - 适合调用方已经持有 `PreparedRunner` 的场景
  - 内部会复用 prepared 模板，按当前胜率语义批量构造 runner 并统计第一组胜率
- 对 Windows `staticlib` 而言，实际链接时可能还需要补充系统库；当前已验证的一种情况是 `clang++` 静态链接 `tswn_capi.lib` 时需要显式补 `ntdll.lib`。

## [0.1.0] - 2026-04-06

### 新增

- 新增 `tswn_capi` crate，提供基于 `tswn_core` 的 DLL C-API。
- 新增 C header：`include/tswn_capi.h`。
- 为 `tswn_capi.h` 补充公开注释，明确输入编码、输出所有权和主要接口语义。
- 新增基础 ABI 能力：
  - `tswn_status_t`
  - `tswn_str_t`
  - `tswn_bytes_t`
  - `tswn_win_rate_result_t`
  - `tswn_player_snapshot_t`
  - `tswn_update_snapshot_t`
- 新增 `Runner` / `PreparedRunner` / `RunUpdates` 的 opaque handle 与释放接口。
- 新增基础导出接口：
  - ABI/version 查询
  - 默认 `eval_rq` / win-rate `eval_rq` 查询
  - last-error 查询与清理
- 新增对局相关接口：
  - 从原始 UTF-8 输入构造 `Runner`
  - 从原始 UTF-8 输入构造 `PreparedRunner`
  - 从 `PreparedRunner` + seed 构造 `Runner`
  - `main_round` / `run_to_completion` / `have_winner`
- 新增结果查询接口：
  - `input_groups`
  - `winner`
  - `all_player_ids`
  - `player_snapshot`
  - `updates` / `targets` / message
- 新增高层 helper：
  - `tswn_win_rate`
  - `tswn_win_rate_with_eval_rq`
  - `tswn_group_win_rate`
  - `tswn_group_win_rate_with_eval_rq`
- 新增 icon 导出接口：
  - RGBA
  - PNG bytes
  - PNG Base64
- 新增 `examples/` 目录，补充以下 C 示例：
  - version / error
  - runner fight
  - prepared runner
  - updates round
  - player snapshot
  - win_rate
  - group_win_rate
  - icon

### 说明

- 输入字符串统一使用 UTF-8 `const char*`。
- 动态输出统一由库分配，并通过 `tswn_str_free` / `tswn_bytes_free` 释放。
- 胜率结果统一返回 `wins` / `total`，百分比由调用方自行计算。
