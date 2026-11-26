# 东方 Boss 第一批移植计划

本批以主线 `2391ee49`（core 0.8.0）为基线，从本地 `kodu16/main`（`cf460c84`）提取实现和测试，再接到现有战斗阶段。Rosmontic 是参考版本与共享模块名，不是第五个 Boss。

## 来源与行为

下表的 fork 路径相对 `crates/tswn_core/src/runtime/`，行号对应 `cf460c84`。移植时保留 KODU16 的注释并标注来源；冴月麟的防御使用该分支已包含的 `118f45e3` 修正。

| Boss | 标识 | fork 源码 | 本批行为 |
| --- | --- | --- | --- |
| 琪露诺 | `cirno@!` | `combat/boss/cirno.rs:1–183` | 冰属性连招，绝对零度改变自身魔力、智慧和发动概率；只免冰伤，仍可被冻结。 |
| 冴月麟 | `rin@!` | `combat/boss/rosmontic.rs:15–35`；`118f45e3:combat/defense.rs` | 固定有符号攻魔；第八次行动退场；在伤害换算后的 `POST_DEFEND` 归零，覆盖跳过回避的攻击。 |
| megia | `megia@!` | `combat/boss/rosmontic.rs:47–135` | 高体力阶段开始的蓄力会被实际伤害打断，即使该击跨过 199 HP 阈值；首次致死恢复 199 HP，第二次正常死亡。 |
| 露米娅 | `rumia@!` | `combat/boss/rumia.rs:1–127` | 黑雾从跳过回避的入口反击；月影双防和毁灭模式会在面板重算时恢复。 |

运行态来自 fork 的 `entity/cirno.rs:1–9` 和 `entity/rosmontic.rs:1–79`，后者只提取本批使用的字段。测试来自 `combat/boss/cirno_tests.rs:1–80` 和 `combat/boss/rosmontic_tests.rs:1–125`，共 11 个用例。

## 主线插入位置

源码路径相对 `crates/tswn_core/src/`。

| 文件 | 位置与理由 |
| --- | --- |
| [`namerena/rules.rs`](../../crates/tswn_core/src/namerena/rules.rs) | 名单、显示名、`BossActionSkill` 与状态免疫表注册四个身份。伤害免疫留在防御阶段，不与状态免疫共用。 |
| [`namerena/build.rs`](../../crates/tswn_core/src/namerena/build.rs) | `PlayerBuild::finish` 的固定面板段写四套八围与实战速度，负攻魔只写入冴月麟的有符号面板。 |
| [`runtime/entity/mod.rs`](../../crates/tswn_core/src/runtime/entity/mod.rs) | 在普通构造器旁新增 `allows_signed_namerena_stats` / `from_namerena_base`，并声明与导出两种运行态。 |
| [`runtime/profile/import.rs`](../../crates/tswn_core/src/runtime/profile/import.rs) | 混合 roster 的名竞分支先校验面板，再调用共享构造入口；临时模板的 Boss 标记来源于解析分类。保留临时模板的默认 kind，供原生初始化替换。 |
| [`runtime/prepared_init/init.rs`](../../crates/tswn_core/src/runtime/prepared_init/init.rs) | 原生 `template_from_namerena_player` 改用同一构造入口；正式玩家用注册表实际 kind 的标记，使魔路径不传 Boss 标记。 |
| [`runtime/entity/cirno.rs`](../../crates/tswn_core/src/runtime/entity/cirno.rs)、[`runtime/entity/rosmontic.rs`](../../crates/tswn_core/src/runtime/entity/rosmontic.rs) | 保存技能概率、回合计数、蓄力目标、一次性复活标记和露米娅形态，不新增 `StatePayload`。 |
| [`runtime/entity/runtime.rs`](../../crates/tswn_core/src/runtime/entity/runtime.rs) | 在 `from_template` 初始化；数字 score 复用时清空；场前快照复位时复制。`apply_runtime_stats_from_template` 在通用升级之后、潜行快照之前恢复露米娅变身和双防，不挪动 `commit_update_hooks` 边界。megia 直接使用已有行动点。 |
| [`runtime/combat/boss/mod.rs`](../../crates/tswn_core/src/runtime/combat/boss/mod.rs) | 声明琪露诺、Rosmontic、露米娅模块及相应测试；集中标注未接线的后续机制。 |
| [`runtime/combat/boss/cirno.rs`](../../crates/tswn_core/src/runtime/combat/boss/cirno.rs)、[`runtime/combat/boss/rosmontic.rs`](../../crates/tswn_core/src/runtime/combat/boss/rosmontic.rs)、[`runtime/combat/boss/rumia.rs`](../../crates/tswn_core/src/runtime/combat/boss/rumia.rs) | 提取专属行动。`has_rosmontic_action` 按 Boss 标记、名字和运行态共同判断；普通单位仅携带共享状态不会接管行动。 |
| [`runtime/combat/round.rs`](../../crates/tswn_core/src/runtime/combat/round.rs) | 在 `prepare_plain_action` 的专属派发段构造两个 `PreparedBossSkillAction`，均传入 `smart`。保留 `POST_ACTION` 的优先级与登记顺序。 |
| [`runtime/combat/skills/boss.rs`](../../crates/tswn_core/src/runtime/combat/skills/boss.rs) | 补齐扫描匹配，并在 `drain_plain_boss_skill_into` 进入两个结算器，由 Boss 脚本处理选敌和抽样。 |
| [`runtime/combat/defense.rs`](../../crates/tswn_core/src/runtime/combat/defense.rs) | `PRE_DEFEND` 入口接黑雾，反击跳过同一入口；`POST_DEFEND` 有序计划接冴月麟和琪露诺；`DIE` 中在主线实验体异常再生之后、常规死亡钩子之前接 megia 的一次性短路。 |
| [`runtime/combat/passive.rs`](../../crates/tswn_core/src/runtime/combat/passive.rs) | `drain_plain_post_damage_skill_chain_into` 的正伤害门槛之后接 megia 打断和露米娅变身，不接 Squall 红温。 |
| [`runtime/combat/boss/cirno_tests.rs`](../../crates/tswn_core/src/runtime/combat/boss/cirno_tests.rs)、[`runtime/combat/boss/rosmontic_tests.rs`](../../crates/tswn_core/src/runtime/combat/boss/rosmontic_tests.rs) | 移植 fork 的 11 例，并验证负攻魔范围、实际冻结与黑雾反击、跨阈值打断、第二次致死和普通单位的行动分派。 |

