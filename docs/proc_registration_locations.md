namer-src/docs/proc_registration_locations.md#L1-9999
# Proc 注册点行号清单（proc_registration_locations.md）

目的
- 本文件列出仓库中已发现的向 `Plr`（owner） 各类 Entry 列表注册 proc 的位置（文件路径、注册的 entry 类型、行为简述）。
- 该列表基于源码检索与人工阅读的结果（不是动态运行结果）。后续建议由脚本生成精确行号并与本清单对齐（我可以在需要时生成）。
- 请把此文件作为迁移到 Rust 时的核对清单：每一条都应在重写时做 1:1 映射或进行明确替代说明。

说明（阅读要点）
- “注册点” 包含两类形式：
  1. 技能/特性通过 `addToProcs()` 将自己注册到 owner 的某个 entry（常见于 `skl/` 或 `boss/` 技能）。
  2. 在执行 `act()` 或状态添加方法里直接调用 `owner.<list>.add(...)`（常见于 `act/` 或 `weapon/`）。
- 本清单按模块（skl / act / boss / weapon / 其它）分组，每条列出：
  - 文件路径（相对于仓库根 `namer-src/`）
  - 注册到的 entry 列表（例如 `predefends`、`postdamages`、`dies` 等）
  - 简要说明：该 proc 的作用与可能带来的副作用（例如是否会调用另一个 Plr 的 API / 是否可能消费 RNG / 是否会在遍历中 unregister）
- 若需精确行号，请运行仓库搜索工具以导出 `owner.<list>.add` 与 `addToProcs()` 的具体行号（我可以帮你生成该清单并写成 `proc_registration_locations.md` 的机器可解析版本）。

索引（按模块）

1) skl（技能）
- `namer-src/skl/reflect.dart` — `owner.predefends.add(this)`
  - 类型：PreDefendEntry
  - 说明：伤害反弹。可能在 predefend 阶段直接调用 `caster.attacked(...)`（会产生跨 Plr 的嵌套调用/递归）；会修改 atp 并可能返回 0 表示完全抵挡。RNG：会用 r.r255 / r.c50 等判定。
- `namer-src/skl/counter.dart` — `owner.postdamages.add(this)`
  - 类型：PostDamageEntry
  - 说明：被动反击。postDamage 中可能注册延迟回调（onUpdateEnd），并在触发时调用 `lastTarget.attacked(...)`（消费 RNG 判定）。
- `namer-src/skl/defend.dart` — `owner.postdefends.add(this)`
  - 类型：PostDefendEntry
  - 说明：防御后修改 dmg（如减半），在 defend 完成后执行，改变最终 dmg。
- `namer-src/skl/hide.dart` — `owner.postdamages.add(this)`，`owner.preactions.add(this.onPreAction)`
  - 类型：PostDamageEntry / PreActionEntry / UpdateStateProc
  - 说明：隐匿相关，postDamage 时可能启用隐匿并注册 preaction；可能在 updateStates 中修改属性。
- `namer-src/skl/upgrade.dart` — `owner.postdamages.add(this)`、`owner.updatestates.add(...)`
  - 类型：PostDamageEntry / UpdateStateProc
  - 说明：升级/成长相关，postdamage、updateStates 会对属性产生影响。
- `namer-src/skl/protect.dart` — `owner.postactions.add(this)`
  - 类型：PostActionEntry
  - 说明：保护队友，在 action 后触发保护逻辑。
- `namer-src/skl/reraise.dart` — `owner.dies.add(this)`
  - 类型：DieEntry
  - 说明：复活效果。注册到 dies，当死亡时可能把 hp 置为正值并阻止移除或调用复活逻辑。
- `namer-src/skl/shield.dart` — 在某些分支中 `owner.postdefends.add(shieldState)` 与 `owner.preactions.add(this)`
  - 类型：PostDefendEntry / PreActionEntry
  - 说明：保护盾态，可能在 defend 后插入 shield 相关处理或在 action 前注册 preaction。
- `namer-src/skl/merge.dart` — `owner.kills.add(this)`
  - 类型：KillEntry
  - 说明：击杀后合并或成长（处理击杀后的特殊效果）。
- `namer-src/skl/*`（其他技能） — 若文件含 `addToProcs()`，则该技能会按照实现注册到对应 entry（需逐文件核对）。

2) act（主动技能 / 行为）
- `namer-src/act/assassinate.dart`
  - 注册：`owner.preactions.add(onPreAction)`、`owner.postdamages.add(onPostDamge)`
  - 说明：潜行/背刺两阶段，先注册 preaction 以改变下一步行为或延迟注册 postdamage 以完成背刺后处理。
