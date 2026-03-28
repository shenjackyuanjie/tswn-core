# Storage 内部可变性方案对比分析

> 创建: 2026-03-28 | 对应 commit: `330cd46` (UnsafeCell 切换) + 当前 HEAD

## 1. 背景

`Storage` 是引擎的核心实体容器,以 `Arc<Storage>` 在整个战斗推进中共享。
由于技能回调、伤害链、死亡处理等路径需要同时持有多个 `&mut Player` 引用,
Rust 标准的借用规则(独占可变引用)无法直接满足需求。

项目历史上主线实际落地过三种方案:

| 阶段 | 方案 | 时间线 |
|------|------|--------|
| A | 直接 `unsafe` 强转 `&self → *mut Storage` | 最初实现 → `330cd46` 之前 |
| B | `UnsafeCell` 包装 + `run_post_kill` 拆分 | `330cd46` 引入 |
| C | 方案 B + nightly `mutable-noalias=no` | 当前状态 |

此外, old 分支在 `63b81ddf87732475ba5b11e77ae098cb9c20e7eb` → `2456131` 一度试验过一个未完成的全局 `EventQueue` 原型。由于它没有进入主线,不计入上表,但下文会单独分析其可行性。

本文档对比三种主线方案的 UB 风险、性能和可维护性,并补充评估 event queue 旁支方案,以及 Dart async / async-like 改造的可行性。

### 1.1 当前现状摘要

| 主题 | 当前判断 |
|------|----------|
| 当前主线状态 | 已经不是“裸 `unsafe` 强转”,而是 **方案 C = UnsafeCell + 局部延迟结算 + `mutable-noalias=no`**。Storage / Player 层 provenance 已修复,kill 回调的同实体别名已修复,剩余风险主要收敛在少数 `on_damage` 重入路径 |
| Dart `async` 的真实作用 | 主要用于 `nextUpdates()`、初始化构建和 HTML 驱动层的分批推进与事件循环让步。核心战斗链 `round() -> step() -> attacked() -> damage()` 仍然是同步栈调用 |
| Rust `async` 的适用性 | **不是**当前问题的直接解。`async/await` 不会放宽 `&mut` 独占规则；真正有效的是 **async-like 的分阶段执行**,即在敏感回调前释放 `&mut Player`,回调后再恢复结算 |
| 当前最优方向 | 短期维持方案 C；中期若要彻底去掉 UB,优先推进方案 D（延迟 `on_damage` / staged damage）,而不是回退到方案 A 或直接全面改写成全局 async/event machine |

---

## 2. 方案 A: 直接 `unsafe` 强转 (`330cd46` 之前)

### 2.1 实现方式

```rust
pub struct Storage {
    players: FastHashMap<PlrId, Player>,  // 普通字段,无 UnsafeCell
    skills: FastHashMap<usize, Skill>,
    // ...其他字段同样是普通类型
}

impl Storage {
    pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).players.get_mut(&ptr)
        }
    }

    pub fn set_in_post_damage(&self, plr: PlrId) {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).in_post_damage_player = Some(plr);
        }
    }
    // 所有需要 &self → 可变的方法都使用相同的 *const → *mut 强转
}
```

**kill 回调路径(旧版):**
```rust
// impl_runtime.rs — on_die_impl 中
if let Some(killer) = storage.just_get_player_mut(caster)
    && killer.get_status().hp > 0
{
    killer.skills.kill(target, (caster, randomer, updates, storage));
    //     ^^^^^^ &mut self (SkillStorage::kill)
    //            kill 回调(如吞噬)内部会调用 storage.just_get_player_mut(caster)
    //            → 产生第二个 &mut Player 指向同一个 killer
}
```

### 2.2 UB 分析

| UB 类型 | 严重程度 | 说明 |
|---------|---------|------|
| **provenance 违规** | 🔴 严重 | `&self` 的 provenance 是只读的。通过 `*const → *mut` 强转并写入,违反了 Rust/LLVM 的 provenance 模型。编译器有权假设 `&self` 指向的内存在整个引用生命周期内不被修改 |
| **`&mut` 别名** | 🔴 严重 | `just_get_player_mut` 返回的 `&mut Player` 的 provenance 来自只读引用,任何写入都是 UB |
| **kill 回调别名** | 🔴 **已触发** | `killer.skills.kill()` 持有 `&mut killer` 的同时,回调内 `just_get_player_mut(killer_id)` 创建了第二个 `&mut killer`。LLVM noalias 优化导致 `set_level(98)` 的写入对后续 `level()` 读取不可见(仍读到旧值 80) |
| **跨实体 split borrow** | 🟡 中等 | `storage.just_get_player_mut(A)` 和 `storage.just_get_player_mut(B)` 同时存在。虽然指向不同 Player,但 `&mut Storage` 的唯一性被破坏。由于 HashMap 的内部稳定性,实际不会出错,但形式上仍是 UB |