## 负攻魔的精确边界

三个条件须同时满足：构造入口具备 `PlayerKindFlags::BOSS` 且名竞解析分类为 `PlayerClass::Boss`；解析后的名字是 `rin`、队伍是 `!`，规范身份键恰为 `rin@!`；面板上的 `attack == -26 && magic == -26`。这里的“规范”指按现有解析器处理：用 `trim_js_line_end` 清除输入末尾和队伍段末尾的 JS 空白，分离 `+` 后缀，再用解析后的名字和队伍拼出身份键。豁免校验不额外剪空白，不做大小写折叠或别名替换，也不删除 `@!`。普通同名角色、其他队伍、缺少 Boss 标记或任一属性不是恰好 -26，都不走豁免。

## 等第二、三批使用时再接

| 延后接线 | fork 中的使用者 |
| --- | --- |
| `PlainAttackOnDamage::Disperse` | 博丽灵梦、魂魄妖梦。 |
| `RosmonticBurn`、烧伤存储及行动末结算 | 帕秋莉、芙兰朵露施加；受术者可以是普通单位。 |
| `RosmonticAbsorbFifth` | 蕾米莉亚。 |
| `begin_rosmontic_action` / `end_rosmontic_action` | 灵梦的梦想与结界、十六夜咲夜的时停、后续角色的专属计时及受术者烧伤；应在实际行动准备段和现有 `POST_ACTION` 链之后插入。 |
| 东方驱散、反射／护盾、伤害折算后的额外减免 | 灵梦结界、红美铃与蕾米莉亚气场、大妖精反射、蕾蒂护盾和妖梦招架等。 |
| 冻结延长、共享有效速度和威力覆盖 | 蕾蒂延长冻结；帕秋莉、橙等改写速度；红美铃和蕾米莉亚气场、橙的化猫改写威力。 |

首批四个 Boss 只用已有的 `None` 和 `Berserk` 命中回调，因此不在 `combat/mod.rs`、`combat/attack.rs` 或行动首尾加空分支。Squall 重制、Clever/WASM、图标素材，以及 [`runtime/mod.rs`](../../crates/tswn_core/src/runtime/mod.rs)、`model_state/mod.rs`、`prepared_init/mod.rs`、`combat/effects.rs` 的 fork 变化均不移植。

## 验收与交付

1. `cargo test -p tswn_core --lib` 通过原有核心测试和新增回归。
2. `cargo test -p tswn_test --features runtime-corpus` 冻结语料保持 126/126，不更新语料基线。
3. `cargo +nightly fmt --check` 无输出；`cargo check --workspace --all-targets` 和同命令加 `--features no_debug` 均通过。
4. `cargo clippy -p tswn_core --all-targets` 不超过基线：库 27 条，库测试 42 条（含重复）。
5. 验收后提交中文 Conventional Commit，推送 `github` 和 `gitea`，用 `git ls-remote` 核对两边提交一致。`kodu16` 保持只读。
