# 验证清单 — Rust 重写（verify_checklist.md）

目的
- 为把当前 Dart 实现重写为 Rust 提供可执行的“验证清单”（checklist + 验证步骤 + 验收准则），确保重写后在行为上与原实现等价（尤其关注事件调用顺序、属性读写、RNG 消耗与特例分支）。
- 此文档面向重写实现的开发者与审核者，尽量把检查点写成可操作的测试/检查步骤。

范围
- 覆盖 Plr 行为链（`attacked`/`defend`/`damage`/`onDamaged`/`onDie`）、Entry（`predefends`/`postdefends`/`postdamages`/`preactions`/`postactions`/`dies`/`kills`/`updatestates` 等）注册与遍历语义、RNG（RC4）消费、以及四类关键特例（直接 `defend`、直接修改 `hp`、直接调用 `onDie`、召唤物伤害共享）。
- 不在此处实现代码；而是提供能用于验证与撰写单元测试 / 集成测试的清单与验收准则。

仓库参考（快速定位）
- `namer-src/plr.dart` — Plr 的字段与生命流程实现（step / action / attacked / defend / damage / onDie）
- `namer-src/proc.dart` — Entry 抽象类型（PreDefendEntry / PostDamageEntry / DieEntry 等）
- `namer-src/docs/rust_design.md` — 已有设计说明与高层 Mermaid 时序图（将以拆分的 mermaid 文件替代）
- `namer-src/docs/mermaid/` — 建议把不同时序图拆分到多个文件（参见下文文件列表）

高层验收准则（必须全部满足）
1. 行为序列等价
   - 在至少以下场景下，重写后的 Rust 实现应产出与原 Dart 实现相同顺序的 `RunUpdate` 事件（时间戳/延迟字段可不同，但事件顺序与文本必须一致）：标准攻击、被反弹、被闪避、被防御并触发 postdefend、触发 postdamage、复活、自爆/直接 onDie、召唤物伤害共享。
2. RNG 消耗等价
   - 对一组给定种子（RC4 状态），Rust 实现对外可复现的 RNG 序列（r3/r63/r127/r255 等调用顺序与次数）必须与 Dart 完全一致。任何 RNG 额外或缺失消费都视为不等价。
3. Entry 调用顺序与变更语义等价
   - `predefends` 在 `attacked` 前全部按注册顺序调用（若其中某个返回特殊值或直接触发其他 `attacked`，其副作用仍按原实现发生）。
   - `defend` 能被“直接调用”的语义保留（即部分 act/skill 直接调用 `defend(...)` 并跳过 `predefends`/`dodge`）。
   - 当遍历 entry 列表时，entry 可以在遍历中 unregister / 链式修改列表（重写需保证与原实现相同行为：是否延迟删除或即时生效需核对）。
4. 属性访问与借用语义安全
   - 列出每个 proc 在运行时读取/写入的字段（hp/mp/atk/def/agl/itl/etc），Rust 实现的所有可变借用（`&mut`）设计需支持这些访问模式而不引入竞争或死借用。
5. 特例分支完整覆盖
   - 四类特例（直接 `defend`、直接改 `hp`、直接 `onDie`、召唤物伤害共享）需在文档与实现中分别写明并针对性测试。

具体检查项（逐条，可作为 PR 审核清单）
A. 文档层（必须存在且内容完备）
- [ ] `namer-src/docs/verify_checklist.md`（本文件）已加入仓库并被团队认领。
- [ ] 新增 `namer-src/docs/plr.md`：详细列出 `Plr` 的字段、每个 Entry 列表的语义、`attacked`/`defend`/`damage`/`onDamaged`/`onDie` 的调用顺序与副作用点。
- [ ] 将原单一 Mermaid 时序图拆成多个文件，存放于 `namer-src/docs/mermaid/`：
  - `mermaid/01_standard_attack_sequence.md` — 标准攻击（attacked -> predefend -> dodge -> defend -> damage -> onDamaged -> onDie）
  - `mermaid/02_direct_defend_sequence.md` — 直接调用 `defend(...)` 的序列（跳过 predefend/dodge）
  - `mermaid/03_direct_hp_modify_sequence.md` — 直接修改 `hp` 的影响（绕过攻击链）
  - `mermaid/04_direct_onDie_sequence.md` — 直接触发 `onDie` / 自爆
  - `mermaid/05_summon_damage_share_sequence.md` — 召唤物伤害共享流程（召唤物 postDamage -> owner.damage）
- [ ] `namer-src/docs/rust_design.md` 保持同步，引用上面拆分后的 mermaid 文件。

B. 代码层（必须逐项核对并生成对应测试）
- [ ] 逐文件「行号清单」：列出仓库中所有 `owner.<list>.add(...)` / `addToProcs()` 的文件与精确行号，保存为 `namer-src/docs/proc_registration_locations.md`（或 csv）。该清单应包括每个注册点注册的 entry 类型（PreDefend/PostDamage/...）和被注册对象类型（Skill/Act/Boss/Weapon）。
- [ ] 对每个注册点，生成 proc 的行为摘要（proc -> 会读取/写入哪些字段，会调用哪些 Plr API，会否消费 RNG）。保存为 `namer-src/docs/proc_field_access.md`。
- [ ] 查出所有直接调用 `defend(...)`、`attacked(...)`、直接改 `hp`、直接调用 `onDie(...)`、以及召唤物对 owner 造成 `owner.damage(...)` 的代码位置并列表（包括行号）。
- [ ] 在 Rust 中设计 Entry trait 与 `Plr` 方法签名，并在文档中列出：`Plr::attacked(...)`（标准入口）与 `Plr::defend(...)`（直接进入防御）的签名必须同时保留。
- [ ] 实现或移植 RC4 PRNG；并在文档中列出所有 Dart 侧对 RNG 的调用点（r.r3 / r.r63 / r.r127 / r.r255 / r.r3?），包括行号。

