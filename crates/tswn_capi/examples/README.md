# C-API Examples

本目录提供 `tswn_capi` 的最小 C 示例。

- `version_and_error.c`: 版本、常量与错误读取
- `runner_fight.c`: 从 raw 输入创建 `Runner` 并跑完整场对局
- `prepared_runner.c`: 复用 `PreparedRunner` 构造多场对局
- `updates_round.c`: 读取 `main_round()` 返回的更新帧
- `win_rate.c`: 调用 `tswn_win_rate`
- `group_win_rate.c`: 调用 `tswn_group_win_rate`
- `icon.c`: 导出 RGBA / PNG / Base64 图标
- `player_snapshot.c`: 读取玩家快照

这些示例默认通过相对路径包含 `../include/tswn_capi.h`。
