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
| 根 `round, entity_slot_count` | `global_num[0,1] = N(f)` | M:338–340 |
| 根 `legacy_step_scheduler` | `global_num[4] = 0/1` | M:346 |
| 根 `input_teams`、`ice_release_events` | `list` 的 `input_member`、`ice_release` 有序引用；外层队伍长度生成 `team_mask` | M:341、347 |
| 根 `entities, template_slots, battle_slots` | 实体表及 `slot` 表，保留作用域；不以数量代替内容 | M:343–345 |
| World `round_order, team_roster, team_alive, flat_alive` | `list` 的四种有序实体引用，团队列表保留外层 runtime team 轴 | M:365–368 |
| World `alive_group_count, round_pos` | `global_num[2,3]`；原整数另作精确控制字段，计数只作机制特征 | M:369–370；W:110 |
| Entity `id` | 建立 `EntityIdx → e` 字典；原编号不进模型 | M:352 |
| Entity `input_team_index, template, runtime, states, slots` | `entity_team[0]`、`entity_template`、下列 runtime 槽、`state` 表、`slot` 表 | M:353–357、360 |
| Entity `state_registration_cursor` | 精确 `order_key`，用于注册次序与 deferred 的比较 | M:357 |
| Entity `compressed_state_flags` | `entity_flags[0..7]` 按低位到高位展开，不依赖 states 非空 | M:358–359；U:17 |
| Runtime `hp, attack, magic, magic_point, wisdom, speed, defense, resistance, agility` | `entity_num[0..8]`，依表列顺序逐项 `N(f)` | M:766、768–775 |
| Runtime `at_boost_bits, attr_sum, atk_sum, attract_bits, shield, protect_pre_defend_skill_count` | `entity_num[9..14]`；bit 字段先还原浮点，最后一项另带 presence | M:776–780、790、793 |
| Runtime `at_boost_millionths` | 丢弃模型通道中的重复近似量；精确旁路保留，优先使用 bits，关系待确认见第 13 节 | M:776–777 |
| Runtime `alive, upgrade_active`；Counter `pending` | `entity_bool[0..2]` | M:767、794、760 |
| Runtime `kind, corpse` | `entity_cat[0,1]`；corpse 词表见第 3.1 节 | M:781、798 |
| Runtime `owner, root_owner, protect_to`；Counter `last_target` | `entity_ref[0..3]`，可空引用有 presence | M:782–783、791、761 |
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
| Template `move_state, policy_overrides, clone_build` | `template_num[16]`；三个枚举进 `template_cat[2..4]`、`inherit_owner_def_res` 进 `template_bool[3]`、四路 presence 进 `template_override_present`；`clone_build` 见下表 | M:460–462；R:33–43；C:70–76 |
| Template `reuse_skills_on_recast, reuse_stats_on_recast, inherit_owner_def_res` | `template_bool[0..2]` | M:463–465 |
| Identity `clan_group, boss_kind` | `clan_equal` 相等关系矩阵、`template_cat[1]`；boss None 用 presence | M:377–378 |
| Identity `boss_action_prob_count, boost_immune_threshold` | `template_num[14,15]` | M:379–380 |
| Identity `immunity`；Immunity `status, threshold` | `immunity_num[0..8]`，按 assassinate/charm/berserk/half/curse/exchange/slow/ice/fire 固定轴及 presence；未知 status 报错 | M:381、385–386、480–495 |
| Skills `lanes`；Skill `skill_id, level, build_level, boost, boosted, fixed_lane_key` | `lane_skill_id`、`lane_num[0,1]`、boost 分类及 `[2,3]`、`lane_bool`、`lane_key`，第 7 节展开 | M:390–417 |
| Boost `kind, base, extra` | `lane_boost_kind`、`lane_num[2,3]`；None 与值为 0 区别 | M:400–403 |
| Skills `merge_lane_order, active_order, pre_action_order, post_damage_order` | 四类 `list`，保留原序并引用所属 template 的 lane | M:413–416 |
| Skills `post_action_after_states`；Deferred `state_cursor, fixed_lane` | `list(deferred)` 的序号、精确 cursor 和 fixed-lane 引用 | M:406–408、417 |
| StateEntry `legacy_order_key, extension_state_id, priority, registration_order, runtime_registration_order` | `state_cat[0,1]`、`state_num[0]`、两个精确 `order_key`；可空 extension 带 presence | M:429–434 |
| StateEntry `hook_mask, payload` | `state_hook[0..63]`；kind 分类和通用 payload 槽，见第 8 节 | M:431、435 |
| Slot `slot_id, bool_value, i64_value, u64_value, template` | `(scope, slot_id)` 分类、typed bool／num／ref／bits、模板引用；必须先经槽语义白名单，不能按存储类型猜 U64 的含义 | M:420–425、56–70 |
| Payload `kind` 及可空载荷字段（Boss 分支除外） | `state_kind` 与第 8 节逐字段表；不丢弃低频分支 | M:528–625 |

