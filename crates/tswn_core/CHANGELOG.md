# 更新日志

## [0.3.10] - unreleased

### CLI

- `namer-pf` 新增 `--mode` 参数，支持按 pp/pd/qp/qd 选择评分项，可重复传入或逗号分隔，不传则默认运行全部四项。

### 修复

- 修复 `ol` overlay 中 `name_factor_enabled` 字段未被解析的问题：JSON 格式的 `ol:{...}` overlay 现在能正确识别 `name_factor_enabled` 开关。

### 重构

- `tswn_cli` 内部模块化大重构：将原先堆在单文件的 `bench.rs` / `fight.rs` / `args.rs` 按职责拆分为 `bench/`（batch / score / winrate / common / output）、`fight/`（driver / trace / raw_bench）和 `args/`（cli / input / parsed）子模块，降低后续维护 CLI 的耦合度。

## [0.3.9] - 2026-05-26

### 行为对齐修复

- 修复回合同步边界：对齐复活、召唤、关联召唤物清理和地震终止在回合内的执行顺序，减少 namer-pf 评分的 pp/pd/qp/qd 分叉。
- 修复玩家状态评分行为：对齐中毒挂载、负面状态清理、pre_defend 零伤害早停、吸血浮点边界和隐匿状态边界，整体减少与 md5.js 的评分差异。
- 修复解除中毒不刷新运行时属性：按 md5.js 的 `PoisonState.K()` 语义，中毒解除只移除状态和注销 `post_action`，不触发 `updateStates`，避免疾走等 pending 状态提前生效。
- 修复浮点常量边界：对齐 `clone` 和 `ice` 技能中浮点常量与 JS 实现的边界差异。
- 修复死亡召唤物护佑候选：已死亡召唤物不再出现在护佑候选列表中，与 md5.js 语义一致。
- 修复 DIY SlotBoost cap：DIY 构造路径中 `SlotBoost` 加成量增加 `boost.min(current)` 封顶，与普通构造行为保持一致。

### CLI

- 新增 `bench pair` 子命令：计算玩家与队友的组合胜率，输出最高的 `--head <N>` 个 batch rate 之和作为最终分数。
- `batch-rate` 新增 `--pure`（纯名字列表）、`--log`（JSONL）输出模式，并通过 `-o` 指定文件。
- `batch-rate` 新增 `--wr-precision <N>` 胜率精度控制。
- `batch-rate` 新增输入名字自动去重，避免重复计算和输出。
- `to-diy` 新增 `--old` 标志，可输出旧版 DIY 格式。
- 修复 `namer-pf` 对 `diy[...]` / `ol:{...}` 内联 overlay 格式的解析。
- 修复 `namer-pf` / `bench score` / `batch-rate` / `bench pair` 等批量评分路径的模板缓存膨胀：对单次消费的 prepare/Runner 构造改走 uncached 路径，避免 profile 名字持续变化时把临时模板堆进全局 `prebuilt_groups_cache`。

### WASM

- 发布 `tswn_wasm 0.2.8`：引入 `tsify` 生成 TypeScript 领域类型，wasm 导出接口从 `JsValue` 改为强类型签名。
- 调整 replay viewer 控制面板 UI，改善桌面和移动端浏览器操作体验。

### 工具

- 新增 `scripts/find_bun_tswn_namer_pf_mismatches.py`：批量对比 Bun 和 Rust 的 namer-pf 评分输出，按 pp/pd/qp/qd 四项分别定位差异。
- 新增 `scripts/bun_score_trace.js`：在 Bun 环境下按步骤输出评分中间结果，辅助定位评分分叉。
- `find_bun_tswn_namer_pf_mismatches.py` 新增重测模式、missing 处理和分段输出辅助。

### 文档

- `docs/analysis/clone_mechanism.md` 补充 SlotBoost cap（`boost.min(current)`）机制说明。

### 测试

- 为浮点边界修复和中毒解除修复补充回归测试。
- 新增 `large 72` 测试用例，覆盖本轮修复涉及的复杂对战场景。

## [0.3.8] - 2026-05-21

### DIY / Overlay

- 重构 DIY 技能存储结构：技能覆盖从按名称映射调整为保持顺序的 `Vec`，移除额外的 `skill_order` 字段，降低构造与导出路径中的查找和维护成本。
- 修复普通构造与 DIY 构造中 `LastBoost` / `SlotBoost` 的应用顺序，使末位翻倍、末位加成和基础等级覆盖的语义更稳定。
- 修复 DIY 分身技能重建时的重复加成问题，避免 clone 技能在衰减下限和 overlay 加成之间被二次放大。
- 优化 `to-diy` 导出：跳过 0 级技能并移除由此产生的多余逗号，输出更紧凑且更接近实际有效配置。
- 更新 DIY 文档，补充 ordered skill slots、clone 继承和验证状态相关说明。

### 性能优化

- 针对引擎热路径做了一轮较大规模优化，减少回合推进、玩家状态访问和技能槽处理中的额外开销。
- 跳过空的 `pre-step`、action 状态、defend / damage hook 路径，降低无效果回调在大量对局中的基础成本。
- 优化 DIY 技能槽去重与导出路径，减少重复技能槽和字符串构造带来的额外分配。
- 调整 release benchmark 配置，启用单 codegen unit，便于获得更稳定的性能采样结果。
- 降低 benchmark runner 自身开销，使固定样本性能数据更能反映核心引擎消耗。

### 跟踪与验证工具

- 新增 / 扩展 Rust 侧 track 工具，用于 case generation、DIY roundtrip、性能样本跟踪和固定样本对比。
- 固定性能样本从 20 个扩展到 30 个，并新增分组汇总，便于观察 1v1、多人混战、组队等不同类型对局的优化收益。
- 更新性能跟踪文档与固定样本结果，记录 `0.3.7` 基线及本轮优化目标。

### 兼容性说明

- 本次以 DIY 行为修复和性能优化为主，不刻意引入新的公开玩法语义。
- DIY / Overlay 内部结构有所调整，直接依赖 `PlayerOverlay.skills` 具体类型的调用方需要按新的 ordered skill slots 表达方式适配。

## [0.3.7] - 2026-05-18

### DIY / Overlay

- 新增 `SkillBoost` 枚举，用于更精确表达技能加成类型，支持普通等级、末位加成和末位翻倍三种语义。
- `PlayerOverlay.skills` 从单纯等级映射扩展为支持 `SkillBoost` 结构，DIY / Overlay 表达能力更完整。
- 新增 `name_factor_enabled` 开关，可按需禁用 `name_factor` 对属性的缩放影响。
- DIY 模式下，若显式提供属性或技能覆盖，武器效果将不再参与构造，避免混入非 DIY 因素。

### 格式与导出

- 扩展紧凑 DIY 格式与 `ol:` JSON 格式，支持技能值写法如 `5`、`40+30`、`2*46`。
- 新增 `Player::to_diy_compact()`，可将已构造玩家导出为紧凑 DIY 字符串。
- 新增 `Player::to_ol_json()`，可将玩家导出为 `ol:` JSON 格式。
- 新增 CLI 子命令 `tswn-cli to-diy <name>`，可直接把名字转换为 DIY / OL 输出。

### 修复

- 修复 DIY clone 重建时的技能衰减下限处理，重新应用 `SkillBoost` 规则，避免分身后的技能等级下限失真。
- 修复紧凑 DIY 格式中 HP 处理不一致的问题：仅前七围做 `+-36` 换算，HP 保持原值。
- 修复 `split_weapon_overlay` 对 JSON 字符串内 `+` 的误切分问题，避免如 `"40+30"` 这类技能值被错误解析。

### 文档

- 更新 `docs/DIY.md`，补充 SkillBoost、DIY clone、导出与 CLI 用法。
- 新增 `docs/analysis/skill_decay.md` 与 `docs/analysis/clone_mechanism.md`。

## [0.3.6] - 2026-05-18

### CLI

- 新增 `namer-pf` 命令：支持按行输入玩家组、组内用 `+` 分隔成员，输出 `pp|pd|qp|qd` 四项评分及总分，便于快速做名字评分对比。
- bench 相关评分路径补充了解析与执行辅助逻辑，方便复用同一套评分入口。

### 修复

- 修复玩家升级中的 `TestEx` 特殊路径，避免同队升级时出现额外污染。
- 补充 `Player::normal_raw_name_base`，使普通玩家构造时的底板语义更接近原始实现。
- 修复召唤物 / 分身复用时的运行时状态残留问题：复用对象时会重置状态并恢复 owner 标记。
- 调整 `post_defend` 状态清理流程：状态在命中清除条件后立即移除，并按需刷新状态，减少状态滞留带来的运行时偏差。
- 修复 clone 的 `name_factor` 继承计算，使分身构造时的属性缩放更接近当前对齐语义。
- 修复 raw score / bench score 路径中的构造方式，使评分对局统一走带 seed 与 `eval_rq` 的构造入口。
- 修正评分统计中的一处回退问题，最终保持赢家判定逻辑与既有语义一致。

### 行为调整

- raw score / bench score 的内部对局构造改为复用 split 后的 group + seed 初始化路径，减少和普通对战构造分叉。
- `post_defend` 的 skill / state 执行后状态更新更及时，状态链行为更稳定。

