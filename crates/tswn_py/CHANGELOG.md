# 更新日志

## [0.1.11] - 2026-04-07

### 新增

- Python 高层 `win_rate(raw, n, eval_rq=None, thread=0)` / `group_win_rate(target, against, n, eval_rq=None, thread=0)` 新增可选 `thread` 参数，语义与 CLI / C-API 对齐：
  - `0`：自动线程数
  - `1`：单线程
  - `n`：指定线程数
- Python prepared 胜率统计内部新增多线程执行路径，线程分发策略与 CLI / C-API 保持一致。

### 变更

- 更新顶层类型存根以对齐新的 `thread` 可选参数签名。

## [0.1.10] - 2026-04-07

### 修复

- 修复 Python 高层 `win_rate(...)` / `group_win_rate(...)` 在 prepared 路径下的 JS profile seed 调度偏移：首局继续不带 seed，后续局数改为从 `seed:33554431@! + i` 递增，避免与 CLI / C-API 的 prepared 胜率语义错开一位。

## [0.1.9] - 2026-04-06

### 新增

- `Runner` 新增：
  - `player_input_group_index(player_id)`：根据玩家 `id` 直接查询其对应的原始输入 `group_index`，便于调用方按输入队伍归属判断胜者属于哪一组，而不必再自行扫描 `input_groups` roster 构造映射。

### 变更

- 更新类型存根以对齐新的 `Runner.player_input_group_index()` 接口。

## [0.1.8] - 2026-04-05

### 新增

- `Runner` 新增：
  - `new_from_groups_with_seed_and_eval_rq(groups, seed, eval_rq)`
  - `prepare_groups(groups)`
  - `prepare_groups_with_eval_rq(groups, eval_rq)`
  - `new_from_prepared_with_seed(prepared, seed)`
- 新增 `PreparedRunner` 类型，用于复用预处理后的分组输入。
- `Runner` 新增 `input_groups` 属性，暴露原始输入顺序对应的队伍 roster。
- `Storage` 新增 `eval_rq` 属性。
- 顶层新增高层 helper：
  - `win_rate(raw, n, eval_rq=None)`
  - `group_win_rate(target, against, n, eval_rq=None)`
- 顶层新增常量：
  - `DEFAULT_EVAL_RQ`
  - `WIN_RATE_EVAL_RQ`
- 顶层新增 `name_to_icon_rgba(name)`，用于获取 16x16 RGBA 原始像素。
- 新增 Python examples：
  - `examples/runner_prepared.py`
  - `examples/runner_eval_rq.py`
  - `examples/win_rate.py`

### 变更

- 将 `tswn_py.pyi` 拆分为多个较短的 stub 文件，减少单文件维护成本。
- 更新类型存根以对齐 `tswn_core` 新公开的 runner / storage API。

## [0.1.7] - 2026-03-22

- 去掉了某些文字

## [0.1.6] - 2026-03-16

### 新增

- `Runner` 新增：
  - `new_from_groups_with_seed(groups, seed)`：对齐 core 新增构造接口
  - `round_tick_new_update_no_capture()`：返回 no-capture 更新容器
- `RunUpdates` 新增：
  - `new_no_capture()`：创建不采集详细帧的容器
  - `reset()`：对齐 core 的复用/重置语义
  - `capture_updates` 属性
  - `had_updates()` 活动标记查询

### 变更

- `RunUpdates.clear()` 内部行为对齐 core `reset()`，避免仅清列表导致活动标记未复位
- 更新 `tswn_py.pyi` 类型存根，补齐上述 API

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
