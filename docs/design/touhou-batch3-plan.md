# 东方 Boss 第三批移植计划

状态：已批准。主线基线 `612ca6b1`，fork 只读来源 `kodu16/main`（`cf460c84`）。沿用[第一批](touhou-batch1-plan.md)和[第二批](touhou-batch2-plan.md)接线。第三批补齐剩余 8 个，累计 20 个；先审计划，再改 Rust、验证、提交和双推。

## 范围与来源

下列行号相对 fork 的 `crates/tswn_core/src/runtime/combat/boss/`，由 `git show kodu16/main:<path>` 核对。保留所移植的 KODU16 中文注释并标明来源。

| Boss | 规范标识 | fork 源码 | 行为 | 专项用例 |
| --- | --- | --- | --- | ---: |
| 芙兰朵露 | `flandre@!` | `flandre_letty_chen.rs:4–87` | 三个分身、自身缩血、低血斩杀、莱瓦汀蓄力与燃烧。 | 2 |
| 蕾蒂 | `letty@!` | `flandre_letty_chen.rs:88–135` | 免冰伤、结晶护盾、冰招与一次性延长的冬日。 | 2 |
| 橙 | `chen@!` | `flandre_letty_chen.rs:136–201` | 化猫改写速敏与威力，按形态限制技能；多段击与行动点。 | 1 |
| 爱丽丝 | `alice@!` | `alice_ran.rs:5–85` | 人偶编队、舞台剧、自爆与重召。 | 2 |
| 幽灵乐团 | `phantom@!` | `phantom_youmu_yuyuko.rs:4–59` | 按人数减伤，分阶段减员和缩减生命上限；合奏支援或攻击。 | 2 |
| 魂魄妖梦 | `youmu@!` | `phantom_youmu_yuyuko.rs:60–109` | 一次性招架、反击连斩、连斩与驱散。 | 1 |
| 西行寺幽幽子 | `yuyuko@!` | `phantom_youmu_yuyuko.rs:110–163` | 高血蓄力被打断、低血蓄力保留；反魂蝶准备净化与缩血、首次致死回血及限时免伤。 | 3 |
| 八云蓝 | `ran@!` | `alice_ran.rs:86–137` | 前鬼后鬼、式神橙、幻神成长、神现与式神死亡返还。 | 2 |

共享源为 `pcb_hooks.rs:1–217`、`pcb_summons.rs:1–169`，以及 fork `touhou.rs` 的 PCB 分支、`runtime/entity/rosmontic.rs` 的 PCB 字段和初始等级。`pcb_tests.rs:1–342` 计 15 个专项用例和 1 个整场用例；`L343–354` 的 Squall 隐藏台词测试排除。整场用例保留采集／快速路径对比，复位另用公开 prepared API 验证。Squall 重制、Clever/WASM、图标素材和无关模块不移植。

## 主线插入点