### 测试与文档

- 补充 `namer-pf` 输入解析测试。
- 更新多份分析文档，重写 `why_ns` 相关说明，补齐当前实现细节。

## [0.3.5] - 2026-05-18

### DIY / Overlay 系统增强

- **新增 `SkillBoost` 枚举**：支持三种技能加成类型以精确建模衰减下限。
  - `Normal(lv)` — 普通技能，无特殊加成
  - `SlotBoost { base, boost }` — 末尾座位加成（`base + boost`）
  - `LastBoost(base)` — 末尾主动技翻倍（`base × 2`）
- **`PlayerOverlay.skills` 类型变更**：从 `HashMap<String, u32>` 改为 `HashMap<String, SkillBoost>`。
- **新增 `name_factor_enabled` 字段**（默认 `true`）：设为 `false` 时强制 `name_factor = 0`，八围不缩放。
- **DIY 模式武器不计入**：当 overlay 包含 `attrs` 或 `skills` 时，`weapon_state` 强制为 `None`。
- **紧凑 DIY 格式兼容 JS**：前七围 `±36`，HP 不变（与 JS 仅对索引 0~6 做 `-= 36` 的行为一致）。

### 内联格式扩展

- 紧凑格式 `diy[...]` 新增 `SkillBoost` 字符串值支持：
  - `"sklfire":5` → `Normal(5)`
  - `"sklheal":"40+30"` → `SlotBoost { base: 40, boost: 30 }`
  - `"sklshadow":"2*46"` → `LastBoost(46)`
- JSON 对象格式 `ol:{...}` 同步支持上述 SkillBoost 格式，且前七围同样 `±36`。

### DIY 分身后 clone 衰减下限

- **`Skill` 新增 `diy_boost` 字段**：存储 `SkillBoost` 元数据，clone 重建时用于计算衰减下限。
- **`SkillStorage` 新增 `is_diy` 标记**：clone 重建时检测 owner 是否为 DIY 构建。
- **DIY clone 加成重新执行**：clamp 到 owner 当前等级后，对 `LastBoost` 执行翻倍（`level *= 2`）、对 `SlotBoost` 执行加 boost（`level += boost`），不依赖 name_base。
- **`build_for_clone` 增强**：当 owner 为 DIY 时，clone 也应用 overlay 技能配置。

### 名字 → DIY/OL 转换

- **新增 `Player::to_diy_compact()`**：将已 build 的玩家导出为紧凑 DIY 格式字符串（含 `@Team`）。
- **新增 `Player::to_ol_json()`**：将已 build 的玩家导出为 `ol:` JSON 格式字符串。
- **新增 `skill_name_for_export(id)`**：技能 ID → overlay 兼容名（如 `sklfire`）。
- **新增 CLI 子命令 `tswn-cli to-diy <name>`**：一键将任意名字转换为 DIY 和 OL 格式输出。

### 修复

- **`split_weapon_overlay` 修复**：新增 `split_by_plus_outside_quotes`，正确处理 JSON 字符串值内的 `+`（如 `"40+30"` 不再被误切分）。
- **紧凑格式 HP 处理修正**：`parse_diy` 和 `to_diy_compact` 仅对前七围 `±36`，HP 原样保留。

### 文档

- 更新 `docs/DIY.md`，补充 SkillBoost、clone 衰减下限、to-diy CLI 等完整文档。
- 新增 `docs/analysis/skill_decay.md` — 5 种衰减技能机制详解。
- 新增 `docs/analysis/clone_mechanism.md` — 分身机制与衰减下限分析。

## [0.3.4] - 2026-05-17

### 对齐

- `RunUpdate::new()` 默认 delay 改为 `delay0=1000`、`delay1=100`，对齐混淆版 `fast-namerena/branch/original/md5.js` 的 `RunUpdate` 构造行为。
- 补齐混淆版 `md5.js` 中的特殊 delay：
  - 反弹、护身符、召唤亡灵使用 `delay0=1500`
  - 雷击每段伤害 update 使用 `delay0=300`
  - 埼玉“觉得有点饿”使用 `delay1=2000`

### 说明

- 本次只同步战斗日志 / 播放节奏相关字段，不改变战斗判定顺序、目标选择或伤害结算语义。
- delay 行为以混淆版 `md5.js` 为准，未混淆 Dart 源仅作为辅助参考。

## [0.3.3] - 2026-05-17

### API

- `WorldState` 新增 `alive_group_count()` 访问器，暴露按 JS `Engine.y.a.Q` 语义维护的存活队伍计数。
- `Storage` 新增 `sync_alive_groups_owned_with_count(groups, alive_group_count)`，用于把 `WorldState` 中维护的 JS 兼容计数与 `alive_groups` 一起同步。
- `Storage::alive_group_count()` 语义调整为返回同步后的 JS 兼容计数，不再每次按 `alive_groups` 动态重算“当前非空队伍数”。

### 修复

- 修复多队伍战斗中默认攻击 / 技能智能选目标对存活队伍数的判断漂移：目标打分路径改为直接使用 `alive_group_count()`，不再临时扫描全体玩家去推导队伍数。
- `FireSkill` 初次附加 `FireState` 时改用 `set_state_no_update()`，避免额外状态刷新导致的运行时分叉。
- `EngineCore` / `Runner` 在同步 `alive_groups` 时显式携带 `WorldState` 维护的 `alive_group_count`，避免 `Storage` 侧重新按当前列表计数而偏离 JS 语义。
- `Storage` 内部将 `alive_group_count` 收敛为独立存储字段（当前实现为 `AtomicUsize`），保持 `&self` 读取简单的同时保留同步后的计数语义。

### 工具

- `tswn_case_miner` 的 TS trace 缓存签名现在同时包含 `out_md5.ts`、同目录 `md5.js` 和 `assets/**`；修改 JS 依赖或资源后会自动失效旧缓存。
- `tswn_case_miner` 默认共享缓存目录从 `<repo>/target/tswn_case_miner_cache` 调整为 `<repo>/tests/tswn_case_miner_cache`；`TSWN_CASE_MINER_TS_CACHE_DIR` / `TSWN_CASE_MINER_BUN_CACHE_DIR` 覆盖方式保持不变。

### 测试

- 新增大样本回放回归 `case 66`、`case 67`、`case 68`，覆盖 clone 熟练度 clamp、多队伍目标打分 / 复活 / 召唤链路等近期分叉样例。
- 为 `tswn_case_miner` 新增 `md5_tool_signature_tracks_md5_js_dependency()`，确认 `md5.js` 修改会触发缓存签名变化。

## [0.3.1] - 2026-05-07

### API

- `SkillTrait` / `Skill` 新增 `charge_step()` 方法，返回 `i32`，让外部能查询 ChargeSkill 当前的蓄力 step 数值。默认返回 `0`，仅 `ChargeSkill` 覆写。
- `SkillTrait` / `Skill` 新增 `assassinate_target()` 方法，返回 `Option<PlrId>`，让外部能查询 AssassinateSkill 当前锁定的潜行目标 ID。默认返回 `None`，仅 `AssassinateSkill` 覆写。

## [0.3.0] - 2026-05-07

### ⚠️ Breaking Changes

- **`PlayerStatus.mp` 字段重命名为 `magic_point`**：直接访问 `status.mp` 的代码需要改为 `status.magic_point`。
- **`Player::mp()` / `set_mp()` 方法已废弃**：请使用 `magic_point()` / `set_magic_point()`。废弃方法仍可通过 `#[allow(deprecated)]` 临时使用，但将在后续版本移除。
- **`Display` 输出格式变更**：`mp|` 改为 `magic_point|`，如有 parse Display 输出依赖需要更新。

### 变更

- `PlayerStatus` 结构体字段 `mp: i32` → `magic_point: i32`
- `Default` 实现同步更新为 `magic_point: 0`
- 所有技能实现（Charge、Disperse、Merge）中的 `.mp()` / `.set_mp()` 调用改为 `.magic_point()` / `.set_magic_point()`
- 调试日志中的 `self.status.mp` 访问改为 `self.status.magic_point`
- `engine_core.rs` 调试日志格式修正：`mp=` 标签原实际输出的是 `move_point`，已改为 `mv=` 消除误导

### 重构

- 废弃方法 `mp()` / `set_mp()` 添加 `#[deprecated]` 注解，引导用户使用新命名
- 新方法 `magic_point()` / `set_magic_point()` 作为正式 API

## [0.2.22] - 2026-05-02

### 新增

- **CLI**: `bench` 和 `bench win-rate` 子命令新增 `--buckets-step N` 参数，支持按指定步长分段输出累积胜率，方便观察胜率随场次增加的收敛趋势。
  - `bench rs`：多组对决时每 N 场输出一次累积胜率
  - `bench win-rate`：两队对决时每 N 场输出一次累积胜率
  - 分段模式下强制单线程以保证输出顺序正确

### API

- `SkillTrait` / `Skill` 新增 `protect_to_id()` 方法，返回 `Option<PlrId>`，让外部能查询 ProtectSkill 当前保护的目标角色 ID。默认返回 `None`，仅 `ProtectSkill` 覆写。
  - 配套 `clear_protect_to()` 的反向查询，用于 WASM 层状态标签展示

