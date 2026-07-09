# runtime v2 custom 迁移审计

> 状态：阶段 B 审计草案
> 范围：`github/main..github/custom` 中与 custom 产品线相关的行为差异
> 目标：把 custom 分支的行为改动逐项落到 runtime v2 extension / policy / renderer / fixture 验收面

---

## 1. 审计来源

审计基准：

```powershell
git diff --stat github/main..github/custom
git diff --name-status github/main..github/custom
```

关键差异文件：

| 类型 | custom 分支文件 |
| --- | --- |
| player kind / overlay | `crates/tswn_core/src/player/impl_attr.rs`, `impl_ctor.rs`, `impl_runtime.rs`, `player/mod.rs` |
| summon / minion | `player/skill/act/summon.rs`, `player/skill/act/minion.rs`, `player/test/minions.rs` |
| merge | `player/skill/skl/merge.rs` |
| replay / show | `crates/tswn_core/src/replay_view.rs`, `crates/tswn_wasm/examples/show-utils.js` |
| runner fixture | `crates/tswn_core/src/engine/test/**`, moved from `crates/tswn_test/src/suite/**` |

---

## 2. custom 行为映射

| custom 改动点 | 证据锚点 | v2 落点 | 当前 v2 状态 | 验收 case |
| --- | --- | --- | --- | --- |
| bed2 player type | `DEFAULT_BED2_HP = 3000`; `PlayerType::Bed2`; `bed2[...]` / `@bed2` marker | `PlayerKindSpec` + `PlayerKindPolicies` + template/entity slot | 已有 bed2 registry/template fixture 覆盖 kind、policy、3000 HP 与 marker slot，并补最小 v2 marker import helper 与 Player facade id-name 归一化桥接；template slot 已保存 typed summon payload 并可由 handler 读取后 spawn；已补 grouped raw bed2 roster 与 mixed legacy/bed2 raw roster -> `PreparedCombatTemplate` helper，并通过 `RuntimeV2Runner` 正式接入 bed2-only / mixed roster 构造、单回合归一化和 run-until-winner 归一化入口 | bed2 构造后固定 HP、技能槽、summon 模板 strict diff |
| bed2 固定 summon 技能 | custom 将 bed2 overlay 设为 `[0,99,0,0,0,99,0,hp]` 并只保留 `sklsummon=255` | `PlayerTemplate::with_kind(...).with_skills([summon])` + policy | v2 fixture 已覆盖固定 summon skill loadout；缺内置 summon 技能迁移 | bed2 只尝试 summon，不扫描普通技能 |
| bed2 summon template 导出 | `summon_overlay_from_player_template`; `overlay_from_built_minion` | template slot 保存 summon/minion 模板；effect handler 生成实体 | v2 fixture 已覆盖 typed summon template payload，`push_summon_from_template_slot` helper 可通过 template slot 读取 `PlayerTemplate` 并 spawn，保留 attr/skills/move_state/owner/root-owner，并对缺失/类型错误 payload 返回结构化错误；`push_summon_from_template_slot_with_message` 已支持 legacy/custom summon 文案 `召唤出[1]`；`ExtensionRegistry` 已提供 skill name / export_name -> v2 `SkillId` 查找面；`CustomBed2Import::*_with_summon_overlay` 与 `RuntimeV2Runner::*_with_summon_overlay` 已能从 bed2-only 与 mixed roster 的 bed2 raw `ol.summon` 解析 attrs、inherit_owner_def_res 与 summon fire/explode active order，并写入/保留 typed template slot；`*_with_shadow_overlay` 已可从 `ol.shadow` 解析 attrs 与 possess active order 写入 typed shadow template slot，缺失 v2 skill export_name 会结构化报错；`*_with_zombie_overlay` 已可从 `ol.zombie` 解析 attrs，并通过 skill export 前缀把 normal / possess / explode 等 overlay skill 写入 typed zombie template slot；`*_with_minion_overlays` 已可一次性导入 summon / shadow / zombie 三个 template slot；`CustomRuntimeV2ImportConfig` 已把 registry、bed2 kind、summon skill 与 minion overlay slot 映射收束成 profile/builder 入口；默认 custom v2 profile 已注册三类 overlay 所需 kind/skill/template slot，并能经默认 mixed raw runner 导入 `ol.summon` / `ol.shadow` / `ol.zombie`；仍缺完整 custom parser 接入 | bed2 summon 的 attr/skills/move_point 与 custom branch 一致，spawn 文案可对齐 legacy |
| summon recast 复用技能 | `reuse_skills_on_recast: is_summon` | summon policy + effect handler | 已有 custom summon 复合 fixture 覆盖 spawn 后 SkillLoadout 与 move_state 保留，并补 `push_summon_recast_from_entity_slot` helper 覆盖死亡后复活复用同一 summon 实体、remembered summon 存活时拒绝重复 spawn、缺少读取 capability 时返回结构化错误；`SkillLoadout` 已拆分固定槽位和主动扫描顺序，`summon_default_skill_loadout` 固化 legacy `[fire, fire, explode]` 固定槽位；`push_summon_recast_from_template_slot` 已覆盖从 typed template slot 读取真实 summon payload 后首次 spawn、死亡后 revive 原实体；`SpawnWithMessage` / `ReviveWithMessage` 已支持 legacy summon 的 `[0]使用[血祭]` + `召唤出[1]` 外显帧序列；`SummonExplode` / `push_summon_explode` 已覆盖自爆 replay、自身死亡、`get_at(true) * (4.0 + fire_mag)` 魔法伤害公式、命中/回避 RNG 与 replay、命中存活目标后的 fire_mag 半层递增、BOSS/BOOST kind 的 fire immune 判定和 hook 顺序数据面；仍缺完整内置 summon 技能迁移与 `attacked()` 全链路 parity | summon recast 后技能继承/复用顺序和 move_point 不漂移，replay 文案顺序可对齐 legacy |
| summon 继承 owner 防御/魔防 | `inherit_owner_def_res: is_summon` | `PlayerKindPolicies::inherit_owner_def_res` + template/runtime def/res | 已有 v2 custom summon fixture 覆盖 spawn 时继承 owner defense/resistance；`run_legacy_summon_recast_from_template_slot_with_config` 可复用 typed summon template 生成/复活真实 summon payload | summon 出场后的防御/魔防展示与 custom 一致 |
| summon/root-owner 伤害路由 | summon clone damage route to root owner | `OwnerResolutionPolicy::RootOwner` | 已接入并在 custom summon 复合 fixture 中覆盖 | root owner 承伤、致死 hook 目标一致 |
| summon 伤害共享 owner | child/summon damage share owner；charge summon 会关闭 share damage | `DamageSharePolicy::ShareToOwner` + template policy override | 已接入并测试 owner 共享致死 hook；复合 fixture 覆盖 summon policy 注册；`PlayerTemplate` 已支持 per-template policy override，可在 charged summon payload 上关闭 `ShareToOwner` 并保留同一 summon kind 的其它策略 | 子实体受伤同步扣 owner，owner 死亡 hook 顺序一致；charge summon 不再分摊伤害 |
| owner 伤害共享 summon | owner damage share alive summons | `DamageSharePolicy::ShareToSummons` | 已接入并在 custom summon 复合 fixture 中覆盖按实体顺序共享 | owner 受伤同步扣存活 summon，顺序稳定 |
| minion heal sharing 移除/调整 | custom minion 行为集中在 `act/minion.rs` 与 `player/test/minions.rs` | player kind policy 或 damage/share policy | 已有 v2 custom minion fixture 固化 damage 仍共享、heal 只作用目标实体，并补 owner death / explicit remove 时 linked minion 按实体顺序清理；`next_minion_name_from_entity_slot` 已覆盖 root owner counter 分配 `owner?N` 名称和 child minion 复用 root counter，`push_minion_from_template_with_allocated_name` 已覆盖分配名称后 spawn 并保留 legacy/custom summon 文案，`push_minion_from_template_slot_with_allocated_name` 已覆盖从 typed template slot 读取真实 minion 模板、分配名称后 spawn；`run_shadow_minion_from_template_slot_with_config` / `run_zombie_minion_from_template_slot_with_config` 已提供可配置正式 handler 并固化 legacy 外显帧序列；`SpawnSilent` 已支持生成 minion 而不额外输出占位 spawn 帧，`minion_display_index_for_entity` 已覆盖 legacy `?N` 展示序号解析；仍需更多内置 minion strict-diff parity | minion 相关 heal 不再产生 custom 分支禁止的共享，owner 死亡同步清理 linked minion，名称计数与展示序号从 root owner 稳定递增 |
| merge 固定槽继承 | custom 保留 `slot_skill` 固定槽语义以避免 merge 错位 | `MergePolicy::FixedLane` | 已接入并测试 fixed lane 合并 | 同槽位技能覆盖，未映射技能 append |
| merge 丢弃未映射技能 | custom 分支支持 drop unmapped 语义 | `MergePolicy::DropUnmappedSkills` | 已接入并测试 drop unmapped | 未映射来源技能不进入 caster loadout |
| merge replay | custom replay 使用吞噬/属性上升展示 | `QueuedEffect::Merge` replay update | 已输出 `[0][吞噬]了[1]` 与 `[0]属性上升` | merge frame 顺序、score 分别为 60/0 |
| HP report replay | custom 新增 `"[0]还剩[2]点血"` 作为 HP marker | replay/show renderer + entity slot | 已用 v2 core replay/show golden 固化 payload 和 `[2]` param，并补 HP bar show renderer fixture；wasm 结构化 replay view 已强制 `show_hp`，可复用现有 actorToken HP 条渲染 | HP marker 强制显示 HP bar，`[2]` 作为 data |
| show 数字高亮 | `show-utils.js` 把 `点血` 纳入数字高亮 | show renderer / wasm show adapter | v2 core show golden 已覆盖 `还剩87点血` 文本；结构化 `Data` part 已让 wasm/show 对 `[2]` 渲染 `message-number` | `还剩87点血` 中 87 被识别为数值 |
| runner fixture 内置化 | `crates/tswn_test/src/suite/**` moved into `crates/tswn_core/src/engine/test/**` | repo 内 extension fixture + strict diff runner | 已有最小 v2 custom runner strict-diff golden 覆盖 spawn、owner def/res、damage share、heal 与 HP marker，并补 linked minion owner-death cleanup、merge 与 multi-round normalized run 的 runner strict-diff golden；仍缺 large/fight_multi legacy 样例 | bed2/summon/merge/minion/custom replay golden 可稳定复跑 |

