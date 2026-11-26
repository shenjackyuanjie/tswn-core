# 东方 Boss 第二批移植计划

状态：已批准。主线基线 `69ea56df`（第一批已合入），只读来源 `kodu16/main`（`cf460c84`）。按[第一批方案](touhou-batch1-plan.md)的注册、运行态和专属行动结构接线；先审计划，再改 Rust、验证、提交和双推。

## 范围与来源

源码行号相对 fork 的 `crates/tswn_core/src/runtime/combat/boss/`，均通过 `git show kodu16/main:<path>` 核对。保留所移植的中文注释并标注 KODU16 来源。

| Boss | 规范标识 | fork 源码 | 本批行为 | 专项用例 |
| --- | --- | --- | --- | ---: |
| 博丽灵梦 | `reimu@!` | `reimu_marisa.rs:4–75` | 梦想天生蓄梦、二重结界、梦想封印驱散、对战斗使魔的伤害倍率。 | 2 |
| 雾雨魔理沙 | `marisa@!` | `reimu_marisa.rs:76–158` | 八卦炉存能与消耗、偷取属性、彗星两段与行动点变化。 | 1 |
| 大妖精 | `daiyousei@!` | `scarlet_support.rs:115–195` | 牺牲复活、与首批琪露诺互动、治疗与疾走净化、双妖精、被动抵消。 | 2 |
| 红美铃 | `hong@!` | `scarlet_support.rs:196–255` | 气场、气值积累、两段蓄气和连击；守护沿用普通被动。 | 1 |
| 小恶魔 | `koakuma@!` | `scarlet_support.rs:256–305` | 逆召唤、死灵之书、两类属性削弱，削弱带命中效果的入伤。 | 1 |
| 帕秋莉 | `patchouli@!` | `scarlet_mansion.rs:4–104` | 五行依赖、永久属性增量、贤者之石、迟缓和燃烧。 | 2 |
| 十六夜咲夜 | `sakuya@!` | `scarlet_mansion.rs:105–168` | 时停、残影、自身敏捷增长和连击；临时改动的敌人防御、敏捷在连击后恢复。 | 1 |
| 蕾米莉亚 | `remilia@!` | `scarlet_mansion.rs:169–221` | 气场、鲜红印记提高出入伤、两类吸血。 | 1 |

共享来源：`touhou.rs:1–455` 只提取本批分支；`scarlet_support.rs:5–113` 只提取 1～3 号召唤与行动；`runtime/entity/rosmontic.rs:1–79`（相对 `crates/tswn_core/src/`）只提取本批字段与等级。`touhou_tests.rs:1–336` 共 11 个专项用例和 1 个共享整场用例。

## 六项新增机制与主线插入位置

以下行号对应主线 `69ea56df`。