### 内部

- `win_rate::run_prepared_win_rate_range` 改为 `pub`，供 `bench` 模块的分段胜率逻辑复用。
- 修复 clippy 警告：`assassinate.rs` collapsible_if、`merge.rs` unused_enumerate_index

## [0.2.21] - 2026-05-01

### 新增

- 为 `win_rate` 模块新增 wasm 平台兼容层：
  - `platform_default_win_rate_workers()`：wasm 下固定返回 1（浏览器不暴露 `std::thread`），非 wasm 平台继续使用 `available_parallelism` 自动检测。
  - `platform_limit_win_rate_workers()`：wasm 下强制钳制为 1，非 wasm 平台仅做 `max(1)` 保底。
  - `resolve_win_rate_workers()` 改为调用上述平台函数，确保 wasm 目标下胜率路径保持单线程。
- `player::impl_runtime`: 暴露 `skill_storage()` 公开方法，返回 `&SkillStorage`，允许外部遍历玩家技能存储（如 WASM 层的状态标签收集）
- `player::skill::SkillTrait`: 新增 `dynamic_update_state_enabled()` trait 方法，默认返回 `false`，供"一段时间内持续生效"的技能标识其短时运行时态
- `player::skill::Skill`: 新增 `dynamic_update_state_enabled()` 方法，结合 `level > 0` 判断技能是否激活
- `player::skill::act::AccumulateSkill`: 实现 `dynamic_update_state_enabled()`，当技能配置了 `on_update_state` 时返回 `true`
- `player::skill::skl::HideSkill`: 实现 `dynamic_update_state_enabled()`，当技能配置了 `on_update_state` 时返回 `true`

### 修复

- `bench.rs`：将 `Option::map_or` 替换为 `is_none_or`（clippy lint）。

### 技术细节

- `skill_storage()` 使用 `#[inline]` 标注，零开销暴露内部技能存储引用
- `dynamic_update_state_enabled()` 在 `Skill` 层级已有 `level > 0` 守卫，确保未学习技能不会误判为启用状态
- `AccumulateSkill` 和 `HideSkill` 的 `dynamic_update_state_enabled()` 受 `on_update_state.is_some()` 约束，仅在配置了短时持续效果时启用

## [0.2.20] - 2026-04-22

### 修复

- 修复 UTF-8 BOM (`U+FEFF`) 导致的首行解析异常，覆盖三条输入路径：
  - `tswn_case_miner`：`load_library()` 与 `read_first_line()` 在读取后 strip BOM 前缀。
  - `tswn-cli`：`read_stdin()` 与 `read_file()` 同理处理 BOM。
  - `tswn_ds3`：`Config::load_from_path()` 在解析 JSON/YAML 前 strip BOM。
  - 三者分别覆盖了号库输入、CLI 原始 namerena 输入、以及 DS3 配置文件的入口。

### 性能

- 继续回收 `0.2.19` 在 `bench win-rate` 路径上的性能回退：
  - 优化 `SkillStorage` 的 dynamic `pre_action` 热路径，去掉 `pre_action()` 内部 clone，减少重复 lookup / contains，并把 membership 独立成哈希集，降低动态启停时的线性检查成本。
  - 优化 `Hide` 的 post_damage 热路径，改用借引用队伍视图、`pending_spawn` / `pending_revival` iterator 和 `SmallVec` 聚合，减少 group clone 与中间 `Vec` 分配。
  - 在 `Storage` 新增 `alive_group_len_containing(...)`，并将 `Assassinate` / `Berserk` / `Charm` / `Curse` / `Disperse` / `Exchange` / `Half` / `Slow` 的目标打分从 `alive_group_containing(...).len()` 线性扫描改成基于 team index 的 O(1) 查询。
  - 在 `pick_smart_target()` / `pick_smart_target_with_level()`（`skill.rs` trait）和 `pick_targets()` / `pick_forced_attack_target()`（`impl_runtime.rs`）中增加 `selected.len() == 1` 提前返回，跳过完整的 scoring + ranking 流水线，降低单候选时的函数调用与 Vec 分配开销。
  - 在 `tick.rs` 的 `select_targets()` 中将 `ally_dead` 构造从 `!ally_alive.contains(id)` 改为 `!is_alive_now(id)`，消除 `contains` 对 `ally_alive` 的线性扫描。
- 按 `docs/performance.md` 的固定口径重跑当前工作树（`--release --features no_debug`，`tswn-cli bench win-rate ... --perf`）：
  - `aaa` vs `bbb`：`100k` 单线程 `1.887s`，多线程 `0.291s`；`1M` 单线程 `18.438s`，多线程 `2.784s`
  - `喘际瞬爆@昀澤` vs `蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall`：`100k` 单线程 `2.891s`，多线程 `0.471s`；`1M` 单线程 `28.488s`，多线程 `3.749s`
- 相比 `0.2.14` 基线，当前回退已缩小到：
  - 单线程慢约 `1.6%~2.3%`
  - 多线程慢约 `7.9%~10.7%`

### 文档

- `docs/build_all.md`：全面迁移至 `uv run` 驱动的构建流程，新增 WSL/Linux 纯环境构建指南、uv 用户的 WSL 增强方案、快捷命令清单与版本对照表。
- `scripts/README.md`：统一将命令示例从 `python scripts/...` 更新为 `uv run scripts/...`，补充 uv 环境说明。

### 验证

- `cargo test -p tswn_core`
- `python .\track_case_miner.py --library .\tests\sqp5900.txt --modes 3v3v3 --case-offset-per-mode 0 --max-cases-per-mode 4000 --keep-going`
- 上述回归口径下，`failed case = 0`

## [0.2.19] - 2026-04-22

### 修复

- 调整 runtime sync 顺序，使复活先于同批 spawn / death 落地；同时补上 `pending_revival_ids_for_group(...)`，让 `Hide` 在同一 action 后半段能看到“刚复活但尚未 sync”的旧队友，但不会把“已经死亡、只是暂时还残留在 alive_group 里的队友”误算进去。
- 对齐 `Assassinate` / `Hide` 的 pre_action 语义：新增 forced-skill 累积接口、动态 pre_action 注册/撤销、`allows_empty_targets()` 与 `uses_attack_aa_sampling()`，修正“skill 内部已锁定目标但外部 targets 为空”这类 JS 常见路径下的 RC4 与出手行为。
- 修正 `merge` / `summon` / `minion` 的固定槽位与运行时视图：`SkillStorage` 现在分离 `slot_skill` 与 `skill`，`merge` 改为按固定槽位继承等级，summon 保留稳定 `k1=[fire, fire, explode]` 语义；shadow / summon / zombie 生成体统一把 `name_factor` 归零，对齐 JS combat minion。
- 修正一组近期定位到的多人对局分叉，包括 revive/spawn 可见性、Hide 在 post_damage 链里的活人计数、Assassinate 的 pending target 行为，以及相关的 merge / minion 继承细节；按 `tests\sqp5900.txt`、`3v3v3`、`max-cases-per-mode=4000` 口径复跑后，`diff_failures = 0`。

### 测试

- 新增 / 扩充回归测试，覆盖：
  - revive 先于 spawn 的 sync 顺序
  - `d8c6` 开场 trace 对齐
  - `Hide` 对 pending revival / stale alive_group 的可见性
  - `Assassinate` 与 `Hide` 的 pre_action / forced-skill 交互
  - `merge` 的固定槽位继承与 `Fengshen` pre_action 注册
  - combat minion 的 `name_factor=0`

### 性能

- 按 `docs/performance.md` 的口径补跑了当前工作树（`--release --features no_debug`，`tswn-cli bench win-rate ... --perf`）：
  - `aaa` vs `bbb`：`100k` 单线程 `1.978s`，多线程 `0.309s`；`1M` 单线程 `19.246s`，多线程 `2.743s`
  - `喘际瞬爆@昀澤` vs `蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall`：`100k` 单线程 `2.895s`，多线程 `0.394s`；`1M` 单线程 `28.692s`，多线程 `3.900s`
- 相比 `docs/performance.md` 记录的 `0.2.14` 基线，这一轮 8 个口径都偏慢；目前结果一致性已经收口，但性能仍值得继续排查。

## [0.2.18] - 2026-04-21

### CLI

- 新增 `tswn-cli diff` 子命令：接受普通对战 raw 输入，并按 runner diff 的逐行格式输出战斗过程，便于直接和现有 diff runner、外部脚本或历史基准样例做文本对账。
- CLI 顶层示例与 banner 分流同步补上 `diff` 模式；`diff` 路径默认跳过启动 banner，避免污染需要逐行比较的输出结果。

### 修复

- 修正 `fight` / raw 输出里 `win_idx=...` 的赢家输入索引映射：现在直接基于 runner 保留的 `input_groups` 还原原始输入顺序，不再通过重复 split 原始文本重新推导，避免个别样例里胜者索引和原始输入位置脱节。
- 调整 `SlowState` 的属性结算优先级，使其与 `HasteState` 处在同一优先级层，再继续依赖状态注册顺序打破平局。这样可以同时对齐 JS 的两类速度状态链：
  - 先 `slow` 后 `haste`
  - 先 `haste` 后 `slow`
