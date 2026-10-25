# 2026-10 工作区维护审查

本报告记录 2026-10-23 的依赖升级、正确性修复、低风险优化和文档审阅。版本查询是当日快照，不是对未来最新版本的承诺。变更摘要见[工作区更新日志](../CHANGELOG.md)。

## 范围与方法

- 基线为 `704a1ac9`；先运行 `cargo test --workspace --locked`，确认升级前工作区测试通过，再修改依赖。
- 审查全部 13 份 Cargo manifest、[主锁文件](../Cargo.lock)、[独立排名工具锁文件](../crates/tswn_name_ranker/Cargo.lock) 和 [Python 构建声明](../crates/tswn_py/pyproject.toml)。
- 分模块审查 Rust 源码、`///`、`//!` 和 C 头文件，另设独立队友只读复核逻辑补丁。Markdown 区分现行指南、设计规范、历史报告与实际测试输入。
- 性能修改围绕已确认的重复工作，保留行为回归；没有同环境 A/B 测量的改动不宣称固定加速比例。
- 不重生成 golden/corpus，不修改用户虚拟环境、全局 Cargo 配置或现有未跟踪目录。

## 依赖结果

### 直接 Rust 依赖

以下为两份锁内的解析版本，不把声明范围变化误算为解析依赖升级。39 种直接依赖的直接使用分支均已达到当日稳定版；`foldhash` 额外的旧分支由上游传递依赖引入。

| 依赖 | 更新前锁版本 | 更新后锁版本 | 状态 |
| --- | --- | --- | --- |
| `anyhow` | `1.0.104` | `1.0.104` | 已最新 |
| `arrow-array` | `60.0.0` | `60.0.0` | 已最新 |
| `arrow-schema` | `60.0.0` | `60.0.0` | 已最新 |
| `axum` | `0.8.9` | `0.8.9` | 已最新 |
| `chrono` | `0.4.45` | `0.4.45` | 已最新 |
| `chrono-tz` | `0.10.4` | `0.10.4` | 已最新 |
| `clap` | `4.6.7` | `4.6.7` | 已最新 |
| `console_error_panic_hook` | `0.1.7` | `0.1.7` | 已最新 |
| `cron` | `0.17.0` | `0.17.0` | 已最新 |
| `eframe` | `0.36.2` | `0.37.0` | 升级后已最新 |
| `egui` | `0.36.2` | `0.37.0` | 升级后已最新 |
| `fastrand` | `2.5.0` | `2.5.0` | 已最新 |
| `foldhash` | `0.2.0` | `0.1.5, 0.2.0` | 直接分支已最新；另有传递旧分支 |
| `fs2` | `0.4.3` | `0.4.3` | 已最新 |
| `image` | `0.25.10` | `0.25.10` | 已最新 |
| `js-sys` | `0.3.106` | `0.3.106` | 已最新 |
| `mimalloc` | `0.1.52` | `0.1.52` | 已最新 |
| `parquet` | `60.0.0` | `60.0.0` | 已最新 |
| `png` | `0.18.1` | `0.18.1` | 已最新 |
| `pyo3` | `0.29.3` | `0.30.1` | 升级后已最新 |
| `rayon` | `1.12.0` | `1.12.0` | 已最新 |
| `rfd` | `0.17.2` | `0.17.2` | 已最新 |
| `rusqlite` | `0.40.2` | `0.40.2` | 已最新 |
| `serde` | `1.0.229` | `1.0.229` | 已最新 |
| `serde-wasm-bindgen` | `0.6.5` | `0.6.5` | 已最新 |
| `serde_arrow` | `0.15.1` | `0.15.1` | 已最新 |
| `serde_json` | `1.0.151` | `1.0.154` | 升级后已最新 |
| `sha2` | `0.11.0` | `0.11.1` | 升级后已最新 |
| `smallvec` | `1.16.2` | `1.16.3` | 升级后已最新 |
| `sysinfo` | `0.39.6` | `0.39.6` | 已最新 |
| `tempfile` | `3.27.0` | `3.27.0` | 已最新 |
| `tokio` | `1.53.1` | `1.54.1` | 升级后已最新 |
| `tokio-postgres` | `0.7.18` | `0.7.18` | 已最新 |
| `toml` | `1.1.6+spec-1.1.0` | `1.1.8+spec-1.1.0` | 升级后已最新 |
| `tower-http` | `0.7.1` | `0.7.1` | 已最新 |
| `tracing` | `0.1.44` | `0.1.44` | 已最新 |
| `tracing-subscriber` | `0.3.23` | `0.3.23` | 已最新 |
| `tsify` | `0.5.8` | `0.5.8` | 已最新 |
| `wasm-bindgen` | `0.2.129` | `0.2.129` | 已最新 |

### 传递依赖与保留原则

最终核对 545 种 registry 包名：343 种已最新、73 种升级后已最新、16 种新增且已最新、113 种仍有上游约束的旧分支。主锁由 591 个 package 节点变为 600 个，独立锁由 145 个变为 144 个；节点数包含不同版本和本地 crate，不能等同依赖种类。