C. 测试层（最少覆盖的自动化测试）
- Basic correctness tests（ deterministic with fixed seed）：
  - [ ] 标准攻击流程：同一初始状态与 seed 下，Dart 与 Rust 输出的 `RunUpdate` 列表（按顺序）完全一致（事件文本与事件顺序一致）。
  - [ ] 直接 `defend` 场景：验证跳过 `predefend`/`dodge` 的行为。
  - [ ] 直接修改 `hp` 场景：验证不会触发 `predefend`/`dodge`/`postdamages`，但会触发 `dies`/`reraise`（如适用）。
  - [ ] 直接调用 `onDie` 场景：验证立即进入死亡处理并触发 `dies` 注册项。
  - [ ] 召唤物伤害共享：在召唤物受伤时验证 owner 收到 damage 并触发 owner.postdamages/onDamaged/onDie。
- RNG sequence tests：
  - [ ] 对一系列典型战斗回合（包含不同技能/weapon），比较 Dart 与 Rust 在每一步的 RNG 消费记录（可在测试中把 RNG 调用先行 hook 成“consume counter”并导出）。
- Entry traversal tests：
  - [ ] 在 entry 的遍历过程中，若某 proc 在运行时 unregister（或新增 entry），重写实现产生的最终执行序列必须与 Dart 等价（写测试覆盖：proc 在 postdamage 中注册 preaction、自删除、链式注册等）。
- 边缘/回归测试：
  - [ ] 多并发触发（虽然引擎单线程），模拟复杂互动（反弹 -> 触发 counter -> summon -> owner.damage 链）并断言事件列表的一致性。

D. 验证步骤（PR 审核时的具体操作）
- 步骤 1（准备）：从 Dart 源构建并运行原实现，在若干固定 seed 下记录 `RunUpdate` 输出（建议导出为 JSON log）。
- 步骤 2（重写实现运行）：在相同 seed 下运行 Rust 实现，导出相同格式的 `RunUpdate` log。
- 步骤 3（差异对比）：
  - 比较两份 log 的事件数量、事件顺序与事件文本。任何不一致需回溯到代码查找 RNG 消耗或 entry 注册/遍历差异。
  - 若发生分叉（分支差异），通过在测试中启用 RNG 调试（输出每次 r.* 的返回值与调用序列）定位首次不同的 RNG 消费点。
- 步骤 4（修复与回归）：在 Rust 实现中修复差异，重新运行回归测试，直到通过所有「测试层」项。
- 步骤 5（审阅）：由另一名开发者审阅「行号清单」「proc 字段访问表」「测试结果」，并在 PR 描述中引用这些文档与测试结果。

E. 验收（合并到 main 的条件）
- [ ] 所有测试（见 C）通过（最好为自动化 CI）。
- [ ] 文档齐全：`plr.md`、行号清单、proc 字段访问表、mermaid 拆分文件都已提交并通过审阅。
- [ ] 在相同 seed 下的 10 个典型战斗回放中，Dart 与 Rust 输出的 `RunUpdate` logs 完全一致（可接受差别：非行为相关的注释/时间戳/延迟数字，但事件顺序与文本必须一致）。
- [ ] 代码审查通过（包含借用/引用策略、RNG 实现、entry 遍历实现、测试覆盖）并且 CI 通过。

附：建议文件与命名（仓库布局建议）
- `namer-src/docs/verify_checklist.md` — （本文件）
- `namer-src/docs/plr.md` — Plr 详细字段与方法签名（待新增）
- `namer-src/docs/proc_registration_locations.md` — 行号清单（每个 `owner.xxx.add` 的文件与行号）
- `namer-src/docs/proc_field_access.md` — proc -> 字段读写/调用表
- `namer-src/docs/mermaid/01_standard_attack_sequence.md`
- `namer-src/docs/mermaid/02_direct_defend_sequence.md`
- `namer-src/docs/mermaid/03_direct_hp_modify_sequence.md`
- `namer-src/docs/mermaid/04_direct_onDie_sequence.md`
- `namer-src/docs/mermaid/05_summon_damage_share_sequence.md`
- `namer-src/docs/tests/` — 测试用的战斗回放输入与期望输出（json logs）

注意 / 限制
- 本检查表可作为验收标准与测试驱动目标，但不是自动化脚本。要把这些检查转成可执行的 CI 步骤，需要将“记录 RunUpdate 的功能”与“比较工具”实现到测试框架中（建议用相同 seed 的回放模式并导出统一 JSON）。
- 我目前无法自动为你生成行号清单或 `plr.md` 文件（若需要我可以继续生成具体文档与测试样例），请在确认后指定下一步：例如 `生成行号清单`、`编写 plr.md`、`拆分 mermaid`、或 `生成测试用例`。

下一步建议（优先顺序）
1. 先生成并提交 `namer-src/docs/proc_registration_locations.md`（行号清单）。  
2. 基于行号清单生成 `namer-src/docs/proc_field_access.md`（proc -> 字段/副作用表）。  
3. 根据上两步产物生成 `namer-src/docs/plr.md`。  
4. 编写并运行最小的 deterministic 单元测试套件来比对 Dart 与 Rust 的输出。

如果你同意，我将按优先级 1 开始：生成行号清单。请回复 `生成行号清单` 或者选择其它下一步。