- 上述调整修复了一组 2v2 / 3v3v3 对局中由速度状态结算顺序漂移引起的 runner diff 分叉与胜率偏差。

### 测试

- 为 `win_idx` 输出补充回归测试，确认赢家输入索引继续对应原始输入顺序。
- 将 runner 大样本回归文件按编号拆分为 `large_56_61` 与 `large_62_65`，便于继续扩充 case 而不让单文件持续膨胀。
- 新增 case 62 至 65 回归测试，覆盖近期对账过程中定位到的 diff_case 04 / 07 / 09 / 11，并保留 case 56 至 61 的原有断言不变。

## [0.2.17] - 2026-04-16

### CLI

- 改进 `tswn-cli bench batch-rate` 的帮助信息，明确 `bench cqp` 与 `bench batch-rate` 是同一个命令的两个名字，并补充别名示例与 `--min-wr/-m` 参数说明。
- `bench batch-rate` 新增 `--min-wr/-m <N>`，允许以 `0..10000` 的万分比阈值过滤终端显示结果；写入 `--out-file` 的 JSONL 结果不受该参数影响。
- `bench batch-rate` 新增批量进度条：按 `选手组 × 靶子组` 的对局数显示总进度，并同时给出按全量平均速度估算的总体剩余时间和按最近 5 个选手速度估算的滑动剩余时间。
- `tswn-cli` 的 clap 顶层 help 现在显式带上 crate 版本号，便于直接确认当前 CLI 二进制版本。

### 性能

- 统一当前 benchmark 的串并行切换阈值到 `n >= 100`：`prepared_win_rate` 不再固定等到 `n >= 2000` 才并行；CLI 评分 benchmark 也同步从 `2000` 下调到 `100`。这让 `bench win-rate`、`bench group-win-rate`、`bench batch-rate`、单组 `bench auto`、raw `!test!` 的评分/胜率路径，以及 `tswn_capi` / `tswn_py` 的 prepared win-rate 在中小样本批次下都能更早利用并行执行。

### 修复

- `bench batch-rate` 的进度条刷新和完成提示现在只在交互式 stderr 终端下启用，避免输出被重定向到文件、管道或 CI 日志时混入 ANSI 清行控制符。

## [0.2.16] - 2026-04-10

### 修复

- 修正名字解析中的 trim/reject 语义混用：不再把 `md5.js` 的 trim 字符集合直接当作 `name/team` 的非法字符集合处理，避免像 `U+3000` 这类 JS 可保留/可裁剪空白在 Rust 里被误判为构造失败。
- 对齐 `md5.js` 的最小相关解析语义：raw 输入现在会按 JS `\s` 规则裁掉每行尾部空白，`+weapon` 后半段会按 `trim_name` 风格去掉两端可裁剪空白与 `133` 扩展边界空白。
- 修正旧 `filter_char` 范围翻译中的边界偏差，不再继续依赖 `9..12` / `8192..8202` 这类 Rust 半开区间写法去近似 JS 的显式码点集合。

### 测试

- 新增回归测试，覆盖：
  - `U+3000` 在 `name/team` 内部时不再误报非法字符
  - raw 行尾 `U+3000` 会在分组前被裁掉
  - `+weapon` 后半段会按 JS 语义裁掉首尾 `U+3000` / `133` 等 trim 字符

### 工程

- 清理当前 workspace 的 clippy warning：`tswn_core` 侧收敛了 `collapsible_if`、`unused_enumerate_index`、测试中的 `needless_update` 等告警；`tswn_ds3` 侧修正了低风险样式问题，并对当前保留的生成系数/未接线辅助接口加上局部 `allow`，使 `cargo clippy --workspace --all-targets` 恢复无 warning 通过。

## [0.2.15] - 2026-04-09

### CLI

- 新增 `tswn-cli bench batch-rate`（别名 `cqp`），支持从靶子列表和选手列表文件批量计算平均胜率；文件中每行一组、组内使用 `+` 分隔，并可通过 `--verbose` 输出逐靶子明细。
- `bench batch-rate` 在输出时会保留选手列表文件中的原始行文本作为标签，默认输出每个选手组的平均胜率与整体吞吐信息，便于直接做批量对比。
- `bench batch-rate` 新增 `--out-file/-o` 与 `--force/-f`，支持将结果按 JSONL 写入文件；每个选手组一行结果，逐行 flush，便于长批次运行时边产出边消费。
- 为常用 CLI 参数补齐短版别名：`--raw/-r`、`--file/-f`、`--single-thread/-s`、`--target/-l`、`--target-list/-l`、`--player-list/-p`，减少日常命令输入长度。

### 修复

- 修正 `bench batch-rate` 的 `--perf` / `--verbose` 参数透传顺序，避免执行阶段把性能统计和明细输出开关对调。

### 文档

- 将本轮新增批量胜率命令相关注释与帮助文案统一改为中文，并补齐内部参数说明，降低后续维护和继续扩展 CLI 时的阅读成本。

## [0.2.14] - 2026-04-08

### 性能优化

- 回迁 `PlayerStateStore` 的单表存储优化：将 `states + state_orders` 双 `HashMap` 合并为单个 `entries` 表，减少状态排序与遍历时的额外查询开销。
- 优化 `run_to_completion()`：
  - 复用单个 `RunUpdates`
  - 使用 `new_no_capture()` 关闭 benchmark 高速路径中的详细帧缓存
  - 保留 `had_updates()` 语义，避免高速路径与普通对局的空回合判定分叉
- 回迁 `Storage` 的 `player_group` 反向索引，并将 `has_alive_enemy_or_pending()` / `is_battle_over()` 改为直接迭代，消除热路径里的 `Vec` 分配与队伍线性扫描。

### 修复

- 修复复用 `RunUpdates` 时的批次 `id` 复用问题：`reset()` 现在会刷新新的批次 `id`，避免 `CounterSkill` 等依赖批次标识的逻辑在 `run_to_completion()` / `bench win-rate` 路径上产生结果分叉。
- 补充最小回归测试，覆盖“复用同一个 `RunUpdates` 时，下一批次仍应允许重新触发反击判定”的场景。

## [0.2.13] - 2026-04-07

### 重构

- 将 prepared 胜率统计的公共逻辑下沉到 `tswn_core::win_rate`，统一复用：
  - `thread=0/1/n` 线程参数语义
  - 自动线程数策略
  - JS profile seed 调度
  - `PreparedRunner` 多线程 worker 分发
  - 第一组胜负判定与 `wins/total` 汇总
  - `init/fight` timing 统计
- `tswn-cli` 的 `bench win-rate`、`bench group-win-rate` 和 `raw !test!` 胜率路径改为统一调用 `tswn_core::win_rate`，减少 CLI/C-API/Python 三侧实现漂移风险。

### 修复

- 修复 `tswn-cli` 的 prepared win-rate benchmark seed 调度偏移：在默认 JS profile seed 语义下，首局仍保持无 seed，但后续局数现在会按 `seed:33554431@! + i` 递增，不再错误地整体前移一位。
- 同步修正单线程与并行 worker 两条 benchmark 路径，避免 `bench win-rate` 在不同线程模式下继续复用同一处 off-by-one seed 调度错误。

## [0.2.12] - 2026-04-06

### CLI

- **新增 `raw` 子命令**：`tswn-cli` 现在提供独立的 `raw` 入口，用于直接运行原始 namerena 输入，不再必须通过 `fight --out-raw` 触发 raw 聚合战斗日志输出。
- **`raw` 支持评分相关参数**：`raw` 子命令新增 `-n, --count` 与 `-t, --thread` 选项，用于控制评分/胜率测试的对局数量与 benchmark 线程数；默认对局数统一提升为 `10000`，并与其他 `bench` 子命令保持一致。
- **`raw` 按 `!test!` 头自动分流**：当输入不以 `!test!` 开头时，`raw` 会直接按原始对战模式输出 raw 聚合战斗日志；当输入以 `!test!` 开头时，则切换到 benchmark 语义。
- **`raw` 的 benchmark 分流规则补齐**：`!test!` 模式下，去掉头部后的有效输入若只有 1 组，则运行评分；若恰好有 2 组，则运行胜率；若达到 3 组及以上，则直接报错，避免继续按不明确语义执行。
- **`raw` 的 `!test!` 识别兼容前导空行**：即使输入最前面存在空行，只要去除首尾空白后以 `!test!` 开头，仍会正确进入 benchmark 分支，保持与实际 raw 文本使用习惯一致。
- **补充 `raw` 路由单测**：为 `!test!` 头识别、去头后 body 保留、前导空行、BOM 前缀和非 `!test!` 输入等情况补充单元测试，降低后续继续调整 CLI 分流逻辑时的回归风险。

## [0.2.11] - 2026-04-06

### 新增

- **补充 `prepare` / `raw` 一致性测试**：新增覆盖多模式、多 case 的 runner 一致性验证，直接比较 `Runner::new_from_namerena_raw(...)` 与 `prepare_groups_with_eval_rq(...) + new_from_prepared_with_seed(...)` 两条路径在相同输入、相同 seed 下的行为是否一致。
- **一致性测试覆盖常见对战模式**：新增测试覆盖：
  - `1v1`
  - `2v2`
  - `3v3v3`
  - `ffa_4`
  - `ffa_6`
  - `ffa_8`
  并为每种模式固定生成 10 个 deterministic case。
