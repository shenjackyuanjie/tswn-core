# FeatureEncoder 设计规格

[返回设计索引](README.md)

状态：契约草案，供实施前复核；本轮只写文档。输入 schema v1，拟议 encoder v1。

## 1. 目标与边界

目标是把当前机制状态编码为固定形状张量，供模型输出各输入队伍的最终获胜概率。本文决定字段去向、形状、引用、数值转换、资格 mask、导出和验收契约；不决定网络结构、损失函数、优化器或校准模型。

遵循 [后续训练和网页约束](battle-analyze.md#后续训练和网页约束)：encoder 只在 Rust 实现一次，另加导出通道；原始 Parquet `state` 是唯一事实来源，格式不变，Python 只负责训练与评估。本轮不实现、不训练、不新增 crate、不修改已有数据或 golden/corpus。

下文“规定／拟议”是本规格的新约束，不表示已有 API。`M:行` 指 `crates/tswn_core/src/runtime/model_state.rs:行`；其余缩写在第 17 节给出完整路径。复合类型叶子已在第 3.1 节逐个展开；仍未展开的字段（如槽语义词表）在第 16 节标为待确认，不能据此宣称完整支持已验收。

## 2. 输入契约

仅接收 `BattleModelState`；边界是初始化完成或完整回合结束。已有导出检查默认注册表、效果队列为空，并调用 `validate()`（M:78、M:190；A:26）。encoder 必须再次验证 schema、引用和自身容量；不能从展示 DTO 反推机制状态。

| 根字段 | 分类与用途 | 定义位置 |
| --- | --- | --- |
| `schema_version` | 控制数据，校验 v1，不作为特征 | M:337、M:279 |
| `round` | 当前机制轮次；允许输入，与最终进度不同 | M:338；U:9 |
| `entity_slot_count` | 机制数据，含预留空 ID 槽 | M:339 |
| `input_teams` | 输入队伍成员引用与顺序，定义标签轴 | M:341 |
| `world` | 机制顺序、游标、队伍视图；`winner_team` 只作终局门禁 | M:342、M:364 |
| `entities` | 全体实体，包括死亡者；模板、运行时、状态、槽 | M:343、M:351；U:10 |
| `template_slots`、`battle_slots` | 两个独立命名空间的机制槽 | M:344、M:345、M:420 |
| `legacy_step_scheduler`、`ice_release_events` | 调度模式及有序事件引用 | M:346、M:347；U:22 |

这些字段均不是展示数据。输入外层的 seed、split、progress、帧元数据和最终标签是审计／监督数据，见 G:215 与第 11 节；`world.winner_team` 虽在 state 内，也禁止进入张量。

嵌套结构清单：`ModelEntity`（M:351）、`ModelWorld`（M:364）、`ModelIdentity/ModelImmunity`（M:375/384）、`ModelSkill/Boost/Deferred/Skills`（M:390/400/406/411）、`ModelSlot`（M:420）、`ModelStateEntry`（M:428）、`ModelTemplate`（M:439）、`ModelPayload` 及分支（M:528–625）、`ModelCounter/ModelPlayerRuntime`（M:759/765）。下面按这些结构穷举字段。

## 3. 字段映射表

表中数字是从 0 开始的槽位，闭区间用 `a..b`；`N(f)` 是第 5 节按完整字段路径选取的变换。`cat` 只输出分类 ID，embedding 权重属于后续模型；`ref` 只参与 gather／关系运算，不作为连续标量。`X(path)` 为具名、类型化的扩展叶子记录，格式见第 4 节；尚未进表的叶子映射是发布阻塞项，不能用 JSON 或字符串哈希替代。

| 输入字段（相对于所列结构） | 张量槽位／处理 | 来源 |
| --- | --- | --- |
| 根 `schema_version`、`world.winner_team` | 只校验；前者防错版，后者非空返回 `AlreadyDecided`，均丢弃以防版本捷径／结果泄漏 | M:337、371 |
| 根 `round, entity_slot_count` | `global_num[0]=N_round(round)`；`global_num[1]=entity_slot_count/16`，固定计数尺度见第 5 节 | M:338–340 |
| 根 `legacy_step_scheduler` | `global_num[4] = 0/1` | M:346 |
| 根 `input_teams`、`ice_release_events` | `list` 的 `input_member`、`ice_release` 有序引用；外层队伍长度生成 `team_mask` | M:341、347 |
| 根 `entities, template_slots, battle_slots` | 实体表及 `slot` 表，保留作用域；不以数量代替内容 | M:343–345 |
| World `round_order, team_roster, team_alive, flat_alive` | `list` 的四种有序实体引用，团队列表保留外层 runtime team 轴 | M:365–368 |
| World `alive_group_count, round_pos` | `global_num[2,3]`；原整数另作精确控制字段，计数只作机制特征 | M:369–370；W:110 |
| Entity `id` | 建立 `EntityIdx → e` 字典；原编号不进模型 | M:352 |
| Entity `input_team_index, template, runtime, states, slots` | `entity_team[0]`、`entity_template`、下列 runtime 槽、`state` 表、`slot` 表 | M:353–357、360 |
| Entity `state_registration_cursor` | 精确 `order_key`，用于注册次序与 deferred 的比较 | M:357 |
| Entity `compressed_state_flags` | `entity_flags[0..4]` 依次为 Shield/Protect/Upgrade/Corpse/Minion；`[5..7]` 必须为 0，否则 `ReservedFlagBitSet{path}`；不依赖 states 非空 | M:358–359；U:17 |
| Runtime `hp, attack, magic, magic_point, wisdom, speed, defense, resistance, agility` | `entity_num[0..8]`，依表列顺序逐项 `N(f)` | M:766、768–775 |
| Runtime `at_boost_bits, attr_sum, atk_sum, attract_bits, shield, protect_pre_defend_skill_count` | `entity_num[9..14]`；bit 字段先还原浮点，最后一项的 presence 为 `entity_num_present[14]` | M:776–780、790、793 |
| Runtime `at_boost_millionths` | 丢弃模型通道中的重复近似量；精确旁路保留，优先使用 bits，关系待确认见第 13 节 | M:776–777 |
| Runtime `alive, upgrade_active`；Counter `pending` | `entity_bool[0..2]` | M:767、794、760 |
| Runtime `kind, corpse` | `entity_cat[0,1]`；corpse 词表见第 3.1 节 | M:781、798 |
| Runtime `owner, root_owner, protect_to`；Counter `last_target` | `entity_ref[0..3]`；`entity_ref_present[0,1]` 跟随实体 mask，`[2,3]` 分别表示 protect_to/last_target 的 Option presence | M:782–783、791、761 |
| Runtime `team` | `entity_team[1]` 指 runtime team 关系轴，不作标签 | M:784 |
| Runtime `flags` | `entity_kind_flags[0..5]` 六个具名位，另存原始 u64 精确旁路 | M:785；E:109、113–118 |
| Runtime `policies` | `entity_cat[2..4]` 三个枚举、`entity_bool[6]` | M:786；E:158–163 |
| Runtime `move_state` | `entity_num[15]` | M:787；R:33–35 |
| Runtime `charge` | `entity_bool[3,4]`、`entity_num[16]` | M:788；R:359–363 |
| Runtime `accumulate` | `entity_bool[5]`、`entity_num[17,18]` 加原始 bit 旁路 | M:789；R:366–370 |
| Runtime `hide` | presence `entity_bool[7]`、`entity_num[19..23]`，`attract_bits` 另存旁路 | M:795；R:80–86 |
| Runtime `assassinate` | presence `entity_bool[8]`、`break_on_damage` `entity_bool[9]`、`target` `entity_ref[4]`、`fixed_lane` 走 X 的 lane 引用 | M:796；R:89–93 |
| Runtime `protect_from` | 有序 `list(protect_from)`，`owner` 为 ref、`level` 为该序号的 X num | M:792；R:74–77；M:239–240 |
| Template `id` | 独立 `PlrId` 局内相等关系键，经本地重映射进入 `template_player_ref`；不与 EntityIdx 强行合并 | M:441 |
| Template `reserved_player_ids_before_spawn` | `template_num[13]`，精确计数旁路保留 | M:442；U:14 |
| Template `identity, skills, kind, team` | 下列 identity 槽、`lane` 表、`template_cat[0]`、`template_team` | M:440、443–445 |
| Template `max_hp, attack, magic, magic_point, wisdom, speed, defense, resistance, agility` | `template_num[0..8]` | M:446–454 |
| Template `at_boost_bits, attr_sum, atk_sum, attract_bits` | `template_num[9..12]`，bit 字段先还原浮点 | M:455、457–459 |
| Template `at_boost_millionths` | 与 runtime 同规则，重复近似量不入模型，精确旁路保留 | M:456 |
| Template `move_state, policy_overrides, clone_build` | `template_num[16]`；三个枚举进 `template_cat[2..4]`、`inherit_owner_def_res` 进 `template_bool[3]`、四路 presence 进 `template_override_present`；`clone_build` 见下行及第 3.1 节 | M:460–462；R:33–43；C:70–76 |
| Template `clone_build: Option<CloneBuildData>` | 整体 presence → `template_bool[4]`；None 时 clone 属性数组置 0、`template_num_present[17..30]=0` 且不产生 clone X 记录 | M:462；C:70–76 |
| Template `reuse_skills_on_recast, reuse_stats_on_recast, inherit_owner_def_res` | `template_bool[0..2]` | M:463–465 |
| Identity `clan_group, boss_kind` | `clan_equal` 相等关系矩阵、`template_cat[1]`；boss presence 为 `template_cat_present[1]` | M:377–378 |
| Identity `boss_action_prob_count, boost_immune_threshold` | `template_num[14,15]` | M:379–380 |
| Identity `immunity`；Immunity `status, threshold` | `immunity_num[0..8]`，按 assassinate/charm/berserk/half/curse/exchange/slow/ice/fire 固定轴及 `immunity_present[0..8]`；未知或重复 status 报错 | M:381、385–386、480–495 |
| Skills `lanes`；Skill `skill_id, level, build_level, boost, boosted, fixed_lane_key` | `list(lanes)` 保留数组顺序；`lane_skill_id`、`lane_num[0,1]`、boost 分类及 `[2,3]`、`lane_bool`、`lane_key` 保留每条 lane 的内容，第 7 节展开 | M:390–417 |
| Boost `kind, base, extra` | `lane_boost_kind`、`lane_num[2,3]` 及 `lane_num_present[2,3]`；None 与值为 0 区别 | M:400–403 |
| Skills `merge_lane_order, active_order, pre_action_order, post_damage_order` | 四类执行 `list`；与 `list(lanes)` 合计五类，共用 V 轴，保留原序和重复项并引用所属 template 的 lane | M:413–416 |
| Skills `post_action_after_states`；Deferred `state_cursor, fixed_lane` | `list(deferred)` 的序号、精确 cursor 和 fixed-lane 引用 | M:406–408、417 |
| StateEntry `legacy_order_key, extension_state_id, priority, registration_order, runtime_registration_order` | `state_cat[0,1]`、`state_num[0]`、两个精确 `order_key`；`state_cat_present[0,1]` 分别为真实 state 存在性和 extension Option presence | M:429–434 |
| StateEntry `hook_mask, payload` | `state_hook[0..63]`；kind 分类和通用 payload 槽，见第 8 节 | M:431、435 |
| Slot `slot_id, bool_value, i64_value, u64_value, template` | `slot_index=(scope,owner,slot_id,value_type)`；`slot_id` 经第 4 节重映射，四分支 presence 进 `slot_field_present`；值与语义分派见第 14 节与第 3.2 节白名单，不能按存储类型猜 U64 的含义 | M:420–425、56–70 |
| Payload `kind` 及可空载荷字段（Boss 分支除外） | `state_kind` 与第 8 节逐字段表；不丢弃低频分支 | M:528–625 |

根/world 列表的长度、实体存在数、模板和记录存在性均由对应 mask 表达；额外 `global_num[5]=entities.len()/15`，与 `entity_slot_count/16` 一样只用第 5 节固定计数尺度。嵌套模板通过同一模板表编码，不用第二套属性／技能规则。没有被表中规则消费的已知字段必须导致字段覆盖测试失败。

`entity_mask/template_mask/lane_mask/state_mask/slot_mask/list_mask/extra_mask` 分别标记上述实体、模板、lane、状态、槽、有序引用及 X 记录的实际行；`runtime_team_mask` 标记独立 runtime team 关系行，不能替代 `team_mask`。`entity_num_present/entity_ref_present`、`template_num_present/template_cat_present` 根据上表可空字段及第 3.1 节整体 Option 派生，完整 shape、槽位和父子约束见第 4 节 presence 清单；它们不是新的机制特征来源。

### 3.1 复合类型叶子

上表引用到的复合类型定义不在 `model_state.rs`，逐个叶子如下（`Option` 结构用一个逻辑 presence 表示整体存在；同值族 presence 槽由它派生，不独立决定。嵌套 Option 仍有自己的存在性，压缩位约束一并列出）：

| 来源 | 叶子 | 去向 |
| --- | --- | --- |
| `ModelEntity.compressed_state_flags`（M:358） | 低 5 位为 Shield/Protect/Upgrade/Corpse/Minion，高 3 位保留 | `entity_flags[0..7]` 共 8 位输出；高 3 位必须为 0，非 0 返回 `ReservedFlagBitSet{path}` |
| `PlayerKindFlags`（E:109） | `BOSS`／`MINION`／`SUMMON`／`BED2`／`BOOST`／`COMBAT_MINION`（E:113–118） | `entity_kind_flags[0..5]`；原始 u64 进精确旁路 |
| `PlayerKindPolicies`（E:158） | `owner_resolution`、`damage_share`、`merge` | `entity_cat[2..4]`，词表 1 起（2／3／3 个有效类） |
| 同上 | `inherit_owner_def_res` | `entity_bool[6]` |
| `MoveState`（R:33） | `speed_points` | `entity_num[15]`／`template_num[16]` |
| `ChargeRuntime`（R:359） | `active`、`post_action_active`、`step` | `entity_bool[3]`、`entity_bool[4]`、`entity_num[16]` |
| `AccumulateRuntime`（R:366） | `active`、`acc_bits`、`charge_bonus_bits` | `entity_bool[5]`、`entity_num[17]`、`entity_num[18]`；两个 bit 另存精确旁路 |
| `HideRuntime`（R:80） | `level`、`attract_bits`、`agility`、`defense`、`resistance` | presence `entity_bool[7]`；`entity_num[19..23]`，`attract_bits` 另存旁路 |
| `AssassinateRuntime`（R:89） | `target`、`break_on_damage`、`fixed_lane` | `entity_ref[4]`、`entity_bool[9]`、presence `entity_bool[8]`、X 的 lane 引用 |
| `ProtectLinkRuntime`（R:74） | `owner`、`level` | `list(protect_from)` 的 target；X num（ordinal 为列表下标） |
| `RuntimeCorpseKind`（R:103） | `None`／`Merge`／`Zombie` | `entity_cat[1]`，词表 1／2／3 |
| `PlayerPolicyOverrides`（R:38） | 三个枚举、`inherit_owner_def_res` | `template_cat[2..4]`、`template_bool[3]`；四路 presence 进 `template_override_present` |
| `ModelTemplate.clone_build`（M:462） | `Option<CloneBuildData>` 整体 | `template_bool[4]`；None 时所有后代 presence=0，数组清零，后代 X 记录不存在 |
| `CloneBuildData`（C:70） | `attrs`、`weapon_attr_bonus` | `template_clone_attr`、`template_clone_weapon_bonus`；均由 `template_bool[4]` 控制整体 presence |
| 同上 | `name_factor_bits`、`child_name_factor_bits` | `template_num[17,18]` + 精确旁路 |
| 同上 | `adjustments` | 按下行的 `CloneStatAdjustments` 逐叶进入 `template_num[19..30]`，继承 clone 整体 presence |
| 同上 | `score_skill_boost_plan: Option<ScoreCloneSkillBoostPlan>` | presence → `clone_initial_boosted_mask` 的 X 记录存在性；None 时该模板不产生它及四个 `clone_slot_boost_*` 记录 |
| `CloneStatAdjustments`（C:29） | `max_hp,attack,magic,wisdom,speed,defense,resistance,agility`、`at_boost_delta_bits`、`attr_sum`、`atk_sum`、`attract_delta_bits` | `template_num[19..30]`；两个 `*_bits` 另存旁路 |
| `ScoreCloneSkillBoostPlan`（C:63） | `initially_boosted_mask`、`slot_boosts: [Option<(u8,u8)>;2]` | 计划为 Some 时必有 `clone_initial_boosted_mask` 的 X bits 记录，即使值为 0；每个 `slot_boosts[i]` 为 Some 才同时产生该 i 的两个 X num 记录，None 时该对都不存在 |

`CloneBuildData` 及其两个嵌套类型的字段已公开（`pub`），encoder 直接读取，不需要投影访问器；相关访问器 `derive_stats`（C:182）、`name_factor`（C:208）、`all_sum`（C:211）只作交叉校验。任何复合类型新增叶子都必须同步本表，否则字段覆盖测试失败。

### 3.2 槽语义白名单（默认注册表）

默认注册表只登记 7 个实体槽、3 个全局模板槽和 0 个 battle 槽（J:265–277），而 encoder 只接受该注册表（M:83），所以下表对支持域是封闭的。**同一个 `U64` 存储槽在本仓库里同时表示实体引用、计数和浮点 bit 三种语义**，因此不能按 `SlotValue` 的存储类型推断用途；逐槽核对结果如下。

| scope | slot_id | export_name | 存储类型 | 机制语义 | encoder 通道 |
| --- | --- | --- | --- | --- | --- |
| entity | 0 | `core.entity.shadow_blueprint` | `PlayerTemplate` | 幻影蓝图缓存（预览结果） | `slot_template` → 同一模板表 |
| entity | 1 | `core.entity.summon_blueprint` | `PlayerTemplate` | 使魔蓝图缓存 | 同上 |
| entity | 2 | `core.entity.zombie_blueprint` | `PlayerTemplate` | 丧尸蓝图缓存 | 同上 |
| entity | 3 | `core.entity.lazy_blueprint_rq` | `U64` = `f64::to_bits(eval_rq)` | 延迟蓝图构造用的运行配置值，不是战斗机制状态 | **排除数值**：整份数据同一常量，且存在性已被 `template_bool[4]` 覆盖；若保留只能作控制字段并先 `from_bits` |
| entity | 4 | `core.entity.summoned_entity` | `U64` = `EntityIdx.0` | “记住的召唤物”实体引用 | X 的 `slot_entity_ref`（重映射），**禁止当数值** |
| entity | 5 | `core.entity.minion_counter` | `U64` 计数 | 召唤／分身命名计数器，单调递增 | `slot_value` 走 `N_f`，原值另走 `raw.slot.u64_value` |
| entity | 6 | `custom.bed2.summoned_entity` | — | 已登记但全仓无读写 | 恒缺失；若将来写入按 entity ref 处理 |
| template | 0 | `custom.bed2.summon_template` | `PlayerTemplate` | bed2 使魔模板 | `slot_template` |
| template | 1 | `custom.bed2.shadow_template` | `PlayerTemplate` | bed2 幻影模板 | `slot_template` |
| template | 2 | `custom.bed2.zombie_template` | `PlayerTemplate` | bed2 丧尸模板 | `slot_template` |

证据：槽登记 J:265–277；蓝图槽写入 `K:init:209–223`、`K:seed:146`，读取 `K:seed:169`、`K:summon:179`、`K:zombie:62`、`K:handlers:47/137`；延迟蓝图 rq 写入 `K:init:172–181`，值来自 `K:prepared:312/359` 的 `eval_rq.to_bits()`；记忆召唤物写入 `K:summon:93`、`K:handlers:124`，读取 `K:summon:12–25/45–50`；命名计数写入 `K:summon:283–293`、`K:minions:11–17`，读取 `K:skills_control:360–370`；bed2 模板槽写入 `K:import:203–221`。

未登记的槽、未知 export_name 或存储类型与本表不符时返回 `UnknownSlotSemantics{path}`（第 14 节）；新增槽必须先更新本表并重算 `Q_max`。

## 4. 张量契约

采用 batch-first、C 连续布局，最右轴连续；`B` 是批大小，线上 `B=1`。一个 manifest 冻结所有容量，禁止每批自动改变 shape；以下为拟议 `baseline-32` profile。

下界按实际输出记录数计算，不按去重后的类别数计算。记 `e` 为实体数、`h_b` 为槽内蓝图模板数、`l` 为所有模板的 lane 总数、`s` 为状态条目数；`ceil_pow2(n)` 表示向上取二的幂。表中容量是拟议 profile 的硬预算，不是引擎上限；“已推导”不等于“所有支持域已实测零溢出”。

| 符号 | 拟议固定值 | 下界推导式、依据与限制 |
| --- | --- | --- |
| `E_max`（实体） | 32 | `E_max ≥ e`，且至少覆盖实测最大值 30；`32=ceil_pow2(30)`。P:161 的 p99=15、max=30，数量依据已实测，新增域仍须验收 |
| `T_max`（输入队伍） | 32 | `T_max ≥ input_teams.len()`；预算取 `E_max=32`，容纳每实体一队的目标规模；仅 3 队配置已实测（P:37），不是任意输入的保证 |
| `R_max`（runtime team） | 32 | `R_max ≥` 第 6 节独立重映射后的 runtime team 行数；同样按 32 个关系行预算，不能用 `T_max` 的验收替代，本维峰值待实测 |
| `H_max`（实体模板及槽内蓝图） | 512 | `H_max ≥ e+h_b`。每个 `ModelSlot` 最多含一个模板（M:420–425），按所有槽都可能持有模板的保守结构预算：`e+h_b ≤ 32+(7×32+3+0)=259`，取 `ceil_pow2(259)=512`；不假定每实体恰有三个蓝图，也不按 `PlrId` 去重。结构上界已推导，实际蓝图分布待实测 |
| `L_max`（所有模板的 lane） | 4096 | `L_max ≥ Σ_h templates[h].skills.lanes.len()`，必须含蓝图。保留 4096 的规划预算：以实测实体模板 max 742（p99 422，P:165）及三类实体蓝图入口（M:116–119）作四份规模预留，`ceil_pow2(4×742)=4096`；这不是蓝图 lane 等于实体 lane 的证明，全局槽模板也须计入实际总量，合计峰值仍待实测（S:193–205） |
| `S_max`（全样本状态条目） | 32 | `S_max ≥ Σ_e entities[e].states.len()`，至少覆盖 P:164 的 max=10；取 `max(E_max,ceil_pow2(10))=32`，是一份每实体一条的规划预留，不限制机制只能一条状态；Boss 分支本轮不映射 |
| `Q_max`（机制槽） | 256 | `Q_max ≥ 7×E_max+3+0=227`，取 `ceil_pow2(227)=256`。默认注册表登记 7 个实体槽、3 个全局模板槽、0 个 battle 槽（J:265–277）；这是槽条目数上界，区别于 `entity_slot_count`，已推导 |
| `V_max`（有序列表项） | 32768 | 五类 lane list 的规划下界为 `5×L_max=20480`；取 `ceil_pow2(20480)=32768`，余下 12288 条合并预留给世界列表、states、protect_from、input_member、ice_release、deferred 与注册序记录。完整公式见下文；含蓝图及执行列表重复项的实际峰值仍待实测 |
| `X_max`（扩展叶子） | 65536 | `X_max ≥ 分身计划叶子 + 保护链叶子 + 暗杀 lane + slot refs/bits + 其余 raw 旁路`。按下文已推导的保守记录上界 `V_max+15×H_max+9×E_max+66×Q_max+3×S_max+2=57730`，取 `ceil_pow2(57730)=65536`；原 8192 不能据此宣称覆盖整个拟议 profile，实际占用和内存仍待实测 |

`V_max` 的规划下界写为 `V_max ≥ (每条 lane 的 list 预算记录数 5)×L_max + 世界列表预算 + states 及其注册序预算 + protect_from 预算 + input_member 预算 + ice_release 预算 + deferred 预算 + 实体注册游标预算`；后七项在本 profile 中共用 12288 条预留，未实测前不为各项编造峰值。

五类是 `lanes/merge_lane_order/active_order/pre_action_order/post_damage_order`。这里的 5 是容量预留系数，不是“每条 lane 在四个执行列表中都必然出现一次”的机制断言（M:37–49）。实际编码必须预先求和：`V_required = lane_lists + W + 3×s + P + I + J_ice + d + e`。其中 `lane_lists=Σ_h(len(lanes)+len(merge_lane_order)+len(active_order)+len(pre_action_order)+len(post_damage_order))`；`W` 为四类世界列表的总条目数，`P/I/J_ice/d` 分别为保护链、输入成员、解冰事件、deferred 条目数。每个 state 产生一条 `states` 和两条注册序记录，每实体另有一条注册游标记录；deferred 的 cursor 与其 list 行共用 `order_key`，不再重复生成 cursor 行。已知嵌套团队列表直接以对应团队行作 owner，不额外生成父记录。执行列表保留重复项，不能未经审计就用 `5×l` 代替 `lane_lists`；`V_required > V_max` 一律返回 `CapacityExceeded`。

`X_required` 逐条计费：每个有分身计划的模板最多 `1+2×2=5` 条计划叶子（C:63–66），每条保护链一条 level，每个有暗杀状态的实体一条 fixed-lane 引用；每槽至多一条实体 ref 或 64 条标志 bit，加原始 `slot_id` 与有效整数值至多两条 raw 记录，故保守按 `66×Q_max` 预算。第 14 节 raw 白名单除槽外至多有 `8×e+10×h+3×s+2` 条；因此 `X_required ≤ 5×h+P+e+66×q+8×e+10×h+3×s+2`，再用 `P ≤ V_max` 得到表中 57730。此处没有假定保护链不重复或“每对实体最多一条”。新增字段必须重算预算；可空记录缺失时不计费，校准与真实占用统计仍待完成。

**实测峰值。** `scripts/measure_encoder_capacity.py` 按本节公式统计了 100k 池（800000 样本，2v2v2，`tests/sqp5900.txt`）：

| 指标 | p50 | p99 | max | 当前 profile |
| --- | --- | --- | --- | --- |
| 实体数 `e` | 7 | 15 | 30 | `E_max=32` |
| 模板数 `h`（含蓝图） | 28 | 60 | 120 | `H_max=512` |
| lane 总数 `l` | 246 | 492 | 882 | `L_max=4096` |
| 五类 lane list 条目 | 761 | 1502 | 2678 | — |
| 世界列表 `W` | 24 | 45 | 86 | — |
| 状态条目 `s` | 0 | 4 | 10 | `S_max=32` |
| 保护链 `P` | 0 | 4 | 10 | — |
| 槽条目 `q` | 22 | 48 | 96 | `Q_max=256` |
| `V_required` | 800 | 1569 | 2800 | `V_max=32768` |
| `X_required` | 531 | 1139 | 2271 | `X_max=65536` |

复现命令：`python scripts/measure_encoder_capacity.py --dataset target/winprob-100k --out target/caps-100k.json`（53 s，只读）。
该池只有 2v2v2，峰值不能直接当作支持域上界（第 9 节允许最多 32 个输入队伍）；但它说明当前预算相对已测分布有 5–20 倍余量。
冻结前须在更宽队伍配置和蓝图密集局上重测，再决定保留结构上界还是收紧到实测区间——按实测收紧（例如 `H_max=128`、`L_max=1024`、`Q_max=128`、`V_max=4096`、`X_max=4096`）时同一计费式的足迹约为 0.4 MiB。

上述容量对应的固定缓冲足迹（`B=1`，按第 4 节张量族与 presence 清单逐项相加）约为 4.16 MiB／样本：`extra_*` 合计 2.38 MiB（其中 `extra_index` 1.00 MiB）、V 轴合计 1.22 MiB（`list_index` 640 KiB、`order_key` 256 KiB、`list_position/order_rank` 各 128 KiB）、`clan_equal` 256 KiB，其余各维合计不到 0.4 MiB。因此 `B=64` 时单批约 266 MiB、`B=512` 约 2.1 GiB；训练侧必须按批流式编码，不能把整库样本一次性物化成张量。这些数字是当前 profile 的 padding 预算，随容量或字段增减重算，不表示实际有效值占用。

| 输出张量族 | dtype 与 shape | 内容 |
| --- | --- | --- |
| `global_num` | f32 `[B,6]` | 第 3 节定义的六项 |
| `entity_num`、`entity_bool`、`entity_flags` | f32 `[B,E_max,24]`；u8 `[B,E_max,10]`；u8 `[B,E_max,8]` | 运行时数值、布尔值和压缩状态位；压缩位的高 3 位必须为 0 |
| `entity_kind_flags` | u8 `[B,E_max,6]` | `PlayerKindFlags` 的六个具名位，原始 u64 只进旁路 |
| `entity_cat`、`entity_ref`、`entity_team`、`entity_template` | i32 `[B,E_max,5]`；i32 `[B,E_max,5]`；i32 `[B,E_max,2]`；i32 `[B,E_max]` | 稠密分类 ID、实体引用、两类队伍引用和模板引用 |
| `template_num`、`template_bool`、`template_cat` | f32 `[B,H_max,31]`；u8 `[B,H_max,5]`；i32 `[B,H_max,5]` | 模板数值、布尔值与稠密分类 ID |
| `template_team`、`template_player_ref` | i32 `[B,H_max]`；i32 `[B,H_max]` | runtime team 关系与独立 PlrId 相等关系 |
| `template_override_present`、`template_clone_attr`、`template_clone_weapon_bonus` | u8 `[B,H_max,4]`；u32 `[B,H_max,8]`；i32 `[B,H_max,8]` | policy overrides 四路 presence、分身属性与武器加成；clone 整体存在性见 `template_bool[4]` |
| `immunity_num`、`clan_equal` | f32 `[B,H_max,9]`；u8 `[B,H_max,H_max]` | 免疫阈值与阵营相等关系 |
| `lane_skill_id`、`lane_boost_kind`、`lane_template`、`lane_key` | i32 `[B,L_max]`；i32 `[B,L_max]`；i32 `[B,L_max]`；i32 `[B,L_max]` | 技能／boost 分类、模板引用及模板内重映射的 fixed key |
| `lane_num`、`lane_bool` | f32 `[B,L_max,4]`；u8 `[B,L_max,1]` | 等级、构建等级、base、extra；boosted |
| `state_entity`、`state_kind`、`state_cat`、`state_hook` | i32 `[B,S_max]`；i32 `[B,S_max]`；i32 `[B,S_max,2]`；u8 `[B,S_max,64]` | 归属、payload kind、legacy/extension 稠密分类、hook bit；kind 的 0=PAD、真实 none=1 |
| `state_num`、`state_ref`、`state_group` | f32 `[B,S_max,9]`；i32 `[B,S_max,4]`；i32 `[B,S_max,3]` | priority + 8 payload 数值槽；4 个实体引用；3 个阵营／队伍关系键 |
| `slot_index`、`slot_value`、`slot_template` | i32 `[B,Q_max,4]`；f32 `[B,Q_max,1]`；i32 `[B,Q_max]` | index=(scope,owner,slot_id,value_type)；`slot_id` 是作用域内稠密类，`value_type` 四分支见第 14 节；数值/bool 与模板引用分流 |
| `list_index` | i32 `[B,V_max,5]` | (owner_scope,owner,field_class,ordinal,target)；field_class 使用第 14 节同一冻结字段表的 list 编号段，target 域由该表固定 |
| `extra_index`、`extra_num`、`extra_ref`、`extra_bool`、`extra_cat`、`extra_bits` | i32 `[B,X_max,4]`；f32 `[B,X_max]`；i32 `[B,X_max]`；u8 `[B,X_max]`；i32 `[B,X_max]`；u32 `[B,X_max,2]` | X 的 (owner_scope,owner,field_class,ordinal) 及互斥类型值；field_class 使用第 14 节同一冻结字段表的 X/raw 编号段，bits 仅精确旁路 |
| `order_key` | u32 `[B,V_max,2]` | 与对应 list 项对齐；第 3 轴 0=lo、1=hi。registration_order 为 u32，hi 恒 0；runtime_registration_order/state_registration_cursor/deferred.state_cursor 按 u64 拆分，见第 13／14 节 |
| `list_position`、`order_rank` | f32 `[B,V_max]`；f32 `[B,V_max]` | 原列表位置与精确顺序键的归一化稠密秩；仅按已定义比较域消费 |
| `runtime_team_mask` | u8 `[B,R_max]` | runtime team 关系行存在性，不表示当前存活／未来获胜资格 |
| `team_mask` | u8 `[B,T_max]` | 输入标签位置存在性；预测结果 f32 `[B,T_max]` 由后续推理端产生，不是 encoder 输出 |

行存在性张量逐名为：`entity_mask` u8 `[B,E_max]`、`template_mask` u8 `[B,H_max]`、`lane_mask` u8 `[B,L_max]`、`state_mask` u8 `[B,S_max]`、`slot_mask` u8 `[B,Q_max]`、`list_mask` u8 `[B,V_max]`、`extra_mask` u8 `[B,X_max]`。每个可空值的 presence 由下表唯一确定；数值 0、None 和 padding 是三种情况。bool 只取 0/1；分类 PAD=0，有效类从 1 起；引用有效值从 0 起，缺失／padding 填 -1，gather 必须先检查 presence 和目标 mask。

**presence 清单。** 表中的下标是最后一轴槽位，省略 batch 与所属行下标；所有 presence 均为 u8 0/1。父行 mask=0 时全部后代 presence=0；同一逻辑存在性在值族中的派生槽必须相等，不能由实现自行选择另一套判定。

| 源字段／作用 | presence 张量与 shape | 槽位及判定规则 |
| --- | --- | --- |
| `runtime.counter.last_target`、`runtime.protect_to`；owner/root_owner/assassinate.target | `entity_ref_present` u8 `[B,E_max,5]` | `[3]`、`[2]` 分别取两个 Option 的 is_some；`[0,1]` 等于 entity_mask，`[4]` 等于 `entity_bool[8]` |
| `runtime.protect_pre_defend_skill_count` 及 runtime 数值族 | `entity_num_present` u8 `[B,E_max,24]` | `[14]` 取 is_some；`[19..23]` 等于 `entity_bool[7]`；其余槽等于 entity_mask，真实 0 不表示缺失 |
| `runtime.hide` | 复用 `entity_bool` u8 `[B,E_max,10]` 的 `[7]` | 整体 is_some，同时控制 hide 五个数值与 `raw.runtime.hide.attract_bits` 记录 |
| `runtime.assassinate`、`runtime.assassinate.fixed_lane`（X） | `entity_bool` u8 `[B,E_max,10]` 的 `[8]`；`extra_mask` u8 `[B,X_max]` | 整体 is_some；fixed_lane 本身不是 Option（R:89–93），整体为 Some 时必须恰有一条 `assassinate_fixed_lane` X 记录，None 时无该记录 |
| `template.clone_build`；clone 数值与两个属性数组 | `template_bool` u8 `[B,H_max,5]` 的 `[4]`；`template_num_present` u8 `[B,H_max,31]` | clone 整体 is_some；`template_num_present[17..30]` 及两个 clone 数组的整体 presence 均由它控制；`[0..16]` 等于 template_mask |
| `clone_build.score_skill_boost_plan` 及 `slot_boosts[i]` | 复用 `extra_mask` u8 `[B,X_max]` | 计划 presence 由 `clone_initial_boosted_mask` 记录存在性表达；每个 `slot_boosts[i]` 的两个记录同时存在／缺失，不能只存一个叶子 |
| `identity.boss_kind` 及模板分类族 | `template_cat_present` u8 `[B,H_max,5]` | `[1]` 取 boss_kind.is_some；`[0]` 等于 template_mask；`[2..4]` 等于下面 override presence 的 `[0..2]` |
| `identity.immunity[]` | `immunity_present` u8 `[B,H_max,9]` | 按第 3 节九个 status 固定轴，记录出现则为 1，阈值 0 仍为 1；缺项为 0，未知或重复 status 拒绝 |
| `policy_overrides` 四路 | `template_override_present` u8 `[B,H_max,4]` | `[0..3]` 依次为 owner_resolution/damage_share/merge/inherit_owner_def_res 的 is_some；最后一路控制 `template_bool[3]`，Some(false) 仍 present=1 |
| `skills.lanes[].boost` | `lane_num_present` u8 `[B,L_max,4]` | `[0,1]` 等于 lane_mask；`[2,3]` 等于 boost.is_some；boost=None 时 lane_boost_kind=1，不是 PAD |
| `state.extension_state_id`、`legacy_order_key` | `state_cat_present` u8 `[B,S_max,2]` | `[1]` 为 extension_state_id.is_some；`[0]` 等于 state_mask；Some(0) 与 None 区分 |
| `state.priority`、各有效 payload 数值 | `state_num_present` u8 `[B,S_max,9]` | `[0]` 等于 state_mask；`[1..8]` 仅对第 8 节当前 kind 实际使用的字段为 1，其他为 0 |
| `poison.caster/target`、`charm.target`、`charm.group_id` | `state_ref_present` u8 `[B,S_max,4]` | poison 的 `[0,1]` 分别取 is_some；charm 的 `[0]` 取 `target.is_some()`、`[1]` 恒为 1（`group_id` 不可空）；其他未使用槽为 0，不能仅凭 kind 把可空引用全部置 1 |
| `charm.group_id/effective_team_idx/source_team_idx` | `state_group_present` u8 `[B,S_max,3]` | charm 的 `[0]` 必为 1；`[1,2]` 分别取两路 Option 的 is_some；非 charm 行全部为 0 |
| `slot.bool_value/i64_value/u64_value/template` | `slot_field_present` u8 `[B,Q_max,4]` | 依此顺序逐项 is_some；真实槽行恰一项为 1，padding 全 0；与第 14 节 value_type 一一对应 |
| `slot_value` 的数值／bool、`slot_template` 的模板引用 | `slot_value_present` u8 `[B,Q_max,1]`；`slot_template_present` u8 `[B,Q_max]` | 前者仅在标量或 bool 语义分派时为 1，ref/标志 bits 分派为 0；后者等于 `slot_field_present[3]` |
| 精确顺序键及其可比较秩 | `order_key_present` u8 `[B,V_max]`；`order_rank_present` u8 `[B,V_max]` | 第 14 节指定含键的 list 行才有前者；存在当前实体比较域时才有后者，非实体蓝图的 deferred 不编造实体秩；其他行及 padding 为 0 |
| 每个 X 记录自身（包括 raw） | 复用 `extra_mask` u8 `[B,X_max]` | 每条实际记录为 1；可空源缺失时不创建记录，padding 为 0；有效零值仍创建记录，类型由 field_class 决定，不另设含糊的 extra_present |

`Option` 缺失时数值／分类／bit 清零，引用填 -1；该填充值不能取代 presence。没有列出额外 presence 的必填字段直接受所属行 mask 控制；按第 8 节 kind 选择的 payload 是有类型的分支，不允许 payload 数值为 0 时被当成“没有此分支”。

**分类落域与词表。** 分类通道只存局内按冻结词表重映射后的稠密分类 ID（0=PAD／缺失占位，有效类从 1 起）；原始 u32／`u32::MAX` 只进精确旁路 `extra_bits` 的 `raw.<字段路径>`。即 **cat = 稠密 ID，原始值只走旁路**，禁止把原值直接强转 i32。

“局内重映射”指每局按同一个 manifest 查表，不是按该局出现顺序临时给类编号；稠密的是完整词表的 `1..K`，单局可以缺少部分有效类。否则同一 embedding ID 会跨局变义。各分类域独立编号、`K ≤ i32::MAX`；缺词表返回 `MissingVocabulary{path}`，未知原值返回 `UnknownCategory{path}`，不得将未知值折叠为 0。固定枚举按下表显式映射，不使用 Rust 判别值强转；有效枚举 `None` 与 `Option::None` 是两回事。

| 分类槽／来源 | 词表来源与映射 | 原始 0、缺失和精确旁路 |
| --- | --- | --- |
| `entity_cat[0]`、`template_cat[0]`：`PlayerKindId` | 共用默认规则 player-kind 注册表加有效哨兵 `u32::MAX` 的词表；manifest 冻结原始 u32→稠密 ID，不根据 P:172 的频次猜类别名称 | 原始 0 若为注册类就映射到正 ID，否则报未知类；`u32::MAX` 必须映射到正 ID，不能成为 -1/PAD。原值分别走 `raw.runtime.kind`、`raw.template.kind`，lo 存 u32、hi=0 |
| `entity_cat[1]`：`RuntimeCorpseKind` | R:103–107：1=`None`、2=`Merge`、3=`Zombie` | 枚举 `None` 是真实类 1，不是缺失；0 只用于 padding，不读取隐式判别值 |
| `entity_cat[2]`、`template_cat[2]`：`owner_resolution` | E:135–156 的具名枚举：1=`SelfEntity`、2=`RootOwner`；runtime 与 override 共用词表 | runtime 必有值；template 的 `Option::None` 为 0/presence=0，有效默认枚举仍为正 ID |
| `entity_cat[3]`、`template_cat[3]`：`damage_share` | 同源枚举：1=`None`、2=`ShareToOwner`、3=`ShareToSummons` | `Some(DamageSharePolicy::None)` 为 1/presence=1，不能与 override 缺失混同 |
| `entity_cat[4]`、`template_cat[4]`：`merge` | 同源枚举：1=`None`、2=`FixedLane`、3=`DropUnmappedSkills` | `Some(MergePolicy::None)` 为 1/presence=1，不能与 override 缺失混同 |
| `template_cat[1]`：`identity.boss_kind` | M:477 按 `crate::namerena::BOSS_NAMES` 的位置投影；manifest 冻结该序列，原始位置 k→类 k+1，不把名字文本送入模型 | `Some(0)` 为类 1/presence=1，`None` 为 0/presence=0；原始位置走 `raw.template.identity.boss_kind`，按 u64 拆分，不先转 i32；这不恢复 Boss 载荷支持 |
| `state_cat[0]`：`legacy_order_key` | 默认规则状态构造点的 legacy-key 白名单，按原始 u32 升序分配 `1..K` 并冻结；M:429 只提供类型，完整白名单仍须按第 16 节审计后写入 manifest | 有效原值 0 也是正 ID；该字段不可空，真实 state 的 presence=1；原值走 `raw.state.legacy_order_key`，hi=0，不与精确注册序字段混用 |
| `state_cat[1]`：`extension_state_id` | 默认规则 `register_state` 登记的状态 ID 词表；按登记 ID 升序生成并冻结，不与 legacy key 或 payload kind 共用词表 | `Some(0)` 映射到正 ID/presence=1，`None` 为 0/presence=0；有效原始 u32 走 `raw.state.extension_state_id`，hi=0 |
| `slot_index[...,2]`：`slot_id` | 按作用域分别从默认注册表的 entity/template/battle 槽表生成原始 u32→稠密 ID；当前有效原值分别为 0..6、0..2、空集（J:265–277） | 各域原始 0 映射到该域类 1；scope 和稠密 `slot_id` 必须一起使用；原值走 `raw.slot.slot_id`，hi=0，禁止把原始 u32 直接塞进 index |

`state_kind` 的独立词表见第 8 节（0=PAD、真实 `none`=1），`lane_skill_id/lane_boost_kind` 见第 6／7 节，`extra_cat` 的分类域由第 14 节字段表指定。第 4 节的 presence 清单负责区分可空分类的 0 与 padding；标识符／关系引用不因本表而变成可学习的数值编号。

所有 padding 数值、分类、bit 均置 0；引用置 -1；mask=0 的位置不参与聚合、注意力或 softmax。死亡实体 mask=1、alive=0；预留空实体 ID 槽不生成实体行。实测槽数最大 31 与实体最大 30 不可混同（P:161–163）。

所有列表保留重复项与空列表边界；空列表由 owner 存在且该 field 无项表示。`list_position=ordinal/max(1,有效列表长度-1)`。`order_rank=rank/max(1,不同键数-1)`：在同实体、同注册序域内对原整数精确排序，相等键同秩；runtime_registration_order、state_registration_cursor、deferred.state_cursor 共享比较域，registration_order 单独成域。这些是当前结构的无量纲关系变换，不依赖 padding 上限或训练分布；精确比较域的消费者仍须按第 16 节核对。

状态原数组序进入 `list(states)`；两套状态注册序与实体注册游标进入第 14 节对应的 `list(order_*)`，target 分别引用状态或实体；deferred.state_cursor 直接附着于 `list(deferred)`，不另建重复记录。slot 等额外原始整数与浮点 bit 使用 `extra_bits` 的专用 `raw.<字段路径>` 类运输；它与同源 num/cat 行是不同记录。本节分类词表允许的原始机制分类值可以走旁路，原始 EntityIdx、PlrId、clan 身份编号及第 11 节禁用字段仍不得加入该类；不能把这项例外扩成任意编号运输。

任一维超限返回 `CapacityExceeded {path, actual, limit}`，禁止截断实体、lane、状态或蓝图。离线按 split／队伍数／容量维度报失败率，Boss 分支按不支持计入失败率，线上不给伪概率；扩大容量须发布新 profile，不能把 padding 容量变成模型语义。

## 5. 归一化与数值域

现有测量仅支持数量尺度；S:187–209 没有收集 HP、等级、属性、payload 标量或蓝图分布。不能把“HP/100”“技能等级/255”写成实测结论，也不能把 P:152 的最终轮数 167 当当前 state 的固定上限。

| 标量类别 | 规定的变换、裁剪与依据 |
| --- | --- |
| 实体数、实体槽数 | `x/15`、`x/16`；裁剪分别 `[0,32/15]`、`[0,64/16]`，依据 P:161、163 的 p99；64 是槽数建议支持边界，超限报错 |
| 当前存活数（若聚合使用） | `x/11`，裁剪 `[0,32/11]`；P:162 的 p99=11，不能反过来构造资格 mask |
| 状态数、实体模板 lane 数（若聚合使用） | `x/4`、`x/422`，裁剪 `[0,32/4]`、`[0,4096/422]`；P:164–165。含蓝图的 lane 总量当前仅做容量校验，尚无独立计数特征；未来增加时须另实测固定尺度，不自动归入 N_f |
| 逐字段标量：HP、属性、护盾、等级、boost、`round`、`world.alive_group_count`、游标位置、概率阈值、payload 数值，以及其余已映射标量字段 | 每个完整字段路径分别拟合 `s_f=max(1,Q50(abs(x)))`，`c_f=max(1,Q99(abs(x)))`；`N_f(x)=clip(sign(x)*ln(1+abs(x)/s_f)/ln(1+c_f/s_f),-4,4)` |
| 已还原的有限浮点系数 | 同一 `N_f`，不能直接把 u64 bits 转 f32；保留正负值，不假定攻击或 HP 必定非负 |
| bool、分类、关系、bit、精确注册序 | bool 0/1；分类不归一化；关系用引用；bit 展开／旁路；注册序精确比较，禁止浮点排序 |

这里按字段来源而非“是否是整数／字段名是否含 count”切分：**机制计数的绝对量**仅指实体数、实体槽数、状态数、实体模板 lane 数、当前存活数，使用上述固定线性尺度；**逐字段标量**使用各自拟合的 `N_f`。`alive_group_count` 是后者的粘性机制字段，不等于从当前存活列表重算的存活数量。`global_num[1,5]` 属于前者，`global_num[0,2,3]` 属于后者，`global_num[4]` 是 bool；任一数值槽只归一类，禁止先线性缩放再套 `N_f`。`list_position/order_rank` 是第 4 节单独定义的结构关系量，不归入这两类标量统计。

所有拟合仅使用已决 train 行中存在的值，不含 padding/None；按字段排序后用 `round((n-1)*q)` 取分位数，与 S:56–59 一致。模板和 runtime 即使同名也各自统计；`round_pos=-1` 若出现作为真实有符号数保留，不充当缺失值。

P:160 的样本轮数 p99=61、max=146 来自 `SampleRow.rounds_advanced`（S:186），尚未核对它与 `state.round` 的偏移关系；它只提示采样范围，不能直接给 `state.round` 定归一化常数。

拟议共享工件 `encoder-manifest.json` 保存 schema/encoder/profile 版本、完整字段表、分类词表、容量、每字段 `s_f/c_f`、样本数、min/max/p50/p99、裁剪率和来源摘要。由 Rust 校准／编码通道生成并读取；训练、原生推理、WASM 绑定加载同一工件及摘要，Python 不拟合或覆盖常数。

没有观测值的字段不能悄悄令 `s_f=c_f=1`，也不接受登记人工常数：只能补足 train 观测后重新拟合，否则返回 `MissingCalibration{path}`。所有有效 f32 必须有限，NaN/Inf 返回 `NonFiniteValue`；裁剪只作用数值特征，不改引用、mask、原始 bit。当前缺少上述标量统计，因此本稿冻结公式和生成方法，数值 manifest 的发布仍待校准。

## 6. 索引与 ID

`EntityIdx` 是引用键（A:30），按 `entities` 行建立稠密字典；重复 ID／悬空 ref 拒绝，引用重映射涵盖世界列表、owner/root_owner、保护、暗杀、毒、魅惑及 Boss（M:197–273）。原 ID 的数值大小既不归一化，也不送 embedding；ID 槽总数和预留出生计数独立保留。

| 编号空间 | 规定 |
| --- | --- |
| 输入队伍 `t` | `0 ≤ t < input_teams.len()`，是输出轴与 `winner_team_index`；不学习绝对 t 的 embedding |
| runtime team `r` | `template.team/runtime.team` 与 `world.team_*` 的关系轴；建立独立映射，禁止强制 `r=t` |
| charm 临时阵营 | `group_id` 为局内关系键；`effective_team_idx/source_team_idx` 各保留独立可空关系，不能改写输入队伍归属（M:579–584；D:44） |
| `clan_group`、`PlrId` | 前者仅相等关系；`PlrId` 是实体槽下标 + 1 的派生值（`K:entity_runtime:732`），只作局内键，不进模型、不送 embedding。初始行动顺序虽由名字派生键排序（`K:roster:386–425`），但结果已完整落在 `round_order`／`team_roster`／`runtime.team` 中，encoder 只读这些列表，不需要名字或 `PlrId` 数值 |
| `skill_id` | 固定 1–50，当前 1–42，43–50 预留；0 只作 PAD（M:281–325）；遇到未启用预留项报错 |
| fixed lane、legacy key、extension、slot ID | 分离词表；fixed lane 只在所属模板内解引用，slot ID 必须同时携带作用域（M:391、420、428） |

技能编号例：`core.skill.revive` 是 `MODEL_SKILL_EXPORTS` 第 22 项（M:305），不能照搬 W:163 的 legacy 技能 ID 16。模板 kind 的 `u32::MAX` 是实测有效类（P:172），必须经第 4 节重映射进 cat 通道，禁止直接塞进 i32，也不能当作 -1/PAD；原始 u32 只走具名精确旁路。其余 kind 词表按已审计规则冻结，基线未出现不等于非法。

## 7. 技能槽方案

采用“每个真实 lane 一行、全样本打包”的方案；不分配 `[E_max,50]` 单等级表，不按 skill_id 合并重复 lane。P:165、180 的中位 211／均值 241.6／最大 742 条说明这一块需单独预算，而 42 个技能类别不构成 lane 数量上限。

`lane_skill_id` 供分类 embedding；`lane_num=[level,build_level,boost.base,boost.extra]`，四项分别按第 5 节归一化。`lane_bool=boosted`，`lane_boost_kind` 的 0=PAD、1=None、2=normal、3=last_boost、4=slot_boost；`lane_num_present[2,3]` 在 boost=None 时为 0，Some 时即使 base/extra 为 0 仍为 1；`[0,1]` 跟随 lane_mask（M:15–37）。

每行带 `lane_template`；模板内原 lane 序号仅由该 lane 唯一一条 `list(lanes)` 记录的 `ordinal` 表示，不另设序号张量，也不从全样本打包行号推断原序。该记录的 owner 必须等于 `lane_template`，target 指向 lane 行；这是归属校验与显式有序关系，不是另复制一份 lane 内容。`fixed_lane_key` 建立独立局内键表；四种执行列表和 deferred 通过类型化引用连接 lane，不拿 skill_id 替代 lane。源字段分别见 M:37–49、413–417；**执行列表的整数已核对为同一模板内 lane 数组的下标**（`K:scheduler:257–275`、`K:round:640–670` 都直接用它索引 `skills()` 与 `level_at()`），不是 `fixed_lane_key` 的值；`fixed_lane_key` 只用于跨局稳定键（`lane_key`），两者不能互换。

deferred 保存原列表顺序和精确 `state_cursor`，与实体状态注册序生成比较关系。对于非实体模板（槽内蓝图），没有当前实体状态时不得编造关联。lane 的 PAD 行 level=0、mask=0；真实零等级 lane 的 mask=1。未知技能、未解析 fixed key、顺序引用越界均拒绝。

嵌套蓝图的技能仍进入同一 L 轴，引用不同模板行；不得重复调用 Python 编码器或将模板序列序列化为整体 JSON embedding。基线 742 不覆盖这些蓝图 lane，冻结前必须补测合计峰值和内存。

## 8. 状态载荷方案

`state_entity` 指向承载该 states 条目的实体行；`state_num[0]` 是 priority，以下 `p0..p7` 对应 `[1..8]`；`r0..r3` 对应 `state_ref[0..3]` 的实体引用（charm 的 `group_id` 也走这里，见下表），`g0..g2` 对应 `state_group[0..2]` 的关系键，当前只有 charm 的 `effective_team_idx`／`source_team_idx` 使用 `g0,g1`。`state_num_present/state_ref_present/state_group_present` 与三个值族同 shape；数值 unused=0/presence=0，引用和关系键 unused=-1/presence=0。kind=0 为 PAD，真实 `none`=1。投影分支由 M:628–753 穷举，规定词表如下。

| kind 编号／字符串 | payload 字段 → 槽位 | 来源 |
| --- | --- | --- |
| 1 `none` | 无 payload；真实条目 mask=1 | M:632 |
| 2 `fire_mag_half_steps` | 同名整数 → p0 | M:530、635 |
| 3 `ice` | `frozen_step` → p0 | M:548–550 |
| 4 `shield_value` | 同名整数 → p0 | M:532、645 |
| 5 `curse` | `prob,multiply` → p0,p1；不假定 prob 已是 `[0,1]` | M:553–556 |
| 6 `poison` | `caster,target` → r0,r1；`atp_bits` 还原后 → p0；`count` → p1 | M:559–564 |
| 7 `haste` | `faster,effective_faster,step` → p0,p1,p2 | M:567–571 |
| 8 `berserk` | `step` → p0 | M:574–576 |
| 9 `charm` | `group_id` → r1（**它是施法者的 `EntityIdx`，必须按实体引用重映射**）；`target` → r0；`effective_team_idx,source_team_idx` → g0,g1（runtime team 关系键）；`step` → p0 | M:579–585；K:skills_team:208–219 |
| 10 `slow` | `step` → p0 | M:588–590 |
| 11 `iron` | `protect,step` → p0,p1 | M:593–596 |
| 12 `covid_boss` | 本轮不映射（Boss 专属）；遇到该 kind 返回 `UnsupportedPayloadKind` | M:599–601 |
| 13 `covid_infection` | 本轮不映射（Boss 派生感染）；同上 | M:604–608、260–263 |
| 14 `saitama_boss` | 本轮不映射（Boss 专属）；同上 | M:611–616 |
| 15 `lazy_boss` | 本轮不映射（Boss 专属）；同上 | M:619–621 |
| 16 `lazy_infection` | 本轮不映射（Boss 派生感染）；同上 | M:624–625 |

禁止把任何子列表平均池化成一个数量；子 list 共享 V 容量并以状态行作 owner。Boss 分支本轮不映射，因此 `CovidInfectionEntry`（Y:17）与两个 Boss 载荷的叶子不需要张量槽位；一旦恢复 Boss 支持，必须按第 3.1 节的粒度补齐，不能静默省略。

校验 kind 与唯一载荷分支匹配；`none` 不得带分支，其他 kind 必须恰有对应分支；错误返回字段路径。`state_cat_present[0]` 跟随 state_mask、`[1]` 表示 extension_state_id 的 Option；状态条目原序、legacy key、hook、priority、两套注册序与实体注册游标共同保留。`compressed_state_flags` 独立编码，因为 U:17 明确压缩 Shield/Protect/Upgrade/Corpse/Minion 不一定出现在 entries。

## 9. 变长与特殊实体

普通池每样本 6–30 实体、存活 2–20（P:161–162），不允许只取六个本体或只取存活者。实体消失／新增／复活改变 mask 或 alive，但输入队伍归属沿用 root_owner 的映射（M:107–111、229–234）。clone/summon/shadow/zombie 为已列出的动态实体类型（API:59、69），不根据名字识别。

模板 kind 的实测计数为 `u32::MAX:4800000, 5:703995, 2:440852, 4:101630, 3:49020`（P:172–173）；只证明本池出现本体与 2–5，未证明数字与四种 minion 名称的对应次序。该对应关系待确认，不按频次猜类别。

允许 2–32 个输入队伍，超过 32 明确报容量错误；各队人数可以不同。3 队胜者计数 `34267/33381/32352` 只是该生成分布（P:167），不能硬编码先手先验或固定三输出。

内置 Boss **本轮明确不支持**：`boss_kind`、`boss_action_prob_count`、`boost_immune_threshold` 这类 identity 字段仍按第 3 节映射（成本极低，且避免字段覆盖测试失败），但 Boss 专属载荷（感染／一拳／懒惰）不映射，遇到即报错；容量与分布统计不以 Boss 覆盖为冻结条件。已支持的 DIY 仍在范围内（A:11），执行失败按 D:64 处理。第三方注册表仍按 M:83 拒绝。

蓝图只使用 state 已导出的模板；M:130–141 的预览和 U:35 的一层派生边界不能扩展成 encoder 自行模拟未来出生。本稿覆盖这些入口的表示方案；恢复 Boss 支持时必须先补第 16 节的容量与数值审计，不能以「基线未出现」当作可忽略。

## 10. team_mask 与获胜资格

规定 `team_mask[b,t] = 1 ⇔ t < state.input_teams.len()`，其余为 0；要求输入队伍非空且至少两队。此 mask 表示输入标签位置存在，不是“未来可复活性已经证明”。保守保留所有真实输入队伍，不作不可复活假设，也不尝试求解复活／召唤可达性。

判胜依据以 [判胜语义](../mechanics/winner.md) 为准：恰好一支 runtime 队伍仍有存活实体才有胜者，全灭无胜者（W:19–45）。复活使“当前零存活”不能成为永久出局证明（W:159–177）；因此禁止 `runtime.alive > 0`、本体 alive、`team_alive` 非空或 `alive_group_count` 作为 team_mask。

当前 alive、存活列表和粘性计数仍是机制输入；W:119、128 记载计数参与目标选择及 post-action 循环，不能因为它不是判胜计数就删掉。标签按 G:202–205 的唯一输入队伍映射读取；空标签不标负，不增加第 33 个“未决”类别。

后续模型只对 mask=1 的 t 做归一化：`p_t=exp(z_t-m)/Σ_valid exp(z_j-m)`；padding 概率为 0，真实队伍概率和为 1。`world.winner_team!=None` 时 encoder 返回 `AlreadyDecided`；终局展示由引擎结果驱动，不把终局送模型。全灭但尚无 winner 的有效 state 可以编码，监督训练仍默认过滤空标签（A:9）。

## 11. 禁止泄漏清单

编码函数签名只接受 state 与冻结配置，采用字段白名单；数据适配器把 labels 和审计键放在独立通道。已存在的生成／读取边界见 A:28、67，G:211–223，D:99；下表规定新增 encoder 的第二道阻断。

| 禁止项 | 禁止理由 | 阻断层 |
| --- | --- | --- |
| 主 seed、战斗 seed、抽样 seed、RC4／RNG checkpoint | 暴露未观察随机性或采样身份 | state 导出边界（U:41）＋encoder 无此参数 |
| `progress`、最终轮数／帧数、进度桶 | 依赖最终停止时间，在线当前帧不可得 | Rust 数据适配器只将 state 交 encoder；G:221、F:20 |
| `winner_team_index`、最终 winner、结果摘要／终止原因 | 直接标签或其代理；截断不是负类 | label 独立文件／buffer，禁止拼接 features |
| `state.world.winner_team` | 当前已决状态会成为答案捷径 | encoder 终局门禁；已有 G:214 也拒绝 |
| split、matchup_id、battle_id、case_index、分片和行号 | 识别集合／阵容／相邻样本，形成记忆捷径 | 适配器路由和审计 sidecar；不进入张量清单 |
| 原始输入、显示名、名字／阵容哈希、clan 文本 | 记忆身份与集合；破坏机制输入约束 | U:27、A:28 的投影边界＋词表只接受已审计机制分类 |
| 原始 EntityIdx、PlrId、clan 编号的数值／哈希 embedding | 分配或身份编号不具备连续距离语义 | encoder 局内引用重映射／相等关系 |
| 未来帧 state、未来生成实体、预取后的 session 状态 | 与用户正在观看的帧错位，使用未来信息 | 推理结果在对应帧生成时保存；第 14 节播放关联 |
| frame/generation ID、日志、图标、展示 DTO、缓存 ID、内存地址 | 展示／生命周期信息，不是独立战斗机制 | API 适配器隔离；U:43–48 |

允许 state.round、机制上已派生的免疫阈值／Boss 身份／蓝图属性／阵营相等关系；它们来自当前机制（U:9、27–35），不等于允许输入名字、名字哈希或未来总轮数。split 可用于选择训练集合与拟合常数，但不能是编码函数的参数或模型特征。

## 12. 置换语义

以下不变性只针对同一个 state 的合法重表示；不能把重新排列原始输入再运行引擎视为同一局面（A:30、54–56）。

| 操作 | 输出规定 | 原因／处理 |
| --- | --- | --- |
| 仅重排 `entities` 存储数组 | 队伍概率不变，实体张量及引用同步等变 | EntityIdx 解引用保持机制关系；世界列表不动（M:197–223） |
| 仅重标输入队伍轴及全部对应引用／标签 | 概率按同一置换等变；撤销置换后不变 | 不学习绝对输入索引；输入成员的机制顺序必须保留 |
| 重排原始输入队伍／队内成员再跑战斗 | 敏感，允许概率变化 | A:54 明确保留输入顺序；不强制消除先手效应 |
| 改 `round_order/round_pos`、roster／alive 列表内部顺序 | 敏感 | U:12 指明影响行动／目标选择，list 的 ordinal 是机制输入 |
| 改 lane／hook 执行列表、状态注册序、保护／Boss 子列表顺序 | 保留差异，不强制不变 | U:16–19；未确认无序语义的列表不擅自排序去重 |
| 一致重标 clan／引用关系键 | 不变 | 只用相等／引用关系；不承诺原始实体 ID 改号是引擎对称性 |
| 增加 padding、换 batch 邻居 | 由 mask 吸收，真实结果不变 | 各张量容量不参与模型评分；不同 profile 的共同有效区语义一致 |

引用索引、scope/field 分类和列表 ordinal 的职责必须分开：ref 只能 gather，field_class 可 embedding，ordinal 表达顺序。不得让模型直接把某实体恰好占稠密第 0 行当成先手；先手信息来自显式世界顺序。

## 13. 浮点与 bit 精度

数值模型通道统一 f32：HP／属性／计数／等级／payload 标量先在 Rust 用 f64 计算第 5 节变换，再按 IEEE 最近偶数舍入到 f32。整数比较、索引解析和 mask 判定必须在此之前完成；不能从已裁剪 f32 还原原始状态。

| 字段类别 | 精度与还原规则 |
| --- | --- |
| `template/runtime.at_boost_bits, attract_bits`、`accumulate.acc_bits, charge_bonus_bits`、`hide.attract_bits`、`poison.atp_bits`、`clone.name_factor_bits, child_name_factor_bits`、`adjustments.at_boost_delta_bits, attract_delta_bits` | 契约采用 `f64::from_bits(u)` 后归一化，同时保留原始 bit 旁路；poison 的 f64 实例见 M:906；`clone.name_factor` 的还原实例见 C:208；**产生式已逐字段核对：以上字段全部由 `f64::to_bits()` 写入**（`C:85–86/96/99/196/200/353/357`、`K:entity_runtime:376–377/387/389/464`、`K:state:160`、`K:handlers:112`），因此统一按 `f64::from_bits` 还原。注意 `hide.attract_bits` 是进入隐藏时对 `runtime.attract_bits` 的拷贝，而 `runtime.attract_bits` 随后被除以 10（`K:entity_runtime:464–469`），两者语义不同，不能互相替代 |
| `PlayerKindFlags` 原始 u64、`score_skill_boost_plan.initially_boosted_mask` | 只走精确旁路与 X 的 bits 通道，不作为数值特征；具名位另由 `entity_kind_flags` 表达 |
| 上述原始 bit、registration_order／runtime_registration_order／state_registration_cursor／deferred.state_cursor | `extra_bits` 与 `order_key` 的第 3 轴统一为 0=lo、1=hi；`lo=(u & 0xffffffff), hi=(u >> 32)`，还原 `u=(u64(hi)<<32) \| u64(lo)`。registration_order 为 u32，hi 恒 0；其余三个顺序字段为 u64，按同一公式拆分。分类原始 u32 也以 lo 保存、hi=0；禁止经 JS Number／f32 中转 |
| `hook_mask`、`compressed_state_flags` | 分别展开 64／8 个 0/1，`bit_i=(u>>i)&1`；compressed_state_flags 的低 5 位依次为 Shield/Protect/Upgrade/Corpse/Minion，高 3 位必须为 0，`u & 0b1110_0000 != 0` 返回 `ReservedFlagBitSet{path}`，不得静默清除；二者均不得压成一个 f32 标量 |
| `ModelSlot.i64_value/u64_value`、外部复合类型的 bit／整数 | 先查语义表；槽内标量在 `slot_value` 用 N，ref 重映射后进 X，浮点 bits 先 from_bits 再归一化并另存 raw，标志进 X 逐位展开；需精确保留的 i64 按二补码 u64 拆分。身份引用只保留重映射关系，不将原始 EntityIdx 偷渡进 raw 槽；value_type 不能取代语义白名单 |
| `at_boost_millionths` | `millionths = round(at_boost * 1e6)`（`C:25`）是有损整数；生产路径只从 f64 生成 bits，反向构造器 `with_at_boost_millionths`（`C:418–426`）只被测试使用，所以**不假定 `bits == millionths/1e6`**，也不使用近似列反推原 bits（M:455–456、776–777） |

精确旁路是编码包的校验／关系计算通道，不直接作为数值特征；原始 state 仍负责无损追溯。正负零原 bit 可区分；非有限浮点保留诊断 bit 并拒绝编码。`extra_bits` 可运输精确值，但未列入字段白名单的 bit 禁止送入模型，尤其不能用它绕过身份／随机数禁令。

## 14. 接口边界

拟议接口为 `FeatureEncoder::encode(&BattleModelState, &EncoderManifest) -> Result<EncodedState, EncodeError>`；批接口只重复相同编码与 padding，不接受 seed、winner、split 或当前 session。此为设计签名，不是已存在的公共 API（现有接口参见 A:17–23、API:3）。

| 层 | 职责与边界 |
| --- | --- |
| 原始数据集 | 继续保存现有 state 和标签，不增加编码列；可重建的张量缓存不能反过来替代原始 state |
| Rust encoder | 引用／词表／数值／mask 的唯一实现；不依赖 Python、展示 DTO、文件格式或模型参数 |
| Rust 数据适配器 | 读取现有 Parquet，单独选择 split、过滤 null 标签，将 state 交 encoder；Arrow/Parquet 留在工具侧，沿用 A:58 的依赖隔离 |
| Python 训练／评估 | 读取已编码数组、标签和审计关联表；只做训练、推理比较、校准与评估，不转换 bit、不重排引用、不另算归一化 |
| Rust 推理 | 使用同 encoder manifest 和模型所声明摘要，摘要不符立即拒绝，不自动采用最新版本 |
| WASM／Python 绑定 | 直接调用同一个 Rust 编码函数，传输连续 typed buffer；不把 u64 装入 JS Number，不重写编码算法 |
| 网页播放 | 生成对应帧时保存预测及 generation/frame 关联，播完该帧再展示；session 释放后仍可读缓存（A:92；API:111、137） |

导出通道规定为独立、可删除重建的编码包：`encoder-manifest.json` + 每批 `batch-manifest.json` + 按张量名称保存的小端 C 连续 `.bin`。批 manifest 记录 dtype、shape、字节数、摘要和实际 B；最后一批使用实际 B，内部容量不变。JSON 只描述编码包，不装载 state 或整块特征内容。

features、精确旁路、labels、审计关联表四个清单分开：labels 为 i32 `[B]` 的 `winner_team_index` 与 u8 `[B]` 的 `label_mask`，空标签存 -1/mask=0；默认训练导出过滤空标签，诊断导出可保留。审计关联含源分片／行号及帧 ID，只用于追溯，不可被训练代码的 features 清单枚举到。

缓存键包括原始数据摘要、schema、encoder 版本、profile、词表／归一化 manifest 摘要；更改任一项必须重编码，禁止混批。导出错误返回行定位与字段路径，不把错误行静默删除。文件容器和张量契约不要求新增 crate，本轮也不建立这些目录或接口。

补充序列化约定：`owner_scope` 的 1–9 依次为 global/entity/template/state/slot/list/input_team/runtime_team/lane；global owner 固定为 0，其他 owner 必须引用有效行。`list_index=(owner_scope,owner,field_class,ordinal,target)` 承载下表登记的有序引用，target 域按 field_class 固定；ordinal 是该 owner 下该类源列表的原下标。`extra_index` 的四元索引由 `field_class` 决定值类型与引用域；`extra_num/extra_ref/extra_bool/extra_cat/extra_bits` 中恰有一条有效通道，其余清零（ref 填 -1）。记录存在性统一使用 `extra_mask`；未知字段或类型不符拒绝，不能猜测或哈希字段名。

**槽的存储类型与值路由。** `slot_index=(scope,owner,slot_id,value_type)`；`scope` 独立使用 1=entity、2=template-global、3=battle-global，后两者 owner=0。`slot_id` 使用第 4 节按作用域重映射的稠密分类 ID，不与 field_class 共用语义。`slot_field_present` 的最后一轴依次对应下表四个有效分支，真实行恰一个分支为 1；全 None 或多个 Some 返回 `InvalidSlotValue{path}`，不能以 value_type=0 接受。

| value_type | `ModelSlot::project` 分支（M:65–71） | 值与 presence 的去向 |
| --- | --- | --- |
| 0 | PAD／无效；不对应真实 SlotValue | 仅 padding 行使用，slot_mask=0，所有 slot presence=0 |
| 1 | `bool`：bool_value | `slot_value[0]=0/1`，`slot_value_present[0]=1`；false 也是有效值 |
| 2 | `i64`：i64_value | 按槽语义白名单分派：标量→slot_value；引用→X ref；标志→X bool/bits；需要精确还原的非身份整数另走 raw，i64 按二补码拆分 |
| 3 | `u64`：u64_value | 同样先查语义白名单；不能仅因存储为 U64 就当连续数值。浮点 bit 语义先 from_bits 后进 slot_value，并另走 raw；实体 ref 与标志 bit 走 X |
| 4 | `template`：template | `slot_template` 指向同一模板表，`slot_template_present=1`；不向 slot_value 写模板编号 |

每个真实槽行的 `value_type` 唯一对应 `slot_field_present[value_type-1]=1`；`slot_value_present` 仅对 bool／标量分派为 1，`slot_template_present` 等于第四路分支 presence。引用／标志分派的 slot_value 为 0/presence=0；未用 slot_template 为 -1/presence=0。四个存储分支与默认注册表每个 `slot_id` 的整数机制语义均已冻结，逐槽白名单见第 3.2 节；未登记槽、未知 export_name 或与本表不符时返回 `UnknownSlotSemantics{path}`，不得按存储类型冒充已审计所有槽。

**field_class 共用一张冻结字段表，list 与 X 不是两个独立编号空间。** 0 永远保留为 PAD；字段表同时登记编号、完整字段路径、owner_scope、target／值类型与可空条件。已发布编号只能追加，不能改义、回收或跨段复用；以下编号是本稿契约分配，不再使用“暂定 1–7”让实现自行决定。

| 编号段 | 使用方 | 分配规则 |
| --- | --- | --- |
| 1–255 | X typed 叶子 | 当前 1–9 见下表，其余保留；新的具名 num/ref/bool/cat/bits 叶子在此段追加 |
| 256–4095 | list | 当前 256–272 见下表，与 X 已占 1–7 完全不重叠；后续列表在此段追加 |
| 4096–2147483647 | X 的 `raw.<字段路径>` | 仅精确 bits 旁路；当前 4096–4121 见下表，新增 raw 字段须显式登记并重算容量，不能根据字符串动态分配 |

| X field_class | 字段、owner 与有效值通道 |
| --- | --- |
| 1 | `assassinate_fixed_lane`；entity owner、ordinal=0，extra_ref→该实体模板的 lane，随 assassinate 整体存在 |
| 2 | `protect_level`；entity owner、ordinal=protect_from 原下标，extra_num=对应 level 的 N_f；该记录与 list(protect_from) 一一配对 |
| 3 | `clone_initial_boosted_mask`；template owner、ordinal=0，extra_bits；计划为 Some 时必有此记录，值 0 不能省略 |
| 4、5 | `clone_slot_boost_0_0`、`clone_slot_boost_0_1`；template owner、ordinal=0，extra_num 分别为 slot_boosts[0] 元组两个值的 N_f；该 Option 为 Some 才同时存在 |
| 6、7 | `clone_slot_boost_1_0`、`clone_slot_boost_1_1`；同上，对应 slot_boosts[1] |
| 8 | `slot_entity_ref`；slot owner、ordinal=0，extra_ref→重映射后的实体；仅在白名单明确该槽为实体引用时使用 |
| 9 | `slot_flag_bit`；slot owner、ordinal=位下标，extra_bool；白名单声明的位宽最多 64，位值 0 也保留对应记录，原始非身份 bit 另走 raw |

当前 1–9 没有使用 extra_cat 的字段，因此这些记录的 extra_cat 均为 0；以后若增加分类叶子，字段表必须同时指明第 4 节规则下的独立分类词表，不能直接使用原始 u32。

| list field_class | 字段、owner 与 target 域 |
| --- | --- |
| 256 | `lanes`；template owner→lane，owner 必须等于该 lane 的 `lane_template`；每条 lane 恰一条记录，ordinal 是该模板内原数组下标 |
| 257、258、259、260 | `merge_lane_order`、`active_order`、`pre_action_order`、`post_damage_order`；template owner→lane，值已核对为模板内 lane 数组下标（第 7 节），保留原序及重复项 |
| 261 | `deferred`；template owner→lane；ordinal 为 post_action_after_states 原下标，同一行的 order_key 保存 deferred.state_cursor |
| 262 | `round_order`；global owner→entity |
| 263、264 | `team_roster`、`team_alive`；runtime_team owner→entity，各队内部顺序独立保留 |
| 265 | `flat_alive`；global owner→entity |
| 266 | `states`；entity owner→state，ordinal 为实体原 states 下标 |
| 267 | `protect_from`；entity owner→被保护链引用的 owner 实体，ordinal 与 X protect_level 一致 |
| 268 | `input_member`；input_team owner→entity，保留各输入队成员顺序 |
| 269 | `ice_release`；global owner→entity |
| 270、271 | `order_registration_order`、`order_runtime_registration_order`；entity owner→state，ordinal 为 states 原下标，order_key 保存对应注册序 |
| 272 | `order_state_registration_cursor`；entity owner→同一 entity，ordinal=0，order_key 保存 state_registration_cursor |

上述已知嵌套团队列表以团队行作 owner，不另占 V 轴的父记录；空列表由对应 owner 与空的该类记录集合表达。`list_position` 对每个 `(owner_scope,owner,field_class)` 按第 4 节公式计算。`order_key_present=1` 当且仅当有效 list 行属于 261、270、271、272；真实键 0 与无键行通过 presence 区分。270 的键是 u32，hi=0；其余为 u64，统一 `[lo,hi]`。`order_rank_present` 还要求存在第 4 节定义的当前实体比较域，蓝图 deferred 无该域时为 0，不能伪造其与当前实体状态的关联；其他无键行的 order_key/order_rank 置 0。

raw 路径的 runtime/template/state/slot/world 前缀指第 3 节相应结构，owner 负责定位局内实例；所有标量 raw 的 ordinal=0、值只在 extra_bits，类型不是模型输入 num/cat。以下是本轮封闭白名单，未出现的可空源不创建记录。

| raw field_class | `raw.<字段路径>` | owner／计费 |
| --- | --- | --- |
| 4096 | `raw.runtime.kind` | entity；原始 u32，hi=0 |
| 4097、4098 | `raw.template.kind`、`raw.template.identity.boss_kind` | template；kind 的 hi=0，boss_kind 仅 Some 时存在 |
| 4099、4100 | `raw.state.legacy_order_key`、`raw.state.extension_state_id` | state；均为 u32，hi=0，extension 仅 Some 时存在 |
| 4101 | `raw.slot.slot_id` | slot；原始 u32，hi=0，模型侧仍只消费作用域内稠密 ID |
| 4102、4103 | `raw.slot.i64_value`、`raw.slot.u64_value` | slot；至多一个有效值记录，且只用于白名单允许的非身份数值／bit，原始 EntityIdx 等身份引用不在例外内 |
| 4104、4105、4106 | `raw.runtime.at_boost_bits`、`raw.runtime.at_boost_millionths`、`raw.runtime.attract_bits` | entity；三个独立记录 |
| 4107、4108、4109、4110 | `raw.runtime.flags`、`raw.runtime.accumulate.acc_bits`、`raw.runtime.accumulate.charge_bonus_bits`、`raw.runtime.hide.attract_bits` | entity；最后一项随 hide 整体存在 |
| 4111、4112、4113、4114 | `raw.template.reserved_player_ids_before_spawn`、`raw.template.at_boost_bits`、`raw.template.at_boost_millionths`、`raw.template.attract_bits` | template；四个独立记录 |
| 4115、4116、4117、4118 | `raw.template.clone_build.name_factor_bits`、`raw.template.clone_build.child_name_factor_bits`、`raw.template.clone_build.adjustments.at_boost_delta_bits`、`raw.template.clone_build.adjustments.attract_delta_bits` | template；四项均随 clone_build 整体存在 |
| 4119 | `raw.state.payload.poison.atp_bits` | state；仅 poison 分支存在 |
| 4120、4121 | `raw.world.alive_group_count`、`raw.world.round_pos` | global；两个原整数控制字段，round_pos 按有符号二补码保留 |

白名单按 owner 的最多记录数正是第 4 节的 `8×e+10×h+3×s+2×q+2`；槽的两个 raw 已计入每槽 66 的总预算，不重复相加。计划 mask 使用 X 类 3，不另复制 raw mask。`raw` 只传精确旁路，不允许借本表恢复原始名字、seed、标签或身份编号输入。

## 15. 测试计划（先于实现）

先准备下表的输入与期望，再实现 encoder；本轮不编写或执行 Rust 测试。以下是未来验收标准，不是测试已通过声明。

| 类别 | 必须覆盖与判定标准 |
| --- | --- |
| 字段覆盖／版本 | 每个 M 中字段有消费或有理由的排除；第 4 节逐名核对张量 dtype/shape 与字段／presence 映射；field_class 全局唯一、编号段不交叉、raw 仅走旁路；新增字段、未知 kind、43–50 未启用技能、schema 不符均报错，不允许 wildcard 吞新字段 |
| 引用／关系 | 稀疏 EntityIdx、预留空槽、同号不同域、死亡 owner、悬空引用；有效引用可还原到同一源实体，非法引用报字段路径 |
| 数值／缺失 | 0、None、PAD、负属性、极大整数、NaN/Inf；逐项核对 presence 清单、父子联动、Some(0)/Some(false)、计划 Some 但 slot_boosts[i]=None；分类 u32::MAX 只能重映射后入 cat，原始 0 类与 PAD 区分；固定计数尺度与 N_f 不重叠 |
| bit 精度 | 0、负零、f64 正常值、超过 `2^53` 的整数和 u64 全 1；第 3 轴固定 0=lo/1=hi，u32 hi=0；compressed_state_flags 低 5 位逐位一致，高 3 位任一置 1 返回 ReservedFlagBitSet，不得静默清除 |
| 技能 | 重复 skill_id、多 lane、零等级、三种 boost、五类 lane list 及 deferred；list(lanes) 的 owner/ordinal/target 与 lane_template 一致，每条 lane 恰有一条数组序记录；执行列表重复项不去重，固定键不与 skill_id 混用 |
| 状态／压缩 | 已映射的 11 个 kind、稀疏状态、所有可空引用；Boss 分支（12–16）必须返回 `UnsupportedPayloadKind`；kind／分支不匹配拒绝，压缩状态独立存在 |
| 蓝图／复合类型 | 实体、全局模板、battle 三域槽；四个 value_type 与 presence 一一对应，slot_id=0 需重映射，未知语义不得猜测；clone_build、policies、charge/accumulate/hide/assassinate、corpse 与 protect_from 逐叶映射，第 3.1 节每个叶子至少一条断言；新增叶子未进表即失败 |
| 变长／容量 | E=6/15/30/32/33，实体槽 31/64/65，T=2/3/32/33；E=33、槽=65、T=33 报错；其他维测试 limit−1/limit/limit+1；验证 V_max≥5×L_max、实际 list 总和（含重复、deferred、注册序）、Q/X 公式及 raw 计费，不用实测子集替代蓝图容量验收 |
| 资格／终局 | 真实队伍零存活仍 mask=1；复活前后 mask 不变；全灭无 winner 不伪造终局；winner 非空拒绝；null 标签无监督权重 |
| 置换／批布局 | 实体重排并修正引用、队伍轴置换、padding、不同 batch 邻居；整数/bit 精确相同或等变，后续概率撤销置换后绝对差 ≤ `1e-6` |
| 机制顺序 | 改行动顺序、游标、技能执行列表、注册序，编码必须保留可辨差异；不要求未来模型对每次修改都给不同概率 |
| 防泄漏 | 固定 state 改 seed/progress/split/label/原输入/未来帧，features 字节完全不变；只有标签／审计通道可变 |
| 跨通道一致 | 同 state/manifest 在 Rust、Parquet 导出、Python/WASM 绑定：整数、mask、bit 完全一致；f32 绝对差 ≤ `1e-6`，同一原生构建重复编码逐字节一致 |
| 分布与支持声明 | 分 train/validation/test、队伍数报每维峰值、溢出率、裁剪率、截断率；Boss 与执行失败的 DIY 计入不支持域；声明支持的数据域要求容量错误数=0，未覆盖域明确标记 |

从仓库根目录可复现本稿引用和原有数量统计；下面统计命令要求已有 release 可执行文件和原始数据，本轮未执行，不触发构建或生成：

```powershell
rg -n '^pub struct|^pub const MODEL_' crates/tswn_core/src/runtime/model_state.rs
rg -n 'state_entry_count|skill_count|lanes \+=' crates/tswn_winprob_dataset/src/stats.rs
target/release/tswn-winprob-dataset.exe stats --out target/winprob-100k
git diff --check
```

新文档未跟踪时 `git diff --check` 不检查其内容，应另读该文件检查行尾空白和占位符。现有 stats 不能生成第 5 节的标量尺度；未来 Rust 校准通道必须另交可复现命令、输入摘要和 train 选择记录，不能把上述命令的结果冒充校准结果。

## 16. 未决问题

以下问题需要实施前人工复核；本轮完成草案不以得到答复为前提。建议项是本稿默认方向，待确认不能被实现端默认为已验证事实。

| 问题 | 选项 | 建议及冻结条件 |
| --- | --- | --- |
| 槽编号及整数语义 | 按注册表白名单分 num/ref/bits；或按 U64 一律数值化 | **已按默认注册表逐槽核对，结论见第 3.2 节**。`U64` 槽同时存在实体引用（`summoned_entity`）、计数（`minion_counter`）与浮点 bit（`lazy_blueprint_rq`）三种语义，按存储类型数值化会在第一类上泄漏原始实体编号；自定义注册表不在支持域（M:83），新增槽须先更新 3.2 表并重算 `Q_max` |
| 顺序、身份和阵营域 | 保留精确键并补查消费者；或假定所有编号都是下标 | **已逐项核对**：执行列表整数是模板内 lane 数组下标，不是 `fixed_lane_key`（第 7 节）；`PlrId` 是实体槽下标 + 1 的派生值，不参与模型输入（第 6 节）；`charm.group_id` 是施法者 `EntityIdx`，与 `clan_group`／runtime team 都不同域，必须按实体引用重映射（第 8 节） |
| 浮点产生式 | 尺度按 train 实测拟合（已定）；各 `*_bits` 的 f64 产生式与 millionths 关系 | **产生式已逐字段核对，见第 13 节**：全部由 `f64::to_bits()` 写入，`millionths` 是有损整数且反向构造器仅测试使用。无观测即 `MissingCalibration`，不登记人工常数；仍不假定 `bits == millionths/1e6` |
| 容量 profile | baseline-32；或补测后单独推出更大 profile | **已实测 100k 池峰值（第 4 节表）**：`e≤30`、`h≤120`、`l≤882`、`q≤96`、`V_required≤2800`、`X_required≤2271`，全部远低于当前预算；该池只有 2v2v2，冻结前须在更宽队伍配置与蓝图密集局上重测，再决定保留结构上界还是收紧到实测区间；不使用静默截断 |
| 资格 mask | 全部输入队伍；或证明复活／召唤不可达后屏蔽 | 建议全部保留，采用第 10 节公式；避免把本池 `0/40000` 的未见事件当引擎保证（W:169） |
| 导出容器 | typed bin + manifest；或后续绑定直接返回 buffers | 建议先提供可重建离线包，绑定复用同一布局；两者不得产生第二份编码算法 |

阅读中发现的描述差异／覆盖缺口（已修正项注明，其余仅记录）：

1. 已在 `crates/tswn_winprob_dataset/README.md:86–88` 修正：stats 只读已提交分片，bench 会在 --out 目录运行 generate/validate 并写入数据；同时区分首次生成与已有 manifest 的 --resume 条件（G:41–60、138–182）。
2. P:126 把 seed 列列在 samples 字节构成中；G:215–224 的 SampleRow 构造无 seed，G:243–250 的 BattleRow 才有 seed。报告口径／历史 schema 来源待确认，本稿不改原数据格式。
3. 已在 `docs/reference/public-api.md:10–14` 修正：新增 BattleModelSession 数据集入口，说明生成器在 PreparedRunner 之上两遍运行，以 next_frame() 取可见帧边界、model_state() 导出状态（G:195、208、213）；Runner/PreparedRunner 是执行内核入口，而不是数据集直接驱动接口。
4. W:119 写计数“唯一用途”为目标选择，W:128 又补充 post-action 用法；本稿保留计数，且遵循 W:174 禁止用作 mask。
5. 数量统计不含蓝图技能、复合载荷叶子和标量尺度（S:187–209）；这是本稿容量与数值 manifest 不能立即冻结的原因。Boss 已按第 9 节移出本轮范围，不再是阻塞项。

## 17. 参考来源表

仅将下表已读文件作为当前行为依据；通过机制文档引用的其他 runtime 文件未直接核查，不声称完成了它们的实现审计。行号以本稿撰写时工作树为准。

| 缩写／主题 | 文件:行（符号或内容） |
| --- | --- |
| 仓库约定 | `AGENTS.md:1`；`docs/README.md:27`（文档约定）；`docs/design/README.md:5`（设计索引） |
| M 输入／校验 | `crates/tswn_core/src/runtime/model_state.rs:78`（RuntimeRunner::model_state）、`:197`（BattleModelState::validate）、`:336`（根结构） |
| M 技能 | `crates/tswn_core/src/runtime/model_state.rs:7`（ModelSkills::from）、`:281`（编号空间）、`:390`（lane 与顺序类型） |
| M 实体／身份 | `crates/tswn_core/src/runtime/model_state.rs:351`（ModelEntity）、`:364`（ModelWorld）、`:375`（ModelIdentity） |
| M 槽／模板 | `crates/tswn_core/src/runtime/model_state.rs:56`（ModelSlot::project）、`:420`（槽）、`:439`（模板）、`:469`（模板投影） |
| M 状态／payload | `crates/tswn_core/src/runtime/model_state.rs:428`（StateEntry）、`:528`（Payload）、`:548`（分支）、`:628`（穷举投影） |
| M runtime／精度例 | `crates/tswn_core/src/runtime/model_state.rs:759`（Counter）、`:765`（PlayerRuntime）、`:906`（poison f64 bit） |
| J 默认槽注册表 | `crates/tswn_core/src/runtime/profile/registry.rs:265–277`（7 个实体槽、3 个全局模板槽；无 battle 槽登记） |
| E 扩展注册表 | `crates/tswn_core/src/runtime/extension.rs:109`（PlayerKindFlags）、`:113`（具名位）、`:158`（PlayerKindPolicies） |
| R 实体运行时 | `crates/tswn_core/src/runtime/entity/runtime.rs:33`（MoveState）、`:38`（PlayerPolicyOverrides）、`:74`（ProtectLinkRuntime）、`:80`（HideRuntime）、`:89`（AssassinateRuntime）、`:103`（RuntimeCorpseKind）、`:359`（ChargeRuntime）、`:366`（AccumulateRuntime） |
| C 实体与分身 | `crates/tswn_core/src/runtime/entity.rs:29`（CloneStatAdjustments）、`:63`（ScoreCloneSkillBoostPlan）、`:70`（CloneBuildData）、`:182`／`:208`／`:211`（访问器） |
| Y 实体状态 | `crates/tswn_core/src/runtime/entity/state.rs:17`（CovidInfectionEntry，本轮不映射） |
| A 总体决策 | `docs/design/battle-analyze.md:13`（状态）、`:52`（Parquet）、`:81`（Rust 单一 encoder 与网页约束） |
| U 审计 | `docs/design/battle-model-state-audit.md:5`（字段）、`:25`（身份／蓝图）、`:37`（排除）、`:50`（校验） |
| W 判胜 | `docs/mechanics/winner.md:17`（判胜）、`:110`（粘性计数）、`:159`（复活／资格） |
| API 绑定 | `docs/reference/public-api.md:5`（入口）、`:55`（DTO）、`:91`（WASM）、`:135`（播放） |
| P 实测 | `docs/perf/reports/winprob-dataset-scale-baseline.md:27`（环境）、`:117`（字节口径）、`:146`（分布）、`:177`（encoder 建议） |
| D 生成器说明 | `crates/tswn_winprob_dataset/README.md:38`（标签）、`:68`（统计／基准）、`:91`（Python） |
| S 统计口径 | `crates/tswn_winprob_dataset/src/stats.rs:45`（Series）、`:130`（collect）、`:187`（实体／状态／技能计数） |
| F 抽样 | `crates/tswn_winprob_dataset/src/sampling.rs:5`（select，排终局与最终轮数分桶） |
| G 生成 | `crates/tswn_winprob_dataset/src/generate.rs:138`（write_shard）、`:186`（generate_battle）、`:202`（标签）、`:211`（样本） |
| K 槽语义与顺序证据 | 前缀均为 `crates/tswn_core/src/`：`runtime/prepared_init/init.rs`（init）、`runtime/prepared_init/seed.rs`（seed）、`runtime/prepared_init/roster.rs`（roster）、`runtime/plain_summon.rs`（summon）、`runtime/plain_zombie.rs`（zombie）、`runtime/handlers/mod.rs`（handlers）、`runtime/handlers/minions.rs`（minions）、`runtime/handlers/states.rs`（states）、`runtime/combat/skills_control.rs`（skills_control）、`runtime/combat/skills_team.rs`（skills_team）、`runtime/combat/round.rs`（round）、`runtime/scheduler.rs`（scheduler）、`runtime/entity/runtime.rs`（entity_runtime）、`runtime/entity/state.rs`（state）、`runtime/profile/import.rs`（import）、`runtime/prepared_init.rs`（prepared） |
