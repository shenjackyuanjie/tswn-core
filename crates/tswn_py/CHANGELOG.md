# 更新日志

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