- **一致性测试比较维度扩展到完整 replay**：除 `winner` 外，还同时比较：
  - `input_groups`
  - battle score
  - full replay trace
  以便尽早发现 `prepare` 路径在排序、构造顺序、初始状态或随机流上的潜在分叉。
- **补充无 seed 情况下的 prepare/raw 一致性测试**：新增单测，直接比较“raw 输入不带 seed”与“PreparedRunner 构造 runner 时不传 seed”两条路径在同一输入下的：
  - `winner`
  - battle score
  - round count
  - replay trace
  以确认 no-seed 语义同样保持一致。

### 修复

- **清理 `tswn_core` 的 clippy 警告**：修正 `map_entry`、`if_same_then_else`、`collapsible_if`、`explicit_auto_deref`、`excessive_precision` 等问题，并对个别稳定辅助函数补充局部 `#[allow(clippy::too_many_arguments)]`，使 `tswn_core` 在严格 `clippy -D warnings` 下恢复通过。
- **清理 `tswn_capi` / `tswn_py` 的 clippy 警告**：补齐 `tswn_capi` 导出 `unsafe extern "C"` 接口的中文 `# Safety` 文档，整理 raw pointer 访问写法，并修正 `tswn_py` 中的冗余闭包等低风险 lint，确保 `core/capi/py`（排除 `ds3`）整体通过严格 clippy 检查。

### 调整

- **`tswn_capi` / `tswn_py` 版本步进**：
  - `tswn_capi` 从 `0.1.0` 升至 `0.1.1`
  - `tswn_py` 从 `0.1.8` 升至 `0.1.9`
- **补充调用方按输入队伍归属判断胜者的辅助接口**：
  - `tswn_capi` 新增 `tswn_runner_player_input_group_index(...)`
  - `tswn_py.Runner` 新增 `player_input_group_index(player_id)`
  便于直接按原始输入 `group_index` 判断玩家归属，而不必调用方自行扫描 `input_groups` roster 构造映射。
- **`tswn_capi` 现在同时产出 `cdylib` 与 `staticlib`**：Windows 打包结果中除 `tswn_capi.dll` / `tswn_capi.dll.lib` 外，还会包含 `tswn_capi.lib`，便于 C/C++ 调用方按需选择动态链接或静态链接。
- **`tswn_capi` / `tswn_py` 默认依赖 `tswn_core/no_debug`**：减少外部分发产物在默认构建路径中的调试噪音，也使 `capi` / `py` 与当前 release 打包配置更一致。

### 验证

- `cargo test -p tswn_core prepared_runner_matches_raw_runner_across_modes_and_cases -- --nocapture`
- `cargo test -p tswn_core no_seed_runner_and_prepared_runner_match -- --nocapture`
- `cargo +nightly fmt --package tswn_core --package tswn_capi --package tswn_py`
- `cargo clippy -p tswn_core -p tswn_capi -p tswn_py --all-targets -- -D warnings`

### 分发与调用说明补充

- **补充 Windows C++/CAPI 使用文档**：新增 `docs/howto/capi_cpp_windows.md`，记录：
  - `clang++` 动态链接 `tswn_capi.dll.lib` 的推荐命令
  - `clang++` 静态链接 `tswn_capi.lib` 时需要额外补 `ntdll.lib`
  - `g++` / MinGW 使用 `.dll.lib` 可能遇到的兼容性问题
  - `PreparedRunner` 的 seed 参数必须传完整 `seed:...` 行，而不是裸 seed 值
- **修正并补充 `tswn_capi/examples` 注释与说明**：
  - `examples/prepared_runner.c` 现在显式说明 `tswn_runner_new_from_prepared()` 应传完整 `seed:...` 字符串
  - `examples/README.md` 现在补充了 Windows `staticlib` 链接注意事项，以及 `PreparedRunner` 的 seed 格式要求

## [0.2.10] - 2026-04-05

### CLI

- **CLI 参数解析迁移到 `clap`**：`tswn-cli` 不再维护超长的手写参数分支，改为基于 `clap` 的结构化参数解析，help 文案也切换为按子命令原生生成，降低后续继续扩展命令时的维护成本。
- **CLI 改为子命令结构**：顶层收敛为 `fight` / `bench` / `icon`，并进一步细分为 `bench auto`、`bench win-rate`、`bench group-win-rate` 以及 `icon show`、`icon b64`、`icon save`，替换旧的平铺 `--bench*` / `--win_rate*` / `--icon*` 入口。
- **`fight` 结果追加赢家输入索引**：普通输出和 `--out-raw` 输出在战斗结束后都会追加 `win_idx=...`，用于直接标识胜者对应的原始输入顺序；普通模式下该行放在 `总战斗分` 之后，作为最终输出尾行。
- **新增 `bench group-win-rate`**：支持 `--target <组>` 搭配可重复的 `-a, --against <组>` 输入，对目标组分别计算多组胜率并输出平均胜率，且每个 `against` 都支持单人或多行组输入，方便批量评估固定目标组。
- **`bench` 统一支持 `--perf` 计时开关**：原先独立的性能测试能力并入 benchmark 路径，`bench auto`、`bench win-rate` 和 `bench group-win-rate` 现在都可以通过 `--perf` 输出 `total/init/fight` 耗时拆分，同时保留原有结果统计。
- **benchmark 线程配置改为显式参数**：移除 `TSWN_BENCH_WORKERS` / `TSWN_WINRATE_WORKERS` 环境变量入口，统一改为 `-t, --thread <N>` 和 `--single-thread`，避免隐藏环境状态影响结果复现。
- **CLI 内部模块化拆分**：将原本超长的 `tswn_cli.rs` 按职责拆成 `args` / `bench` / `fight` / `icon` 模块，主入口只保留 banner 和命令分发，降低继续维护 benchmark 与图标逻辑时的耦合。
- **`bench win-rate` 结果重新对齐 JS 基准**：修复 benchmark 将排序后队伍误判成输入第一队的问题，并让 `run_to_completion()` 复用正常 `main_round()` 推进逻辑，消除高速路径与普通 fight 的胜者分叉；默认 `bench win-rate` 同时改为复刻 JS `ProfileWinChance` 的 seed 调度（首局无 seed，后续从 `seed:33554431@!` 开始），使默认模式重新对齐 `md5.js::win_rate`，而 `--keep-rq` 继续保持普通 `fight + seed:i@!` 语义用于对照。

### 分发与打包

- **新增 `tswn_capi` 分发脚本**：增加 `scripts/build_capi.py`，可一键构建 `tswn_capi` 并整理分发目录，统一收集 `include/`、`lib/`、`examples/`、`README.txt` 与 `MANIFEST.txt`，方便直接对外提供 C-API 包。
- **新增聚合打包脚本**：增加 `scripts/build_all.py`，支持一次性构建并打包 `capi` / `cli` / 现有 `tswn_py` 产物，默认生成带版本信息的 bundle 名与 zip 名，例如 `tswn_core_0_2_10_capi_0_1_0_py_0_1_8_bundle.zip`。
- **CLI 分发文件名带版本号**：聚合打包时，`tswn-cli` 会整理为带 `tswn_core` 版本号的可执行文件名（如 `tswn-cli_alpha_0_2_10.exe`），降低外部分发时多版本覆盖和混淆的风险。
- **Python 分发补充示例源码**：聚合打包结果中的 `py/` 目录除了已有 wheel 外，还会一并收集 `crates/tswn_py/examples/`，方便直接参考 Python 调用方式。
- **补充分发文档与脚本说明**：完善 `scripts/README.md`，补充 `build_all.py` / `build_capi.py` / `build_py.py` 的用途、典型命令与输出位置说明；bundle 顶层 `README.txt` 也同步加入 `tswn_core` / `tswn_capi` / `tswn_py` 版本概览和主要产物说明。
- **聚合包额外收集现有 Linux CLI / C-API 产物**：在仓库中已经存在 Linux 构建结果时，`build_all.py` 现在会把 `target/release/tswn-cli` 与 `target/release/libtswn_capi.so` 一并带入最终 bundle，使同一个分发包可以同时包含 Windows 的 `.exe` / `.dll` 与现有 Linux 的 CLI / `.so`。
- **C-API README 补充跨编译器示例**：`tswn_capi` 的分发 README 与 `examples/README.md` 新增了 MSVC / clang / gcc 的示例编译参数说明，并补充 Windows 下动态库查找与运行方式说明；其中 MSVC 与 clang 的 Windows 编译方式已在当前打包结果上实测验证。

### 性能