- 替换已被撤回的 `clipboard-win 5.4.1`，现为 `5.4.3`。最终两锁无 yanked、prerelease 或 git 来源依赖；`+spec-1.1.0` 是 build metadata，不是预发布。
- 保持既有 feature 与上游 semver 要求；不以 `[patch]` 或精确版本覆盖硬换不兼容分支。
- 依赖声明的最高 Rust 要求为 `1.95`（egui/eframe 系列和 sysinfo），实际验证使用 Rust `1.99.0`；元数据扫描不等于完整 MSRV 编译验证。
- WASM 的 `wasm-bindgen` 和本地 CLI 均为 `0.2.129`，生成 glue 时完全匹配。
- egui 0.37 适配截图回调、滚动偏移 getter、滚轮来源及 ID API；PyO3 0.30 移除已成为默认行为的 `skip_from_py_object` 标记，保留既有 Python 类与返回值。

### Python 与可选工具

| 构建包 | 更新前要求 | 更新后要求 |
| --- | --- | --- |
| setuptools | `>=68` | `>=84.2.1` |
| wheel | 未设下限 | `>=0.48.0` |
| setuptools-rust | `>=1.8` | `>=1.13.0` |

Python 包无运行时第三方依赖。构建的 `packaging`、`semantic_version` 由隔离环境解析；NumPy、PyArrow 以及浏览器测量使用的 Playwright/linkedom 是可选分析工具，本仓库未固定其版本，本轮不改用户环境。

## 已修复的问题与优化

| 范围 | 问题与处理 | 回归重点 |
| --- | --- | --- |
| CLI | 拒绝非有限阈值；盘符之外的冒号不再绕过路径检查 | 实际 clap 输入、合法 drive/UNC、非法附加冒号 |
| 调试查询 | 初始化未完成时其他线程可能读到默认标志；以 `Once` 保护 | 隔离子进程预设环境，32 线程首次并发查询 |
| RC4 | 预览遗漏本步交换后的数组值；仅修 `peek_next_u8` | 对照克隆后消费、碰撞边界、连续观察与完整状态不变 |
| 天梯 | i32 先相减会溢出或回绕；先转 f64 再相减 | i32 极值方向、普通结果 |
| C 图标 | 错误码丢失、成功残留旧错误；复用统一 FFI 边界 | 非法 UTF-8、空指针、成功清错与释放 |
| DS4 | 先删目标后 rename 可能丢旧输出；直接替换 | 正常覆盖、暂存丢失、Drop 清理、Windows 独占目标 |
| 编码器 | 复用输入验证得到的容量计量，减少重复遍历 | 输出与容量边界、失败后清理，不冻结内部调用次数 |
| 图标缓存 | 消除冗余 key clone；命中先借用查询 | 完整 key、命中/miss/禁用、输出顺序与 RNG 不变 |
| 可选 PNG | 无条件图标导入导致关闭默认 feature 时无法编译；条件导入并明确拒绝缺失能力的展示请求 | 无图标战斗/模型会话正常，展示请求返回 `UNSUPPORTED_OPTION`，PNG 专属测试正确门控 |

## 文档审查

补充所有权、错误状态、线程、采样局号、容量及平台计时等有效内容。短字段、枚举值和冻结测试输入不机械扩写。修正现行命令、Python 构建后端、护盾显示与历史报告适用状态等过时说明。

Markdown 审查覆盖基线全部 **148 份**（其中 8 份实际测试输入只读保留），脚本审查覆盖 **33 份**。逐项清单、适用状态和验证边界见[文档审查覆盖报告](reviews/2026-10-maintenance-doc-audit.md)。Rust 基线共 **463 份**：核心运行时组清点 136 份并逐块审完原有 52 份带文档文件的 1616 行文档，逻辑重点覆盖随机流、会话、prepared、批量与名字构建；编码器/CLI 组审阅 52 份 Rust 与 2 份 JS；胜率与计时 2 份由 lead 审查。绑定/界面组审阅 67 份 Rust、C 头文件和 TypeScript 声明。工具/测试组覆盖 206 份：95 份实现与公共支撑源码实读、11 份固定系数表维度与说明核对、95 份测试 fixture/注册分片结构及文档扫描、5 份集成测试只读审查。该分类不把文件清点或固定数据结构检查冒充所有源码逐行复核。

## 验证记录

验证环境为 Windows x86_64、Rust 1.99.0、Python 3.12.10、Node 24.19.0。清理后的 Cargo 验证统一使用命令级无 wrapper、关闭增量、开发/测试 debug 信息为 0 的设置，未修改项目配置。

