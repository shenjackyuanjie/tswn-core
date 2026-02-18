# Plr（单位 / 角色）详解

目的
- 本文档面向准备将 Dart 项目重写为 Rust 的开发者，详细记录 `Plr`（单位） 的字段、关键方法、事件（Entry）列表的语义与调用时序、以及在重写时需要注意的借用/可变性与测试要点。
- 该文件基于 `namer-src/plr.dart` 的实现（请参照源码以确认具体行号），用于在重写时作为权威参考。

参考源码位置
- `namer-src/plr.dart` — Plr 的定义与实现（字段、构建流程、step/action、attacked/defend/damage/onDamaged/onDie 等关键函数）
- `namer-src/proc.dart` — 各种 Entry 的接口定义（如 `PreDefendEntry`, `PostDamageEntry`, `DieEntry` 等）
- 在文档示例中以代码标识符（如 `Plr::attacked`）表示相应行为点，供 Rust 签名参考。

总览（职责）
- `Plr` 储存单位的静态属性与运行时状态（属性值、HP/MP、技能、武器、条目列表）。
- 管理单位的生命周期：构建/升级、步进（`step()` -> `action()`）、技能执行、被攻击与防御、伤害应用及死亡处理。
- 提供多个可插拔挂钩（Entry 列表）以允许技能/武器/Boss 插入自定义行为。

一、重要字段（摘要）
请以源码为准，下列为常用关键字段与含义（字段名与用途尽量与源码一致）：

- 标识与显示
  - `idName`, `baseName`, `clanName`, `dispName`, `fullName`
- 属性（数值）
  - `atk`, `def`, `spd`, `agl`, `mag`, `mdf`, `itl`（智力 / 命中 / 其它按实现）
  - `hp`, `maxhp`, `mp`
  - `attr[]`（经过计算的属性数组）
- 状态、元数据与技能
  - `skills`（技能列表）
  - `sortedSkills`（用于选择的排序列表）
  - `actions`（可作为主动行为的 `ActionSkl`）
  - `dftAct`（默认动作）
  - `meta`（元数据 map，用于状态/效果）
- 行为计量
  - `spsum`（行动点累计器）
  - `step`（可能用于技能计时）
- 扩展点：各种 Entry 列表（下列列表是 Plr 行为插槽）
  - `presteps`
  - `preactions`
  - `postactions`
  - `predefends`
  - `postdefends`
  - `postdamages`
  - `updatestates`
  - `dies`
  - `kills`
  - 以及其他可能存在的 `presteps` / `presteps` 变体（请以源码实际字段为准）

二、构建 / 初始化顺序（非常重要）
重写时必须严格保留以下初始化顺序（原实现对顺序敏感，改变会导致数值/技能等级差异）：

1. `weapon.preUpgrade()` — 武器修改原始属性或技能等级
2. `initRawAttr()` — 初始化原始属性（基值）
3. `initSkills()` — 创建/赋值技能实例（包含技能 level）
4. `weapon.postUpgrade()` — 武器在技能创建后进一步影响
5. `addSkillsToProc()` — 将 `skills` 中的具有 `addToProcs()` 的技能注入 `Plr` 的 Entry 列表
6. `initValues()` / `updateStates()` — 根据当前 `meta` / `skills` / `weapon` 等计算最终属性

建议在 Rust 侧按此流程实现 builder / init pipeline；若无法精确一模一样，也需保证最终属性与技能等级可与 Dart 验证用例对齐。

三、行为主线（调用时序）
标准攻击与防御链（最常见）

- 调用入口：`Plr::attacked(atp, isMag, caster, ondmg, r, updates)`
  1. 运行 `predefends` 列表（按注册顺序）——每个 `PreDefendEntry` 可以修改 `atp`，或在某些情况下直接执行反弹等操作（例如 `SklReflect` 会直接调用 `caster.attacked(...)`）。
  2. Dodge 判定（`Alg.dodge`）——若闪避则结束，产出 dodge 类型的 `RunUpdate`。
  3. `defend(atp, ...)` —— 计算防御系数（`Alg.getDf`），得到 `dmg`，并执行 `postdefends`（可能进一步修改 `dmg`）。
  4. `damage(dmg, caster, ondmg, r, updates)` —— 应用 hp 减少，产出 `RunUpdate.damage`，调用 `ondmg` 回调（技能传入），最后触发 `onDamaged()`.
  5. `onDamaged()` 检查 `hp<=0`，若为真则调用 `onDie(oldhp, caster, r, updates)`，并在 `onDie` 中运行 `dies` 列表与 `Grp.die()`。

