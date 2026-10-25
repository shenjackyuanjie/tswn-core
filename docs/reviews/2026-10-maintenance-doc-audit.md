# 2026-10 文档审查覆盖与验证

[返回文档中心](../README.md)

## 范围与方法

本记录对应 task-6，审查日期为 2026-10-23。基线是领取任务时 `git ls-files '*.md' '*.MD'` 得到的 **148 份受跟踪 Markdown**，另审查 **33 份脚本源文件**（22 份 Python、9 份 JavaScript、2 份 shell 环境脚本）。[脚本索引](../../scripts/README.md) 已计入 148 份，不重复计数；本记录是审查后新增的产物，不回填进基线数量。

- **现行说明**：核对 README、原理指南、操作指南、公共契约、机制与设计状态，结合 Cargo manifest/metadata、CLI 参数定义、会话/绑定实现和脚本参数解析检查入口、feature、输入输出及资源约束。
- **历史材料**：逐份检查版本/提交/开发状态、标题结构、命令所属时代和链接。保留发布记录、CHANGELOG、性能数字及原始生成报告；不把历史成功或收益追认为本次验证结果，也不重新执行所有历史实验。
- **测试输入**：八份 Markdown fixture 由共享 suite 的 `include_str!` 消费，不作为普通文档改写；按引用和 Git blob 核对保留。压力语料说明仅按 lead 授权修正失效维护命令，未改 txt/json。
- **协作边界**：本任务不修改 Rust/C/TOML，不读本地凭据文件，不操作已有未跟踪 `.qoder/`，不执行暂存、提交或历史重置；总更新日志与综合维护汇总归 lead。

### 分类数量

| 分类 | 数量 | 审查边界 |
| --- | ---: | --- |
| 现行文档与索引 | 61 | 入口、命令、feature、契约与链接 |
| 更新日志 | 12 | 适用版本、历史性质、链接与保留范围 |
| 测试输入 | 8 | 只读引用与指纹 |
| 历史归档 | 5 | 适用版本、历史性质、链接与保留范围 |
| 未实施设计提案 | 1 | 保留提案基线与未实施状态 |
| 历史差分报告 | 1 | 适用版本、历史性质、链接与保留范围 |
| 历史测量报告 | 21 | 适用版本、历史性质、链接与保留范围 |
| 历史版本记录 | 39 | 适用版本、历史性质、链接与保留范围 |
| 合计 | **148** | 原受跟踪 Markdown 全覆盖 |

## 主要过时点与处理