---

## 3. v2 fixture 切分顺序

1. **bed2 registry/import fixture**：已注册 `custom.bed2` kind、固定 summon skill、HP marker slot，并覆盖 `bed2[...]` / `@bed2` marker 到 v2 template 的最小导入、Player facade id-name 归一化桥接、typed summon template payload 读取后 spawn、grouped raw bed2 roster、mixed legacy/bed2 raw roster 到 `PreparedCombatTemplate` 的 helper、bed2-only 与 mixed roster 的 bed2 raw `ol.summon` -> typed summon template slot 的 parser-facing runner 入口、summon / shadow / zombie 三类 minion overlay 组合导入入口，以及 `RuntimeV2Runner` 的 bed2-only / mixed roster 与 raw namerena fixture 形状正式构造、seed 初始 RNG 对齐、单回合与 run-until-winner 归一化入口。
2. **summon policy fixture**：已覆盖 root-owner 路由、owner/summon 伤害共享、spawn 后技能保留、owner defense/resistance 继承，以及 `push_summon_recast_from_entity_slot` / `push_summon_recast_from_template_slot` 死亡后原实体复活复用、活体 remembered summon 防重复 spawn、缺少读取 capability 的结构化错误；已补从 typed template slot 读取真实 summon payload 后输出 legacy summon replay 文案路径，后续补完整内置 summon 技能迁移。
3. **minion fixture**：已覆盖 owner damage share 仍生效、minion heal 不向 owner 或 sibling minion 共享、owner death / explicit remove 清理 linked minion，以及从 template slot 读取真实 minion 模板后按 root owner entity slot 递增分配 `owner?N` minion 名称并按 legacy/custom 文案 spawn；shadow/zombie handler 已提升为带槽位参数的正式 helper，并覆盖 `幻术` / `召唤亡灵` 外显帧序列；后续补更多内置 minion strict-diff parity。
4. **merge fixture**：使用 `FixedLane` 与 `DropUnmappedSkills` 两组 golden 覆盖 replay 与 loadout。
5. **HP marker renderer fixture**：已用 core replay/show payload 固化 `还剩[2]点血` 展示与数值 data，并补 HP bar show renderer payload；wasm 结构化 replay view 已对 HP marker 强制 `show_hp`。
6. **runner fixture**：已新增最小 v2 strict-diff golden，并把 linked minion owner-death cleanup、merge 与 multi-round run-until-winner 纳入归一化 runner golden；已从 custom 分支 large / fight_multi 真实 raw 输入抽出初始化 parity golden，覆盖 seed RNG、team 编号和 round/alive 派生视图；large 真实 raw 已补完整 run-until-winner normalized golden，fight_multi 真实 raw 已补前 4 轮 normalized prefix golden 与完整 run-until-winner 终局 golden，固定 RNG checkpoint、HP/MP/防御/魔防、action/frame、world 派生视图、winner 与 guard 状态；后续继续把关键样例扩展到完整逐回合 runner golden。