### 2.3 实际被 LLVM 利用的 UB

`330cd46` 的 commit message 明确记录:

> 优化导致 `set_level(98)` 的写入对后续 `action()` 中的读取不可见（仍读到旧值 80）。

这是 LLVM 基于 noalias 假设将 `level()` 的读取提前/缓存的结果。当两个 `&mut Player` 指向同一个 player 时,通过一个引用的写入对另一个引用不可见。

---

## 3. 方案 B: `UnsafeCell` 包装 + `run_post_kill` (`330cd46`)

### 3.1 实现方式

```rust
pub struct Storage {
    players: UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>,
    //       ^^^^^^^^^^                    ^^^^^^^^^^
    //       外层: 允许通过 &self 修改 HashMap 本身
    //       内层: 允许通过 &HashMap 获取 &mut Player
    skills: UnsafeCell<FastHashMap<usize, Skill>>,
    // ...全部字段均 UnsafeCell 包装
}

unsafe impl Send for Storage {}  // UnsafeCell 不是 Sync,需手动声明
unsafe impl Sync for Storage {}

impl Storage {
    fn players_ref(&self) -> &FastHashMap<PlrId, UnsafeCell<Player>> {
        unsafe { &*self.players.get() }
    }

    pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        self.players_ref()
            .get(&ptr)
            .map(|player| unsafe { &mut *player.get() })
    }
}
```

**kill 回调路径(新版):**
```rust
// store.rs — run_post_kill() 独立函数
pub fn run_post_kill(keys, caster, target, randomer, updates, storage) {
    for skill_key in keys {
        // 1. 临时取出技能实现,释放 &mut Player
        let (mut skill_type, level) = {
            let killer = storage.just_get_player_mut(caster).unwrap();
            let skill = killer.skills.store.get_mut(&skill_key).unwrap();
            (skill.take_skill_type(), skill.level())
            // killer 引用在此处结束
        };
        // 2. 无 &mut Player 存活时调用回调
        let triggered = skill_type.kill_with_level(level, target, args);
        // 3. 放回技能实现
        {
            let killer = storage.just_get_player_mut(caster).unwrap();
            let skill = killer.skills.store.get_mut(&skill_key).unwrap();
            skill.put_skill_type(skill_type);
        }
    }
}
```

### 3.2 UB 分析

| UB 类型 | 严重程度 | 与方案 A 对比 |
|---------|---------|-------------|
| **provenance 违规** | 🟢 **已修复** | `UnsafeCell::get()` 返回的 `*mut T` 具有合法的可变 provenance。编译器知道 UnsafeCell 内部可能被修改,不会做错误的缓存优化 |
| **`&mut` 别名** | 🟡 仍存在 | 两个 `&mut Player` 同时指向同一个 Player 仍是 UB(Rust 保证 `&mut T` 独占)。UnsafeCell 解决了容器层面的 provenance,但 **不能** 使两个 `&mut T` 同时合法 |
| **kill 回调别名** | 🟢 **已修复** | `run_post_kill` 通过 `take_skill_type` 临时取出并释放引用,确保回调执行时无重叠 `&mut Player` |
| **跨实体 split borrow** | 🟢 **已修复** | 内层 `UnsafeCell<Player>` 使得从同一个 HashMap 获取两个不同 Player 的 `&mut` 不再需要 `&mut HashMap`。只需 `&HashMap` → `&UnsafeCell<Player>` → `*mut Player` → `&mut Player` |

### 3.3 仍存在的 UB

核心问题:当 `player.step(&mut self, ...)` 执行时,`&mut self` 是通过 `just_get_player_mut(actor)` 获得的。在 `step()` 的调用链中:

```
tick → storage.just_get_player_mut(actor) → &mut player_A
     → player_A.step(&mut self)
       → player_A.action(&mut self)
         → player_A.attacked(target_B)
           → storage.just_get_player_mut(B) → &mut player_B  // OK: 不同实体
             → player_B.damage()
               → on_damage 回调
                 → storage.just_get_player_mut(A) → &mut player_A  // UB! 与外层 &mut self 重叠
```

这类 UB 在 `on_damage` 回调中仍然存在(约 8 个技能实现),但 LLVM noalias 优化在此路径上尚未观察到实际错误行为(因为回调通常修改的是不同字段,或者在当前优化级别下未被利用)。

---

## 4. 方案 C: 方案 B + `mutable-noalias=no` (当前状态)

在方案 B 基础上添加:
- `rust-toolchain.toml`: `channel = "nightly"`
- `.cargo/config.toml`: `rustflags = ["-Z", "mutable-noalias=no"]`

**效果:** 完全禁用 LLVM 对 `&mut` 引用的 noalias 标注,使得即使存在 `&mut` 别名,LLVM 也不会进行基于独占假设的优化(值缓存、指令重排、死代码消除等)。