- **`no_debug` 下的 `win-rate` 速度补充验证**：
  - `aaa` vs `bbb`、`100000` 场：Rust 单线程约 `2.749s`（`36379 场/s`），默认多线程约 `0.493s`（`202824 场/s`，相对单线程约 `5.57x`）；同样输入下 Bun `md5.js::win_rate` 约 `12.674s`（`7890 场/s`），Rust 单线程约快 `4.61x`，默认多线程约快 `25.71x`。
  - `喘际瞬爆@昀澤` vs `蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall`、`100000` 场：Rust 单线程约 `4.247s`（`23545 场/s`），默认多线程约 `0.843s`（`118609 场/s`，相对单线程约 `5.04x`）；同样输入下 Bun `md5.js::win_rate` 约 `15.524s`（`6442 场/s`），Rust 单线程约快 `3.66x`，默认多线程约快 `18.41x`。两组样例中默认自动线程数都比强制 `-t 16` 更快，说明当前默认线程策略比直接拉满 `16` 线程更适合这类 `win-rate` workload。

### 清理

- **移除遗留的 `haste/iron` 调试输出并清空 `no_debug` 构建 warning**：删除 `crates/tswn_core/src/player/skill/act/haste.rs` 中只用于排查的 `haste_post_action` 调试输出，并同步整理 `crates/tswn_core/src/player/skill/act/iron.rs` 的未使用参数；`cargo build -p tswn_core --bin tswn-cli --release --features no_debug` 现已恢复到无 warning 通过。

### 验证

- `cargo check -p tswn_core --bin tswn-cli`
- `cargo build -p tswn_core --bin tswn-cli --release --features no_debug`
- `python scripts/build_capi.py --release`
- `python scripts/build_all.py --release --clean`
- 在 WSL 下执行 `cargo build -p tswn_capi --release` 与 `cargo build -p tswn_core --bin tswn-cli --release --features no_debug`，并确认 `target/release/libtswn_capi.so` 与 `target/release/tswn-cli` 能被聚合包一并收集
- `start-vs-pwsh.ps1` + `cl /nologo /Iinclude examples\version_and_error.c /link /OUT:examples\version_and_error.exe lib\tswn_capi.dll.lib`
- `clang -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`

## [0.2.9] - 2026-04-05

### 修复

- **修复 Protect 防御链提前刷新疾走倍率**：`ProtectState` 相关的 `pre_defend` 清态路径不再无条件 `update_states()`，改为只有在被清除状态确实影响属性时才重算。这样可以避免把 `HasteState` 的延迟倍率提前刷进 `speed`，修复 `3v3v3` 大样本中由此引发的行动顺序漂移、默认攻击目标偏移和后续 RC4 连锁分歧。
- **Reraise 继续保持 JS 语义**：`Reraise` 仍然只在当前死亡链中直接回 HP，不额外排一次 revival/sync；补充中文注释说明这个选择已经过全量 `1w2 case` 验证，避免后续误改回错误同步路径。

### 调试与排查

- **补充定向调试探针**：保留了 `raw update`、`tick_end`、`update_states`、`haste/iron/assassinate/default attack` 等窗口探针，便于继续对照 `md5.js` 排查随机流、状态刷新和选技链路差异。所有探针默认受环境变量控制，不影响正常运行。
- **补充 md5.js 对照注释**：在 `fast-namerena/branch/latest/md5.js` 中补充了 `Reraise`、`Haste`、默认攻击等关键逻辑的说明，降低下一轮继续对账时的阅读成本。

### 验证

- `cargo +nightly fmt --package tswn_core`
- `cargo test -p tswn_core --lib`
- `python .\track_case_miner.py -q --modes 1v1,2v2,3v3v3,ffa --ffa-sizes 4,6,8 --case-offset-per-mode 0 --max-cases-per-mode 2000 --keep-going`
- 固定大样本结果：`diff_failures = 0`、`ts_failures = 0`、`rust_failures = 0`

## [0.2.8] - 2026-04-05

### 调试与排查

- **case miner 支持稳定分段排查**：`tswn_case_miner` 与 `track_case_miner.py` 新增 `--case-offset-per-mode`，允许按每种模式的稳定生成顺序跳过前 N 个唯一 case，直接检查后续区间，避免前面已经修过的样本反复重跑，缩短大样本排查反馈周期。
- **TS 缓存命中信息显式可见**：case miner 现在会统计并输出 `TS cache hit`、`TS cache miss` 与 `bun` 调用次数，同时把这些统计写入 `summary.json`，便于区分“命中共享 trace 缓存”和“真的重新调用了 bun”。
- **tracker 非 quiet 模式改为流式显示阶段信息**：`track_case_miner.py` 在非 `-q` 下不再吞掉 `cargo run` 输出，会直接显示 Rust 编译日志以及 miner 的阶段提示，降低“卡在编译/缓存/比对哪一步”不透明的问题。
- **比较范围变化时不再误判回归**：tracker 现在会把 `case_offset_per_mode`、`max_cases_per_mode`、`modes` 等关键配置纳入比较范围检查；当本轮与上轮/存档点的测试区间不一致时，直接提示“比较范围已变化”，避免把不同区间的 failed case 误报成修复或退步。

### 文档

- **补充 Cargo 二进制名提醒**：在 `rule.md` 与 `sby_test.md` 中明确写出 Cargo 的普通 CLI bin 名是 `tswn-cli`，不是源码文件名 `tswn_cli`；并补充 `--out-raw --file` 的 failed case 重放示例，减少 agent 误用 `cargo run --bin tswn_cli` 的报错。
- **同步记录 offset 用法**：文档中的固定 SBY 命令已补上 `--case-offset-per-mode` 示例，并明确它是“每种模式独立生效”的稳定切片，而不是全局总 offset。

### 验证

- `cargo fmt --package tswn_core`
- `cargo test -p tswn_core --bin tswn_case_miner`

## [0.2.7] - 2026-04-03

> 提交范围: 87732e4..e21368d

### 修复

- **战斗时序与状态链继续对齐 JS**：将 `Charge` 的 `post_action` 拆成早晚两段，`ProtectState` 按 JS 的混合注册顺序进入防御链；`post_defend` 改为 skill/state 统一优先级链，并在同优先级下按 `registration_order` 稳定排序，修复 `Protect` / `Reflect` / `Iron` 等路径的顺序漂移。
- **战斗结束时中断尾部状态链**：战斗已结束时不再继续执行无意义的 `post_action` 尾链，同时修复 `Iron` 在死亡后到期时遗漏“从铁壁中解除”日志的问题。
- **Charm / alive 视图 / 复活同步顺序修复**：引入 `flat_alive` 对齐 JS `Engine.e` 的全局存活顺序，修复 `CharmState` 的 `post_action` 优先级、`recharm` 的 `effective/source team` 判定，以及同 tick 下“复活先于死亡移除”的同步顺序，避免 FFA 目标链和 `all_alive` 顺序分叉。
- **隐匿与幻影统计继续对齐**：`Hide` 改为统计本回合 pending 幻影，并进一步限制为只统计施法者自己本回合生成的幻影，减少同回合幻影场景下的触发偏差。
- **召唤物 / 分身生命周期与命名修复**：血祭/使魔重施会复用已死亡召唤物对象并按 revival 刷新状态；补齐 minion 命名与 root owner `?n` 编号规则，修复 clone display name override，并让 shadow 使用稳定 `sort_int`。
- **pending spawn / post_kill / 战斗结束日志修复**：将 pending enemy spawn 纳入敌方存活判定，避免误跳过吞噬等 `post_kill`；战斗结束后召唤物不再额外输出“被击倒了”，只保留“消失了”。
- **状态链迭代与反伤顺序修复**：`post_action` 状态链中的 `alive` 参数会随迭代实时刷新，`Reflect` 反伤与 `kill/post_kill` 回调顺序同步到 JS 行为。
- **Protect / merge / devour 联动修复**：`ProtectState` 使用 `set_state_no_update`，避免 `Haste` 蓄力加成提前生效；同时修复 `Protect` 技能 level 在 `merge/devour` 后不更新的问题。
- **技能数值与生命周期对齐**：保留 `Quake` 的 JS 原始浮点常量，修正聚气清除后的重用倍率生命周期，并为使魔技能补 `boosted` 标记判断。

### 调试与排查

- **case miner 样本扩展与测试补强**：case miner 支持每模式 500 样本，补充 summon / charm / world sync / iron / hide 等回归测试，并整理 player 相关测试结构以便继续对账。
- **补充已知问题定位用例**：新增 `merge` 技能槽位不匹配时隐藏技能不继承的测试用例，先保留为 `ignore` 以持续跟踪未完全收敛的问题。

### 验证

- 持续以 `cargo test -p tswn_core` 和固定 case miner / SBY 对账驱动本轮回归
- 覆盖 `Protect` / `Charm` / `Summon` / revive / `flat_alive` 等重点路径

## [0.2.6] - 2026-03-29

> 提交范围: 01a8a7f..6ea9390

### 修复

