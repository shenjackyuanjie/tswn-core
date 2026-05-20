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

- `Runner` / `PreparedRunner` 生命周期
- `RunUpdates` 基本读取
- `win_rate` / `group_win_rate` / `prepared_win_rate`
- icon RGBA / PNG / Base64

## 版本与快照字段

- 版本查询：
  - `tswn_capi_version()`：返回 `tswn_capi` 包装层版本
  - `tswn_core_version()`：返回 `tswn_core` 版本
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