根/world 列表的长度、实体存在数、模板和记录存在性均由对应 mask 表达；额外 `global_num[5]=N(entities.len())`。嵌套模板通过同一模板表编码，不用第二套属性／技能规则。没有被表中规则消费的已知字段必须导致字段覆盖测试失败。

### 3.1 复合类型叶子

上表引用到的复合类型定义不在 `model_state.rs`，逐个叶子如下（`Option` 结构用一个 presence 表示整体存在，不用逐叶子 presence）：

| 来源 | 叶子 | 去向 |
| --- | --- | --- |
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
| `CloneBuildData`（C:70） | `attrs`、`weapon_attr_bonus` | `template_clone_attr`、`template_clone_weapon_bonus` |
| 同上 | `name_factor_bits`、`child_name_factor_bits` | `template_num[17,18]` + 精确旁路 |
| `CloneStatAdjustments`（C:29） | `max_hp,attack,magic,wisdom,speed,defense,resistance,agility`、`at_boost_delta_bits`、`attr_sum`、`atk_sum`、`attract_delta_bits` | `template_num[19..30]`；两个 `*_bits` 另存旁路 |
| `ScoreCloneSkillBoostPlan`（C:63） | `initially_boosted_mask`、`slot_boosts[2]` | X 的 `bits` 与四个 `num` 叶子 |

`CloneBuildData` 及其两个嵌套类型的字段已公开（`pub`），encoder 直接读取，不需要投影访问器；相关访问器 `derive_stats`（C:182）、`name_factor`（C:208）、`all_sum`（C:211）只作交叉校验。任何复合类型新增叶子都必须同步本表，否则字段覆盖测试失败。

## 4. 张量契约

采用 batch-first、C 连续布局，最右轴连续；`B` 是批大小，线上 `B=1`。一个 manifest 冻结所有容量，禁止每批自动改变 shape；以下为拟议 `baseline-32` profile。

| 符号 | 固定值 | 依据与限制 |
| --- | --- | --- |
| `E_max`（实体） | 32 | P:161：min 6、中位 7、均值 7.6、p90 10、p99 15、max 30；向上留 2 个槽，不表示引擎上限 |
| `T_max`（输入队伍）、`R_max`（runtime team） | 各 32 | 3 队是唯一已测配置（P:37）；32 为支持小规模 FFA 的建议容量，须独立验收 |
| `H_max`（实体模板及槽内蓝图） | 128 | 建议预算，非实测上限；实体模板与三类预览入口见 M:116–119，不能推断每实体恰有三个蓝图 |
| `L_max`（所有模板的 lane） | 4096 | P:165 实体模板 lane 总量 max 742、p99 422；蓝图技能不在该统计内（S:193–205），4096 是待压测预算 |
| `S_max`（全样本状态条目） | 32 | P:164：中位 0、p99 4、max 10；预留罕见复合状态空间，Boss 分支本轮不映射 |
| `Q_max, V_max, X_max`（槽、有序列表项、扩展叶子） | 512、4096、8192 | 尚无实测分布；X 预算含分身评分计划与保护链叶子，冻结前须补统计，不得宣称零溢出 |

