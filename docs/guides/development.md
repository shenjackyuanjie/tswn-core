# 开发入口与模块边界

[返回使用指南](README.md)

命令默认从仓库根目录执行。成员与依赖以根 [Cargo.toml](../../Cargo.toml) 为准；各工具的参数和发布边界见对应 crate README，不在本页重复维护。

## 代码放在哪里

| 职责 | 所属模块与说明 |
| --- | --- |
| 名字解析、模板、战斗与 CLI | [tswn_core](../../crates/tswn_core/README.md)：`namerena` 准备数据，`runtime` 执行战斗，`cli_api` 提供共用会话，`encoder` 编码实验状态 |
| 跨语言适配 | [C](../../crates/tswn_capi/README.md)、[Python](../../crates/tswn_py/README.md)、[WASM](../../crates/tswn_wasm/README.md)：消费 core 接口，不维护第二套战斗或编码算法 |
| 界面与共用评分后端 | [Openbox](../../crates/tswn_openbox/README.md)：GUI/CLI 在 `tswn_openbox`，评分、预设与资源在 `tswn_openbox_backend`，DS4 可直接复用后端 |
| DS4 批处理 | [tswn_ds4](../../crates/tswn_ds4/README.md)：流程、输入/历史文件、单人/配对模型与外部 ABCP5 适配 |
| 胜率训练数据 | [tswn_pwp](../../crates/tswn_pwp/README.md)：生成、校验、校准、manifest 与安全离线导出；张量字段映射仍只在 core encoder 内实现 |
| 排名与周期更新 | [lane ranker](../../crates/tswn_lane_ranker/README.md)、[ladder](../../crates/tswn_ladder/README.md)：分别使用 SQLite 与 PostgreSQL，数据库集成验证和普通单元测试分开 |
| 跨 crate 与冻结回归 | `tswn_test`：集成测试、JS exact trace 与压力 golden；基线维护说明见 [runtime_stress](../../crates/tswn_test/cases/runtime_stress/README.md) |
| 自动化与分析 | [scripts](../../scripts/README.md)：构建打包、差分、真实绑定验收、性能测量与只读数据 loader |

`tswn_name_ranker` 依赖独立校准数据，在 workspace 的 `exclude` 中，不作为常规 workspace 验收的一部分。不要因为它不参与默认构建就删除其源码或数据约定。

## 验证怎么选

日常修改先运行受影响 crate 的测试；影响 core 行为至少执行 `cargo test -p tswn_core`，跨模块整理再执行整个 workspace：

```powershell
cargo test --workspace
cargo +nightly fmt --check
git diff --check
```

Rust 格式化必须使用 nightly；非测试源文件接近 1000 行时按职责拆分。模块拆分保留入口和行为，不顺带改变协议、数值基线或随机调用顺序。

发布前的 release/no_debug 回归与性能口径见 [性能准则](../perf/guides/performance.md)：

```powershell
python scripts/check_runtime_release.py --corpus
```

前端纯模块和脚本行为测试不需要先构建 WASM 包：

```powershell
bun test ./crates/tswn_wasm/tests
node --test crates/tswn_wasm/tests/show-*.test.mjs
uv run --no-project --python 3.12 --with numpy python -m unittest discover -s scripts -p 'test_*.py'
```

真实 C/Python/WASM 调用、文档示例执行和页面 DOM 验收需先准备对应产物或依赖，见 [脚本索引](../../scripts/README.md#battlesession-验收)。Openbox/WASM 可见界面改动还需截图或录屏；只拆分测试文件不属于界面改动。

## 测试与文档各管什么

- 测试验证实际输入输出、状态、边界、错误传播、资源释放及公共契约；重复实现对比和 golden 有各自的行为证据，不按文件大小或断言数量机械删除。
- 源码目录、已删除入口、固定默认配置和架构选择留在文档，不用字符串扫描、源码正则或常量清单测试阻止未来修改。
- 新增字段时同步映射说明，并验证实际值/presence/引用；只把字段名登记进静态名单不算编码覆盖。
- 不重生成 `tswn_test/cases`、性能原始 JSON 或历史报告来让本次结构整理通过。

## 产物与提交

构建、差分和一次性诊断输出留在 `target/` 或对应 `dist/`，不混入源码提交。原有本地输入、虚拟环境和配置不属于可随手删除的“仓库垃圾”。测试新生成的配置仅在确认来源与内容后清理，不使用 `git clean -fdx` 清空本地资产。

按文档、行为变更、内部重构等主题分块提交，提交信息用中文 Conventional Commit。推送规则见 [AGENTS.md](../../AGENTS.md#远程仓库与推送)：同一分支/tag 必须推到 `github` 和 `gitea` 两端。