| UB 类型 | 状态 |
|---------|------|
| provenance 违规 | 🟢 由 UnsafeCell 修复 |
| kill 回调别名 | 🟢 由 run_post_kill 修复 |
| on_damage 回调别名 | 🟡 形式上仍 UB,但 noalias=no 防止 LLVM 利用 |
| 跨实体 split borrow | 🟢 由内层 UnsafeCell 修复 |

---

## 5. 方案对比矩阵

| 维度 | A: 裸 unsafe 强转 | B: UnsafeCell | C: B + noalias=no |
|------|-------------------|---------------|-------------------|
| **Storage 层 provenance** | ❌ 无效(只读→写入) | ✅ 合法(UnsafeCell) | ✅ 合法 |
| **Player 层 provenance** | ❌ 无效 | ✅ 合法(内层 UnsafeCell) | ✅ 合法 |
| **kill 回调别名** | ❌ 已触发 UB | ✅ take/put 修复 | ✅ 修复 |
| **on_damage 别名** | ❌ UB | ❌ UB | ⚠️ UB 但 noalias=no 防护 |
| **跨实体 split borrow** | ❌ UB(共享 &mut HashMap) | ✅ 合法 | ✅ 合法 |
| **需要 nightly** | ❌ 不需要 | ❌ 不需要 | ✅ 需要 |
| **noalias 优化** | ✅ 保留(但有 UB 风险) | ✅ 保留(但有 UB 风险) | ❌ 全局禁用 |
| **运行时性能** | 基线 | = 基线(零开销) | ≈ 基线(< 1%) |
| **代码复杂度** | 最简单 | 中等(accessor 对) | 中等 |
| **Send/Sync** | 自动派生 | 需手动 `unsafe impl` | 需手动 |
| **clippy 告警** | `mut_from_ref` (局部 allow) | `mut_from_ref` (impl-level allow) | 同左 |
| **release 模式正确性** | ❌ 38 test failures | ❓ 未验证(预计仍有部分失败) | ✅ 11 failures(= debug 模式) |

### 5.1 扩展性矩阵

这里的“扩展单个技能/状态”特指**沿用现有生命周期阶段**新增或改写技能；“修改核心顺序”特指改动 `damage / on_damaged / on_die / kill / revive` 一类主流程。

| 方案 | 扩展单个技能/状态 | 扩展新流程点 | 修改核心伤害/死亡顺序 | Dart 读者上手 | 长期维护 | 简评 |
|------|-------------------|--------------|----------------------|--------------|----------|------|
| A: 裸 unsafe 强转 | 高 | 中 | 低 | 高 | 低 | 写法最接近旧直觉,但核心路径越改越危险 |
| B: UnsafeCell | 高 | 中 | 中低 | 中高 | 中 | 语义比 A 正确,但仍需记住别名边界 |
| C: B + noalias=no | 高 | 中 | 中 | 中高 | 中 | 当前主线; 技能扩展不难,核心改动仍要理解 staged queue |
| D: 延迟 `on_damage` | 高 | 中高 | 中高 | 中高 | 高 | 在保留现有风格下,把最危险路径也显式阶段化 |
| EventQueue | 中 | 高 | 高 | 中 | 高(稳态) | 稳态最整洁,但迁移成本和语义重写成本都最高 |
| E: Arena + token | 中 | 中高 | 中 | 中 | 高 | 最 Rust 原生,但重签名和调用改造面很大 |
| 真实 Rust `async` | 低 | 低 | 低 | 低 | 低 | 不能直接解决别名问题,只会扩大执行模型复杂度 |

### 5.2 对不同扩展任务的友好度

| 扩展任务 | 当前方案 C | 方案 D | EventQueue | 结论 |
|----------|------------|--------|------------|------|
| 调整已有 `act/skl` 行为 | 高 | 高 | 中 | 当前主线已足够 |
| 新增一个与现有阶段对齐的技能 | 高 | 高 | 中 | 看过 Dart 的人通常可以较快迁移 |
| 新增一个新的阶段/钩子 | 中 | 中高 | 高 | 若未来频繁加阶段,EventQueue 才开始体现结构优势 |
| 重写 `damage/kill/death` 顺序 | 低中 | 中高 | 高 | 这是方案 C 当前最不友好的区域 |
| 短期交付新功能 | 高 | 中高 | 低 | EventQueue 不适合当下作为短期手段 |

### 5.3 Dart 读者迁移成本