- `namer-src/act/charge.dart`
  - 注册：`owner.postactions.add(onPostAction)`、`owner.updatestates.add(onUpdateState)`
  - 说明：蓄力类，在 action 后注册后置行为与状态更新。
- `namer-src/act/iron.dart`
  - 注册：`owner.postdefends.add(onPostDefend)`、`owner.postactions.add(onPostAction)`、`owner.updatestates.add(onUpdateState)`
  - 说明：铁壁，postdefend 用来在 defend 后额外修改 dmg 或状态。
- `namer-src/act/shadow.dart`
  - 注册：`owner.dies.add(shadow.onOwnerDie)`
  - 说明：幻影 / 关联在 owner 死亡时触发（在 owner's dies 列表上注册回调）。
- `namer-src/act/summon.dart`
  - 注册：`owner.dies.add(summoned.onOwnerDie)`（在召唤者上）与 `postdamages.add(onPostDamage)`（在 summoned 内部）
  - 说明：召唤物的实现会把自己的 `postdamages` 注册为转移伤害（把一部分伤害转给 owner）；并在 owner 上注册 summoned 的 onOwnerDie（owner 死亡时的副作用）。
- `namer-src/act/accumulate.dart`
  - 注册：`owner.updatestates.add(onUpdateState)`
  - 说明：聚气类状态在 updateStates 中生效。
- 其他 act（部分示例）
  - `namer-src/act/berserk.dart` — `target.preactions.add(this)`
  - `namer-src/act/charm.dart` — `target.updatestates.add(this)`、`target.postactions.add(onPostAction)`
  - `namer-src/act/curse.dart` — `target.postdefends.add(this)`、`target.updatestates.add(onUpdateState)`
  - `namer-src/act/haste.dart` — `target.updatestates.add(this)`、`target.postactions.add(onPostAction)`
  - `namer-src/act/ice.dart` — `target.updatestates.add(this)`、`target.presteps.add(preStepImpl)`
  - `namer-src/act/poison.dart` — `target.postactions.add(this)`
  - `namer-src/act/slow.dart` — `target.updatestates.add(this)`、`target.postactions.add(onPostAction)`
  - `namer-src/act/disperse.dart` — 直接调用 `target.defend(...)`（重要：绕过 predefends 与 dodge）
  - （注意：部分 act 会直接修改 hp、直接调用 defend 或直接调用 onDie，需单独列出并测试）

3) boss（Boss 特性）
- `namer-src/boss/boss.dart`
  - 行为：boss 的 `addToProcs()` 会遍历其 `skills` 并调用每个 skill 的 `addToProcs()`，从而把 boss 的技能统一注入到 owner 上（所以 boss 的技能会以技能的注册点出现在 owner 的 entry 列表）。
- 具体 boss 示例：
  - `namer-src/boss/aokiji.dart` — `addToProcs()` 会做 `owner.postdefends.add(this)`（Aokiji 的防御 / 吸收逻辑）
  - `namer-src/boss/ikaruga.dart` — `owner.postdefends.add(this)`（Ikaruga 的吸收奇数伤害）
  - `namer-src/boss/covid.dart` — `owner.postdamages.add(this)`（后置 damage 处理）
  - `namer-src/boss/lazy.dart` — `owner.postdamages.add(this)`（后置 damage）
  - `namer-src/boss/mario.dart` — `owner.dies.add(this)`（死亡时触发）
  - `namer-src/boss/saitama.dart` — `owner.postdefends.add(this.onPostDefend)`
  - `namer-src/boss/slime.dart` — `owner.dies.add(this)`（死后分裂/生成子体）
- 说明：boss 的 `addToProcs()` 通常在 boss 初始化阶段调用，使 boss 特性被注入到 boss 自身作为 `Plr` 的 entry 列表上。重写时可采用相同模式。

4) weapon（武器）
- `namer-src/weapon/deathnote.dart` — `owner.postdamages.add(onPostDamage)`
  - 类型：PostDamageEntry
  - 说明：死亡笔记武器在对敌造成伤害时注册后置处理（如捕捉目标、额外效果）。
- `namer-src/weapon/rinick_modifier.dart` — 在 upgrade/modify 过程中会对某些 skill 设置 level 并调用 `skl.addToProcs()`（间接注册）
  - 说明：weapon 常通过改变技能等级或直接创建技能实例并加入 `p.skills`，进而在 `addSkillsToProc()` 时把这些技能注入 owner。部分武器也会直接在 init 过程注册 entry（请核对具体实现）。

5) 其它 / 框架
- `namer-src/plr.dart`
  - 说明：定义了 `Plr` 的各种 `MList<...>` 字段：`preactions`、`postactions`、`predefends`、`postdefends`、`postdamages`、`dies` 等；并包含 `addSkillsToProc()` 方法会遍历 `skills` 并对 `level > 0` 的技能调用 `addToProcs()`（这是技能统一注册到 owner 的关键步骤）。