| 验证 | 结果与边界 |
| --- | --- |
| 升级前 `cargo test --workspace --locked` | 通过，建立行为基线 |
| clean 后 `cargo test --workspace --all-features --locked` | 从零重建通过；核心 610、CLI 91、完整 runtime corpus 124 项通过，保留原有显式忽略的文档示例 |
| 核心 release/no_debug 与 release corpus | 清理前实际通过：核心 608 项、corpus 124 项；后续增加的 feature 适配另以完整 debug 配置测试验证 |
| `cargo test -p tswn_core --no-default-features --lib --locked` | 614 项通过，含无 PNG 的真实会话/错误回归 |
| `cargo check -p tswn_openbox --no-default-features --bin openbox-cli --locked` | 通过，验证无头入口 |
| `cargo clippy --workspace --all-targets --all-features --locked` | 退出 0；仍有参数数量、复杂类型、可简化判断等非阻断警告，未使用 `-D warnings` 冒充零告警 |
| `RUSTDOCFLAGS=-D warnings cargo doc --workspace --all-features --no-deps --locked` | 通过，所有工作区 Rustdoc 无警告 |
| `cargo +nightly fmt --all --check` 与独立排名 crate 格式检查 | 通过；仅使用 nightly |
| 真实 WASM target 构建、同版本 Node glue、接口测试 | 清理前实际通过：8 项，含规范 DTO、错误码、会话/回放一致性 |
| Python 实际扩展 API | 清理前实际通过：类型 stub、胜率、评分、批量/配对、DIY、图标和战斗会话 |
| Rust CLI / Python / C / WASM 四端比对 | 清理前实际通过：4 份冻结输入各完整/截断两档，共 8 场 payload 精确一致 |
| 公共 API 文档的 Rust/Python/JavaScript 示例 | 清理前实际通过；验证脚本将 Rust stdout 保存为普通日志，避免 Windows NUL 设备权限依赖 |
| 页面纯模块与脚本测试 | 43 项前端通过；含 NumPy 的 6 项 Python 单元测试通过 |
| 文档审阅检查 | 专项扫描覆盖当时 151 份文档的本地路径/标题锚点，8 份 Markdown fixture 与原 blob 相同；详见专项报告 |

独立 `tswn_name_ranker` 的依赖已编译到本 crate，随后因既有两份校准资源缺失失败；本轮不宣称其完整构建通过。Python 最新构建依赖能解析并启动 PEP 517 构建，完整 wheel 在 Cargo metadata 下载非宿主依赖时遭全局缓存写入拒绝，本轮未修改该目录权限；可在允许写 Cargo 缓存的本机终端单独验收 wheel。真实数据库、其他操作系统、OHOS、最低 Rust 1.95、PGO 和浏览器性能未在本轮重测。

### 原生 GUI 实机验收边界

`ui_capture` 编译和原有 GUI 输入/滚动测试通过，但本机默认 WGPU 后端启动截图驱动时发生 `0xc0000005` 原生访问冲突，未生成完整截图。为区分升级回归，另建基线 `704a1ac9` 的隔离工作树，使用旧 egui/eframe 0.36.2、相同 CWD/会话/参数/低占用构建设置与默认后端对照，旧版同样复现访问冲突。故该现象不是仅在升级后出现，具体环境/既有原生路径根因尚未定位，也不将新版 GUI 实机验收记为通过。

有效补充观察：新程序 `--help` 退出 0；强制 Vulkan 的进程退出 0，但未产生约定的四张 PNG，不能算截图成功。关闭新版自动系统字体回退仍然复现访问冲突，该未提交实验已撤回。GL 等待实验被停止；CDB 在本环境没有生成有效崩溃栈，不作为归因证据。未继续扩展后端试错或提交猜测性的字体/渲染默认值变更。

用户随后明确要求该访问冲突暂按 RDP 环境问题忽略，本轮据此停止排查，不将它作为交付阻塞项。需要时再在本机非 RDP 桌面做新旧对照；这里保留实际观测，不将未完成的截图验收改记为通过。临时基线工作树已移除，并曾重新构建当前分支，避免旧版覆盖同名可执行文件。用户之后又自行执行了 `cargo clean`；此前验证记录与源码提交不受影响，本轮不再重复重建覆盖用户最新的清理。

### 本机构建环境适配

本机全局启用了 `sccache`。本次使用 `cc 1.10.0` 重编译 C++ 依赖时，探测将该 wrapper 与 MSVC 组合误识别为 GNU，导致 mimalloc 编译报 `/WX-` 文件不存在。旧版 cc 也有从 `RUSTC_WRAPPER` 选择缓存的逻辑，因此这里仅记录实际环境问题，不据此断定新版回归。以下命令级覆盖已实测可用，保留最新依赖与 allocator，不修改全局配置：

```powershell
cargo --config 'build.rustc-wrapper=""' check -p tswn_core --lib --locked
```

未启用该全局 wrapper 的普通构建无需额外配置。采用相同覆盖的完整验证会在记录中注明。