1. **PGO 帮助与实际流程不一致。** 聚合脚本实际对未跳过的 CLI 和 Openbox 启用 PGO，旧帮助只写 CLI 并直接宣称约 25% 收益。现已修正帮助，明确 Openbox 训练靶子仍指旧资源目录，当前完整发布用普通 release；历史报告只加适用范围，不改数据。本轮未运行 PGO 或吞吐对照。
2. **WASM 错误码清单过时。** 现行 options 解析返回 `INVALID_ARGUMENT`，不再是 `INVALID_OPTIONS`；补齐正式六类错误码与 `RUNTIME_FAILED`，单独说明兼容胜率错误。JS 抛出的是 `{code,message}` 普通对象，不是 Error 实例。
3. **图标编码说明不完整。** C、Python、WASM 的 Base64 helper 返回带 `data:image/png;base64,` 前缀的 data URL；PNG bytes 才是文件字节。绑定负责人确认输出格式未改变；C 图标错误码/旧错误清除修复的测试由绑定任务负责，本任务只同步契约。
4. **构建前提需跟进依赖升级。** 依赖负责人确认当前全功能依赖要求 Rust 至少 1.95，Python 仍为 3.12+；PEP 517 构建依赖下限为 setuptools 84.2.1、setuptools-rust 1.13.0、wheel 0.48.0。已对照最终 manifest，未把依赖下限当作已通过的最低工具链验收。
5. **本机 sccache 排障缺少可执行入口。** 本次重编译观察到 wrapper/编译器检测组合问题，lead 已实际验证单命令关闭 rustc wrapper 有效；依赖复核确认旧版 cc 也有相同 wrapper 回退路径，不判定为新版 cc 的普遍回归。构建指南补上命令和 `/WX-` 症状，不改全局配置，不把定向 check 当作完整 release 通过。
6. **原理指南护盾显示落后于 0.5.11 页面。** 修正“HP 右侧”和“有熟练度即常驻底轨”；现行是按变化显示，只有盾变化时独占槽位，两者同时变化时上下叠放，前后盾值均零时无空底轨。前端 43 项行为测试已实际通过。
7. **冻结器维护命令已经不存在。** `tswn_test` 无 `freeze_runtime_golden` bin；语料说明改为只读比较，语义变更先单独审查再生工具、输入与预期。未重生成任何基线。
8. **设计与历史入口混淆。** 设计索引补入尚未实施的 Runtime 架构提案；版本索引明确历史命令相对对应版本，保留 0.3.11/0.5.2 的开发中状态。既有 CHANGELOG 历史条目保持原样。
9. **脚本帮助并非通用协议。** 补充需要 psycopg、Node stdin JSON、浏览器工具、无 `--help` 的直接验证入口与 macOS C 库定位限制；校正文档验证脚本的 BattleSession 往返说明。按 lead 要求为仓库 Python 扩展构建补 `--locked`；临时独立 smoke crate 保留首次无锁的离线流程。
10. **Python 安装、入口与限额指向含糊。** 本地 wheel 安装改为选择匹配解释器/平台的具体文件；评分 helper 对应 `bench auto` 单组输入，注册在 `tswn_py` 顶层，不存在单独的 Python `tswn_py.cli_api` 子模块。进阶 `Runner.build_replay(limit)` 按可见事件数而非帧数计量，仍受主回合与无进展保护，相关 README 和迁移指南已区分。

## 实际验证

执行环境：Windows，Python **3.12.10**、Node **24.19.0**、Rust **1.99.0**。本任务不重复全 workspace 编译；全量测试、Clippy、rustdoc、完整 corpus 和真实跨绑定示例由 lead 统一执行。

| 验证 | 实际结果 | 边界 |
| --- | --- | --- |
| 全部 Python AST 解析 | **22/22 通过** | 检查脚本可解析，不代表执行所有外部工作流 |
| 安全的 Python 帮助入口 | **16/16 通过** | 含 13 个顶层入口及 rate/pf/round 三个子命令；不连接数据库、不构建产物 |
| Node 脚本语法 | **9/9 通过** | `node --check`；不加载真实 WASM、外部 md5.js 或浏览器 |
| 脚本单元测试 | **6 通过，0 跳过** | `python -m unittest discover -s scripts -p 'test_*.py'`，包含实际临时包/ZIP 和 NumPy 文件读取 |
| WASM 页面纯模块行为 | **43 通过，0 跳过** | `node --test crates/tswn_wasm/tests/show-*.test.mjs`，无需重建 WASM |
| 核心 CLI 帮助 | **13/13 通过** | 复用现有 debug CLI，涵盖 fight/runtime/bench/namer-pf/icon/to-diy |
| CLI 实际 JSONL | **通过** | alpha/beta + 固定 seed，完整获胜、1 轮截断、空输入 stderr/非零退出；逐条 JSON 解析与帧数校验 |
| DIY/OL 实际往返 | **通过** | 分别导出 alpha/beta 并保持分组和 seed，初始属性及整场 initial/frame/result 精确相等 |
| 全文档的本地路径/标题锚点 | **151 份、1163 处链接，0 问题** | 原 148 份及本次三份新增报告/日志，含行内与引用式链接；不验证外部 HTTP 可用性或源码行号长期稳定性 |
| 八份 Markdown fixture | **8/8 与 HEAD blob 相同** | 并核对共享 suite 的 include_str 引用；没有执行或重写 golden |
| 差异空白检查 | **通过** | 所属文件执行 `git diff --check`；Git 的 LF/CRLF 提示不是失败 |
| Cargo 目标与 feature | 最终锁定 metadata **通过**，11 个成员 | 实际执行 `cargo metadata --no-deps --format-version 1 --locked`，核对全部 bin/default feature；不启动编译 |