| 输出张量族 | dtype 与 shape | 内容 |
| --- | --- | --- |
| `global_num` | f32 `[B,6]` | 第 3 节定义的六项 |
| `entity_num/bool/flags` | f32 `[B,E_max,24]`；u8 `[B,E_max,10]`；u8 `[B,E_max,8]` | 运行时数值、布尔值和压缩状态位 |
| `entity_kind_flags` | u8 `[B,E_max,6]` | `PlayerKindFlags` 的六个具名位，原始 u64 只进旁路 |
| `entity_cat/ref/team/template` | i32 `[B,E_max,5]`；i32 `[B,E_max,5]`；i32 `[B,E_max,2]`；i32 `[B,E_max]` | 分类、实体引用、两类队伍引用和模板引用 |
| `template_num/bool/cat` | f32 `[B,H_max,31]`；u8 `[B,H_max,5]`；i32 `[B,H_max,5]` | `template_team/template_player_ref` 各 i32 `[B,H_max]` |
| `template_override_present`、`template_clone_attr`、`template_clone_weapon_bonus` | u8 `[B,H_max,4]`；u32 `[B,H_max,8]`；i32 `[B,H_max,8]` | policy overrides 四路 presence、分身属性与武器加成；clone 整体存在性见 `template_bool[4]` |
| `immunity_num`, `clan_equal` | f32 `[B,H_max,9]`；u8 `[B,H_max,H_max]` | 阵营编号只变成相等关系 |
| `lane_skill_id/boost_kind/template/key` | 各 i32 `[B,L_max]` | key 是 template 内引用键，经局内重映射 |
| `lane_num/bool` | f32 `[B,L_max,4]`；u8 `[B,L_max,1]` | 等级、构建等级、base、extra；boosted |
| `state_entity/kind`、`state_cat/hook` | i32 `[B,S_max]`、i32 `[B,S_max,2]`、u8 `[B,S_max,64]` | 归属、分类、legacy/extension 分类、hook bit |
| `state_num/ref/group` | f32 `[B,S_max,9]`；i32 `[B,S_max,4]`；i32 `[B,S_max,3]` | priority + 8 payload 数值槽；4 个实体引用；3 个阵营／队伍关系键 |
| `slot_index/value/template` | i32 `[B,Q_max,4]`；f32 `[B,Q_max,1]`；i32 `[B,Q_max]` | index=(scope,owner,slot_class,value_type)，数值或 bool 值、模板 ref；引用和 bits 进入下述 typed 记录 |
| `list_index` | i32 `[B,V_max,5]` | (owner_scope,owner,field_class,ordinal,target)，target 域由 field_class 固定；嵌套列表以父记录作 owner |
| `extra_index/num/ref/bool/cat/bits` | i32 `[B,X_max,4]`；f32/i32/u8/i32 `[B,X_max]`；u32 `[B,X_max,2]` | X 的 (owner_scope,owner,field_class,ordinal) 及互斥类型值；bits 仅精确旁路，不默认送模型 |
| `order_key` | u32 `[B,V_max,2]` | 按低／高 32 位存注册游标和顺序值，与对应 list 项对齐；模型消费由精确比较导出的关系 |
| `list_position/order_rank` | 各 f32 `[B,V_max]` | 原列表位置和精确顺序键的归一化稠密秩，规则见下文 |
| `runtime_team_mask` | u8 `[B,R_max]` | runtime team 关系行存在性，不表示当前存活／未来获胜资格 |
| `team_mask`、预测结果 | u8 `[B,T_max]`；f32 `[B,T_max]` | encoder 只产生 mask；概率由后续推理端产生 |

每个实体／模板／lane／state／slot／list／extra 张量族分别带 u8 `[B,该容量]` 的存在 mask。每个可空值另有同槽 shape 的 presence；数值 0、None 和 padding 是三种情况。bool 只取 0/1；分类 PAD=0，有效类从 1 起；引用有效值从 0 起，缺失／padding 填 -1，gather 必须先检查 presence 和目标 mask。