- `namer-src/proc.dart`
  - 说明：定义了 Entry 接口（`PreDefendEntry`、`PostDamageEntry`、`DieEntry` 等）与 `*Impl` 的封装，技能/weapon/boss 基于这些接口创建可注册的 proc。

建议的下一步（强烈建议执行）
1. 生成精确行号清单（机器可解析）
   - 目标文件：`namer-src/docs/proc_registration_locations.md`（本文件）以及 `namer-src/docs/proc_registration_locations.csv` 或 `.json`（便于自动化映射）。
   - 内容字段：`file_path, line_number, entry_list, symbol_name, brief_note`。
2. 为每个注册点生成“proc -> 字段访问表”（proc_field_access.md）
   - 说明每个 proc 在执行时会读/写哪些 `Plr` 字段（hp/mp/attr/ss/meta 等）、是否会调用其它 `Plr` 的方法（如 `attacked`/`defend`/`damage`/`onDie`）、是否消费 RNG、以及是否会在遍历中 self-unlink。
3. 将 `addToProcs()` 的调用点与 `owner.<list>.add(...)` 的直接注册点都纳入审核（避免遗漏）。
4. 为关键特例（直接 defend / 直接改 hp / 直接 onDie / summon damage share）单独列出所有触发源代码位置并写成测试用例（deterministic with fixed seed）。

快速备忘（已在代码中确认的注册点示例，非穷举）
- skl:
  - `namer-src/skl/reflect.dart` — `owner.predefends.add(this)`
  - `namer-src/skl/counter.dart` — `owner.postdamages.add(this)`
  - `namer-src/skl/defend.dart` — `owner.postdefends.add(this)`
  - `namer-src/skl/hide.dart` — `owner.postdamages.add(this)`, `owner.preactions.add(this.onPreAction)`
  - `namer-src/skl/reraise.dart` — `owner.dies.add(this)`
  - `namer-src/skl/merge.dart` — `owner.kills.add(this)`
- act:
  - `namer-src/act/assassinate.dart` — `owner.preactions.add(onPreAction)`、`owner.postdamages.add(onPostDamge)`
  - `namer-src/act/charge.dart` — `owner.postactions.add(onPostAction)`、`owner.updatestates.add(onUpdateState)`
  - `namer-src/act/iron.dart` — `owner.postdefends.add(onPostDefend)`、`owner.postactions.add(onPostAction)`、`owner.updatestates.add(onUpdateState)`
  - `namer-src/act/shadow.dart` — `owner.dies.add(shadow.onOwnerDie)`
  - `namer-src/act/summon.dart` — `owner.dies.add(summoned.onOwnerDie)`；`PlrSummon` 内 `postdamages.add(onPostDamage)`
  - `namer-src/act/accumulate.dart` — `owner.updatestates.add(onUpdateState)`
- boss:
  - `namer-src/boss/aokiji.dart` — `owner.postdefends.add(this)`
  - `namer-src/boss/ikaruga.dart` — `owner.postdefends.add(this)`
  - `namer-src/boss/covid.dart` — `owner.postdamages.add(this)`
  - `namer-src/boss/lazy.dart` — `owner.postdamages.add(this)`
  - `namer-src/boss/mario.dart` — `owner.dies.add(this)`
  - `namer-src/boss/saitama.dart` — `owner.postdefends.add(this.onPostDefend)`
  - `namer-src/boss/slime.dart` — `owner.dies.add(this)`
- weapon:
  - `namer-src/weapon/deathnote.dart` — `owner.postdamages.add(onPostDamage)`
  - `namer-src/weapon/rinick_modifier.dart` — 间接通过修改 skill 并触发 `skl.addToProcs()`
- 其它：
  - `namer-src/plr.dart` — `addSkillsToProc()`（扫描 skills 并把具有 `addToProcs()` 的技能注入 owner）
  - `namer-src/boss/boss.dart` — boss 初始化时枚举其技能并调用 `skl.addToProcs()`（统一注入）

结语
- 本文件为人工整理的初版“proc 注册点”清单，覆盖了代码库中多数显式注册点与常见示例。为了完整与准确地在 Rust 中做 1:1 映射，建议下一步自动化生成包含精确行号与 symbol 名称的清单（CSV / JSON），并基于该清单逐条实现与测试。
- 如果你授权，我接下来会生成机器可解析的行号清单（`proc_registration_locations.csv` / `.json`）并同时生成 `proc_field_access.md`（proc -> 读写字段表）。请回复 `生成行号清单` 或 `生成行号与字段映射`。