- **冰冻术蓄力加成与雷击循环条件修复**：补齐 `Ice` 在 charge 激活时的行为分支，并修正 `Thunder` 的循环/终止检查，避免雷击段数与 JS 基准漂移。
- **同级状态后处理顺序对齐注册顺序**：当多个状态拥有相同 `post_action` / `post_damage` 优先级时，改为按状态注册顺序执行，修复同级状态结算顺序不稳定的问题。
- **召唤物追踪名称槽位复用**：同一 root owner 重复召唤 Summon 时复用既有 `?n` 追踪槽位，避免 CLI 与 case miner 把同一追踪实体误判成新的召唤物。
- **主人死亡时待同步使魔清理**：owner 死亡时，尚未同步进 world 的 linked pending minion 也会被标记死亡并按 linked cleanup 顺序处理，避免 storage/world 视图分叉。
- **召唤物吞噬继承固定技能槽位修复**：修复 Summon 被吞噬或继承后固定技能槽位错位的问题，与 JS 固定槽位合并语义对齐。
- **同回合幻影生成后的同步顺序修复**：同一 action 内新生成的 shadow 在 owner 随后死亡时，会先进入 round roster 再移除，修复 `round_pos` 少减一次导致的后续调度漂移。
- **隐匿仅依据真实 alive group 触发**：`Hide` 不再在缺失真实存活队伍时回退到 clan 级别统计，修复“无同队存活者但同 clan 玩家仍触发隐匿”的误判。

### 调试与排查

- **`TSWN_DEBUG_ACTION` 支持包含匹配**：调试目标不再要求与 `id_name()` 完全相等，允许用子串直接命中 `?n`、幻影和 linked minion，降低 failed case 定位成本。
- **补充 action / state / death 的细粒度 RC4 追踪**：为 `action`、`post_action`、`post_damage`、`die` 等路径增加 actor/id/rc4 级别日志，方便对照 `md5.js` 缩小随机消费与调度差异。

### 验证

- `cargo test -p tswn_core` 全量通过（159 passed）
- 固定 SBY 大样本对比中，`diff_failures` 从 13 降至 6，且 `ts_failures = 0`、`rust_failures = 0`

## [0.2.5] - 2026-03-27

### 修复

- **战斗结束后停止 post-action 尾部**：行动决定战斗结束后，同回合的 MP 回复/换行/post_action 链不再继续执行，与 md5.js 对齐，消除大量 EOF 式的额外结尾日志（如 `铁壁解除`、`疾走解除` 等）。
- **隐匿存活友军计数对齐魅惑有效队伍**：隐匿触发判断现在在魅惑状态存在时使用魅惑有效队伍（`allyGroup`），而非原始队伍，修复被魅惑后隐匿触发错误的问题。
- **诅咒 post_defend 优先级对齐 JS ga4**：`CurseState` 的 `post_defend` 优先级从 110 修正为 10000，使诅咒增伤在 defend/iron/shield 之后结算，与 md5.js 的钩子顺序对齐。
- **刺杀目标陈旧锁清理**：强制行动开始前，若刺杀锁定的目标已死亡，现在会提前清除该锁，避免行动模式因陈旧锁而漂移。
- **隐匿 post-damage 存活友军计数修正**：`post_damage` 回调中的友军存活计数现在过滤掉 `alive()` 为假的队友，防止刚死亡的队友仍出现在存活快照中导致额外的 RNG 消耗。
- **召唤兽伤害分摊死亡顺序修复**：新增 `in_post_damage` 标记（对应 JS `PlrSummon.aR`），在 `SummonShareDamageSkill` 将伤害分摊给 owner 期间阻止 owner 死亡时立即处理使魔死亡，确保死亡顺序为 `[owner, summon]`。
- **复活不再清除冰冻状态**：`revive_with_hp` 不再重置冰冻标志，对齐 JS 的 `reraise/revive` 仅设置 HP 的语义。
- **post_defend 刷新与诅咒结算时机**：铁壁被 `post_defend` 击破时立刻刷新状态快照，避免 `attract` 读取旧值；诅咒附着移至实际伤害回调之后，防止护身符复活等路径把幸存目标错误标为诅咒。
- **蓄力中聚气联动**：聚气激活时若蓄力仍处于激活状态，补 500 行动条并将临时攻击倍率从 1.7 提升到 2.7，与 JS charge 活跃时的聚气结算对齐。
- **冰冻选目标评分修正**：智能选目标时对已带冰冻状态的目标得分减半，避免重复优先打同一已冻结单位，与 JS 基准实现对齐。
- **晚启用技能的 proc 与动作可见性**：吞噬/复制后新启用的技能现在按单技能补注册 proc；动作阶段显式跳过 build 时未进入 action 列表的技能，对齐 JS `clone` 在 `p.az()` 之后升级但不会 retroactively 进入 k4 的语义。
- **heal 清除诅咒补输出解除事件**：`heal` 在 `clear_negative_states()` 清除 `CurseState` 后现在输出"从诅咒中解除"消息，与 JS/Dart 对齐；负面状态解除消息的输出顺序改为字母序（berserk→charm→curse→ice→poison→slow），与 Dart `clearStates` 的 `sort()` 遍历一致。
- **Protect 空魅惑队伍时 RC4 消耗修复**：修复保护技能在魅惑队伍为空时仍错误消耗 RC4 随机数的问题。
- **行动中技能升级被旧等级覆盖修复**：修复行动执行过程中发生技能升级后，旧等级数据被写回覆盖新等级的问题。
- **净化后正向状态清除顺序修复**：修复净化（disperse）触发后正向状态清除顺序不一致的问题。
- **混合大小写名称评分哨兵类别修复**：JS 的 `lC` 逻辑调整计数时临时抬高的是 `OTHER(3)` 而非 `UPPER(2)`，Rust 之前用错类别，导致 `tOeyDD` 这类混合大小写 ASCII 名称的 `name_factor` 计算偏差，现已对齐。
- **链式魅惑继承已解析队伍视角**：`CharmState` 新增 `effective_team_idx` 字段，链式魅惑后的友军视角（选目标、保护、隐藏、反击等路径）现在优先使用已解析队伍，与 md5.js 的 `allyGroup` 继承语义对齐。
- **post_kill 回调中 `&mut Player` 别名 UB 修复**：`on_die_impl()` 的 killer kill 回调链持有 `&mut Player` 期间，`MergeSkill::kill_with_level()` 会对同一玩家创建第二个 `&mut Player`，违反 Rust 别名规则，导致 LLVM `noalias` 优化在 release 构建中丢失写入。新增 `run_post_kill()`，在调用每个 kill 回调前通过 `take_skill_type()` 临时取出技能实现并释放引用。

### 优化

- **`TSWN_DEBUG` 环境变量收口 + `no_debug` 编译期消除**：将所有直接 `std::env::var("TSWN_DEBUG_*")` 调用替换为 `crate::debug::*` 函数；启用 `no_debug` feature 时这些函数内联为 `const false/None`，零开销。实测（aaa vs bbb 10w 场，`--features no_debug`）单线程快约 25%，多线程快约 7.2x（消除 env 锁竞争）。
- **win_rate rq 兼容与构造热路径优化**：
  - `eval_name` 中的 `rq` 从全局共享状态改为显式配置（`Storage.eval_rq`），避免模式切换残留与多线程 worker 间的状态污染。
  - 引入可复用的 `PreparedRunner` 句柄，`prepare_groups_with_eval_rq` + `new_from_prepared_with_seed` 接口；win_rate 路径先按当前 rq 预构建一次模板，每局直接按 seed 构造 `Runner`，省去缓存键哈希、锁读取和模板命中分发的重复开销。
  - CLI `--win_rate` / `--win_rate_st` 新增可选开关 `--keep-rq`：默认保持 win_rate 使用 `rq=6` 的兼容行为，显式带上则回到普通对战的 `rq=4` 语义，方便对照排查差异。
  - 修正狂暴收尾日志：强制攻击在击倒最后一个敌人、战斗已结束时，不再继续执行 `forced_action_states`，避免额外输出"从狂暴中解除"等与 JS 不一致的尾部日志。

### 验证

- SBY 测试差异从 311 持续下降到 13（多轮修复）
- `cargo test --workspace --quiet` 全量通过

## [0.2.4] - 2026-03-25

### 修复

- **魅惑(Charm)团队比较修复**：修复魅惑技能中团队比较逻辑，现在使用施法者的有效团队（考虑施法者自身是否被魅惑）来判断是否同队，与 JS 实现对齐。
- **post_damage 优先级排序**：新增 `post_damage_priority()` 方法到 `SkillTrait`，支持技能指定 post_damage 钩子的执行优先级。
  - 默认优先级为 10000
  - 刺杀(Assassinate) 使用 `i32::MAX` 确保在隐匿(Hide)之后执行
  - 修复了"潜行被识破"与"发动隐匿"的输出顺序问题
- **linked minion 清理顺序修复**：owner 死亡时，linked minion 改为按队伍 roster 顺序清理，避免 `?1` 在 `?0` 前消失导致日志反序。
- **净化取消文案对齐**：移除 `[垂死]` 被净化时额外“垂死属性被打消”提示，与 `md5.js` 行为对齐。
  - 具体实现为：清理正向状态时仅在“目标仍存活（hp > 0）”场景输出取消文案，避免死亡结算前插入额外行。

### 验证

- SBY 测试差异从 366 降至 311（修复 55 个案例）
- `cargo test --workspace --quiet` 全量通过

## [0.2.3] - 2026-03-22

### 新增
- `tswn-cli` 新增 `--out-raw` 选项（仅普通对战模式生效），输出聚合战斗日志格式。

### 调整
- `--out-raw` 模式下，输出对齐 `fast-namerena/branch/latest/out_md5.ts`：
  - 按回合分段聚合日志
  - 动作行追加同段后续事件
  - 使用空行分隔段落
