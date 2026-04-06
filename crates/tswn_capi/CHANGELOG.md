# 更新日志

## [0.1.1] - 2026-04-06

### 新增

- 新增输入队伍查询辅助接口：可根据 `player_id` 直接查询其对应的原始输入 `group_index`，便于调用方按输入队伍归属判断胜者属于哪一组，而不必再自行扫描 `input_groups` roster 构造映射。

### 说明

- 该接口返回的是原始输入顺序对应的队伍索引语义，可与 `tswn_runner_input_group_count` / `tswn_runner_input_group_copy` 保持一致理解。

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
