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
- `win_rate` / `group_win_rate`
- icon RGBA / PNG / Base64