- 启用 `--out-raw` 时，仅输出聚合日志，不输出欢迎信息、玩家状态与对局结果摘要。
- `--help` 文案补充 `--out-raw` 使用说明。

## [0.2.2] - 2026-03-22

- 去掉了某些文字

## [0.2.1] - 2026-03-16

### 新增
- CLI 新增单线程 benchmark 命令：
  - `--bench-st` / `--bench-raw-st` / `--bench-file-st`
  - `--win_rate_st`
- benchmark 新增并发线程环境变量：
  - `TSWN_BENCH_WORKERS`
  - `TSWN_WINRATE_WORKERS`（兼容旧变量，行为同上）

### 优化
- `--bench` 默认走并行优化路径（含评分模式与胜率模式）
- `Runner::new_from_groups_with_seed` 增加预构建玩家模板缓存，减少重复构造/升级/build 成本
- `SkillStorage` 多处热路径移除临时 `Vec` clone，改为按索引遍历，降低分配与拷贝开销
- `--win_rate_st` 单线程路径进一步优化，尽量缩小与多线程模式差距

### Python 绑定
- `tswn_py` 同步新增接口：
  - `Runner.new_from_groups_with_seed`
  - `Runner.round_tick_new_update_no_capture`
  - `RunUpdates.new_no_capture` / `RunUpdates.reset` / `RunUpdates.capture_updates` / `RunUpdates.had_updates`

### 性能
- 在 `target\release\tswn-cli.exe --win_rate aaa bbb 10000` 场景：
  - 多线程典型耗时约 `0.059s`
  - 单线程典型耗时约 `0.214s`（较此前约 `0.344s` 明显下降）

### 验证
- `cargo test --workspace --quiet` 全量通过

## [0.2.0] - 2026-03-15

### ⚠️ Breaking Changes
- 状态系统不再依赖 `Any/downcast`：`StateTrait` 移除 `as_any/as_any_mut`，改为稳定 `StateTag` + `state_type_id` 校验路径
- `SkillExt` 不再要求实现 `Any`
- `RunUpdates` 引入可选帧采集开关，`run_to_completion` 默认走无帧采集高速路径

### 新增
- 技能注册中心：新增 `register_skill_factory`，支持外部注册/覆盖技能工厂
- Boss 注册中心：新增 `register_boss_handler`，统一 Boss 初始化/行动/免疫策略扩展入口
- Hook 动态扩展通道：新增 `ActorHookDyn` 与 `EngineCore` 的 `*_hook_dyn` 注册 API
- Runner 新增 `new_from_groups_with_seed`，支持复用已解析分组输入

### 优化
- `--win_rate` 改为并行模拟，并支持 `TSWN_WINRATE_WORKERS` 覆盖并发 worker 数
- 胜率热点路径改为低分配实现（延迟构建 `RunUpdate`、`SmallVec` 小集合优化、动态负载均衡）
- 玩家目标选择热路径减少临时分配与排序开销
- Release 配置优化：`lto = "fat"`、`codegen-units = 1`

### 性能
- 在 `target\release\tswn-cli.exe --win_rate aaa bbb 10000` 场景下，典型耗时由约 `0.422s` 降至约 `0.064s`（机器/负载相关）

### 验证
- `cargo test --workspace --quiet` 全量通过

## [0.1.9] - 2026-03-15

### 新增
- 将根目录的 `CHANGELOG.md` 迁移到 `crates/tswn_core/CHANGELOG.md`，统一管理核心库更新日志
- 给 `RC4` 新增了 `peek_u8` 方法，可以在不修改状态的情况下查看下一个随机字节
- 加了不少注释

### 优化
- 优化存储系统 (`storage.rs`) 和世界状态同步 (`world_state.rs`)
- 完善玩家系统 (`player/mod.rs`)，增强可维护性
- 改进 RC4 算法实现 (`rc4.rs`)，提升随机数生成效率
- 内部代码重构和性能优化

## [0.1.8] - 2025-03-09

### 新增

- 新增 `debug` 模块，统一管理所有调试环境变量
- CLI 帮助信息中添加了完整的调试环境变量说明

### 修改

- 将 `state.rs` 中默认开启的调试输出改为受 `TSWN_DEBUG_STATE` 环境变量控制
- 将 `covid.rs` 中默认开启的调试输出改为受 `TSWN_DEBUG_COVID` 环境变量控制
- 所有调试输出现在默认关闭，需要设置对应的环境变量才会输出

### 调试环境变量

| 环境变量 | 说明 |
|----------|------|
| `TSWN_DEBUG_ACTION=<名字>` | 调试特定玩家的行动 |
| `TSWN_DEBUG_STATS` | 调试玩家属性计算 |
| `TSWN_DEBUG_WORLD` | 调试世界状态同步 |
| `TSWN_DEBUG_TICK` | 调试每个 tick 的执行 |
| `TSWN_DEBUG_PICK` | 调试目标选择逻辑 |
| `TSWN_DEBUG_DODGE` | 调试闪避逻辑 |
| `TSWN_DEBUG_DODGE_ALL` | 调试所有玩家的闪避 |
| `TSWN_DEBUG_DIE` | 调试死亡处理 |
| `TSWN_DEBUG_STATE` | 调试状态系统（状态设置/清除/追踪） |
| `TSWN_DEBUG_COVID` | 调试 COVID Boss 相关逻辑 |
| `TSWN_DEBUG_FIRE` | 调试火焰技能 |
| `TSWN_DEBUG_HEAL` | 调试治疗技能 |
| `TSWN_DEBUG_UPGRADE=<名字>` | 调试升级技能 |
| `TSWN_DEBUG_REFLECT` | 调试反射技能 |
| `TSWN_TRACE_RC4` | 追踪 RC4 随机数状态 |

---

## [0.1.7] - 2025-03-09

> 提交范围: bb7b3f5..05ff54c

### 新功能

#### 图标系统重构

- **图标生成算法**：完整复现 JS/Dart 的 `Sgl.createFromName()` 算法
  - 支持 `name@team` 格式解析（同队玩家共享图标）
  - RC4 密钥生成 + S 表映射 + 颜色选择
  - 颜色距离矩阵预计算（`OnceLock` 懒加载）
- **精灵数据**：新增 `src/player/sprite_data.rs`
  - 38 个前景形状 alpha 映射（每个 16x16 = 256 字节）
  - 8 个边框深色覆盖层
  - 8 个边框不透明度掩码
- **提取工具**：新增 `examples/extract_sprites.rs` 从 PNG 提取精灵数据

#### CLI 工具增强

- **Benchmark 模式**：
  - `--bench [N]` — 自动检测模式（1组→评分，2+组→胜率）
  - `--bench-raw` / `--bench-file` — 支持命令行/文件输入
  - 评分测试：普通评分 + !评分
  - 胜率测试：team1 vs team2
- **图标生成**：
  - `--icon <名字>` — 显示图标信息 + ANSI 真彩色终端渲染
  - `--icon-b64 <名字>` — 输出 Base64 PNG data URL（需 `png_render` feature）
  - `--icon-path <目录> <名字>` — 保存 PNG 文件（需 `png_render` feature）
- **输入处理**：支持 `--raw`、`--file`、stdin 三种输入方式

#### 玩家系统扩展

- **状态系统**：新增 `src/player/status.rs` - `PlayerStatus` 结构体
  - 完整属性：HP、MP、攻击、防御、速度、敏捷、魔法、抗性、智力等
  - Display trait 格式化输出
- **Boss 系统**：新增 `src/player/boss/mod.rs`
  - 4 种 Boss 类型：Covid、Lazy、Saitama、Generic
  - Boss 免疫阈值系统
  - Boss 默认行动逻辑
- **技能系统**：新增 `skill/act/mod.rs`、`skill/skl/mod.rs`、`skill/store.rs`
- **玩家实现**：新增 `impl_attr.rs`、`impl_ctor.rs`、`impl_runtime.rs`
- **其他**：新增 `weapons.rs` 武器系统、`eval_name.rs` 名称评估

### 改进

- **引擎优化**：简化 `engine/test/runner/large_41_45.rs` 测试代码
- **依赖更新**：Cargo.toml 新增 `png` 依赖用于精灵提取

### 统计

| 指标 | 数值 |
|------|------|
| 提交数 | 4 |
| 文件修改 | 23 |
| 新增行数 | +2083 |
| 删除行数 | -705 |
| 新增文件 | 12 |

### 新增文件列表

```
examples/extract_sprites.rs    - 精灵提取示例
src/player/sprite_data.rs      - 精灵数据常量
src/player/status.rs           - 玩家状态结构体
src/player/boss/mod.rs         - Boss 系统
src/player/eval_name.rs        - 名称评估
src/player/impl_attr.rs        - 玩家属性实现
src/player/impl_ctor.rs        - 玩家构造实现
src/player/impl_runtime.rs     - 玩家运行时实现
src/player/weapons.rs          - 武器系统
src/player/skill/act/mod.rs    - 技能行动
src/player/skill/skl/mod.rs    - 技能定义
src/player/skill/store.rs      - 技能存储
src/player/test.rs             - 玩家测试
src/player/icon_render/test.rs - 图标渲染测试
```