---

## 4. 已落地能力

- `PlayerKindSpec` / `PlayerKindPolicies` 可表达 custom kind 与行为策略。
- `CustomBed2Import` 已覆盖 `bed2[...]` / `@bed2` marker 到 v2 bed2 template 的最小导入面，并通过 `parse_player_facade_raw` 对接 `Player::raw_namerena_to_idname` 的名字/队伍归一化结果。
- grouped raw bed2 roster 已可跳过 seed 行、按输入队伍顺序分配 team/id，并转换为 `PreparedCombatTemplate`。
- bed2-only 与 mixed roster / runner import 已新增 parser-facing summon/shadow/zombie overlay 入口、组合 minion overlay 入口和 `CustomRuntimeV2ImportConfig` profile 入口，可复用现有 `ol:` overlay parser，把首个 bed2 `ol.summon` 的 attrs、inherit_owner_def_res 和 `sklfire1` / `sklfire2` / `sklexplode` active order 转为 typed v2 `PlayerTemplate` payload；`ol.shadow` 的 attrs 与 possess active order 可转为 typed shadow template slot；`ol.zombie` 的 attrs 与 normal / possess / explode 等 overlay skill 可通过 skill export 前缀转为 typed zombie template slot；需要技能映射的路径均通过 registry `export_name` 查找 v2 `SkillId` 后写入 template slot；core 已提供 `default_custom_runtime_v2_import_config`，且默认 profile 已注册三类 overlay 的默认 kind、template slot 与 summon fire/explode、minion possess/heal export，可直接经默认 mixed raw runner 导入 `ol.summon` / `ol.shadow` / `ol.zombie`；core `cli_api` 已提供显式 profile 与默认 profile 两组 `custom_runtime_v2_*` / `default_custom_runtime_v2_*` helper，并统一校验 `max_rounds > 0`；CLI 已提供 `runtime-v2 normalized-run` 默认 profile JSON 命令，并补结构化 JSON golden 与 zero max_rounds 错误覆盖；C API 已提供 `tswn_default_custom_runtime_v2_normalized_run_json` 默认 profile JSON 入口，并补结构化 JSON golden 与 zero max_rounds 错误覆盖；Python 已提供 `default_custom_runtime_v2_normalized_run` 默认 profile dict 入口，并补 dict golden 与 zero max_rounds 错误覆盖；wasm 已提供 `default_custom_runtime_v2_normalized_run` 默认 profile typed 入口，并补 typed view golden 固定 rounds、RNG、entity stats、action/frame、`UpdateTypeView` 字段形状与 zero max_rounds `INVALID_INPUT` 错误覆盖；作为不替换 legacy API 的外层 custom profile 调用面。
- mixed legacy/bed2 raw roster 已可通过 legacy `Player` facade 导入普通玩家，同时对 bed2 marker 使用 custom bed2 template importer。
- `RuntimeV2Runner` 已可从 bed2-only / mixed roster 与 raw namerena 文本构造正式 v2 runner，复用 legacy 空行分组 / seed 独占组解析形状，并把 raw seed 初始化后的 RC4 checkpoint、team 编号、round/alive 派生视图对齐到 legacy `Runner` / `WorldState`，输出单回合与 run-until-winner 的 `NormalizedOutcome` 供 strict diff / runner golden 复用；core 已把默认 custom v2 profile 构造和 raw import + normalized run 包成专用 helper，CLI / C API 已接入默认 profile JSON normalized run，Python 已接入默认 profile dict normalized run，wasm 已接入默认 profile typed normalized run，其他绑定层可先调用默认 profile helper 再逐步切换 CLI/wasm/Python/C API。
- `TemplateSlotStorage` 已可保留 typed `PlayerTemplate` payload，extension context 通过 `ReadTemplateSlots` capability 读取 bed2 summon 模板，`push_summon_from_template_slot` helper 会校验 payload 并交给 `QueuedEffect::SpawnWithMessage` 生成实体；`push_summon_from_template_slot_with_message` 可为真实 summon handler 指定 `召唤出[1]` 等 legacy/custom 外显文案。
- `OwnerResolutionPolicy::RootOwner` 已覆盖 summon/root-owner 伤害路由。
- `DamageSharePolicy::ShareToOwner` / `ShareToSummons` 已覆盖 owner 与 summon 伤害共享；`PlayerTemplate` policy override 已支持 charged summon 这类 per-entity 差异，避免为了关闭 share damage 拆出额外 kind。
- `SkillPostActionPhase::Late` 已覆盖 charge 这类 legacy x2 尾部 post_action：普通 post_action skill 先于 state 收尾，charge late handler 晚于 state 收尾并负责清理 `ChargeRuntime` / `at_boost`。
- `AccumulateRuntime` 已覆盖聚气主动 runtime：初始倍率、Charge 加成、move_point 奖励、Charge late 清理后的倍率回落，以及 clear-positive priority 100 的取消消息和 JS reset multiplier。
- `StateStore::clear_positive_states_with_ordered_messages` 已覆盖 v2 已迁移正面状态：Shield 清 payload 不出消息，Haste priority 300 且死亡静默，Iron priority 400；消息排序按 priority / registration_order / legacy key 对齐 legacy。
- `EntityRecord::clear_positive_messages` / `SkillContext::clear_owner_positive_messages` 已把 runtime positive 与 state positive 清理合成单一数据面，覆盖 Accumulate、Charge、Haste、Shield、Iron 的清理与 priority 排序，后续 Disperse handler 可直接复用。
- `QueuedEffect::DisperseAttack` / `DisperseHit` 已覆盖 legacy Disperse 魔法攻击数据面：`[0]使用[净化]` replay、target-side `pre_defend` 攻击量改写/截断、magic attack 回避 RNG / `[0][回避]了攻击` replay、target-side `post_defend` 伤害改写、minion 目标 atp 翻倍、legacy damage frame 后立即执行 `on_disperse` 清目标 runtime/state positive 并扣目标 MP，且 lethal 命中会先保留 alive 语义输出 Haste 解除消息再执行 DIE/KILL hook；`run_disperse_skill` 已能通过 `SkillContext::selected_target` 在 round `PRE_ACTION` 中复用当前已选目标投递攻击，`PlayerTemplate` / `PlayerRuntime` 已承载 `wisdom`、`attr_sum` / `atk_sum` / `attract` 目标评分数据并补 `score_disperse_target` 公式测试，round `PRE_ACTION` 已接入 legacy smart roll 消耗、Disperse 多候选抽样、评分排序与 selected target 透传。
- `SkillLoadout` 已区分 fixed lanes 与 active order，默认 summon loadout helper 固化 `[fire, fire, explode]` 固定槽位，避免后续 merge 读取被主动顺序洗牌影响。
- `PlayerKindPolicies::inherit_owner_def_res` 已覆盖 custom summon 继承 owner 防御/魔防的数据面。
- `push_summon_recast_from_entity_slot` 已通过 owner entity slot 记录 summon 实体，并覆盖死亡后重施复活同一 `EntityIdx`、保留技能 loadout 和 owner/root-owner 元数据；`push_summon_recast_from_template_slot` 已把 typed template slot payload 接入同一路径，`run_legacy_summon_recast_from_template_slot_with_config` 已作为可配置正式 handler 复用该数据面并保留 legacy summon 外显文案顺序：先输出 `[0]使用[血祭]`，再在 spawn 或 revive 原实体时输出 `召唤出[1]`；helper 会在 remembered summon 仍存活时返回 `RememberedSummonAlive`，缺少 `ReadTemplateSlots` / `ReadAllies` 时返回 capability 错误，避免静默生成第二个 summon。
- `SummonExplode` effect 与 `push_summon_explode` helper 已覆盖 legacy 自爆的 `[0]使用[自爆]` replay、自爆者 HP 归零/移出 alive views、`get_at(true) * (4.0 + fire_mag)` 魔法伤害公式、target-side `pre_defend` 攻击量改写/截断、受击回避 RNG / `[0][回避]了攻击` replay、target-side `post_defend` 伤害改写、ShieldState 护盾吸收/耗尽 payload、CurseState r63 触发伤害倍增 replay / 未触发耗 RNG / damage<=0 不耗 RNG、IronState 吸收削伤 / 防御 replay 判定 / 击破换行和打消 replay / post_action step 递减和自然解除 replay、PoisonState post_action 毒性发作 / 持续伤害 / 自然解除 / 致死不释放、HasteState / CharmState / SlowState post_action step 递减、死亡静默清理、自然解除换行 replay 与 legacy 210 优先级顺序、BOSS/BOOST kind 的 `on_fire` immune RNG、命中存活且未免疫目标后的 fire_mag 半层递增，以及目标受击 DIE/KILL 链后再执行自爆者 DIE；仍需后续扩展更多内置 defend skill/state 的静态迁移和 strict-diff parity。
- `PlayerTemplate` / `PlayerRuntime` 已承载 agility，mixed roster import 会把 legacy `status.agility` 带入 v2，用于 magic attack 的回避判定。
- `StateEntry` 已支持 `FireMagHalfSteps` payload，可用 v2 `StateStore::fire_mag` 读取 legacy `FireState.fire_mag` 的 0.5 递增层数，作为后续火球和 summon 自爆公式 parity 的数据面。
- `PlayerTemplate` / `PlayerRuntime` 已承载 magic、magic_point 与 at_boost_millionths，raw legacy import 会带入 MP，`NormalizedOutcome` / strict diff 已覆盖 MP；`PlayerRuntime::get_at` 已复刻 legacy `get_at(true/false)` 的 RNG 取值公式，summon 自爆 amount 已接到 `get_at(true) * (4.0 + fire_mag)` 并按目标 resistance 魔法防御取整。
- `QueuedEffect::Heal` 已用 custom minion fixture 固化不触发 owner/summon damage share。
- linked minion cleanup 已接入 v2 damage/remove pipeline，owner 致死或显式 remove 时按实体顺序把存活 minion 标记死亡、移出 round/alive views 并输出 `[1]消失了`。
- `next_minion_name_from_entity_slot` 已通过 root owner entity slot 记录 minion 名称计数，root owner 自身与 child minion 都按 `owner?N` 稳定分配；`push_minion_from_template_with_allocated_name` 已把名称分配、模板改名与 `SpawnWithMessage` 串起来，`push_minion_from_template_slot_with_allocated_name` 已把 typed template slot 读取接入同一路径；`run_shadow_minion_from_template_slot_with_config` 与 `run_zombie_minion_from_template_slot_with_config` 已作为可配置正式 handler 复用导入阶段生成的真实模板和 legacy/custom 出场文案；`SpawnSilent` 与 silent minion helper 已支持 zombie 这类自定义复合 replay 场景；shadow-style fixture 已固化 `幻术` 的使用帧与召唤帧顺序，zombie-style fixture 已固化 `召唤亡灵` 的换行、召唤和转化帧顺序；`minion_display_index_for_entity` 已按 legacy `minion_display_index` 规则把 `?N` 转为 1-based 展示序号；child minion 缺少 `ReadAllies` capability 时返回结构化错误，避免跨实体读取绕过 capability。
- `MergePolicy::FixedLane` / `DropUnmappedSkills` 已覆盖 custom merge 数据面。
- `RuntimeFrame::render_core_replay` / `render_core_show` 已提供 show 迁移前的最小 golden 面。
- HP marker show renderer fixture 已固化 `hp-bar` payload，保留 `[2]` HP 数值给展示层使用。
- `build_replay_view_frame` 已对 `"[0]还剩[2]点血"` 强制输出 player part `show_hp`，wasm/show 可复用现有结构化 `actorToken` 血条渲染，并通过 `Data` part 标记数值。
- `show-wasm.js` 已新增显式 `buildV2NormalizedReplay()` adapter，可把 wasm `default_custom_runtime_v2_normalized_run()` 的 rounds/actions/frames 转成当前 show-compatible replay shape；当前默认 show 路径已切到该 adapter，legacy `FightSession` 作为显式 fallback 保留。
- `show.html` 默认会调用 v2 adapter 生成可播放 replay，可通过 `engine=legacy` / `runtime=legacy` / `engine=fight_session` 显式回退 `FightSession`，分享链接会保留当前 runtime 选择。
- `show-wasm.test.mjs` 已覆盖纯 adapter 输出和 `buildFrameRows()` HTML chunk 渲染，固定 v2 normalized run 到 show-compatible players / states / rows / clips / sequential HP bar / recover HP bar / multi-target sidebar / winner row / summoned entity first-appearance / removed entity disappearance shape 的最小验收面。
- `show-routing.js` / `show-routing.test.mjs` 已把 URL-safe input、v2 默认、`engine`/`runtime` alias、legacy fallback、非法 input 报错和 runtime 分享链接保留逻辑抽成可单测路由面，降低后续 legacy 删除风险。
- `show-page-contract.test.mjs` 已固定 `show.html` v2 默认页面契约：runtime mode DOM、module script、`show.js` runtime routing、adapter 调用和 runtime 分享链接保留逻辑，作为后续 legacy fallback 删除前的最小页面 wiring golden。
- 最小 custom runner strict-diff golden 已把 spawn、share、heal、HP marker 和 world 派生视图接入同一验收面。
- linked minion owner-death cleanup 已接入 runner strict-diff golden，固定 owner 致死后的消失帧、round/alive 派生视图与 winner 汇总。
- merge 已接入 runner strict-diff golden，固定吞噬/属性上升帧、score 与 fixed-lane 技能槽继承结果。
- multi-round run-until-winner 已接入 runner strict-diff golden，固定逐回合 action/frame、累计 score、winner 和 guard 状态。

---

## 5. 未完成项

- summon 完整内置技能迁移，完整 custom DIY/OL parser 与 CLI/wasm/Python/C API 切换接入（已有 parser-facing summon/shadow/zombie、组合 minion overlay 导入入口、默认 custom v2 profile、core `cli_api` custom v2 profile helper、CLI / C API 默认 normalized-run JSON 入口、Python 默认 normalized-run dict 入口，以及 wasm 默认 normalized-run typed 入口），以及更多内置 minion handler 参数化/strict-diff parity。
- custom fight_multi runner 归一化 golden 已覆盖完整 run-until-winner 终局，后续继续扩展到完整逐回合 replay 行为。
- 将审计表中的每个验收 case 接入 strict diff 或稳定单测。