注意：上述顺序中的每一步都可能有被动技能 / weapon 注册的 Entry 插入副作用（例如 `postdamages`、`postactions` 等）。

特例分支（必须在实现中显式保留）
- 直接调用 `defend(...)`：某些技能/act 直接调用 `target.defend(...)`，跳过 `predefends` 与 `dodge`。
  - 示例：`act/disperse.dart`, `act/assassinate.dart`, `act/thunder.dart`，以及部分 boss。
- 直接修改 `hp`：某些技能直接将 `hp` 赋值或调整（如 `revive`, `clone`, `half`, `reraise`），这会绕过完整的 `attacked/defend/damage` 链。
- 直接调用 `onDie(...)`：自爆或强制死亡（例如幻影、召唤物自爆）直接进入死亡处理，绕过常规伤害计算。
- 召唤物伤害共享 / 伤害转移：召唤物 `postDamage` 可能会调用 `owner.damage(dmg ~/ 2, ...)`，导致 owner 的 `onDamaged/onDie` 在召唤物受伤时被触发。

四、Entry 列表语义（设计要点）
- Entry 列表是按顺序调用的可插拔钩子。重要语义如下：
  - 注册顺序决定调用顺序（append semantics）。
  - 在遍历时，Entry 可以在运行中移除自己或添加其它 Entry（原实现允许某些 self-unlink 操作）。
  - 在 Rust 中实现时必须保证遍历时对列表的修改语义与 Dart 一致（常见策略：遍历时复制列表快照；或记录延迟变更并在遍历结束合并）。
  - Entry 的具体接口应明确（例如 `PreDefendEntry::pre_defend(&mut self, atp, is_mag, caster, target, ondmg, r, updates) -> f64`）。

五、Plr API 建议（用于 Rust 重写时的最小签名建议）
以下为建议的 Rust 风格签名（供设计参考）：

- pub fn step(&mut self, r: &mut R, updates: &mut RunUpdates)
- pub fn action(&mut self, r: &mut R, updates: &mut RunUpdates)
- pub fn attacked(&mut self, atp: f64, is_mag: bool, caster: &Rc<RefCell<Plr>>, ondmg: OnDamage, r: &mut R, updates: &mut RunUpdates)
- pub fn defend(&mut self, atp: f64, is_mag: bool, caster: &Rc<RefCell<Plr>>, ondmg: OnDamage, r: &mut R, updates: &mut RunUpdates)
- pub fn damage(&mut self, dmg: i32, caster: &Rc<RefCell<Plr>>, ondmg: OnDamage, r: &mut R, updates: &mut RunUpdates)
- pub fn on_damaged(&mut self, old_hp: i32, caster: &Rc<RefCell<Plr>>, r: &mut R, updates: &mut RunUpdates)
- pub fn on_die(&mut self, old_hp: i32, caster: &Rc<RefCell<Plr>>, r: &mut R, updates: &mut RunUpdates)

说明：
- `OnDamage` 可以是一个 trait 对象类型或回调函数，代表技能/weapon 提供的额外伤害处理回调。
- 参数中的 `Rc<RefCell<Plr>>` 是示意性的，实际可根据项目对共享可变性的选择（`Rc<RefCell>` 或 `Arc<Mutex>`）调整，但必须支持在回调中访问并修改其它 `Plr`。

六、并发 / 借用 / 可变访问策略（Rust 注意事项）
- `Plr` 实例在技能反弹（`SklReflect`）或 `PlrSummon.postDamage` 调用 owner 的 `damage` 时，会出现跨对象调用（一个 Plr 会在另一个 Plr 的方法执行期间被调用）。因此在 Rust 中需要：
  - 使用 `Rc<RefCell<Plr>>`（单线程）或 `Arc<Mutex<Plr>>`（多线程，原实现为单线程建议使用 `Rc<RefCell>`）以允许可变借用跨调用。
  - 小心避免长时间持有 `RefMut` 造成后续访问冲突，尤其在 `attacked` 中调用 `caster.attacked(...)` 时可能需要临时放弃借用或使用内部分离策略（例如把必要的字段复制到栈上）。