所有 padding 数值、分类、bit 均置 0；引用置 -1；mask=0 的位置不参与聚合、注意力或 softmax。死亡实体 mask=1、alive=0；预留空实体 ID 槽不生成实体行。实测槽数最大 31 与实体最大 30 不可混同（P:161–163）。

所有列表保留重复项与空列表边界；空列表由 owner 存在且该 field 无项表示。`list_position=ordinal/max(1,有效列表长度-1)`。`order_rank=rank/max(1,不同键数-1)`：在同实体、同注册序域内对原整数精确排序，相等键同秩；runtime_registration_order、state_registration_cursor、deferred.state_cursor 共享比较域，registration_order 单独成域。这些是当前结构的无量纲关系变换，不依赖 padding 上限或训练分布；精确比较域的消费者仍须按第 16 节核对。

状态原数组序进入 `list(states)`，注册序值进入对应 `list(order_*)`，后者的 target 引用实体／状态／deferred 项。slot 等额外原始整数与浮点 bit 使用 `extra_bits` 的专用 `raw.<字段路径>` 类运输；它与同源 num 行是不同记录，原始编号与禁用字段不得加入该类。

任一维超限返回 `CapacityExceeded {path, actual, limit}`，禁止截断实体、lane、状态或蓝图。离线按 split／队伍数／容量维度报失败率，Boss 分支按不支持计入失败率，线上不给伪概率；扩大容量须发布新 profile，不能把 padding 容量变成模型语义。

## 5. 归一化与数值域

现有测量仅支持数量尺度；S:187–209 没有收集 HP、等级、属性、payload 标量或蓝图分布。不能把“HP/100”“技能等级/255”写成实测结论，也不能把 P:152 的最终轮数 167 当当前 state 的固定上限。

| 标量类别 | 规定的变换、裁剪与依据 |
| --- | --- |
| 实体数、实体槽数 | `x/15`、`x/16`；裁剪分别 `[0,32/15]`、`[0,64/16]`，依据 P:161、163 的 p99；64 是槽数建议支持边界，超限报错 |
| 当前存活数（若聚合使用） | `x/11`，裁剪 `[0,32/11]`；P:162 的 p99=11，不能反过来构造资格 mask |
| 状态数、实体模板 lane 数（若聚合使用） | `x/4`、`x/422`，裁剪 `[0,32/4]`、`[0,4096/422]`；P:164–165；含蓝图总量必须另拟合 |
| `round`、粘性计数、游标位置、HP、属性、护盾、等级、boost、概率阈值、payload 的数值 | 每个完整字段路径分别拟合 `s_f=max(1,Q50(abs(x)))`，`c_f=max(1,Q99(abs(x)))`；`N_f(x)=clip(sign(x)*ln(1+abs(x)/s_f)/ln(1+c_f/s_f),-4,4)` |
| 已还原的有限浮点系数 | 同一 `N_f`，不能直接把 u64 bits 转 f32；保留正负值，不假定攻击或 HP 必定非负 |
| bool、分类、关系、bit、精确注册序 | bool 0/1；分类不归一化；关系用引用；bit 展开／旁路；注册序精确比较，禁止浮点排序 |

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
| `clan_group`、`PlrId` | 前者仅相等关系；后者单独相等关系域，不使用名字哈希重建；PlrId 是否还有相对次序用途待确认 |
| `skill_id` | 固定 1–50，当前 1–42，43–50 预留；0 只作 PAD（M:281–325）；遇到未启用预留项报错 |
| fixed lane、legacy key、extension、slot ID | 分离词表；fixed lane 只在所属模板内解引用，slot ID 必须同时携带作用域（M:391、420、428） |

