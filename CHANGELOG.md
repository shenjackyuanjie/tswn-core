# 工作区更新日志

本文件记录跨 crate 的维护与兼容性变更；各 crate 的版本发布历史仍以对应 CHANGELOG 为准。

## 未发布 · 2026-11-26 模块结构与仓库边界整理

- **BREAKING**：`BattlePlayerState.source_id` 全链路更名为 `root_owner_id`（core DTO / Python dict 键与类型存根 / TypeScript 声明 / C ABI 的 JSON 输出同步），与 `RuntimePlayerSnapshot.root_owner_id` 统一「根主人」命名。语义不变，不保留旧名。见 [核心更新日志](crates/tswn_core/CHANGELOG.md#unreleased)、[WASM 更新日志](crates/tswn_wasm/CHANGELOG.md#unreleased)、[Python 更新日志](crates/tswn_py/CHANGELOG.md#unreleased)。
- `tswn_core` 的 13 个模块由 `foo.rs` 改为 `foo/mod.rs`，并清除 5 处 `#[path = "..."]` 外挂测试声明，全 crate 不再混用这两种风格。公开 API 路径保持不变。
- `runtime/` 顶层散落的 4 个 `plain_*.rs` 收进 `runtime/combat/` 并按该目录命名族改名：`plain_summon` / `plain_zombie` / `plain_assassinate` → `skills_summon` / `skills_zombie` / `skills_assassinate`，`plain_slime` → `boss_slime`。原因是 `plain` 在函数名里表示角色类别（对应 `boss_`），而 `/combat` 的文件名表达机制类别（对应 `skills_*`），原名把角色前缀当成了文件名。函数名与类型名不变。`BattleScratch` 由 `runtime::scratch` 并入 `runtime::combat`（`crate::runtime::BattleScratch` 仍可访问）；`handlers/states.rs` 的蓄力与聚气技能函数拆到新增的 `handlers/skills.rs`。
- `runtime/tests/` 由 60 个文件合并为 40 个，同主题碎片测试归组并去除重复夹具，`#[test]` 总数保持 433 不变；另删 2 个无断言价值的测试（静态表去重自洽、零长数组重言式）。
- `tswn-cli` 的 `bench/winrate.rs` 改名为 `win_rate.rs`，`run_bench_winrate` 等连写标识符统一为蛇形；`runtime/combat` 新增命名约定说明（文件名表机制类别、函数名表角色，以及 `drain_*_into` 与 `run_*_into` 的边界）。
- `.gitignore` 的 `tests` / `*.txt` / `assets` 三条全局通配改为锚定规则，反向 `!` 放行由 20 行降为 0；`icon_sprites.png` 移入 `crates/tswn_core/assets/`，两个 md5 探针脚本移入 `scripts/`。见 [核心更新日志](crates/tswn_core/CHANGELOG.md#unreleased)。

## 未发布 · 2026-11-23 Boss 吞噬对齐

- 吞噬 Boss 时按原版技能槽序转移等级，修复蚊、索尼克等八个 Boss 的技能等级错位。两场多 Boss 对局已加入 CLI 回归测试。见 [核心更新日志](crates/tswn_core/CHANGELOG.md#unreleased)。

## tswn_wasm 0.5.13 · 2026-11-21

- 完整回放页支持 `!test!` 普通／强化评分与单挑。Worker 在累计 1,000 和 10,000 场时展示评分及技能／动作频率，可随时停止。
- WASM 导出可步进的 `ScoreTestSession`；事件采集仅在新会话中启用，现有普通批量评分和 CLI 不变。见 [WASM 更新日志](crates/tswn_wasm/CHANGELOG.md#0513---2026-11-21)。

## tswn_core 0.7.2 · 2026-11-21

- 新增饥饿离场即获胜的 `SAITAMA@!`，补齐 Rinick 属性修改器及其隐藏 Boss 别名。灭霸、生命之轮、感染联动、反弹、状态解除和终局行动按原版复查。
- Boss 组队样例的 1,000 场胜负与原版一致。另逐场验证 3,684 个名字对 13 个原版 Boss 和 Rinick 的 51,576,000 场单挑。基线、样本和复现口径见 [胜率复查](docs/reviews/2026-11-21-boss-win-rate-parity.md)。
- 规范回放补全体力、目标及动画载荷，Python 和 WASM 同步声明相应字段，旧接口保留数值投影。原版对照和历史压力用例见 [战报复查](docs/reviews/2026-11-20-vite-boss-replay.md)与[续轮复查](docs/reviews/2026-11-20-vite-boss-followup.md)。
- 固定阵容批量胜率复用 profile seed 密钥缓冲、小队身份串和场前状态，在存活队伍变动时重新判胜。9 组普通 1v1、5 组普通 2v2 共 458,752 局的逐 seed 对照一致。
- CLI 文件和 stdin 读取默认整行跳过非 UTF-8 内容，并报告跳过行数。完整变更见 [核心更新日志](crates/tswn_core/CHANGELOG.md#072---2026-11-21)。

## Openbox 0.5.4 · 2026-11-21

- Openbox、`openbox-cli` 和 `tswn-cli` 读取文本时统一整行跳过无效 UTF-8，`tswn-cli` 的标准输入也适用。`openbox-cli filter` 已移除原 `--skip-invalid-utf8` 参数。见 [Openbox 更新日志](crates/tswn_openbox/CHANGELOG.md#054---2026-11-21) 和 [后端更新日志](crates/tswn_openbox_backend/CHANGELOG.md#054---2026-11-21)。

## Openbox 0.5.3 · 2026-11-19

- Pair 按完整队伍计算组队加成，ToDIY 导出使用 `++` 分隔成员。其余变更见 [Openbox 更新日志](crates/tswn_openbox/CHANGELOG.md#053---2026-11-19) 和 [后端更新日志](crates/tswn_openbox_backend/CHANGELOG.md#053---2026-11-19)。

## tswn_core 0.7.1 · 2026-11-19

- 加快普通 1v1、2v2 的批量胜率计算。四局交错准备 RC4 初始状态，复用阵容视图，并减少少候选选敌、空 hook 和免回放战斗推进的开销。
- 逐 seed 对照覆盖 14 组普通对局、共 458,752 局，检查胜负、回合数与随机状态。版本明细见 [核心更新日志](crates/tswn_core/CHANGELOG.md#071---2026-11-19)。

## 未发布 · 2026-11-18 补全 mimalloc 分配器与跨制品所有权说明

- 明确 `mimalloc_alloc` 让库堆与调用方进程的 `malloc` / `free` 不共享：`tswn_str_t`、`tswn_bytes_t` 与各 opaque 句柄只能用创建它的那份库的 `*_free` 归还；错分配器释放会破坏堆并直接终止调用方进程，不会以 `tswn_status_t` 或 last_error 的形式失败。
- 同一进程可同时装载 C 库与 Python 扩展（`scripts/verify_battle_cross_binding.py` 即此用法），但资源不得跨制品转移，两份制品各持有一份 mimalloc。静态链接在同一 crate graph 内会因第二个 `#[global_allocator]` 编译失败，动态库边界没有该保护；宿主自带分配器时可在构建时关闭 `tswn_core` 的默认 feature。
- 只改文档与注释：C ABI、导出签名、JSON 契约、Python 调用签名与默认战斗行为均不变。

## 未发布 · 2026-11-12 吸收 custom 分支的 bed2 半资源策略

- 通过 `merge(custom)` 把 `custom` 并入 main（内容提交署名原作者 1cy3ish）：新增 `MergePolicy::FixedLaneHalfResources`，bed2 召唤物合并时只获得目标一半的 MP 差值与行动点；其它合并策略保持全量，默认对局与 golden 基线不变。
- 分支上落后的实现没有回灌：summon/overlay 体系已按 main 重构，代码按现有结构重写；encoder 的 merge policy 编码新增值 4。

## 未发布 · 2026-11-12 合入 fix/Rinick-feature（默认关闭）

- 通过 `merge(rinick)` 合入 `fix/Rinick-feature` 的两处语义补丁并保留分支历史，语义放进两个默认关闭的 feature：`hide_berserk_compat`（隐匿不再吞掉狂暴强制行动）与 `curse_caster_charge`（诅咒蓄力按施法者判定）。
- 默认构建行为与冻结语料不变：`cargo test -p tswn_test --features runtime-corpus` 仍 124 全绿；开启 `hide_berserk_compat` 有 14 条语料差异、`curse_caster_charge` 有 1 条（`fight_multi_3`），属于预期且只记录在 [评审文档](docs/reviews/2026-10-fix-rinick-feature.md#落地方式2026-11-12)，仓库没有 bless 工具，不改 `golden.json`。
- `tswn_core` / `tswn_wasm` / `tswn_py` / `tswn_test` 均转发这两个 feature，可分别开关。

## 未发布 · 2026-11-12 合并 instant 开发线

- 把 `instant` 的「外部驱动对局」并入 main：`battle_replay_with_actions`、`BattleSession::new_with_actions/queue_actions/paused_actor/paused_forced_action/peek_magic_cost/preview_targets/`（Python）`drain_frames`、被动决策暂停（`passive_decisions`/`passive_decision_mode`）、多受控队伍与 `controlled_minions` 全部保留。
- 暂停与续行统一到一套实现：指令队列、暂停标记与药水状态挂在 `CombatRuntime::control`，会话在 `advance_round` 留检查点，缺指令时整回合回滚，`submit_action` 后重放同一行动，不再消耗行动条与随机流；同时保留「带新脚本重建会话」的批式用法。
- 指令线格式统一为 `[actor, skill, packed]`：`skill` 用 overlay 公开编号（`255` 为普攻），`packed` 低字节是 MP 消耗、高字节是药水编号；解码、公开编号表与严格校验集中在 `runtime::control`，`battle_replay_forced` 与 `forced_hits`/`forced_taken` 作为兼容入口保留。
- 能力卡（`cards` feature）与强制行动子系统按 main 语义保留，`set_initial_hp`/`set_abilities`/`start_hp_pct` 与新会话 API 并存；蓄力必中判定同时接受 `charge.active` 与 3 倍 `at_boost`（蓄力三段）。

## 未发布 · 2026-11-11 强制行动、受控会话与能力卡

- `tswn_core`：新增卡牌强制行动与受控会话子系统（`BattleSession` 暂停/续行、`set_initial_hp`、`battle_replay` 指令重放），已进入 main；上层绑定按各自进度导出。
- `tswn_core` / `tswn_wasm`：能力卡子系统以可选 feature `cards` 带入（默认关闭），开启后 `set_abilities` 让卡牌减耗、独立乘区与幻术熟练度上限生效；来源 `KODU16/tswn-cards@929d0db`，遗物（relic）相关代码未随本批移植。
- 关闭该 feature 时相关字段、函数与调用点都不参与编译，默认构建的战斗规则、随机流与既有测试数量不变；开关方式见各 crate 的 README 与 CHANGELOG。

## 未发布 · 2026-10-30 日志补全

- 按各模块 manifest、提交历史与当前实现补全未发布条目；版本号与发布日期维持现状，旧版本记录当时验证的测试数量和性能结论。
- 主线新增 Boss/武器的分阶段兼容移植、Openbox 可选 CSV 列提取/行筛选、评分项全不选时不过滤及私有 GUI 模块可见性整理，以及此前维护中的绑定错误边界、依赖和数据工具修复；具体入口与限制见下列模块日志。
- 核心热路径、名字评分准备和 Rinick 冷快照优化已随 `perf/ffa-profile-20261027` 合并（merge commit `e0e7c671`）进入 main；细节仍记在 core 日志的分支专节。性能范围与已知失败见[简报](docs/perf/reports/performance-branch-2026-10-29.md)，提速幅度按对局而定。
- 分支诊断工具还包括可选 PGO 构建脚本与独立 `allocation-probe`（`publish = false`）；两者属于可选的诊断辅助，不随默认构建启用。本版本没有新的逐轮测量数据。

### 模块日志索引

版本列为当前 manifest，`Unreleased` 标题中的拟议下一版本属于计划版本，尚未发布。

| 模块 | 当前版本 | 日志 |
| --- | --- | --- |
| `tswn_core` | 0.7.2 | [更新日志](crates/tswn_core/CHANGELOG.md) |
| `tswn_capi` | 0.6.3 | [更新日志](crates/tswn_capi/CHANGELOG.md) |
| `tswn_py` | 0.5.4 | [更新日志](crates/tswn_py/CHANGELOG.md) |
| `tswn_wasm` | 0.5.13 | [更新日志](crates/tswn_wasm/CHANGELOG.md) |
| `tswn_openbox` | 0.5.4 | [更新日志](crates/tswn_openbox/CHANGELOG.md) |
| `tswn_openbox_backend` | 0.5.4 | [更新日志](crates/tswn_openbox_backend/CHANGELOG.md) |
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
