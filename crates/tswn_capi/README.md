# tswn_capi

`tswn_capi` 提供基于 `tswn_core` 的 DLL C-API。

- 头文件：`include/tswn_capi.h`
- 输出形式：`cdylib` + `staticlib`
- 字符串输入统一为 UTF-8 `const char*`
- 动态输出统一使用 `tswn_str_t` / `tswn_bytes_t`，并由库侧 `*_free` 释放
- Windows 下通常会同时产出：
  - `tswn_capi.dll`
  - `tswn_capi.dll.lib`（导入库，供动态链接使用）
  - `tswn_capi.lib`（staticlib，供静态链接使用）
- Linux/macOS 下仍以 `libtswn_capi.so` / `libtswn_capi.dylib` 等动态库产物为主

当前版本已覆盖：

- 正式 `BattleSession` 增量对局与 JSON DTO
- Advanced `Runner` / `PreparedRunner` 生命周期
- `RunUpdates` 基本读取
- `win_rate` / `group_win_rate` / `prepared_win_rate`
- `tswn-cli` 对齐的高层 helper（`*_json` / `tswn_to_diy`）
- icon RGBA / PNG / Base64

推荐使用 [跨语言高层 API 约定](../../docs/reference/public-api.md) 中的 JSON helper；`Runner` 句柄和基础 `win_rate` 仍保留为 Advanced API。

## BattleSession（ABI 4）

`tswn_battle_options_default()` 初始化包含 `struct_size` 的 options，默认 include_icons=0、max_rounds=20,000。`tswn_battle_session_new()` 也接受 NULL options。尺寸小于永久冻结的 V1 最小尺寸、零轮数、非有限 eval_rq 或非 0/1 图标开关会拒绝。

V1 prefix 永久兼容，旧 caller -> 新 library 继续可用：未提供的新尾字段使用 core 默认值，未知尾部由旧 library 忽略。实现只读取冻结的 `BattleOptionsV1`，不能随 public struct 的尺寸增长提高最小要求。

未来演进必须遵循：

- 每个版本永久保留冻结的 prefix size（`BATTLE_OPTIONS_V1_SIZE`、V2_SIZE 等）。
- 新字段 offset 必须 >= V1_SIZE，禁止复用 V1 tail padding；必要时增加显式 padding / reserved 区域。
- 按 `struct_size >= field_end_offset` 单独读取每个扩展字段，缺失时使用 `BattleOptions::default()` 对应值。
- 本轮不改变 V1 layout，不新增 options 字段，ABI 仍为 4。

用 `initial_states_json()` 读取初始状态，反复 `next_frame_json()` 消费 frame，最后 `result_json()` 读取结果。后两者使用 `has` 标志，没有值时输出空字符串结构 `{NULL,0}`。状态/原因可以用 `status()` / `stop_reason()` 查询；计数、赢家与最终状态都包含在结果 JSON 中。所有函数名以 `tswn_battle_session_` 开头。

字符串使用 `tswn_str_free()`，会话使用 `tswn_battle_session_free()` 释放。终止后 next_frame 幂等，错误不冒充 truncated。参见 [完整 C 示例](examples/battle_session.c) 与 [跨语言 contract](../../docs/reference/public-api.md)。本轮增加符号，ABI 保持 4。

`tswn_battle_options_default()` 永久只写入 V1 并将 `struct_size` 设置为 V1_SIZE，避免新 library 覆盖旧 caller 的小缓冲区。未来扩展选项需另增带容量参数的初始化接口；不能扩大此历史函数的写入范围。

## 版本与快照字段

- 版本查询：
  - `tswn_capi_version()`：返回 `tswn_capi` 包装层版本
  - `tswn_core_version()`：返回 `tswn_core` 版本
- `tswn_capi_abi_version()`：0.6.0 返回 ABI `4`；从 ABI 3 升级的调用方需要重新编译并重新链接。
- `tswn_player_snapshot_t` 的蓝量字段统一使用 `magic_point`；不再提供 `mp` 别名。

## 胜率接口线程参数

`tswn_capi` 的高层胜率接口现在都带有 `thread` 参数：

- `0`：自动线程数
- `1`：单线程
- `n`：指定多线程数量

覆盖接口：

- `tswn_win_rate(...)`
- `tswn_win_rate_with_eval_rq(...)`
- `tswn_group_win_rate(...)`
- `tswn_group_win_rate_with_eval_rq(...)`
- `tswn_prepared_win_rate(...)`
- `tswn_prepared_win_rate_with_eval_rq(...)`

自动线程数策略与 `tswn_cli` 保持一致：优先使用 `available_parallelism()`，再按当前总局数上限收敛。

## CLI 对齐高层接口

为避免在 C ABI 上暴露大量变长结构体，`tswn_capi` 对 `tswn-cli` / `tswn_py` 的高层 helper 统一补了一层字符串/JSON 导出：

- 详细胜率：`tswn_win_rate_summary_json(...)` / `tswn_team_win_rate_summary_json(...)` / `tswn_group_win_rate_summary_json(...)`
- 评分与命配：`tswn_score_json(...)` / `tswn_namer_pf_json(...)`
- 批量对抗与配对：`tswn_batch_rate_json(...)` / `tswn_pair_rate_json(...)`
- 导出与解析：`tswn_to_diy(...)` / `tswn_to_diy_batch_json(...)` / `tswn_icon_info_json(...)` / `tswn_parse_group_lines_json(...)`
- 完整回放：`tswn_battle_replay_json(...)`，返回可直接渲染的 initial_states / final_states / frames / rows / clips JSON
- 标准化轨迹：`tswn_default_custom_runtime_normalized_run_json(...)`

这些接口返回的 `tswn_str_t` 都需要由调用方使用 `tswn_str_free()` 释放。

高层调用失败后，除 status 外还可通过 `tswn_last_error_code()` 与 `tswn_last_error_message()` 获取稳定错误码和可读说明。