本轮磁盘空间一度耗尽，先清理了本项目可重建的增量缓存。随后按用户明确要求停止所有验证作业并执行 `cargo clean`，删除 37,001 个文件、释放 27.1 GiB，D 盘恢复约 94.3 GB 可用空间。之后串行重建，命令级设置 `CARGO_INCREMENTAL=0`、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`，减少缓存增长；没有将这些本机验证设置写入项目或全局配置。

## 保留的后续改进

1. **预设读取与状态构造解耦**：Openbox 默认状态构造会在当前目录创建 `setting/`，测试因此产生目录。后续注入预设根目录或无 I/O 工厂；本轮不引入全局 `chdir`，避免并行测试相互影响。
2. **独立单号排名工具补齐资源**：该 crate 按既有设计排除于 workspace，缺两份编译期校准资源。本轮已更新其锁并审阅源码，不伪造数据掩盖缺失。
3. **外部系统独立验证**：真实 PostgreSQL 回写、校准 Python 环境和非 Windows 平台需要相应资源；单元测试不冒充这些集成验证。
4. **性能由测量驱动**：扩大缓存或调整调度前，先在固定输入、相同 feature 和工具链下做 A/B；本轮不为尚未观察到的瓶颈增加基础设施。

### 具体重构候选与验证前提

| 优先级 | 观察位置与问题 | 下一步 |
| --- | --- | --- |
| 高 | [lane 缓存读取](../crates/tswn_lane_ranker/src/db/groups.rs) 与 [pairwise 复用](../crates/tswn_lane_ranker/src/pairwise/rates.rs) 主要按 ID 读取，不按采样/core 机制版本完整区分 | 定义缓存身份，加入采样策略、样本数、eval_rq 与机制版本，再补同 ID 跨版本失效回归和数据迁移 |
| 高 | [ladder 存储](../crates/tswn_ladder/src/storage.rs) 与 [回写编排](../crates/tswn_ladder/src/updator.rs) 的分数、handicap、排名和历史不是一个跨步骤事务；数值解码还需核实数据库 schema | 在独立 PostgreSQL 做中途失败注入，确定事务边界与失败补偿；先确认 NUMERIC 实际使用再改解码 |
| 中 | [lane HTTP](../crates/tswn_lane_ranker/src/web.rs) 与 [name HTTP](../crates/tswn_name_ranker/src/web.rs) 有同步重计算；[lane 任务](../crates/tswn_lane_ranker/src/service.rs) 缺同赛道互斥和 panic 终态恢复 | 统一 blocking 任务入口、限并发、取消和终态；用并发请求压测响应与资源释放 |
| 中 | [DIY 导出](../crates/tswn_core/src/bin/tswn_cli/to_diy.rs) 有写入错误被忽略；[编码批次分配](../crates/tswn_core/src/encoder/batch.rs) 缺总字节预算；[调度缓冲](../crates/tswn_core/src/bench_sched.rs) 乱序积压可随任务量增长 | 分别补失败 writer、超大 shape/预算和慢首项等真实行为回归，再传播 I/O 错误、引入 checked 分配或背压 |
| 中 | [PWP 统计](../crates/tswn_pwp/src/stats.rs) 与 [校准](../crates/tswn_pwp/src/calibrate.rs) 的 manifest 前置校验需加强；[基准 CPU 统计](../crates/tswn_pwp/src/bench.rs) 是累计值 | 以损坏 manifest 和阶段边界建立独立回归，区分累计与阶段增量，再调整校验/显示 |
| 低 | [随机整数 API](../crates/tswn_core/src/rc4.rs) 有既有有效上界限制；[DS4 输出复制](../crates/tswn_ds4/src/pipeline.rs) 双次追加意图待确认 | 随机接口另设计 checked/typed 入口，保留旧消费契约；双追加先与 C++/golden 对照，不直接当作已确认缺陷修改 |


## 旧传递分支清单

列出仍保留旧版本的包及其直接父约束。部分包同时有最新分支；这不代表所列代码全部在当前 Windows target 执行。完整解析关系以两份受版本控制的锁文件为准。

| 包 | 保留版本 → 当日最新 | 直接父依赖要求 |
| --- | --- | --- |
| `allocator-api2` | `0.2.21` → `0.4.0` | hashbrown 0.16.1: ^0.2.9 |
| `atspi` | `0.29.0` → `0.30.0` | accesskit_unix 0.24.0: ^0.29 |
| `atspi-common` | `0.13.0` → `0.14.0` | accesskit_atspi_common 0.21.0: ^0.13; atspi 0.29.0: ^0.13.0; atspi-proxies 0.13.0: ^0.13.0 |
| `atspi-proxies` | `0.13.0` → `0.14.0` | atspi 0.29.0: ^0.13.0 |
| `base64` | `0.22.1` → `0.23.1` | postgres-protocol 0.6.12: ^0.22 |
| `bit-set` | `0.10.0` → `0.11.1` | naga 30.0.1: ^0.10; wgpu-core 30.0.1: ^0.10; wgpu-hal 30.0.1: ^0.10 |
| `bit-vec` | `0.9.1` → `0.10.1` | bit-set 0.10.0: ^0.9.1; wgpu-core 30.0.1: ^0.9.1 |
| `bitflags` | `1.3.2` → `2.13.2` | core-graphics 0.23.2: ^1.0; core-graphics-types 0.1.3: ^1.0; redox_syscall 0.4.1: ^1.1.0 |
| `block2` | `0.5.1` → `0.6.2` | objc2-app-kit 0.2.2: ^0.5.1; objc2-cloud-kit 0.2.2: ^0.5.1; objc2-contacts 0.2.2: ^0.5.1; objc2-core-data 0.2.2: ^0.5.1; objc2-core-image 0.2.2: ^0.5.1; objc2-core-location 0.2.2: ^0.5.1; objc2-foundation 0.2.2: ^0.5.1; objc2-link-presentation 0.2.2: ^0.5.1; objc2-metal 0.2.2: ^0.5.1; objc2-quartz-core 0.2.2: ^0.5.1; objc2-ui-kit 0.2.2: ^0.5.1; objc2-uniform-type-identifiers 0.2.2: ^0.5.1; objc2-user-notifications 0.2.2: ^0.5.1; winit 0.30.13: ^0.5.1 |
| `calloop` | `0.13.0` → `0.14.5` | calloop-wayland-source 0.3.0: ^0.13.0; smithay-client-toolkit 0.19.2: ^0.13.0; winit 0.30.13: ^0.13.0 |
| `calloop-wayland-source` | `0.3.0` → `0.4.1` | smithay-client-toolkit 0.19.2: ^0.3.0 |
| `core-foundation` | `0.9.4` → `0.10.1` | core-graphics 0.23.2: ^0.9; core-graphics-types 0.1.3: ^0.9.4; winit 0.30.13: ^0.9.3 |
| `core-graphics` | `0.23.2` → `0.25.0` | winit 0.30.13: ^0.23.1 |
| `core-graphics-types` | `0.1.3` → `0.2.0` | core-graphics 0.23.2: ^0.1 |
| `downcast-rs` | `1.2.1` → `2.0.2` | wayland-backend 0.3.17: ^1.2 |
| `fallible-iterator` | `0.2.0` → `0.3.0` | postgres-protocol 0.6.12: ^0.2; postgres-types 0.2.14: ^0.2; tokio-postgres 0.7.18: ^0.2 |
| `fax` | `0.2.7` → `0.3.0` | tiff 0.11.3: ^0.2.6 |
| `fearless_simd` | `0.4.1` → `1.1.0` | vello_common 0.2.0: ^0.4.0 |
| `foldhash` | `0.1.5` → `0.2.0` | hashbrown 0.15.5: ^0.1.2 |
| `fontique` | `0.11.1` → `0.12.0` | egui_system_font_provider 0.37.0: ^0.11.1 |
| `getrandom` | `0.2.17, 0.3.4` → `0.4.3` | const-random-macro 0.1.16: ^0.2.0; ahash 0.8.12: ^0.3.1 |
| `glifo` | `0.3.0` → `0.4.0` | vello_cpu 0.2.0: ^0.3.0 |
| `glow` | `0.17.0` → `0.18.0` | egui_glow 0.37.0: ^0.17.0; wgpu-hal 30.0.1: ^0.17 |
| `harfrust` | `0.12.0` → `0.15.0` | epaint 0.37.0: ^0.12.0 |
| `hashbrown` | `0.15.5, 0.16.1` → `0.17.1` | petgraph 0.8.3: ^0.15.0; gpu-allocator 0.28.0: ^0.16.0; rsqlite-vfs 0.1.1: ^0.16.1 |
| `jni-sys` | `0.3.1` → `0.4.1` | ndk 0.9.0: ^0.3; ndk-sys 0.6.0+11769913: ^0.3.0 |
| `libloading` | `0.8.9` → `0.9.0` | ash 0.38.0+1.3.281: ^0.8; dlib 0.5.3: >=0.7, <0.9; glutin 0.32.3: ^0.8.0; khronos-egl 6.0.0: ^0.8; wgpu-hal 30.0.1: ^0.8/^0.8; x11rb 0.13.2: ^0.8.0 |
| `libredox` | `0.1.25` → `0.2.0` | orbclient 0.3.55: ^0.1; whoami 2.1.3: ^0.1.17 |
| `linux-raw-sys` | `0.4.15` → `0.12.1` | rustix 0.38.44: ^0.4.14/^0.4.14 |
| `lock_api` | `0.4.14` → `0.5.0` | parking_lot 0.12.5: ^0.4.14 |
| `matchit` | `0.8.4` → `0.9.2` | axum 0.8.9: =0.8.4 |
| `miniz_oxide` | `0.8.9` → `0.9.1` | png 0.18.1: ^0.8 |
| `moxcms` | `0.8.1` → `0.9.1` | image 0.25.10: ^0.8.0 |
| `objc2` | `0.5.3` → `0.6.5` | accesskit_ios 0.2.1: ^0.5.1; accesskit_macos 0.27.1: ^0.5.1; block2 0.5.1: ^0.5.2; objc2-app-kit 0.2.2: ^0.5.2; objc2-cloud-kit 0.2.2: ^0.5.2; objc2-contacts 0.2.2: ^0.5.2; objc2-core-data 0.2.2: ^0.5.2; objc2-core-image 0.2.2: ^0.5.2; objc2-core-location 0.2.2: ^0.5.2; objc2-foundation 0.2.2: ^0.5.2; objc2-link-presentation 0.2.2: ^0.5.2; objc2-metal 0.2.2: ^0.5.2; objc2-quartz-core 0.2.2: ^0.5.2; objc2-symbols 0.2.2: ^0.5.2; objc2-ui-kit 0.2.2: ^0.5.2; objc2-uniform-type-identifiers 0.2.2: ^0.5.2; objc2-user-notifications 0.2.2: ^0.5.2; winit 0.30.13: ^0.5.2 |
| `objc2-app-kit` | `0.2.2` → `0.3.2` | accesskit_macos 0.27.1: ^0.2.0; objc2-link-presentation 0.2.2: ^0.2.2; winit 0.30.13: ^0.2.2 |
| `objc2-cloud-kit` | `0.2.2` → `0.3.2` | objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-contacts` | `0.2.2` → `0.3.2` | objc2-core-location 0.2.2: ^0.2.2 |
| `objc2-core-data` | `0.2.2` → `0.3.2` | objc2-app-kit 0.2.2: ^0.2.2; objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-core-image` | `0.2.2` → `0.3.2` | objc2-app-kit 0.2.2: ^0.2.2; objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-core-location` | `0.2.2` → `0.3.2` | objc2-cloud-kit 0.2.2: ^0.2.2; objc2-ui-kit 0.2.2: ^0.2.2; objc2-user-notifications 0.2.2: ^0.2.2 |
| `objc2-foundation` | `0.2.2` → `0.3.2` | accesskit_ios 0.2.1: ^0.2.0; accesskit_macos 0.27.1: ^0.2.0; objc2-app-kit 0.2.2: ^0.2.2; objc2-cloud-kit 0.2.2: ^0.2.2; objc2-contacts 0.2.2: ^0.2.2; objc2-core-data 0.2.2: ^0.2.2; objc2-core-image 0.2.2: ^0.2.2; objc2-core-location 0.2.2: ^0.2.2; objc2-link-presentation 0.2.2: ^0.2.2; objc2-metal 0.2.2: ^0.2.2; objc2-quartz-core 0.2.2: ^0.2.2; objc2-symbols 0.2.2: ^0.2.2; objc2-ui-kit 0.2.2: ^0.2.2; objc2-uniform-type-identifiers 0.2.2: ^0.2.2; objc2-user-notifications 0.2.2: ^0.2.2; winit 0.30.13: ^0.2.2/^0.2.2 |
| `objc2-link-presentation` | `0.2.2` → `0.3.2` | objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-metal` | `0.2.2` → `0.3.2` | objc2-core-image 0.2.2: ^0.2.2; objc2-quartz-core 0.2.2: ^0.2.2 |
| `objc2-quartz-core` | `0.2.2` → `0.3.2` | objc2-app-kit 0.2.2: ^0.2.2; objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-symbols` | `0.2.2` → `0.3.2` | objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-ui-kit` | `0.2.2` → `0.3.2` | accesskit_ios 0.2.1: ^0.2.0; winit 0.30.13: ^0.2.2 |
| `objc2-uniform-type-identifiers` | `0.2.2` → `0.3.2` | objc2-ui-kit 0.2.2: ^0.2.2 |
| `objc2-user-notifications` | `0.2.2` → `0.3.2` | objc2-ui-kit 0.2.2: ^0.2.2 |
| `orbclient` | `0.3.55` → `0.4.6` | winit 0.30.13: ^0.3.47 |
| `parking_lot` | `0.12.5` → `0.13.0` | arboard 3.7.0: ^0.12; eframe 0.37.0: ^0.12.5; epaint 0.37.0: ^0.12.5; tokio 1.54.1: ^0.12.0; tokio-postgres 0.7.18: ^0.12; wgpu 30.0.1: ^0.12.3; wgpu-core 30.0.1: ^0.12.3; wgpu-hal 30.0.1: ^0.12.3 |
| `parking_lot_core` | `0.9.12` → `0.10.0` | parking_lot 0.12.5: ^0.9.12 |
| `phf` | `0.11.3, 0.12.1, 0.13.1` → `0.14.0` | cron 0.17.0: ^0.11; chrono-tz 0.10.4: ^0.12; tokio-postgres 0.7.18: ^0.13 |
| `phf_generator` | `0.11.3` → `0.14.0` | phf_macros 0.11.3: ^0.11.1 |
| `phf_macros` | `0.11.3` → `0.14.0` | phf 0.11.3: ^0.11.3 |
| `phf_shared` | `0.11.3, 0.12.1, 0.13.1` → `0.14.0` | phf 0.11.3: ^0.11.3; phf_generator 0.11.3: ^0.11.2; phf_macros 0.11.3: ^0.11.2; phf 0.12.1: ^0.12.0; phf 0.13.1: ^0.13.1 |
| `pollster` | `0.4.0` → `1.0.1` | rfd 0.17.2: ^0.4 |
| `quick-xml` | `0.41.0` → `0.42.0` | wayland-scanner 0.31.11: ^0.41 |
| `r-efi` | `5.3.0, 6.0.0` → `7.1.0` | getrandom 0.3.4: ^5.1; getrandom 0.4.3: ^6 |
| `rand` | `0.8.8` → `0.10.3` | phf_generator 0.11.3: ^0.8 |
| `rand_core` | `0.6.4` → `0.10.1` | rand 0.8.8: ^0.6.0 |
| `read-fonts` | `0.41.0` → `0.46.0` | fontique 0.11.1: ^0.41.0; harfrust 0.12.0: ^0.41.0; skrifa 0.44.0: ^0.41.0 |
| `redox_syscall` | `0.4.1, 0.5.18` → `0.9.5` | winit 0.30.13: ^0.4.1; parking_lot_core 0.9.12: ^0.5 |
| `rsqlite-vfs` | `0.1.1` → `0.2.0` | sqlite-wasm-rs 0.5.5: ^0.1.0 |
| `rustc-hash` | `1.1.0` → `2.1.3` | naga 30.0.1: ^1.1; naga-types 30.0.1: ^1.1; wgpu-core 30.0.1: ^1.1 |
| `rustix` | `0.38.44` → `1.1.5` | calloop 0.13.0: ^0.38; calloop-wayland-source 0.3.0: ^0.38.4; smithay-client-toolkit 0.19.2: ^0.38.15; winit 0.30.13: ^0.38.4 |
| `sctk-adwaita` | `0.10.1` → `0.12.0` | winit 0.30.13: ^0.10.1 |
| `serde_derive_internals` | `0.29.1` → `0.30.0` | tsify-macros 0.5.8: ^0.29 |
| `skrifa` | `0.44.0` → `0.49.0` | egui_system_font_provider 0.37.0: ^0.44.0; epaint 0.37.0: ^0.44.0; glifo 0.3.0: ^0.44.0 |
| `smithay-client-toolkit` | `0.19.2, 0.20.0` → `0.21.1` | sctk-adwaita 0.10.1: ^0.19.0; winit 0.30.13: ^0.19.2; smithay-clipboard 0.7.3: ^0.20.0 |
| `smol_str` | `0.2.2` → `0.3.7` | winit 0.30.13: ^0.2.0 |
| `sqlite-wasm-rs` | `0.5.5` → `0.6.2` | rusqlite 0.40.2: ^0.5.1 |
| `strict-num` | `0.1.1` → `0.2.0` | tiny-skia-path 0.11.4: ^0.1 |
| `syn` | `2.0.119` → `3.0.9` | enumflags2_derive 0.7.12: ^2.0; enumn 0.1.14: ^2.0.46; jni-macros 0.22.4: ^2.0; jni-sys-macros 0.4.1: ^2; num_enum_derive 0.7.6: ^2; phf_macros 0.11.3: ^2; phf_macros 0.14.0: ^2; pin-project-internal 1.1.13: ^2.0.1; serde_derive_internals 0.29.1: ^2.0.46; thiserror-impl 1.0.69: ^2.0.87; tracing-attributes 0.1.31: ^2.0; tsify-macros 0.5.8: ^2.0; windows-implement 0.60.2: ^2.0; windows-interface 0.59.3: ^2.0; zbus-lockstep-macros 0.5.2: ^2.0; zerocopy-derive 0.8.63: ^2.0.46 |
| `thiserror` | `1.0.69` → `2.0.21` | calloop 0.13.0: ^1.0.7; ndk 0.9.0: ^1.0.23; smithay-client-toolkit 0.19.2: ^1.0.30 |
| `thiserror-impl` | `1.0.69` → `2.0.21` | thiserror 1.0.69: =1.0.69 |
| `tiny-skia` | `0.11.4` → `0.12.0` | sctk-adwaita 0.10.1: ^0.11 |
| `tiny-skia-path` | `0.11.4` → `0.12.0` | tiny-skia 0.11.4: ^0.11.4 |
| `vello_common` | `0.2.0` → `0.3.0` | glifo 0.3.0: ^0.2.0; vello_cpu 0.2.0: ^0.2.0 |
| `vello_cpu` | `0.2.0` → `0.3.0` | epaint 0.37.0: ^0.2.0 |
| `wasi` | `0.11.1+wasi-snapshot-preview1` → `0.14.7+wasi-0.2.4` | getrandom 0.2.17: ^0.11; mio 1.2.4: ^0.11.0 |
| `wasip2` | `1.0.4+wasi-0.2.12` → `2.0.1+wasi-0.2.12` | getrandom 0.3.4: ^1; wasi 0.14.7+wasi-0.2.4: ^1.0.1 |
| `wayland-protocols-experimental` | `20250721.0.1` → `20251230.0.3` | smithay-client-toolkit 0.20.0: ^20250721.0.1 |
| `weezl` | `0.1.12` → `0.2.1` | tiff 0.11.3: ^0.1.10 |
| `windows-collections` | `0.3.2` → `0.100.0` | windows 0.62.2: ^0.3.2 |
| `windows-core` | `0.62.2` → `0.100.0` | accesskit_windows 0.35.1: ^0.62.0; fontique 0.11.1: ^0.62.2; iana-time-zone 0.1.65: >=0.56, <=0.62; wgpu-hal 30.0.1: ^0.62; windows 0.62.2: ^0.62.2; windows-collections 0.3.2: ^0.62.2; windows-future 0.3.2: ^0.62.2; windows-numerics 0.3.1: ^0.62.2 |
| `windows-future` | `0.3.2` → `0.100.0` | windows 0.62.2: ^0.3.2 |
| `windows-implement` | `0.60.2` → `0.100.0` | windows-core 0.62.2: ^0.60.2 |
| `windows-interface` | `0.59.3` → `0.100.0` | windows-core 0.62.2: ^0.59.3 |
| `windows-link` | `0.2.1` → `0.100.0` | chrono 0.4.45: ^0.2; gethostname 1.1.0: ^0.2.1; jni 0.22.4: ^0.2; libloading 0.8.9: ^0.2; parking_lot_core 0.9.12: ^0.2.0; windows-core 0.62.2: ^0.2.1; windows-future 0.3.2: ^0.2.1; windows-numerics 0.3.1: ^0.2.1; windows-result 0.4.1: ^0.2.1; windows-strings 0.5.1: ^0.2.1; windows-sys 0.61.2: ^0.2.1; windows-threading 0.2.1: ^0.2.1 |
| `windows-numerics` | `0.3.1` → `0.100.0` | windows 0.62.2: ^0.3.1 |
| `windows-result` | `0.4.1` → `0.100.0` | wgpu-hal 30.0.1: ^0.4; windows-core 0.62.2: ^0.4.1 |
| `windows-strings` | `0.5.1` → `0.100.0` | windows-core 0.62.2: ^0.5.1 |
| `windows-sys` | `0.52.0, 0.59.0` → `0.61.2` | glutin 0.32.3: ^0.52; glutin_egl_sys 0.7.1: ^0.52; winit 0.30.13: ^0.52.0; rustix 0.38.44: >=0.52, <=0.59 |
| `windows-targets` | `0.52.6` → `0.53.5` | windows-sys 0.52.0: ^0.52.0; windows-sys 0.59.0: ^0.52.6 |
| `windows-threading` | `0.2.1` → `0.100.0` | windows-future 0.3.2: ^0.2.1 |
| `windows_aarch64_gnullvm` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_aarch64_msvc` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_i686_gnu` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_i686_gnullvm` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_i686_msvc` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_x86_64_gnu` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_x86_64_gnullvm` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `windows_x86_64_msvc` | `0.52.6` → `0.53.1` | windows-targets 0.52.6: ^0.52.6 |
| `winnow` | `0.7.15` → `1.0.4` | cron 0.17.0: ^0.7.0 |
| `wit-bindgen` | `0.57.1` → `0.63.0` | wasip2 1.0.4+wasi-0.2.12: ^0.57.1 |
| `x11rb` | `0.13.2` → `0.14.0` | arboard 3.7.0: ^0.13; winit 0.30.13: ^0.13.0 |
| `x11rb-protocol` | `0.13.2` → `0.14.0` | x11rb 0.13.2: ^0.13.2 |
| `xml-rs` | `0.8.29` → `1.0.0` | gl_generator 0.14.0: ^0.8 |
| `zbus` | `5.19.0` → `6.1.0` | accesskit_unix 0.24.0: ^5.19; atspi-common 0.13.0: ^5.5; atspi-proxies 0.13.0: ^5.5 |
| `zbus-lockstep` | `0.5.2` → `0.7.0` | atspi-common 0.13.0: ^0.5.1; zbus-lockstep-macros 0.5.2: ^0.5.2 |
| `zbus-lockstep-macros` | `0.5.2` → `0.7.0` | atspi-common 0.13.0: ^0.5.1 |
| `zbus_macros` | `5.19.0` → `6.1.0` | zbus 5.19.0: ^5.19.0 |
| `zbus_xml` | `5.2.1` → `6.0.0` | zbus-lockstep 0.5.2: ^5.0.2; zbus-lockstep-macros 0.5.2: ^5.0.2 |

## 分块提交记录

下表为实现、兼容修复与分模块契约提交。仓库说明、更新日志和本报告随最后的 `docs(repo)` 提交收口；最新完整历史可用 `git log 704a1ac9..HEAD` 查看。所有提交只保存在本地，没有推送远端。

| 提交 | 主题 |
| --- | --- |
| 386da992 | chore(deps): 升级稳定版依赖并适配 PyO3 与 egui |
| d0607806 | fix(core): 修正随机预览的交换槽位结果 |
| 35aea0d9 | fix(core): 等待调试标志完成并发初始化 |
| 48d8a214 | fix(cli): 拒绝非有限阈值与额外路径冒号 |
| 8bee1368 | perf(encoder): 复用验证阶段的容量计量 |
| c9314803 | fix(capi): 统一图标接口错误与清理契约 |
| 828a957c | fix(ds4): 替换失败时保留已有输出文件 |
| 6634b92a | fix(ladder): 避免极端评分差值溢出 |
| b8941932 | docs(bindings): 补充跨语言所有权与错误边界 |
| 1a3ad1eb | docs(openbox): 明确后台任务与预设资源边界 |
| a925485c | docs(tools): 补充排名流程与缓存持久化约束 |
| 0f2af07f | docs(data): 明确采样编码与冻结回归契约 |
| ba6d6ddc | docs(core): 完善运行时编码与命令行契约 |
| f338e3e4 | fix(core): 补齐可选图标能力并复用快照缓存 |
| 8d6bbaaf | fix(scripts): 改善绑定验证的锁定与日志 |