技能编号例：`core.skill.revive` 是 `MODEL_SKILL_EXPORTS` 第 22 项（M:305），不能照搬 W:163 的 legacy 技能 ID 16。模板 kind 的 `u32::MAX` 是实测有效类（P:172），不能用 -1 表示该类或把它当 padding。其余 kind 词表按已审计规则冻结，基线未出现不等于非法。

## 7. 技能槽方案

采用“每个真实 lane 一行、全样本打包”的方案；不分配 `[E_max,50]` 单等级表，不按 skill_id 合并重复 lane。P:165、180 的中位 211／均值 241.6／最大 742 条说明这一块需单独预算，而 42 个技能类别不构成 lane 数量上限。

`lane_skill_id` 供分类 embedding；`lane_num=[level,build_level,boost.base,boost.extra]`，四项分别按第 5 节归一化。`lane_bool=boosted`，`lane_boost_kind` 的 0=PAD、1=None、2=normal、3=last_boost、4=slot_boost；后两项数值 presence 在 boost=None 时为 0（M:15–37）。

每行同时带所属模板与原 lane 序号；模板内 lane 数组顺序进入 `list(lanes)`。`fixed_lane_key` 建立独立局内键表；四种执行列表和 deferred 通过类型化引用连接 lane，不拿 skill_id 替代 lane。源字段分别见 M:37–49、413–417；执行列表整数究竟指 lane 下标还是 fixed key 的逐项解引用规则在该文件没有定义，须补审计后冻结。

deferred 保存原列表顺序和精确 `state_cursor`，与实体状态注册序生成比较关系。对于非实体模板（槽内蓝图），没有当前实体状态时不得编造关联。lane 的 PAD 行 level=0、mask=0；真实零等级 lane 的 mask=1。未知技能、未解析 fixed key、顺序引用越界均拒绝。

嵌套蓝图的技能仍进入同一 L 轴，引用不同模板行；不得重复调用 Python 编码器或将模板序列序列化为整体 JSON embedding。基线 742 不覆盖这些蓝图 lane，冻结前必须补测合计峰值和内存。

## 8. 状态载荷方案

`state_num[0]` 是 priority，以下 `p0..p7` 对应 `[1..8]`；`r0..r3` 为实体引用，`g0..g2` 为关系键。每个槽有 presence；unused=0/presence=0。kind=0 为 PAD，真实 `none`=1。投影分支由 M:628–753 穷举，规定词表如下。

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
| 9 `charm` | `group_id,effective_team_idx,source_team_idx` → g0,g1,g2；`target` → r0；`step` → p0 | M:579–585 |
| 10 `slow` | `step` → p0 | M:588–590 |
| 11 `iron` | `protect,step` → p0,p1 | M:593–596 |
| 12 `covid_boss` | 本轮不映射（Boss 专属）；遇到该 kind 返回 `UnsupportedPayloadKind` | M:599–601 |
| 13 `covid_infection` | 本轮不映射（Boss 派生感染）；同上 | M:604–608、260–263 |
| 14 `saitama_boss` | 本轮不映射（Boss 专属）；同上 | M:611–616 |
| 15 `lazy_boss` | 本轮不映射（Boss 专属）；同上 | M:619–621 |
| 16 `lazy_infection` | 本轮不映射（Boss 派生感染）；同上 | M:624–625 |

禁止把任何子列表平均池化成一个数量；子 list 共享 V 容量并以状态行作 owner。Boss 分支本轮不映射，因此 `CovidInfectionEntry`（Y:17）与两个 Boss 载荷的叶子不需要张量槽位；一旦恢复 Boss 支持，必须按第 3.1 节的粒度补齐，不能静默省略。

校验 kind 与唯一载荷分支匹配；`none` 不得带分支，其他 kind 必须恰有对应分支；错误返回字段路径。状态条目原序、legacy key、hook、priority、两套注册序与实体注册游标共同保留。`compressed_state_flags` 独立编码，因为 U:17 明确压缩 Shield/Protect/Upgrade/Corpse/Minion 不一定出现在 entries。

## 9. 变长与特殊实体

