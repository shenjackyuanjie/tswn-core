# 工作区更新日志

本文件记录跨 crate 的维护与兼容性变更；各 crate 的版本发布历史仍以对应 CHANGELOG 为准。

## 未发布 · 2026-10-30 日志补全

- 按各模块 manifest、提交历史与当前实现补全未发布条目；版本号与发布日期维持现状，旧版本记录当时验证的测试数量和性能结论。
- 主线新增 Boss/武器的分阶段兼容移植、Openbox 可选 CSV 列提取/行筛选、评分项全不选时不过滤及私有 GUI 模块可见性整理，以及此前维护中的绑定错误边界、依赖和数据工具修复；具体入口与限制见下列模块日志。
- 核心热路径、名字评分准备和 Rinick 冷快照优化已随 `perf/ffa-profile-20261027` 合并（merge commit `e0e7c671`）进入 main；细节仍记在 core 日志的分支专节。性能范围与已知失败见[简报](docs/perf/reports/performance-branch-2026-10-29.md)，提速幅度按对局而定。
- 分支诊断工具还包括可选 PGO 构建脚本与独立 `allocation-probe`（`publish = false`）；两者属于可选的诊断辅助，不随默认构建启用。本版本没有新的逐轮测量数据。

### 模块日志索引

版本列为当前 manifest，`Unreleased` 标题中的拟议下一版本属于计划版本，尚未发布。

| 模块 | 当前版本 | 日志 |
| --- | --- | --- |
| `tswn_core` | 0.7.0 | [更新日志](crates/tswn_core/CHANGELOG.md) |
| `tswn_capi` | 0.6.3 | [更新日志](crates/tswn_capi/CHANGELOG.md) |
| `tswn_py` | 0.5.4 | [更新日志](crates/tswn_py/CHANGELOG.md) |
| `tswn_wasm` | 0.5.12 | [更新日志](crates/tswn_wasm/CHANGELOG.md) |
| `tswn_openbox` | 0.5.2 | [更新日志](crates/tswn_openbox/CHANGELOG.md) |
| `tswn_openbox_backend` | 0.5.2 | [更新日志](crates/tswn_openbox_backend/CHANGELOG.md) |
| `tswn_lane_ranker` | 0.1.0 | [更新日志](crates/tswn_lane_ranker/CHANGELOG.md) |
| `tswn_name_ranker` | 0.1.0 | [更新日志](crates/tswn_name_ranker/CHANGELOG.md) |
| `tswn_ladder` | 0.2.0 | [更新日志](crates/tswn_ladder/CHANGELOG.md) |
| `tswn_ds4` | 0.2.1 | [更新日志](crates/tswn_ds4/CHANGELOG.md) |
| `tswn_pwp` | 0.0.1 | [更新日志](crates/tswn_pwp/CHANGELOG.md) |
| `tswn_test` | 0.0.0 | [更新日志](crates/tswn_test/CHANGELOG.md) |

## 未发布 · 2026-10-24 性能与正确性复测

- 新增公开 API 微基准与可复现的串行 A/B 驱动：按维护前 `704a1ac9`、维护后 `cfee708d` 的各自锁文件构建，保存二进制和输入哈希，产品运行逻辑保持不变。
- 完成 34 个配置的五轮对照及三组七轮独立确认。fixed30 总体变化在 ±1.5% 内，未观察到全项目级别的加速；重点初测的回退复测未再现；Python/WASM 图标压力场景长尾与 PWP 校验偏慢是遗留观察项。
- PWP 生成与校验的 2700 局、20285 样本跨版本一致。真实校准缺少 `state.payload.shield_value`，完整契约拒绝该输入，端到端 encode 尚未完成。
- Web 基准支持隔离包/输出、重复次数及计时外截图开关。远程桌面无法截图时，两版使用相同的不截图配置，DOM 计时只用于计时；视觉验收尚未进行。
- 重新完成 17 项正确性与质量检查，包括完整 workspace/all-features、无默认 feature 的 614 项核心测试、release/no_debug、124 项冻结 corpus、四端 8 场精确 payload、43 项前端模块及公共 API 文档示例。Clippy 的非阻断警告保持原状。
- 按整项配置隔离外部 Bevy 编译造成的污染，慢样本全部保留。原始证据 ZIP 纳入 Git，二进制快照另存既有忽略的 dist 目录，`cargo clean` 不会删除这两处。
- 详细结论、复现命令、归档哈希和未验收边界见[完整复测报告](docs/perf/reports/maintenance-2026-10-24.md)与[机器可读结果](docs/perf/maintenance_2026-10-24_benchmark.json)。