| Dart 概念 | tswn 对位 | 迁移难度 | 说明 |
|-----------|-----------|----------|------|
| `Plr` 对象直接互调 | `PlrId + Arc<Storage> + just_get_player_mut()` | 高 | 这是最大的心智切换: 不再是对象引用直达,而是显式经由 storage 取回实体 |
| `proc.dart` 的 `PreActionProc / PostDamageProc / KillProc ...` | `SkillTrait` 各默认方法 + `ProcKind` 注册 | 低 | 概念和命名高度对位,看过 Dart 后很容易找到 Rust 对应入口 |
| `meta / updatestates / presteps` | `StateTrait` + 状态容器 + `update_state / on_pre_step` | 中 | Rust 把状态行为集中进 trait,结构更规整,但少了 Dart 那种“对象挂链”直觉 |
| `RunUpdates.onUpdateEnd` | `on_update_end()` + `run_update_end()` | 中 | 语义相近,只是驱动位置从对象层移动到了 tick 层 |
| `onDie / kill` 的即时递归 | `run_post_kill()` + `pending_*` 队列 | 中高 | 需要先理解“为什么要延迟结算”和“借用何时必须释放” |
| 全局流程注入 | `HookPipeline` | 中 | 这是 Rust 版新增的结构化扩展点,Dart 源码里没有这么显式 |

| 任务类型 | 看过 Dart 源码的人在 tswn 中的上手速度 |
|----------|----------------------------------------|
| 阅读已有技能实现 | 快 |
| 新增/改写单个技能 | 较快 |
| 新增状态或 target scoring | 中等偏快 |
| 修改 `damage / on_die / post_kill` 核心路径 | 中等偏慢 |
| 改动整个引擎推进模型 | 慢 |

---

## 6. 回退到方案 A 的利弊分析

### 6.1 如果回退 + 保留 `mutable-noalias=no`

| 方面 | 影响 |
|------|------|
| 正确性 | ✅ noalias=no 阻止 LLVM 利用别名 UB,release 模式应与当前一致 |
| kill 回调 | ⚠️ `run_post_kill` 被移除,回退到 `killer.skills.kill()` 内联调用。但因为 noalias=no,set_level 可见性 bug 不会触发 |
| provenance | ❌ 退化:`&self → *mut Storage` 的 provenance 无效。虽然当前 LLVM 不利用此信息,但 **Miri** 和未来的编译器优化可能会捕获 |
| 代码量 | 减少 ~140 行(accessor 对 + run_post_kill + take/put_skill_type) |
| 性能 | 无变化(UnsafeCell 本身零开销;noalias=no 的开销已经在方案 C 中计入) |
| 可维护性 | 略微简化(无 `_ref()`/`_mut()` accessor 对) |
| Miri 兼容性 | ❌ 更差(Miri 会报告所有 `*const → *mut` 写入为 UB) |

### 6.2 如果回退 + 不用 `mutable-noalias=no` (回到最原始状态)

| 方面 | 影响 |
|------|------|
| 正确性 | ❌ **release 模式 38 test failures**(已验证) |
| kill 回调 | ❌ **实际触发 UB**(set_level 不可见) |
| toolchain | ✅ 可用 stable |
| 性能 | ⚠️ noalias 优化理论上有效,但在此项目中实测 < 1% 差异 |

### 6.3 结论

**回退到方案 A 没有实质收益:**

| 方面 | 结论 |
|------|------|
| 性能 | `UnsafeCell` 是 `#[repr(transparent)]`,编译后与裸类型完全相同。`run_post_kill` 多执行两次 `std::mem::replace`(仅在有 kill 回调时),可忽略不计 |
| 正确性 | 方案 A 的 provenance 违规更严重。当前无实际影响(因 noalias=no),但未来 Rust 编译器可能收紧对 provenance 的检查 |
| 代码量 | 方案 A 少 ~140 行,但全是机械性的 accessor 对,理解负担很低 |
| 维护安全 | 方案 B 的 `UnsafeCell` 在语义上正确传达了“这个字段会被内部可变地修改”的意图。方案 A 的 `*const → *mut` 强转没有任何编译器层面的保护 |

---

## 7. 旁支方案: 全局 EventQueue (`63b81dd` 原型)

### 7.1 实现方式

`63b81ddf87732475ba5b11e77ae098cb9c20e7eb` 的原型并没有把 `Storage` 做成内部可变容器,而是尝试把**跨实体副作用**改写成全局命令队列:

```rust
pub enum Event {
    TryAttack { caster: PlrId },
    Attack { caster: PlrId, target: PlrId, is_mag: bool },
    DealDamage { caster: PlrId, target: PlrId, dmg: i32 },
}

pub type SkillArgs<'d> =
    (PlrId, &'d mut RC4, &'d mut RunUpdates, &'d mut EventQueue);
```

关键特征:

1. `Runner` 独占持有 `Storage` 和 `EventQueue`,在 `process_events()` 中统一 drain。
2. `Player::action()` 不直接攻击目标,而是 `push(Event::TryAttack { caster })`。
3. 技能实现拿不到 `Storage`,只能 `push(Event::DealDamage { .. })` 这类命令。`FireSkill` 就是典型例子。
4. `Attack` 事件在 `Runner` 中通过 `SlotMap::get_disjoint_mut()` 的包装 `get_two_players_mut(caster, target)` 一次性拿到 attacker/target 两个不同玩家。