1. **身份与派发。** 在[规则表](../../crates/tswn_core/src/namerena/rules.rs)补八个正式身份、显示名、攻击分类与状态免疫条目，在[固定面板](../../crates/tswn_core/src/namerena/build.rs)补 fork 八围和实战速度（八围速度 +160）。原生和混合导入沿用前两批入口。注册三组 Boss 脚本、PCB 共享模块和测试，在[共享运行态](../../crates/tswn_core/src/runtime/entity/rosmontic.rs)保存本批字段及等级，保留反序列化默认值。扩展[专属行动判定](../../crates/tswn_core/src/runtime/combat/boss/rosmontic.rs#L7-L21)，只接正式 Boss 与带本批实际角色号的战斗召唤物，普通单位仅有烧伤、神现等共享状态不会接管行动。[准备段](../../crates/tswn_core/src/runtime/combat/round.rs#L583-L612)和[结算段](../../crates/tswn_core/src/runtime/combat/skills/boss.rs#L164-L176)继续用 `PreparedBossSkillAction.smart`；不从 fork 搬回 `RosmonticRuntime.smart`，不重复抽样。在[东方派发](../../crates/tswn_core/src/runtime/combat/boss/touhou.rs#L210-L322)并入本批技能槽顺序、门槛与选敌；芙兰、幽幽子释放已蓄力技能时按该招选敌、不再付 MP，保留空目标保护。
2. **行动首尾与净化。** 把 PCB 首尾接入既有[东方行动钩子](../../crates/tswn_core/src/runtime/combat/boss/touhou.rs#L97-L156)：开头推进橙变形、妖梦未触发招架到期、乐团减员及反魂蝶；尾部在主线 `POST_ACTION` 链、东方结界和烧伤之后推进受益者自己的神现；不移动 `quilt_post_action`、`UPDATE_END` 或终局短路。仍走[实际行动](../../crates/tswn_core/src/runtime/combat/round.rs#L187-L205)与[行动后段](../../crates/tswn_core/src/runtime/combat/round.rs#L351-L367)的门槛，冰冻导致跳过时两端均不推进。反魂蝶和治疗人偶复用[主线东方净化](../../crates/tswn_core/src/runtime/combat/boss/touhou.rs#L186-L208)：同步移除负面状态并按既有规则刷新，清烧伤本身不提交挂载状态。自然魅惑到期仍走[同步清除](../../crates/tswn_core/src/runtime/handlers/states.rs#L127-L151)的 `clear_owner_state`；重魅保留主线只变更来源、不提前换有效队伍的口径，提交仍在[面板重算](../../crates/tswn_core/src/runtime/entity/runtime.rs#L643-L658)的 `apply_runtime_stats_from_template` 中。不移植 fork 的魅惑处理或 `POST_ACTION` 排序改写。
3. **冰伤与冻结延长。** 蕾蒂三种冰招、俄罗斯人偶冰枪显式传 `DamageKind::Ice` 与 `PlainAttackOnDamage::Ice`；桌灵转仍是非冰伤，诅咒按原招抽样。不把冰属性伤害免疫写成状态免疫。对实际命中且可冰冻的目标，[命中入口](../../crates/tswn_core/src/runtime/combat/defense.rs#L1037-L1061)叠加 `1024 + 1024 * frost_mode + 普通蓄力的 2048`，保留首次装载刷新、已有冰冻只加时的边界。延长的冬日使用模式 13，其余按技能的模式与复位规则。对蕾蒂的冰伤免疫在折算后归零，不触发冰冻命中回调；主动状态施加仍按主线免疫表。
4. **威力与神现面板。** 在[速度与面板重算](../../crates/tswn_core/src/runtime/entity/runtime.rs#L594-L658)的现有 Rosmontic 分支扩展橙的速敏变化和神现六围翻倍（不改智慧与 MP），使用同一有效速度门槛，避免重复计算已有垂死速度。在[攻击倍率重算](../../crates/tswn_core/src/runtime/entity/runtime.rs#L913-L945)合入化猫 ×2，同步位表示与百万分比。神现满三个自身行动（服从现有终局规则，可被驱散）；只改专属派生值时走不提交钩子的重算，净化等已有提交场景仍沿用原入口。
5. **防御、正伤与复活。** [PRE_DEFEND](../../crates/tswn_core/src/runtime/combat/defense.rs#L115-L143)在普通防御和回避之前按缺血比例抽取蕾蒂结晶护盾；[POST_DEFEND](../../crates/tswn_core/src/runtime/combat/defense.rs#L416-L461)先保留被子豁免，再在有序护盾前处理蕾蒂冰伤免疫、乐团按剩余成员每名减伤 15%、幽幽子免伤与妖梦招架。妖梦先消费招架标记、归零入伤并加行动点，再走略过回避但仍进入防御／致死的三段反击，反击期间仍执行其他防御，但不能消耗对方的招架。仅在[正伤受伤链](../../crates/tswn_core/src/runtime/combat/passive.rs#L23-L37)打断幽幽子高血蓄力；在[既有 DIE 链](../../crates/tswn_core/src/runtime/combat/defense.rs#L687-L716)的首批专属复活后、通用死亡钩子前尝试反魂蝶，首次致死恢复 10 HP，不走移除；免伤期过后的再次致死走正常死亡。逐点编辑 `round.rs`、`entity/runtime.rs`、`defense.rs`，不整段替换，不移动 `commit_update_hooks`。

## PCB 召唤与死亡钩子

- **同一原生生成入口。** PCB 使用[幻影蓝图预览](../../crates/tswn_core/src/runtime/prepared_init/seed.rs#L158-L175)，覆写固定面板、起始行动点、吸引力、内部身份、展示名；清原生幻影技能和伤害分摊，只有前鬼保留保护被动（100）。派发角色号为分身 `4`、人偶 `10..=18`、式神及幻神 `20..=23`。用原有 [`SpawnWithMessage`](../../crates/tswn_core/src/runtime/combat/effects.rs#L399-L412)生成并立即取回子体设置专属状态与初始生命。`owner` 是直接施法者、`root_owner` 来自施法者，基础队伍与 clan 由[生成入口](../../crates/tswn_core/src/runtime/entity/runtime.rs#L1058-L1107)继承；[世界登记](../../crates/tswn_core/src/runtime/world.rs#L246-L257)同步单队 roster、行动顺序、队内与扁平存活表。PCB 不另写世界队伍索引，也不把魅惑的有效队伍当子体的实际归属。
- **人偶重召基于实况。** 按直接 owner、存活、HP 和角色号查询。两个爱丽丝就算同队也不共享人偶；同一类人偶仍有存活者时禁止重复召唤，法兰西与奥尔良同类，最后一只死亡后重开。自爆走[自爆死亡链](../../crates/tswn_core/src/runtime/combat/effects.rs#L675-L698)，本体与自己的子体不作为自爆伤害对象。普通死亡移出行动与存活表，但历史 roster 在本局保留到完整复位。
- **式神返还接真实死亡。** 放在既有 DIE 链确认本次不能复活、HP 仍不大于零的出口，普通致死与自爆共用；角色 `20..=22` 标记 `child_death_notified` 后才向仍存活的直接召唤者返还对应面板增量，重入／重复死亡处理不叠加。幻神、其他子体、属主死亡带来的连带消失不返还。不另设子体销毁队列，不替换主线属主清理。

## 改变生命上限后的完整 prepared 复位

**沿用现成复位链，以整场回归证明其覆盖面；不新增一套模板快照。** 主线的 [`PreparedRuntimeRunner::reset_with_seed`](../../crates/tswn_core/src/runtime/runner.rs#L737-L778)先走[实体及共享状态复位](../../crates/tswn_core/src/runtime/runner.rs#L944-L963)。[实体表](../../crates/tswn_core/src/runtime/entity/runtime.rs#L1000-L1011)截断场中新增槽，然后由[实体记录](../../crates/tswn_core/src/runtime/entity/runtime.rs#L905-L911)恢复模板热字段、runtime、状态和实体槽。其中 [`PlayerTemplate::reset_battle_fields_from`](../../crates/tswn_core/src/runtime/entity/mod.rs#L640-L664)已明确恢复 `max_hp`，之后按本局 seed 重写队伍和行动点，不会用上一局的值覆盖上限。seed 从固定阵容[重填视图](../../crates/tswn_core/src/runtime/prepared_init/roster.rs#L500-L593)，经[交换入口](../../crates/tswn_core/src/runtime/prepared_init/seed.rs#L116-L127)替换 [`WorldArena` 行动／队伍／存活视图](../../crates/tswn_core/src/runtime/world.rs#L131-L181)，同时重建存活队索引、人数统计、游标和胜者状态。普通 roster 回退现有整表克隆；score 复用的[固定实体](../../crates/tswn_core/src/runtime/entity/runtime.rs#L1014-L1029)也经过同一个模板热字段复位。若回归发现复位缺口，立即暂停并报告具体层级；本批不自行修改 runner 或实体复位逻辑。

- **两个真实 prepared 场景。** 分别把幽灵乐团、幽幽子放进带爱丽丝的阵容，由该 Boss 的实际行动触发乐团减员／反魂蝶准备缩血，由爱丽丝真实召唤；各场复位前都要证明目标 `template.max_hp` 确实下降，子体曾登记。保留一个活子体并让另一子体走死亡链，覆盖同时存在的行动／存活条目和死亡后历史 roster 条目。芙兰分身也修改 `max_hp`，用同一复位入口补覆盖。整数缩血不得留下 HP 为 0 却仍登记存活的本体：幽幽子保留 fork 的下限 1；芙兰 1 HP 的非聪明分身也保留最后一点，并用边界用例验证。
- **复位前后两套断言。** 从同一个 prepared 原型创建全新 runner 作为下一 seed 的场前对照（至少一组不同 seed）；对复用 runner 调 `reset_with_seed` 后比较完整实体表、固定实体的 `template` 与 `runtime`、行动表、各队 roster／存活表、扁平存活表、队伍派生查询、回合数与胜者；逐个断言旧子体 ID 在实体表和所有世界视图中均不存在。再次走公开复位并开战，比对新鲜 runner 的战果、回合和末态，检测交换回 seed 缓冲区的旧索引不会跨两局重现。不能用 `PlayerRuntime::reset_battle_state_from` 单点检查代替。

## 测试与交付

- 移植 16 个 fork 用例，按主线 API 调整；在 8 个身份的整场用例中比对采集／快速版本的胜者、回合、实体和世界，覆盖身份识别与固定面板。另按上节添加完整复位用例。
- 蕾蒂用确定性命中查实际冰伤与普通／诅咒伤差异、护盾顺序、冻结 `frozen_step` 精确值（基底、延长的冬日、普通蓄力和已有冰冻叠加），不能只断言 `is_frozen`。橙以实际威力与速敏验证入形／退形及面板重算。妖梦覆盖未用招架到期、成功后耗尽、双方都招架不重复递归。
- 爱丽丝断言直接与根归属、同队多爱丽丝隔离、正常死亡／自爆后世界索引和重新召唤。八云蓝由致死或自爆让子体各返还一次，对同一子体再跑检查，断言防、抗、攻、速、敏的增量及重算后不再叠加；神现和幻神分别测时长与数值。
- **魅惑交叉：** 用真实行动让同一目标的魅惑在 `POST_ACTION` 到期且带烧伤：当轮先同步清魅、恢复原有效队伍，再跳烧伤；另让反魂蝶的行动开头净化同一个原本将在当轮到期的魅惑与烧伤：选敌时立刻恢复原队，当轮无第二次清魅、无燃烧伤害。保留重魅只更新来源、直到主线提交才换有效队伍的断言。
- **冰冻交叉：** 用真实调度让冰冻行动被跳过，验证反魂蝶准备／免伤或妖梦招架的行动前计数、神现与烧伤的行动后计数都不推进；解冻后本人第一次正常行动只推进一次，对手行动也不推进这些计数。不用循环碰概率替代稳定断言。
- 验收：`cargo test -p tswn_core --lib`（原 991 + 新增）；`cargo test -p tswn_test --features runtime-corpus`（126/126，不改基线）；只用 `cargo +nightly fmt` 格式化，`cargo +nightly fmt --check` 无输出；`cargo check --workspace --all-targets` 与同命令加 `--features no_debug` 均 0 error；`cargo clippy -p tswn_core --all-targets` 不高于库 27／库测试 42（含重复）。
- 验收后提交中文 Conventional Commit，附 `Co-authored-by: gpt-6.2-sol <gpt-6.2-sol@openai.com>`。核对 remote 后推送 `github` 与 `gitea`，通过 `git ls-remote` 核对同一提交；`kodu16` 绝不推。通过 noticer 向 `namerena_tech` 上报开始、计划完成并暂停、阻塞与结束。