普通池每样本 6–30 实体、存活 2–20（P:161–162），不允许只取六个本体或只取存活者。实体消失／新增／复活改变 mask 或 alive，但输入队伍归属沿用 root_owner 的映射（M:107–111、229–234）。clone/summon/shadow/zombie 为已列出的动态实体类型（API:58、68），不根据名字识别。

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

编码函数签名只接受 state 与冻结配置，采用字段白名单；数据适配器把 labels 和审计键放在独立通道。已存在的生成／读取边界见 A:28、67，G:211–223，D:97；下表规定新增 encoder 的第二道阻断。

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
| `template/runtime.at_boost_bits, attract_bits`、`accumulate.acc_bits, charge_bonus_bits`、`hide.attract_bits`、`poison.atp_bits`、`clone.name_factor_bits, child_name_factor_bits`、`adjustments.at_boost_delta_bits, attract_delta_bits` | 契约采用 `f64::from_bits(u)` 后归一化，同时保留原始 bit 旁路；poison 的 f64 实例见 M:906；`clone.name_factor` 的还原实例见 C:208；其余底层产生式须在实施前核对，不能仅凭字段名完成验收 |
| `PlayerKindFlags` 原始 u64、`score_skill_boost_plan.initially_boosted_mask` | 只走精确旁路与 X 的 bits 通道，不作为数值特征；具名位另由 `entity_kind_flags` 表达 |
| 上述原始 bit、注册游标／runtime_registration_order／deferred.state_cursor | 精确旁路按 `lo=(u & 0xffffffff), hi=(u >> 32)` 存两个 u32；还原 `u=(u64(hi)<<32) | u64(lo)`，禁止经 JS Number／f32 中转 |
| `hook_mask`、`compressed_state_flags` | 分别展开 64／8 个 0/1，`bit_i=(u>>i)&1`；不得压成一个 f32 标量 |
| `ModelSlot.i64_value/u64_value`、外部复合类型的 bit／整数 | 先查语义表；计数用 N，ref 重映射，浮点 bits 用 from_bits，标志按位展开；i64 精确旁路按二补码 u64 拆分 |
| `at_boost_millionths` | 精确整数可从旁路还原；本稿不假定 `bits == millionths/1e6`，不使用近似列反推原 bits（M:455–456、776–777） |

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
| 网页播放 | 生成对应帧时保存预测及 generation/frame 关联，播完该帧再展示；session 释放后仍可读缓存（A:92；API:110、136） |

导出通道规定为独立、可删除重建的编码包：`encoder-manifest.json` + 每批 `batch-manifest.json` + 按张量名称保存的小端 C 连续 `.bin`。批 manifest 记录 dtype、shape、字节数、摘要和实际 B；最后一批使用实际 B，内部容量不变。JSON 只描述编码包，不装载 state 或整块特征内容。

features、精确旁路、labels、审计关联表四个清单分开：labels 为 i32 `[B]` 的 `winner_team_index` 与 u8 `[B]` 的 `label_mask`，空标签存 -1/mask=0；默认训练导出过滤空标签，诊断导出可保留。审计关联含源分片／行号及帧 ID，只用于追溯，不可被训练代码的 features 清单枚举到。

缓存键包括原始数据摘要、schema、encoder 版本、profile、词表／归一化 manifest 摘要；更改任一项必须重编码，禁止混批。导出错误返回行定位与字段路径，不把错误行静默删除。文件容器和张量契约不要求新增 crate，本轮也不建立这些目录或接口。

补充序列化约定：owner_scope 的 1–9 依次为 global/entity/template/state/slot/list/input_team/runtime_team/lane；global owner 固定为 0，其他 owner 必须引用有效行。field_class 采用冻结字段表编号，有效类从 1 开始，已发布编号只允许追加、不允许重新解释。extra 的四元索引由 field_class 确定 value_type，num/ref/bool/cat/bits 只允许一条有效通道。`slot_index.scope` 独立使用 1=entity、2=template-global、3=battle-global，后两者 owner=0；完整槽语义词表仍是第 16 节待确认项。第 3.1 节用到的 X field_class 暂定 1=`assassinate_fixed_lane`（ref→lane）、2=`protect_level`（num）、3=`clone_initial_boosted_mask`（bits）、4–7=`clone_slot_boost_{0,1}` 的两个 u8（num），与槽语义词表各自独立编号。