- Entry 列表的遍历必须允许 entry 在运行时 unregister 自身或新增 entry。实现策略：
  - 遍历前复制列表（快照），在快照上执行；变更写入到原始列表或变更队列中。
  - 或者以索引递增遍历并允许在迭代过程中记录删除操作（deferred apply）。

七、RNG（RC4）一致性要求
- 原实现使用 RC4-based PRNG (`R`) 并存在多种消费方式（`r.r3()`, `r.r63()`, `r.r127()`, `r.r255()` 等）。
- 重写时必须：
  - 移植相同 PRNG 或在设计上提供与原实现完全等价的序列（包括初始化与每次调用的消耗）。
  - 在文档/代码中列出所有 RNG 消耗点并在测试中比对序列（见 `verify_checklist.md`）。
- 常见问题：若新增或省略任意一次 RNG 调用，之后所有概率相关行为会立即分叉，导致回放与日志不一致。

八、测试与验证建议（必须覆盖的场景）
建议实现以下最小测试集（使用固定 seed）：

1. 基础攻击链测试（无任何 proc） — 验证 `attacked->defend->damage->onDamaged` 的基础流程。
2. 反弹测试（`SklReflect`） — 验证反弹会调用 `caster.attacked` 并生成正确的 `RunUpdate` 顺序。
3. Counter / postdamage 场景 — 验证 `postdamages` 的 `onPostDamage` 可能注册延迟回调并在 `updates.onUpdateEnd` 时执行。
4. 直接 `defend` 场景（`disperse` / `assassinate`） — 验证跳过 `predefend` 与 dodge。
5. 直接修改 hp 场景（`revive` / `clone` / `half`） — 验证不会触发攻击链上的挂钩。
6. 召唤物伤害共享 — 验证召唤物受伤时会触发 owner 的 `damage/onDamaged/onDie`。
7. RNG 序列一致性测试 — 在一场完整回合中比对各次 `r.*()` 的返回值序列。

九、文档互链（维护建议）
- 把本文件与以下文档互链并保持同步：
  - `namer-src/docs/rust_design.md`
  - `namer-src/docs/verify_checklist.md`
  - `namer-src/docs/mermaid/*`（拆分后的时序图文件）
  - `namer-src/docs/proc_registration_locations.md`（将来生成的行号清单）
  - `namer-src/docs/proc_field_access.md`（将来生成的 proc -> 字段访问表）
- 每次在源码中增加/修改 `owner.<list>.add(...)` 或 `addToProcs()` 时，同步更新 `proc_registration_locations.md` 与 `proc_field_access.md`。

十、常见陷阱（Checklist 风格）
- [ ] 是否保留 `attacked` 与 `defend` 两个入口（不同语义）？
- [ ] 是否保持初始化顺序不变？
- [ ] Entry 遍历时 self-unlink 的语义是否与原实现一致？
- [ ] RNG 实现是否完全兼容（包括调用次数）？
- [ ] 召唤物与 owner 间的跨对象调用是否安全且序列一致？
- [ ] 是否为每个特例写了确定性单元测试并与 Dart 输出对比？

附：示例（行为说明片段）
- `SklReflect`（`predefends`）：
  - 在 `predefend` 阶段可能会直接调用 `caster.attacked(atp2, true, owner, ondmg, r, updates)`，并返回 `0.0` 表示原攻击被反弹/抵消。重写时必须允许 `predefend` 内部触发对其他 `Plr` 的 `attacked` 调用并保持调用序列一致。
- `PlrSummon.postDamage`（召唤物）：
  - 在召唤物被伤害时调用 `owner.damage(dmg ~/ 2, is_mag, caster, ondmg, r, updates)`，需要在 owner 上执行 `damage` 流程，从而产生 owner 的 `onDamaged`/`onDie` 等。

结束语
- `Plr` 是战斗引擎的核心。文档记录了需重点关注的字段、调用时序、Entry 列表语义、以及重写时常见风险。建议在开始 Rust 实现前先生成并审阅以下两个文件（行号清单与 proc 字段访问表），并据此逐步实现最小骨架与测试集。
- 如果你希望，我可以继续：生成 `proc_registration_locations.md`（行号清单）、`proc_field_access.md`（proc -> 字段/副作用表），或把上文建议的 Rust 签名转成初始代码骨架。请指示下一步。