也就是说,这个方案本质上是 **single-thread command buffer / event sourcing**: 业务代码负责“描述接下来要发生什么”,真正改写实体状态的地方尽量收敛到 `Runner::process_events()`。

### 7.2 理论上能解决什么问题

如果把这个思路贯彻到底,它确实可以**从架构层面**消灭当前主线的大部分 UB 来源:

| 问题 | EventQueue 彻底化后的状态 | 原因 |
|------|--------------------------|------|
| `&self → &mut Storage` provenance | ✅ 可避免 | `Runner` 直接持有 `&mut Storage`,不需要共享引用下的内部可变性 |
| 跨实体 split borrow | ✅ 可避免 | `Attack/Heal/Revive` 等都在事件边界统一结算,不同实体可通过 `get_disjoint_mut` 或串行处理拿到 |
| 回调重入拿到第二个 `&mut same_player` | ✅ **若严格禁止回调直接碰 Storage** | 技能/状态回调只发命令,不直接递归获取玩家引用 |
| `mutable-noalias=no` 依赖 | ✅ 可移除 | 不再依赖共享引用下的 `&mut Player` 别名技巧 |

这是它相对方案 B/C 的最大优点:它不是“让 UB 暂时不炸”,而是试图把**可变访问权集中到事件分发器**里,从语义上规避别名。

### 7.3 为什么 `63b81dd` 原型本身还不够

问题在于,`63b81dd` 离“彻底事件化”还差得很远。它更像一个方向验证,不是可直接回收的成品方案。

| 不足 | 说明 |
|------|------|
| 覆盖面极窄 | 这版 `Event` 只有 `TryAttack / Attack / DealDamage` 三种。`pre_action`、`post_action`、`on_die`、`kill`、`revive`、state clear、summon/remove/sync world` 等都还没有进队列 |
| 伤害生命周期仍是同步栈 | `Attack` 事件最终还是直接进入 `target.attacked() → damage() → on_damaged() → run_post_damage()`。最复杂的 reentrant 生命周期并没有被命令化 |
| “安全”很大程度来自功能未长全 | 当时 `SkillArgs` 里没有 `Storage`,技能回调根本做不了现在这些跨实体读写、查队伍、查 state、召唤/转移伤害/复活等操作 |
| 事件负载表达力不够 | 当前主线很多路径不仅需要 `(caster, target, dmg)`,还需要 `on_damage` 回调语义、kill/revive 顺序、forced action、目标域、召唤物 owner 关系、world sync 等上下文 |
| 顺序语义未定型 | `VecDeque + FIFO` 只解决了“延后执行”,没有天然解决“何时插队/何时立刻结算/哪些事件必须同帧递归 drain” |

后续 old 分支在 `2456131` 一度把 `Event` 扩展到 `Step/PreAction/PostAction/OnDie/Kill/Revive` 等占位,但 `process_events()` 同时被直接标注为“虽说我不确定,但是我觉得这个方案不可行”。这说明当原型开始接近真实战斗语义时,复杂度已经明显上升。

### 7.4 对当前主线的可行性评估

**结论先说:** 对当前主线来说,全局 EventQueue 仍然**理论可行**,但它是一次**架构重写**,不是一个能替代 `UnsafeCell + 局部延迟回调` 的小修补。

当前代码规模下,它至少会碰到以下现实成本:

| 成本项 | 当前规模/影响 |
|--------|---------------|
| `just_get_player_mut()` 改造面 | 当前非测试源码里有 **112 处** 调用点 |
| 行为入口数量 | 当前 `player/skill` 与 `player/boss` 下有 **37 个** `impl SkillTrait for ...` 行为实现文件 |
| 直接伤害入口分散 | 非测试源码中有 **19 个文件** 直接调用 `.attacked()` / `.damage()` |
| 生命周期钩子重签名 | 当前至少有 **10 处** `pre_action/post_action/post_damage/kill` 相关实现要改成“只发事件,不直接改状态” |
| 队列边界重设计 | 需要新增或重写 `ApplyDamage`、`RunOnDamage`、`RunPostDamage`、`OnDie`、`Kill`、`Revive`、`Spawn`、`Remove`、`ClearStates`、`SyncWorld` 等事件,并为每个事件定义严格顺序 |

这件事的工作量**至少与方案 E (Arena + token)** 同级,很多情况下还会更大,因为它不只是改 API,还要重写语义边界。Arena/token 方案至少还能保留“同步调用”的业务形态; 全局 EventQueue 则要求大量逻辑改写成“提交命令 + Runner 解释执行”。

### 7.5 与当前主线的关系

值得注意的是,当前主线其实已经吸收了这个思路里**最划算的部分**:

| 已吸收的思路 | 当前落点 | 价值 |
|--------------|----------|------|
| 局部 command buffer | `run_post_kill()` 先释放 `&mut killer`,再执行 kill 回调,最后放回技能实现 | 定点消除同实体 kill 回调别名 |
| 延迟同步生命周期 | `pending_spawns / pending_remove_players / death_queue / pending_revivals` | 把最敏感的实体生命周期顺序改成受控同步 |
| 受控 drain loop | `run_update_end()` | 统一处理回合末继续触发 |

换句话说,主线并不是完全放弃了 event queue 思路,而是把它**局部化**了: 只在真正会引发别名/顺序问题的路径上做延迟结算,而不是把整个战斗系统都改写成通用事件机。

### 7.6 结论

**是否可行?** 可行,但前提是把它当成一次**完整的命令缓冲架构重构**,而不是当前 UB 的快捷修复。

**是否值得现在做?** 对“消除当前剩余 UB”这个目标来说,通常不值得:

| 判断依据 | 说明 |
|----------|------|
| 剩余问题规模 | 当前剩余 UB 已经收敛到少数 `on_damage` 重入路径,用方案 D 的定点拆分就能解决 |
| 性能压力 | `mutable-noalias=no` 的性能成本实测 < 0.2%,没有逼着项目立刻换架构的性能压力 |
| 主要收益方向 | 全局 EventQueue 的主要收益是“架构纯度 / 事件日志化 / 更强的执行边界”,不是眼下最短路径 |

因此,**不建议为了处理当前这批 UB 而重启全局 EventQueue 方案**。只有当项目未来明确想走“命令缓冲 / replayable event log / 强约束 ECS”路线时,它才值得作为长期重构方向重新评估。

---

## 8. Dart async / async-like 方案评估

### 8.1 Dart 里的 `async` 实际解决了什么

原版 namerena 确实在战斗驱动层用了 `async`,但它的职责主要是**把战斗推进包装成“可暂停、可分批返回更新”的外层接口**,而不是把每个技能/伤害回调都变成异步状态机。

`fgt.dart` 的主线结构大致如下:

```dart
Future<RunUpdates> nextUpdates() async {
    if (_winner != null) {
        await _checktime();
        return updates;
    }
    while (_winner == null) {
        round(updates);
        if (updates.updates.isNotEmpty) {
            return updates;
        }
    }
}