## 15. 测试计划（先于实现）

先准备下表的输入与期望，再实现 encoder；本轮不编写或执行 Rust 测试。以下是未来验收标准，不是测试已通过声明。

| 类别 | 必须覆盖与判定标准 |
| --- | --- |
| 字段覆盖／版本 | 每个 M 中字段有消费或有理由的排除；新增字段、未知 kind、43–50 未启用技能、schema 不符均报错；不允许 wildcard 吞新字段 |
| 引用／关系 | 稀疏 EntityIdx、预留空槽、同号不同域、死亡 owner、悬空引用；有效引用可还原到同一源实体，非法引用报字段路径 |
| 数值／缺失 | 0、None、PAD、负属性、极大整数、NaN/Inf；所有成功张量有限，presence 与值独立；min/p50/p99 和裁剪边界按 manifest 计算 |
| bit 精度 | 0、负零、f64 正常值、超过 `2^53` 的整数和 u64 全 1；lo/hi 往返逐 bit 相等，hook/压缩位逐位一致 |
| 技能 | 重复 skill_id、多 lane、零等级、三种 boost、四类执行顺序及 deferred；每条 lane 和每条引用恰好保留一次，固定键不与 skill_id 混用 |
| 状态／压缩 | 已映射的 11 个 kind、稀疏状态、所有可空引用；Boss 分支（12–16）必须返回 `UnsupportedPayloadKind`；kind／分支不匹配拒绝，压缩状态独立存在 |
| 蓝图／复合类型 | 实体、全局模板、battle 三域槽；clone_build、policies、charge/accumulate/hide/assassinate、corpse 与 protect_from 逐叶映射，第 3.1 节每个叶子至少一条断言；新增叶子未进表即失败 |
| 变长／容量 | E=6/15/30/32/33，实体槽 31/64/65，T=2/3/32/33；E=33、槽=65、T=33 报错；其他维测试 limit−1/limit/limit+1 |
| 资格／终局 | 真实队伍零存活仍 mask=1；复活前后 mask 不变；全灭无 winner 不伪造终局；winner 非空拒绝；null 标签无监督权重 |
| 置换／批布局 | 实体重排并修正引用、队伍轴置换、padding、不同 batch 邻居；整数/bit 精确相同或等变，后续概率撤销置换后绝对差 ≤ `1e-6` |
| 机制顺序 | 改行动顺序、游标、技能执行列表、注册序，编码必须保留可辨差异；不要求未来模型对每次修改都给不同概率 |
| 防泄漏 | 固定 state 改 seed/progress/split/label/原输入/未来帧，features 字节完全不变；只有标签／审计通道可变 |
| 跨通道一致 | 同 state/manifest 在 Rust、Parquet 导出、Python/WASM 绑定：整数、mask、bit 完全一致；f32 绝对差 ≤ `1e-6`，同一原生构建重复编码逐字节一致 |
| 分布与支持声明 | 分 train/validation/test、队伍数报每维峰值、溢出率、裁剪率、截断率；Boss/DIY 计入不支持域；声明支持的数据域要求容量错误数=0，未覆盖域明确标记 |

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
| 槽编号及整数语义 | 按注册表白名单分 num/ref/bits；或按 U64 一律数值化 | 建议白名单；M:56 仅证明存储类型，未给每个 slot_id 含义，冻结前逐槽核对 |
| 顺序、身份和阵营域 | 保留精确键并补查消费者；或假定所有编号都是下标 | 建议前者；确认各技能顺序整数域、PlrId 是否参与排序／出生、charm.group_id 与 clan/runtime team 是否同域，不能猜 |
| 浮点产生式 | 尺度按 train 实测拟合（已定）；仍须核对各 `*_bits` 的 f64 产生式与 millionths 关系 | 无观测即 `MissingCalibration`，不登记人工常数；不假定 `bits == millionths/1e6` |
| 容量 profile | baseline-32；或补测后单独推出更大 profile | 建议先评估本稿 32／128／4096／8192 预算，实体／蓝图／slot／X 峰值全量统计后冻结；不使用静默截断 |
| 资格 mask | 全部输入队伍；或证明复活／召唤不可达后屏蔽 | 建议全部保留，采用第 10 节公式；避免把本池 `0/40000` 的未见事件当引擎保证（W:169） |
| 导出容器 | typed bin + manifest；或后续绑定直接返回 buffers | 建议先提供可重建离线包，绑定复用同一布局；两者不得产生第二份编码算法 |

