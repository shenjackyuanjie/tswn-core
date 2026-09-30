# 更新日志

## [Unreleased]

### 新增

- 从 `tswn_openbox` 提取共用计算后端与预设，提供独立的无界面 Rust library；Openbox GUI、
  `openbox-cli` 和 DS4 直接复用，不依赖 `eframe`／`egui`，消除 Openbox 与 DS4 的循环依赖。
- 提供 `backend` 与 `presets` 模块，保留 `to-diy`、`namer-pf`、批量胜率和配队任务入口、
  参数／进度类型及 `*_observed` 增量事件接口；迁移原有解析、文件输出、技能榜与矩阵执行逻辑。
- 提供 `load_target_presets_from_root()`，按指定工作目录读取预设并补齐缺失的内嵌默认资源，
  不覆盖用户文件；保留带权靶子与队友配置的解析和加载接口。

### 兼容性

- `tswn_openbox::backend` 与 `tswn_openbox::presets` 通过再导出保留旧公开路径；输出格式、
  加权计算、稳定排序与取消语义不变，相关业务回归测试随实现迁移到本 crate。

### 说明

- 当前包版本为 `0.4.5`，沿用提取时的 Openbox 版本号，不代表本 crate 已独立发布。
- 提取前的功能与性能变更记录见 `../tswn_openbox/CHANGELOG.md`；本日志从拆分开始记录。
