# 更新日志

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