void round(RunUpdates updates) {
    roundPos = (roundPos + 1) % players.length;
    players[roundPos].step(rander, updates);
}

int damage(int dmg, Plr caster, OnDamage ondmg, R r, RunUpdates updates) {
    // ...应用伤害...
    updates.add(update);
    ondmg(caster, this, dmg, r, updates);
    return onDamaged(dmg, oldhp, caster, r, updates);
}
```

这说明 Dart 版的 `async` 有三个核心用途:

| 用途 | 说明 |
|------|------|
| 外层驱动分帧 | `nextUpdates()` 以 `Future<RunUpdates>` 形式向 UI/HTML runner 分批返回更新 |
| 让出浏览器事件循环 | 通过 `Future.delayed(...)` / `Timer(...)` 避免长时间卡住页面线程 |
| 初始化阶段异步化 | `buildAsync()`、资源加载等流程天然适合 Future 化 |

但同一时间,它**没有**改变战斗内核的关键事实:

| 未改变的事实 | 说明 |
|--------------|------|
| 战斗主链仍是同步调用 | `round()` / `step()` / `action()` / `attacked()` / `damage()` 都仍是同步调用 |
| `onDamage` 与 `onDamaged` 之间没有 Future 边界 | `ondmg(caster, this, ...)` 之后立刻执行 `onDamaged(...)` |
| Dart 不存在 Rust 式 `&mut` 独占约束 | Dart 允许多个对象引用别名同一 `Plr`,因此原版并不需要处理 Rust 这类借用规则 |

因此,Dart async 在原版里解决的是**UI 驱动与节流问题**,不是**可变别名合法性**问题。

### 8.2 为什么“直接改成 Rust async”不能自动修掉 UB

如果把当前 Rust 代码机械地改成 `async fn`,例如:

```rust
async fn damage(&mut self, dmg: i32, caster: PlrId, ...) -> i32 {
        self.status.hp -= dmg;
        on_damage(caster, self.as_ptr(), dmg, ...).await;
        self.on_damaged(dmg, old_hp, caster, ...)
}
```

它并不会天然解决现有问题,原因如下:

| 问题 | `async` 的实际效果 |
|------|--------------------|
| **`&mut` 独占规则** | **完全不变**。`async fn` 只是把函数改写成状态机,Rust 借用规则照样适用 |
| **跨 `await` 持有 `&mut Player`** | Future 会把这份独占借用保存在状态机里。只要还没恢复执行,这份借用就仍然活着,并不能安全地再次从 storage 拿同一个 player |
| **如果在 `await` 前先释放 `&mut Player`** | 那本质上已经手动做了 staged split / continuation。真正起作用的是“释放借用后再继续”,不是 `async` 关键字本身 |
| **签名改造成本** | 当前 `SkillTrait` / `post_damage` / `kill` / `pre_action` / `post_action` 等大量 trait 与函数签名都要 async 化。Rust 这类场景通常需要 `async_trait` 或装箱 Future,改造面很大 |
| **执行顺序语义** | 现在引擎是严格单线程、同步、可预测顺序。引入 executor / poll / wake 后,必须重新定义“哪些 await 点允许插队、何时恢复、同 tick 内 drain 到什么程度” |

更关键的一点是:当前问题不是“缺少暂停能力”,而是“**暂停前没有先释放对同一实体的独占借用**”。如果这一点不做,`async` 只会把同一个借用冲突搬进 Future 状态机里；如果这一点做了,那已经是在显式分阶段了。

### 8.3 真正可借鉴的是“async-like 的分阶段状态机”

从这个角度看,对当前项目真正有价值的不是“上 async runtime”,而是借鉴 async 的**暂停/恢复边界**思想,把同步栈改写成显式阶段:

1. **阶段 1**: 持有 `&mut target`,执行 `damage_core()` / 计算伤害 / 写入 HP / 产出 update。
2. **阶段 2**: 释放 `&mut target`,执行 `on_damage` 回调。
3. **阶段 3**: 重新获取 `&mut target`,执行 `on_damaged()` / `post_damage` / `on_die`。

这本质上就是一个**手写 continuation / 手写 Future 状态机**,但它比真实 `async` 更适合当前战斗引擎:

- 不需要引入 executor、waker、poll 语义。
- 不需要把整条技能 trait 链改成 async。
- 顺序边界完全由引擎显式控制,更容易对齐 JS/Dart 语义。
- 数据可以保持为普通结构体 / 临时上下文,不会引入额外的 Future 装箱与生命周期复杂度。

如果继续抽象下去,甚至可以把它写成一个小型的 staged command:

```rust
struct PendingDamage {
        target: PlrId,
        caster: PlrId,
        dmg: i32,
        old_hp: i32,
}
```

然后由驱动层按“apply -> callback -> finalize”的顺序推进。这个思路与方案 D 本质一致,只是表述上更接近 coroutine / continuation。

### 8.4 当前主线其实已经在用这种思路

严格来说,当前主线并不是完全“同步到底”,而是已经局部采用了 async-like 的暂停/恢复模式:

1. `run_post_kill()` 先取出技能实现、释放 `&mut killer`,再执行 kill 回调,最后放回技能实现。
2. `pending_spawns / pending_remove_players / death_queue / pending_revivals` 把最容易出顺序问题的实体生命周期改成延迟同步。
3. `run_update_end()` 通过受控 drain loop 推进回合末回调,本质上也是分阶段执行。

所以,如果要“借 Dart async 的灵感”,最自然的延伸不是全面 async 化,而是**沿着现有局部 staged execution 的方向继续推进**,把 `on_damage` 这条剩余危险路径也拆开。

### 8.5 结论

**能不能借 async 思路?** 能,但要区分两个层次:

| 层次 | 判断 |
|------|------|
| 真实 Rust async/await | 不建议作为当前 UB 问题的直接解。它不会放宽借用规则,却会显著扩大签名和执行模型改造面 |
| async-like / staged / continuation 风格 | 非常值得借鉴,而且当前主线已经证明这种局部拆分是有效的 |

换句话说:

| 目标 | 建议 |
|------|------|
| 修掉剩余 UB | 最优路径仍然是方案 D 这种“手写分阶段 damage”方案 |
| 提供像 Dart `nextUpdates()` 那样的可暂停外层接口 | 可以在引擎驱动层额外封装 iterator / stream-like API,但这与内部别名修复是两个独立问题 |
| 长期架构纯化 | 应在“局部 staged execution”与“全局 EventQueue / command buffer”之间做取舍,而不是先把全项目改成 async fn |

---

## 9. "彻底消除 UB" 的可行路径

当前方案 C 仍有 `on_damage` 回调中的 `&mut Player` 别名 UB(约 8 个技能实现),只是被 `mutable-noalias=no` 掩盖。

### 9.1 方案 D: 延迟 on_damage 回调 (推荐的 async-like 增量改进)

类似 `run_post_kill` 的 `take/put` 模式,将 `damage()` 拆分:

```rust
// 当前: damage() 内部直接调用 on_damage 回调
pub fn damage(&mut self, dmg: i32, caster: PlrId,
              on_damage: OnDamageFunc, ...) -> i32 {
    // ...
    on_damage(caster, self.as_ptr(), dmg, ...);  // ← UB: on_damage 可能获取 &mut self
    self.on_damaged(...)
}