## 未发布 · 2026-10-23 维护

### 依赖与兼容性

- 核对全部 13 份 Cargo manifest、工作区与独立单号排名工具的两份锁文件，以及 Python 构建依赖。
- 将 egui/eframe 从 0.36.2 升级至 0.37.0，PyO3 从 0.29.3 升级至 0.30.1；同步更新 Tokio、serde_json、sha2、smallvec、toml 等兼容稳定版本。
- 刷新传递依赖，替换已被撤回的 clipboard-win 5.4.1；上游仍要求的旧 semver 分支保持原状。
- 更新 Python 的 setuptools、wheel、setuptools-rust 构建要求；用户本地虚拟环境保持不动。

### 正确性修复

- CLI 的非负阈值拒绝 NaN、无穷及数值溢出，并拒绝 Windows 盘符之后的额外路径冒号，保留合法盘符和 UNC 路径。
- 调试配置的首次初始化改用同步完成边界，防止并发查询读取尚未初始化的标志。
- 天梯预期胜率与分差计算先转换再相减，覆盖完整 i32 分数范围，避免 debug 溢出与 release 回绕。
- C 图标 data URL 接口复用统一 FFI 错误边界，保留稳定输入错误码，并在成功时清除当前线程的旧错误。
- RC4 的 `peek_next_u8` 按交换后的排列预览下一字节，覆盖输出下标与交换位置重合的情形，且不推进状态；实际消费随机数的实现保持不变。
- DS4 单文件提交直接重命名替换目标，取消先删旧文件的窗口；替换失败时保留旧内容，并清理未提交的临时文件。
- 修复关闭 `png_render` 时核心战斗库无法编译的问题；无图标战斗与模型会话保持可用，展示会话请求图标时明确返回 `UNSUPPORTED_OPTION`，不生成空图或占位图。

### 性能与小重构

- FeatureEncoder 复用输入验证得到的容量计量，减少重复遍历，保持张量契约、容量错误与失败后缓冲清理行为。
- 战斗快照投影减少冗余图标键复制，图标缓存命中时先借用查询，保持完整 key、输出顺序和 PNG/data URL 内容。
- 这些改动以减少实际重复工作为目标；局部优化暂无同环境 A/B 测量数据，本版只记录改动内容。

### 文档

- 按模块审阅 Rust 的 `///`、`//!` 与 C API 契约，补充输入输出、所有权、错误传播、采样身份和平台计时语义。
- 校对现行 README、构建指南、公共 API 说明和脚本帮助，保留归档设计、历史性能报告与冻结测试输入的原始性质。
- golden/corpus 保持既有基线，战斗随机数推进和对局规则不变。

### 验证与本地构建维护

- 按用户要求执行 `cargo clean`，释放 27.1 GiB；随后以命令级关闭增量及开发/测试调试信息的方式串行从零重建，仓库与全局构建配置保持原状。
- 全 workspace/all-features 测试、无默认 feature 的核心测试、Clippy、严格 Rustdoc 和 nightly 格式检查通过；真实 Python API、Rust/CLI/Python/C/WASM 四端比对与公共文档示例通过。
- 原生 GUI 在当前环境启动访问冲突，新旧版本同条件均能复现；实机截图与完整 wheel 尚未验收。详细范围、原因、后续建议见[维护审查报告](docs/maintenance-2026-10.md)。
