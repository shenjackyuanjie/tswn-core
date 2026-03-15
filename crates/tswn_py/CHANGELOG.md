# 更新日志

## [0.1.5] - 2026-03-15

### 新增

- 扩展 `Runner` Python API，新增：
  - `split_namerena_into_groups`
  - `main_round`
  - `alives_flat` / `alives`
  - `all_plrs` / `all_plr_len`
- 扩展 `WorldState` Python API，新增：
  - `players`、`winner`、`all_plrs`、`all_plr_len`
  - `roster_count`、`team_index_of`、`team_roster`、`team_alive`
  - `contains_alive`、`winner_roster`
- 扩展 `Storage` Python API，新增分组/存活组/pending spawn 查询与状态接口：
  - `get_player_or_pending_by_id`、`get_pending_spawn_player_by_id`
  - `get_group`、`group_containing`、`group_index_of`
  - `alive_group_containing`、`alive_group_at_team_of`
  - `all_alive_ids`、`all_player_ids`
  - `pending_spawn_count`、`pending_spawn_count_for_owner`
  - `pending_spawn_ids_for_owner`、`pending_spawn_ids_for_group`
  - `alive_group_count`、`needs_sync`
- 扩展 `Player` Python API，新增更多运行时与状态只读字段（如 HP/MP、攻防速敏魔抗智、`player_type`、`weapon_name` 等）。
- 扩展 `RC4` Python API，新增构造与更多核心方法暴露（`update`、`round`、`next_u8`、`next_i32`、`xor/encrypt/decrypt`、`cXX`、`rXX` 系列等）。
- 扩展 `RunUpdates` / `RunUpdate` Python API：
  - `RunUpdates.on_update_end`、`RunUpdates.len()`、`RunUpdates.is_empty()`
  - `RunUpdate.is_win()`、`RunUpdate.is_none()`、`RunUpdate.is_next_line()`

### 变更

- 更新 `tswn_py.pyi` 类型存根，与新增暴露 API 保持一致。

## [0.1.4] - 2026-03-15

### 新增

- 重构 Python 绑定代码，将包装类分离到独立模块
  - 新增 `player.rs` 和 `rc4.rs`，分别实现 `Player` 和 `RC4` 的 Python 包装
  - 重命名 `wrapper.rs` 为 `wrapper/mod.rs`，整理模块结构
- 完善类型存根文件
  - 多次更新 `__init__.pyi` 和 `tswn_py.pyi`，确保类型定义与 Rust 实现一致
  - 修正了 `Storage` 和 `RC4` 的类型注解
- 添加版本管理模块 `_version.py`，统一管理包版本
- 改进错误处理，完善 `error.rs` 中的异常定义
- 更新项目配置（`Cargo.toml`、`MANIFEST.in`、`pyproject.toml`）以支持正确打包
- 添加 `CHANGELOG.md` 文件，开始记录版本历史

## [0.1.3] - 2026-03-15

### 新增

- 新增包装类
  - `RC4`
    - 核心算法实现
  - `Storage`
    - 玩家数据存储和管理
  - `WorldState`
    - 世界状态管理和同步

## [0.1.2] - 2026-03-14

### Fixed

- 修复了RunUpdates -> RunUpdate滚木的问题