// 改为: damage_core() 只计算伤害,不调用回调
pub fn damage_core(&mut self, dmg: i32, ...) -> (i32, bool) {
    // 计算伤害并应用
    // 返回 (实际伤害, 是否需要执行 on_damage)
}
```

然后在调用方(通常是 `attacked()` 或各技能的 action 实现)中:
1. 通过 `just_get_player_mut(target)` 获取 `&mut target` → 调用 `damage_core()`
2. 释放 `&mut target`
3. 调用 `on_damage` 回调(此时无 `&mut target` 存活)
4. 重新获取 `&mut target` → 调用 `on_damaged()`

**影响面**: 约 8 个 on_damage 回调 + `attacked()`/`defned()` 的调用方(~20 处)。
**工作量**: 约 150-200 行改动。
**收益**: 消除最后一类 `&mut Player` 别名 UB,可以移除 `mutable-noalias=no`,回到 stable toolchain。

它从工程形态上看,本质就是一个**手写的 async-like continuation**: 先完成伤害核心阶段,暂时“挂起”后续结算,待危险回调执行完后再恢复。这也是为什么它比“直接把 damage 改成 async fn”更贴近问题本质。

### 9.2 方案 E: Arena + token 模式 (大规模重构)

将 `Storage` 改为 arena 分配器,用 `SlotMap<PlrId, Player>`:

```rust
pub struct Storage {
    players: SlotMap<PlrId, Player>,
}