| 机制 | 插入点 | 范围、顺序与行为 |
| --- | --- | --- |
| 1. `PlainAttackOnDamage::Disperse` | 在[战斗回调枚举](../../crates/tswn_core/src/runtime/combat/mod.rs#L101-L114)加变体，在[普攻命中回调](../../crates/tswn_core/src/runtime/combat/attack.rs#L886-L943)接现有 `apply_disperse_hit_into`。 | 梦想封印先过东方免疫判定，再清东方气场／结界及铁甲、显式护盾、压缩护盾（同步清 `runtime.shield`）；这一步不扣 MP，回避仍可发生。最终正伤害才触发通用命中回调并扣 MP；不改原生驱散攻击路径。 |
| 2. `RosmonticBurn`、存储和跳伤 | 同一[命中回调](../../crates/tswn_core/src/runtime/combat/attack.rs#L910-L938)接帕秋莉的施加函数；在[共享运行态](../../crates/tswn_core/src/runtime/entity/rosmontic.rs#L4-L16)记录施法者、未结算威力和剩余次数，由行动末钩子读取。 | 折算伤害大于 4、目标 HP 正且未触发 `firex` 免疫才点燃；初次 1 次、再次叠威力并重置为 2 次。普通单位按需分配共享运行态，燃烧或属性覆盖本身不赋予专属行动。按 fork 用目标魔力换算，先更新剩余量再走既有[伤害](../../crates/tswn_core/src/runtime/combat/attack.rs#L845-L855)和[致死链](../../crates/tswn_core/src/runtime/combat/infection/mod.rs#L483-L502)，不再走回避或重新点燃；大妖精支援清燃烧；仅清燃烧不额外提交通用状态钩子。 |
| 3. `begin_rosmontic_action` / `end_rosmontic_action` | [真实行动准备段](../../crates/tswn_core/src/runtime/combat/round.rs#L187-L212)通过状态劫持／冰冻门槛后，接在 `begin_squall_action` 后、`prepare_plain_action` 前；[行动后段](../../crates/tswn_core/src/runtime/combat/round.rs#L350-L365)紧跟 `drain_post_action_chain_into`，在 `quilt_post_action` 和 `UPDATE_END` 前。 | 开始阶段推进灵梦梦想和咲夜敏捷／时停；梦想在施放后第六次自身行动开放，咲夜的时停从 1 开始、后续自身行动计数超过 4 时结束。结束阶段先推进灵梦结界（施放回合是四次之一，自然到期扣 128 行动点），再结算行动者燃烧。保留终局短路及主线对状态劫持仍执行行动后链的行为，不在调度步进、敌人回合或冰冻短路中推进；采集与快速路径共用。 |
| 4. 东方驱散、反射／护盾、额外伤害修正 | [通用驱散](../../crates/tswn_core/src/runtime/combat/defense.rs#L18-L39)在 Squall 自身清理后接东方清理；[PRE_DEFEND](../../crates/tswn_core/src/runtime/combat/defense.rs#L112-L128)开头接大妖精；[伤害换算](../../crates/tswn_core/src/runtime/combat/attack.rs#L774-L818)接小恶魔；[POST_DEFEND 尾段](../../crates/tswn_core/src/runtime/combat/defense.rs#L515-L539)在有序防御（含普通护盾）之后、既有 Squall 尾处理之前接灵梦和蕾米莉亚。 | 驱散清灵梦结界和红美铃／蕾米莉亚气场及印记，保留梦想、五行、已消耗槽位和属性覆盖；纯东方清理只重算面板，不额外提交挂载钩子。大妖精先验证来源存活，掷 `byte < 85`，再按 fork 双半字节公式抽 MP 消耗；成功让本次入射威力归零，不新增反击或递归。小恶魔在除以防御后、进入常规 `POST_DEFEND` 前对非空命中效果或非 `Default` 伤害种类取 `13/20`；把打向小恶魔的普通物理 `SklAttack` 纳入既有分类。灵梦对剩余正伤害先取 `4/5`，再用结界将至多 100 的结果压成 1，超过则扣 100 并破结界；蕾米莉亚每枚印记增加 75% 入伤并向下取整。保留黑雾、首批免伤、护盾登记顺序和 Squall／实验体的既有处理。 |
| 5. 共享属性、有效速度和威力覆盖 | [面板重算](../../crates/tswn_core/src/runtime/entity/runtime.rs#L652-L710)通用升级与露米娅修正之后、潜行快照之前恢复倍率及加量；[有效速度](../../crates/tswn_core/src/runtime/entity/runtime.rs#L593-L606)仅对速度有 Rosmontic 覆盖的单位切换基值；[攻击倍率重算](../../crates/tswn_core/src/runtime/entity/runtime.rs#L867-L888)合入气场倍率再同步两种存储表示。 | 复用 `ros_stat` 记录魔理沙偷取／加速、帕秋莉五行、小恶魔削弱、咲夜成长及大妖精复活增益。红美铃气场提高防御和威力；蕾米莉亚用印记调攻魔、速度和威力，强蓄力附加倍率在施放时快照。有效速度仍应用现有疾走／迟缓／懒感染顺序，在使用已含升级的运行态速度时不再叠加垂死速度。保留普通单位的原有速度计算、未提交疾走／魅惑及所有既有 `commit_update_hooks` 边界。 |
| 6. `RosmonticAbsorbFifth` | 在同一[命中回调](../../crates/tswn_core/src/runtime/combat/attack.rs#L910-L968)加入变体并复用 `apply_absorb_on_damage`。 | 碎心、冈格尼尔按落地的正伤害值（而非剩余 HP）向下取整回复五分之一；沿用施法者存活、最大 HP 和日志规则。绯红之击仍走现有半额吸血，不改通用回调。 |

本批只用东方脚本自身的状态免疫抽样（`ros_immune`，含 BOOST 的整字节抽样）；不改现有通用免疫抽样。帕秋莉水系复用既有迟缓，于本招命中清算中修正为单层。表中禁止移动的提交边界，也不通过移植 fork 的重算函数间接改变。

## 注册、行动与跨批依赖

1. 在[规则表](../../crates/tswn_core/src/namerena/rules.rs)注册八个身份及显示名、默认攻击分类、免疫条目；红美铃复用守护槽 26（等级 200）。在[固定面板](../../crates/tswn_core/src/namerena/build.rs)为八个 Boss 加 fork 八围与实战速度（八围速度 +160），仅对已解析为 Boss 的身份生效。原生与混合导入复用第一批构造器，不改冴月麟的负攻魔豁免。
2. 在[Boss 模块入口](../../crates/tswn_core/src/runtime/combat/boss/mod.rs)声明三组脚本、`touhou` 共享模块及测试；[运行态初始化、复用和复位](../../crates/tswn_core/src/runtime/entity/runtime.rs)沿用可选装箱、场前重设和快照复制。只为本批身份预置技能等级，保留首批状态字段和反序列化默认值；不增加通用 `StatePayload`。
3. 扩展[既有 Rosmontic 判定](../../crates/tswn_core/src/runtime/combat/boss/rosmontic.rs#L5-L20)接八个正式 Boss 及本批三种有显式角色标记的召唤物；[prepare_plain_action](../../crates/tswn_core/src/runtime/combat/round.rs#L581-L610)仍构造带 `smart` 的 `PreparedBossSkillAction`；在[专属技能派发](../../crates/tswn_core/src/runtime/combat/skills/boss.rs#L165-L177)把 `prepared.smart` 交给本批选择与结算，不重新抽样，保留首批专属入口的行为。保留 fork 的技能槽顺序、MP 抽样、选敌与空目标检查；魔理沙／红美铃第二段先于常规抽签，无需重复付 MP。
4. 大妖精复活按主线[复活路径](../../crates/tswn_core/src/runtime/combat/skills/lifecycle.rs#L93-L106)恢复满血和 `alive`，并重新加入行动与存活名单；若目标为正式琪露诺（另保留 fork 的 `lilywhite@!` 识别，不注册新 Boss），攻、防、速、敏、魔、抗翻倍并保存覆盖，智慧与 MP 不变。大妖精最后走[既有自身死亡链](../../crates/tswn_core/src/runtime/combat/effects.rs#L675-L698)。不改琪露诺的独立脚本、冰免或绝对零度规则；验证复活后仍能走其专属行动，面板重算不擦掉翻倍。
5. 妖精、逆召唤和残影复用[幻影蓝图预览](../../crates/tswn_core/src/runtime/prepared_init/seed.rs#L159)与[带消息的生成队列](../../crates/tswn_core/src/runtime/combat/effects.rs#L399-L412)，沿用队列的所属队伍、所有者、存活登记。仅覆写本批面板、行动点、名称和角色，清空普通幻影技能与伤害分摊；小恶魔的逆召唤「姆Q」独立掷失败骰，并按施法者魔力对目标智慧判定回避；确认命中后与通用魅惑共用从 `drain_plain_charm_skill_into` 提取的 `apply_plain_charm_hit_into`。共享函数不抽随机数：首魅后走既有全量重算，重魅只更新来源并保留当前有效队伍，魅惑到期依主线原有同步清除。
6. 不接蕾蒂冻结延长／护盾、橙化猫、妖梦招架，也不接第三批其他行动／状态。逐点编辑 `round.rs`、`entity/runtime.rs`、`defense.rs`；不移植 fork 的 `POST_ACTION` 排序改写、Squall 重制、实验体改动、Clever/WASM 或图标素材。

## 用例与验收

- 逐例移植 12 个 fork 用例，调整为主线 API。共享整场用例检查八个身份均完成战斗、捕获／不捕获的一致性及重置恢复；用真实输入确认固定面板与注册生效。
- 补上 fork 单元直接调用未覆盖的接线断言：普通单位在真实回合于现有 `POST_ACTION` 后跳伤、仍走普通派发；烧伤致死移出世界；冰冻／他人行动不推进自身计时；梦想封印先破盾、命中后才扣 MP；大妖精反射抵消及琪露诺复活后的名单、六围及重算；咲夜时停豁免蕾米莉亚／同 Boss；验证有序防御先于结界及吸血舍入。确定性种子或现有测试接口优先，不靠重复试跑。
- 运行 `cargo test -p tswn_core --lib`：原 972 个及新增用例全过。运行 `cargo test -p tswn_test --features runtime-corpus`：冻结语料 126/126，不修改语料基线。
- 格式化只用 `cargo +nightly fmt`；`cargo +nightly fmt --check` 无输出。`cargo check --workspace --all-targets` 和追加 `--features no_debug` 均 0 error。`cargo clippy -p tswn_core --all-targets` 不高于基线（库 27、库测试 42，含重复）。
- 确认范围、工作区和远端后提交中文 Conventional Commit，加入 `Co-authored-by: gpt-6.2-sol <gpt-6.2-sol@openai.com>`；双推 `github`、`gitea` 并用 `git ls-remote` 核对提交。`kodu16` 仅只读，绝不推。用 noticer 向 `namerena_tech` 上报开始、计划暂停、阻塞及结束。