阅读中发现的描述差异／覆盖缺口（仅记录，不修改原文件）：

1. D:77 说明 bench 调用 generate/validate，D:86 却说“两条子命令都不修改数据”；G:138–182 明确生成文件。应区分 stats 只读与 bench 生成输出。
2. P:126 把 seed 列列在 samples 字节构成中；G:215–224 的 SampleRow 构造无 seed，G:243–250 的 BattleRow 才有 seed。报告口径／历史 schema 来源待确认，本稿不改原数据格式。
3. API:10、13 概括数据集走根级 Runner，但 G:195、208 直接通过 BattleModelSession 两遍运行；文档缺少该专用状态入口的分工说明，并非已证明底层路径冲突。
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
| E 扩展注册表 | `crates/tswn_core/src/runtime/extension.rs:109`（PlayerKindFlags）、`:113`（具名位）、`:158`（PlayerKindPolicies） |
| R 实体运行时 | `crates/tswn_core/src/runtime/entity/runtime.rs:33`（MoveState）、`:38`（PlayerPolicyOverrides）、`:74`（ProtectLinkRuntime）、`:80`（HideRuntime）、`:89`（AssassinateRuntime）、`:103`（RuntimeCorpseKind）、`:359`（ChargeRuntime）、`:366`（AccumulateRuntime） |
| C 实体与分身 | `crates/tswn_core/src/runtime/entity.rs:29`（CloneStatAdjustments）、`:63`（ScoreCloneSkillBoostPlan）、`:70`（CloneBuildData）、`:182`／`:208`／`:211`（访问器） |
| Y 实体状态 | `crates/tswn_core/src/runtime/entity/state.rs:17`（CovidInfectionEntry，本轮不映射） |
| A 总体决策 | `docs/design/battle-analyze.md:13`（状态）、`:52`（Parquet）、`:81`（Rust 单一 encoder 与网页约束） |
| U 审计 | `docs/design/battle-model-state-audit.md:5`（字段）、`:25`（身份／蓝图）、`:37`（排除）、`:50`（校验） |
| W 判胜 | `docs/mechanics/winner.md:17`（判胜）、`:110`（粘性计数）、`:159`（复活／资格） |
| API 绑定 | `docs/reference/public-api.md:5`（入口）、`:54`（DTO）、`:90`（WASM）、`:134`（播放） |
| P 实测 | `docs/perf/reports/winprob-dataset-scale-baseline.md:27`（环境）、`:117`（字节口径）、`:146`（分布）、`:177`（encoder 建议） |
| D 生成器说明 | `crates/tswn_winprob_dataset/README.md:38`（标签）、`:68`（统计／基准）、`:89`（Python） |
| S 统计口径 | `crates/tswn_winprob_dataset/src/stats.rs:45`（Series）、`:130`（collect）、`:187`（实体／状态／技能计数） |
| F 抽样 | `crates/tswn_winprob_dataset/src/sampling.rs:5`（select，排终局与最终轮数分桶） |
| G 生成 | `crates/tswn_winprob_dataset/src/generate.rs:138`（write_shard）、`:186`（generate_battle）、`:202`（标签）、`:211`（样本） |