// 所有方法签名从 Player::step(&mut self, ..., &Arc<Storage>)
// 改为自由函数: step(plr_id: PlrId, storage: &mut Storage)
```

**影响面**: 全部 162 个 `just_get_player_mut` 调用点 + 所有 `Player` 方法签名 → **全项目重构**。
**工作量**: 1000+ 行改动,涉及 ~50 个文件。
**收益**: 完全消除 unsafe,标准 Rust 借用检查。
**风险**: 每次通过 storage 间接访问 player 需要额外的 HashMap 查找,hot path 可能有 5-10% 性能下降。

### 9.3 方案 F: RefCell 运行时检查 (调试辅助)

用 `RefCell<Player>` 替代 `UnsafeCell<Player>`,在 debug 模式下检测别名:

```rust
players: UnsafeCell<FastHashMap<PlrId, RefCell<Player>>>,
```

**优点**: debug 模式下 RefCell 会 panic 报告重复借用。
**缺点**: release 模式下 RefCell 的 borrow flag 检查有 ~2-3% 开销。可通过 feature flag 在 release 模式下切回 UnsafeCell。

---

## 10. 推荐路线

| 时间尺度 / 目标 | 建议 | 原因 |
|-----------------|------|------|
| 短期 | 维持方案 C,无需任何改动 | 零性能损失,release 正确性已验证 |
| 中期（可选） | 实施方案 D（延迟 `on_damage` / async-like staged damage） | 消除最后的 `&mut` 别名 UB,移除 nightly 依赖和 `mutable-noalias=no`,回到 stable toolchain |
| 若未来想保留 Dart 风格的“可暂停推进接口” | 在引擎外层封装 iterator / stream-like 驱动 | 这可以改善外部接口形态,但不要把它当成别名问题的主修复手段 |
| 长期（仅在愿意做架构重写时） | 全局 EventQueue | 理论上可完全去掉 UnsafeCell 和 noalias=no,但改造面 ≥ 方案 E,且需要重写事件顺序语义 |
| 不推荐 | 回退到方案 A | 零收益,更差的 provenance 安全性,还会丢失 `run_post_kill` 的别名修复 |

---

## 11. 性能基准参考

已完成的基准测试 (mario vs luigi, 100000 场, single-thread, `--features no_debug`):

### win_rate_st

| 版本 | Run 1 | Run 2 | Run 3 | 平均 |
|------|-------|-------|-------|------|
| Baseline (stable, noalias=yes) | 3.279s (30494/s) | 3.113s (32126/s) | 3.399s (29422/s) | ~3.264s |
| 当前 (nightly, noalias=no) | 3.136s (31884/s) | 3.155s (31691/s) | 3.113s (32125/s) | ~3.135s |

### --perf (含 init)

| 版本 | Run 1 | Run 2 | Run 3 | 平均 fight |
|------|-------|-------|-------|-----------|
| Baseline | 5.990s | 5.992s | 6.213s | ~3.037s |
| 当前 | 6.062s | 6.196s | 6.053s | ~3.041s |

**结论**: fight 部分差异 < 0.2%,在噪声范围内。`mutable-noalias=no` 对此项目无可测量的性能影响。

UnsafeCell 本身是 `#[repr(transparent)]` 的零开销抽象;`run_post_kill` 的 `take/put` 每次 kill 回调两次 `mem::replace`,在正常战斗中 kill 事件频率远低于主循环,开销不可测量。