脚本帮助与语法的原始输出留在本地 `target/doc-audit-validation.json`，CLI 冒烟摘要在 `target/docs-review-smoke/cli-validation.json`；它们是忽略的验证产物，不作为源码交付。

### 未执行与后续建议

- 未重复全 workspace / release / corpus / Clippy / rustdoc，也未运行真实四端比对和文档执行脚本；这些由 lead 在最终锁文件上统一验证。
- 未构建 Windows/Linux/macOS 完整发布包、OHOS 产物或最低 Rust 1.95，未连接真实数据库/ABCP5、未跑 PGO 和浏览器性能；文档只陈述已确认的前提与历史范围。
- Openbox PGO 训练路径仍需独立代码任务适配；跨绑定脚本的 macOS 动态库定位仍需独立适配。新性能结论应重新同机交替 A/B。
- [历史 0.3.8 命名报告](../perf/fixed_cases_30_results/perf_cases_0.3.8.md) 内部版本记录为 0.3.7；保留原始报告，复现时按内部元数据而非仅文件名解释，不“校正”历史测量。
- 根目录 checkpoint 入口仍有按历史 corpus 数量写死的帮助文案；本任务移除根 README 对固定数量的依赖，后续维护该工具时可改为当前 Cargo 测试范围，不在文档任务中改其逻辑。

## 原 148 份 Markdown 逐项覆盖

“保留”表示按上文类别完成相应审查，不等于重新验证其中的全部历史性能或机制数值。表中路径链接同时用于完整覆盖复核。

| 序号 | 文件 | 分类 | 处置 |
| ---: | --- | --- | --- |
| 1 | [AGENTS.md](../../AGENTS.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 2 | [README.md](../../README.md) | 现行文档与索引 | 修正构建前提与固定测试数量表述 |
| 3 | [tswn_capi/CHANGELOG.md](../../crates/tswn_capi/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 4 | [tswn_capi/README.md](../../crates/tswn_capi/README.md) | 现行文档与索引 | 补图标 data URL、错误码与释放契约 |
| 5 | [examples/README.md](../../crates/tswn_capi/examples/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 6 | [tswn_core/CHANGELOG.md](../../crates/tswn_core/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 7 | [tswn_core/README.md](../../crates/tswn_core/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 8 | [bin/README.md](../../crates/tswn_core/src/bin/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 9 | [tswn_ds4/CHANGELOG.md](../../crates/tswn_ds4/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 10 | [tswn_ds4/README.md](../../crates/tswn_ds4/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 11 | [tswn_ladder/CHANGELOG.md](../../crates/tswn_ladder/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 12 | [tswn_ladder/README.md](../../crates/tswn_ladder/README.md) | 现行文档与索引 | 格式检查限定所属 package |
| 13 | [tswn_lane_ranker/CHANGELOG.md](../../crates/tswn_lane_ranker/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 14 | [tswn_lane_ranker/README.md](../../crates/tswn_lane_ranker/README.md) | 现行文档与索引 | 格式检查限定所属 package |
| 15 | [tswn_name_ranker/CHANGELOG.md](../../crates/tswn_name_ranker/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 16 | [tswn_name_ranker/README.md](../../crates/tswn_name_ranker/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 17 | [tswn_openbox/CHANGELOG.md](../../crates/tswn_openbox/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 18 | [tswn_openbox/README.md](../../crates/tswn_openbox/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 19 | [tswn_openbox_backend/CHANGELOG.md](../../crates/tswn_openbox_backend/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 20 | [tswn_pwp/CHANGELOG.md](../../crates/tswn_pwp/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 21 | [tswn_pwp/README.md](../../crates/tswn_pwp/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 22 | [tswn_py/CHANGELOG.md](../../crates/tswn_py/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 23 | [tswn_py/README.md](../../crates/tswn_py/README.md) | 现行文档与索引 | 修 wheel、图标、评分入口与回放事件限额 |
| 24 | [tswn_py/README_BUILD.md](../../crates/tswn_py/README_BUILD.md) | 现行文档与索引 | 同步 Python 构建工具下限与工具链说明 |
| 25 | [tswn_test/CHANGELOG.md](../../crates/tswn_test/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 26 | [runtime_stress/README.md](../../crates/tswn_test/cases/runtime_stress/README.md) | 现行文档与索引 | 仅修失效冻结器说明；txt/json 不变 |
| 27 | [fixtures/fight_large.md](../../crates/tswn_test/src/suite/fixtures/fight_large.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 28 | [fixtures/fight_multi_1.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_1.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 29 | [fixtures/fight_multi_2.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_2.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 30 | [fixtures/fight_multi_3.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_3.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 31 | [fixtures/fight_multi_4.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_4.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 32 | [fixtures/fight_multi_5.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_5.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 33 | [fixtures/fight_multi_7.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_7.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 34 | [fixtures/fight_multi_8.md](../../crates/tswn_test/src/suite/fixtures/fight_multi_8.md) | 测试输入 | 按测试引用与 Git blob 核对，原样保留 |
| 35 | [tswn_wasm/CHANGELOG.md](../../crates/tswn_wasm/CHANGELOG.md) | 更新日志 | 保留历史正文、版本状态与原始结果 |
| 36 | [tswn_wasm/README.md](../../crates/tswn_wasm/README.md) | 现行文档与索引 | 修正稳定错误码与图标返回格式 |
| 37 | [examples/README.md](../../crates/tswn_wasm/examples/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 38 | [tests/README.md](../../crates/tswn_wasm/tests/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 39 | [docs/README.md](../../docs/README.md) | 现行文档与索引 | 链接本次完整覆盖记录 |
| 40 | [archive/README.md](../../docs/archive/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 41 | [archive/dart-architecture.md](../../docs/archive/dart-architecture.md) | 历史归档 | 保留历史正文、版本状态与原始结果 |
| 42 | [archive/legacy-readme.md](../../docs/archive/legacy-readme.md) | 历史归档 | 保留历史正文、版本状态与原始结果 |
| 43 | [archive/project-origins.md](../../docs/archive/project-origins.md) | 历史归档 | 保留历史正文、版本状态与原始结果 |
| 44 | [archive/sby-test-plan.md](../../docs/archive/sby-test-plan.md) | 历史归档 | 保留历史正文、版本状态与原始结果 |
| 45 | [archive/why-ns-review.md](../../docs/archive/why-ns-review.md) | 历史归档 | 保留历史正文、版本状态与原始结果 |
| 46 | [core-guide/README.md](../../docs/core-guide/README.md) | 现行文档与索引 | 区分初稿版本与现行复核范围 |
| 47 | [core-guide/battle-flow.md](../../docs/core-guide/battle-flow.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 48 | [core-guide/contributing-and-debugging.md](../../docs/core-guide/contributing-and-debugging.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 49 | [core-guide/datasets-and-extension.md](../../docs/core-guide/datasets-and-extension.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 50 | [core-guide/interfaces-and-tools.md](../../docs/core-guide/interfaces-and-tools.md) | 现行文档与索引 | 修 JS 错误对象与 Python 顶层 helper 入口 |
| 51 | [core-guide/name-to-player.md](../../docs/core-guide/name-to-player.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 52 | [core-guide/project-map.md](../../docs/core-guide/project-map.md) | 现行文档与索引 | 更新护盾条显示位置与触发条件 |
| 53 | [core-guide/randomness-and-compatibility.md](../../docs/core-guide/randomness-and-compatibility.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 54 | [core-guide/runtime-model.md](../../docs/core-guide/runtime-model.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 55 | [core-guide/sessions-and-replay.md](../../docs/core-guide/sessions-and-replay.md) | 现行文档与索引 | 同步 JS 错误对象表述 |
| 56 | [core-guide/skills-and-hooks.md](../../docs/core-guide/skills-and-hooks.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 57 | [core-guide/statuses-and-minions.md](../../docs/core-guide/statuses-and-minions.md) | 现行文档与索引 | 修正空盾不留底轨与 HP/盾叠放条件 |
| 58 | [design/README.md](../../docs/design/README.md) | 现行文档与索引 | 补未实施 Runtime 提案的可达入口与状态 |
| 59 | [design/battle-analyze.md](../../docs/design/battle-analyze.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 60 | [design/battle-model-state-audit.md](../../docs/design/battle-model-state-audit.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 61 | [design/battle-session.md](../../docs/design/battle-session.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 62 | [design/feature-encoder-spec.md](../../docs/design/feature-encoder-spec.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 63 | [design/runtime-redesign-assessment.md](../../docs/design/runtime-redesign-assessment.md) | 未实施设计提案 | 保留文中基线与“尚未实现”状态 |
| 64 | [diff/README.md](../../docs/diff/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 65 | [diff/bun-tswn-round-mismatches.md](../../docs/diff/bun-tswn-round-mismatches.md) | 历史差分报告 | 保留历史正文、版本状态与原始结果 |
| 66 | [guides/README.md](../../docs/guides/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 67 | [guides/build-all.md](../../docs/guides/build-all.md) | 现行文档与索引 | 同步工具链要求；补单次禁用 sccache 排障 |
| 68 | [guides/c-api-cpp-windows.md](../../docs/guides/c-api-cpp-windows.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 69 | [guides/development.md](../../docs/guides/development.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 70 | [guides/diy-validation.md](../../docs/guides/diy-validation.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 71 | [guides/openharmony.md](../../docs/guides/openharmony.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 72 | [guides/runtime-0.5-migration.md](../../docs/guides/runtime-0.5-migration.md) | 现行文档与索引 | 区分进阶 build_replay 的事件 limit 与主回合保护 |
| 73 | [mechanics/README.md](../../docs/mechanics/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 74 | [mechanics/clone.md](../../docs/mechanics/clone.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 75 | [mechanics/name-factor-rq.md](../../docs/mechanics/name-factor-rq.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 76 | [mechanics/skill-decay.md](../../docs/mechanics/skill-decay.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 77 | [mechanics/winner.md](../../docs/mechanics/winner.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 78 | [perf/README.md](../../docs/perf/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 79 | [perf/benchmark-history.md](../../docs/perf/benchmark-history.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 80 | [fixed_cases_30_results/perf_cases.md](../../docs/perf/fixed_cases_30_results/perf_cases.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 81 | [fixed_cases_30_results/perf_cases_0.3.10.md](../../docs/perf/fixed_cases_30_results/perf_cases_0.3.10.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 82 | [fixed_cases_30_results/perf_cases_0.3.8.md](../../docs/perf/fixed_cases_30_results/perf_cases_0.3.8.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 83 | [guides/amd-uprof.md](../../docs/perf/guides/amd-uprof.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 84 | [guides/fixed30-benchmark.md](../../docs/perf/guides/fixed30-benchmark.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 85 | [guides/openbox-pair-probe.md](../../docs/perf/guides/openbox-pair-probe.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 86 | [guides/performance.md](../../docs/perf/guides/performance.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 87 | [reports/cqp-runtime-baseline.md](../../docs/perf/reports/cqp-runtime-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 88 | [reports/openbox-live-2026-09-29.md](../../docs/perf/reports/openbox-live-2026-09-29.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 89 | [reports/openbox-pair-parallelism-2026-09-15.md](../../docs/perf/reports/openbox-pair-parallelism-2026-09-15.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 90 | [reports/openbox-review-2026-09-29.md](../../docs/perf/reports/openbox-review-2026-09-29.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 91 | [reports/optimization-opportunities-0.5.3.md](../../docs/perf/reports/optimization-opportunities-0.5.3.md) | 历史测量报告 | 仅补 PGO 历史口径与当前发布边界 |
| 92 | [reports/python-battle-session-baseline.md](../../docs/perf/reports/python-battle-session-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 93 | [reports/rc4-profile-2026-09-05.md](../../docs/perf/reports/rc4-profile-2026-09-05.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 94 | [reports/runtime-0.4.0-baseline.md](../../docs/perf/reports/runtime-0.4.0-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 95 | [reports/runtime-0.4.2-1ed1258-allocator-snapshot.md](../../docs/perf/reports/runtime-0.4.2-1ed1258-allocator-snapshot.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 96 | [reports/runtime-0.4.2-d813e5f-snapshot.md](../../docs/perf/reports/runtime-0.4.2-d813e5f-snapshot.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 97 | [reports/runtime-0.4.3-9d3a3b9-release-benchmark.md](../../docs/perf/reports/runtime-0.4.3-9d3a3b9-release-benchmark.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 98 | [reports/runtime-0.5.0-749fcd1-release-benchmark.md](../../docs/perf/reports/runtime-0.5.0-749fcd1-release-benchmark.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 99 | [reports/runtime-four-way-2026-07-15.md](../../docs/perf/reports/runtime-four-way-2026-07-15.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 100 | [reports/target-selection-optimization.md](../../docs/perf/reports/target-selection-optimization.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 101 | [reports/ub-fix-no-debug.md](../../docs/perf/reports/ub-fix-no-debug.md) | 历史测量报告 | 仅补旧 CLI/工具链历史状态 |
| 102 | [reports/web-streaming-baseline.md](../../docs/perf/reports/web-streaming-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 103 | [reports/winprob-dataset-scale-baseline.md](../../docs/perf/reports/winprob-dataset-scale-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 104 | [reports/winprob-hp-baseline.md](../../docs/perf/reports/winprob-hp-baseline.md) | 历史测量报告 | 保留历史正文、版本状态与原始结果 |
| 105 | [reference/README.md](../../docs/reference/README.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 106 | [reference/diy-overlay.md](../../docs/reference/diy-overlay.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 107 | [reference/public-api.md](../../docs/reference/public-api.md) | 现行文档与索引 | 对照现行入口、源码与链接，保留 |
| 108 | [releases/0.2.0.md](../../docs/releases/0.2.0.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 109 | [releases/0.2.1.md](../../docs/releases/0.2.1.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 110 | [releases/0.2.10.md](../../docs/releases/0.2.10.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 111 | [releases/0.2.11.md](../../docs/releases/0.2.11.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 112 | [releases/0.2.12.md](../../docs/releases/0.2.12.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 113 | [releases/0.2.13.md](../../docs/releases/0.2.13.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 114 | [releases/0.2.14.md](../../docs/releases/0.2.14.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 115 | [releases/0.2.15.md](../../docs/releases/0.2.15.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 116 | [releases/0.2.16.md](../../docs/releases/0.2.16.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 117 | [releases/0.2.17.md](../../docs/releases/0.2.17.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 118 | [releases/0.2.18.md](../../docs/releases/0.2.18.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 119 | [releases/0.2.19.md](../../docs/releases/0.2.19.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 120 | [releases/0.2.2.md](../../docs/releases/0.2.2.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 121 | [releases/0.2.20.md](../../docs/releases/0.2.20.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 122 | [releases/0.2.21.md](../../docs/releases/0.2.21.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 123 | [releases/0.2.22.md](../../docs/releases/0.2.22.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 124 | [releases/0.2.3.md](../../docs/releases/0.2.3.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 125 | [releases/0.2.4.md](../../docs/releases/0.2.4.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 126 | [releases/0.2.5.md](../../docs/releases/0.2.5.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 127 | [releases/0.2.6.md](../../docs/releases/0.2.6.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 128 | [releases/0.2.7.md](../../docs/releases/0.2.7.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 129 | [releases/0.2.8.md](../../docs/releases/0.2.8.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 130 | [releases/0.2.9.md](../../docs/releases/0.2.9.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 131 | [releases/0.3.0.md](../../docs/releases/0.3.0.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 132 | [releases/0.3.1.md](../../docs/releases/0.3.1.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 133 | [releases/0.3.10.md](../../docs/releases/0.3.10.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 134 | [releases/0.3.11.md](../../docs/releases/0.3.11.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 135 | [releases/0.3.2.md](../../docs/releases/0.3.2.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 136 | [releases/0.3.3.md](../../docs/releases/0.3.3.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 137 | [releases/0.3.4.md](../../docs/releases/0.3.4.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 138 | [releases/0.3.5.md](../../docs/releases/0.3.5.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 139 | [releases/0.3.6.md](../../docs/releases/0.3.6.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 140 | [releases/0.3.7.md](../../docs/releases/0.3.7.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 141 | [releases/0.3.8.md](../../docs/releases/0.3.8.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 142 | [releases/0.3.9.md](../../docs/releases/0.3.9.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 143 | [releases/0.5.1.md](../../docs/releases/0.5.1.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 144 | [releases/0.5.2.md](../../docs/releases/0.5.2.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 145 | [releases/0.6.0.md](../../docs/releases/0.6.0.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 146 | [releases/0.7.0.md](../../docs/releases/0.7.0.md) | 历史版本记录 | 保留历史正文、版本状态与原始结果 |
| 147 | [releases/README.md](../../docs/releases/README.md) | 现行文档与索引 | 标明历史命令适用范围与 0.3.11 未发布状态 |
| 148 | [scripts/README.md](../../scripts/README.md) | 现行文档与索引 | 补依赖、无 --help 入口与当前验证边界 |

## scripts 源文件逐项覆盖

[脚本 README](../../scripts/README.md) 在上一表内；以下列出其余 33 份受跟踪文件，不把包内部模块或 stdin 桥接程序当成统一 CLI。

| 序号 | 文件 | 处置 |
| ---: | --- | --- |
| 1 | [_tswn_diff/__init__.py](../../scripts/_tswn_diff/__init__.py) | 核对内部模块说明、参数与顶层路由；不连接数据库 |
| 2 | [_tswn_diff/common.py](../../scripts/_tswn_diff/common.py) | 核对内部模块说明、参数与顶层路由；不连接数据库 |
| 3 | [_tswn_diff/pf.py](../../scripts/_tswn_diff/pf.py) | 核对内部模块说明、参数与顶层路由；不连接数据库 |
| 4 | [_tswn_diff/rate.py](../../scripts/_tswn_diff/rate.py) | 核对内部模块说明、参数与顶层路由；不连接数据库 |
| 5 | [_tswn_diff/round.py](../../scripts/_tswn_diff/round.py) | 核对内部模块说明、参数与顶层路由；不连接数据库 |
| 6 | [scripts/benchmark_md5_runtime.cjs](../../scripts/benchmark_md5_runtime.cjs) | 核对手写参数；无 --help，不启动外部 JS 负载 |
| 7 | [scripts/benchmark_py_battle_session.py](../../scripts/benchmark_py_battle_session.py) | 核对模块文档、参数定义与实际 --help |
| 8 | [scripts/benchmark_wasm_battle_session.mjs](../../scripts/benchmark_wasm_battle_session.mjs) | 核对 stdin JSON 协议，不当作命令行帮助执行 |
| 9 | [scripts/benchmark_web_streaming.mjs](../../scripts/benchmark_web_streaming.mjs) | 核对 Playwright、Edge、分发目录；未重测性能 |
| 10 | [scripts/build_all.py](../../scripts/build_all.py) | 修 PGO 帮助：覆盖未跳过的 CLI/Openbox，不承诺收益 |
| 11 | [scripts/build_capi.py](../../scripts/build_capi.py) | 核对模块文档、参数定义与实际 --help |
| 12 | [scripts/build_py.py](../../scripts/build_py.py) | 核对模块文档、参数定义与实际 --help |
| 13 | [scripts/build_wasm.py](../../scripts/build_wasm.py) | 核对模块文档、参数定义与实际 --help |
| 14 | [scripts/bun_profile_trace.js](../../scripts/bun_profile_trace.js) | 核对参数解析与输出说明，保留 |
| 15 | [scripts/bun_score_trace.js](../../scripts/bun_score_trace.js) | 核对参数解析与输出说明，保留 |
| 16 | [scripts/check_runtime_release.py](../../scripts/check_runtime_release.py) | 核对模块文档、参数定义与实际 --help |
| 17 | [scripts/dump_battle_wasm.mjs](../../scripts/dump_battle_wasm.mjs) | 核对 Node 包定位、环境覆盖与 stdin JSON 协议 |
| 18 | [scripts/md5_winner_probe.cjs](../../scripts/md5_winner_probe.cjs) | 核对参数解析与输出说明，保留 |
| 19 | [scripts/measure_encoder_capacity.py](../../scripts/measure_encoder_capacity.py) | 核对模块文档、参数定义与实际 --help |
| 20 | [scripts/ohos-env.ps1](../../scripts/ohos-env.ps1) | 核对点导入、SDK 输入与导出变量；未连接 SDK/设备 |
| 21 | [scripts/ohos-env.sh](../../scripts/ohos-env.sh) | 核对点导入、SDK 输入与导出变量；未执行交叉构建 |
| 22 | [scripts/pgo_build.py](../../scripts/pgo_build.py) | 补 Openbox 训练资源失效状态与复测要求 |
| 23 | [scripts/read_encoded_batches.py](../../scripts/read_encoded_batches.py) | 核对模块文档、参数定义与实际 --help |
| 24 | [scripts/read_winprob_dataset.py](../../scripts/read_winprob_dataset.py) | 核对模块文档、参数定义与实际 --help |
| 25 | [scripts/test_build_all.py](../../scripts/test_build_all.py) | 脚本单元回归实际执行 |
| 26 | [scripts/test_read_encoded_batches.py](../../scripts/test_read_encoded_batches.py) | 真实文件与 NumPy 读取单元回归实际执行 |
| 27 | [scripts/tswn_diff.py](../../scripts/tswn_diff.py) | 核对模块文档、参数定义与实际 --help |
| 28 | [scripts/verify_battle_cross_binding.py](../../scripts/verify_battle_cross_binding.py) | 补真实产物、只读输入与当前平台定位边界 |
| 29 | [scripts/verify_battle_docs.py](../../scripts/verify_battle_docs.py) | 补独立 smoke crate、产物前提与无帮助参数说明 |
| 30 | [scripts/verify_py_cli_api.py](../../scripts/verify_py_cli_api.py) | 中文说明与帮助；按 lead 要求为仓库 build 加 --locked |
| 31 | [scripts/verify_wasm_battle.test.mjs](../../scripts/verify_wasm_battle.test.mjs) | 核对真实 Node WASM 包前提；绑定验收交 lead |
| 32 | [scripts/verify_web_playback.mjs](../../scripts/verify_web_playback.mjs) | 核对 linkedom/vm 模块前提；完整 DOM 验收交 lead |
| 33 | [scripts/winprob_hp_baseline.py](../../scripts/winprob_hp_baseline.py) | 核对模块文档、参数定义与实际 --help |

## 建议分块提交

1. `docs(build): 校正工具链前提与本机编译排障`：根 README、Python 构建说明、全量构建指南。
2. `docs(api): 对齐绑定错误、图标格式与护盾展示`：C/Python/WASM README 与相关原理指南。
3. `docs(tools): 校正脚本帮助与历史适用边界`：脚本索引、脚本文档/帮助、设计/历史索引、语料维护说明；Python 验证构建的 --locked 可独立成 `chore(test)` 小提交。
4. `docs(review): 记录全量文档审查覆盖与实际验证`：本记录与文档中心入口。提交统一由 lead 完成。
