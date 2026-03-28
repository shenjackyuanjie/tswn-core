# Storage 内部可变性方案对比分析

> 创建: 2026-03-28 | 对应 commit: `330cd46` (UnsafeCell 切换) + 当前 HEAD

## 1. 背景

`Storage` 是引擎的核心实体容器,以 `Arc<Storage>` 在整个战斗推进中共享。
由于技能回调、伤害链、死亡处理等路径需要同时持有多个 `&mut Player` 引用,
Rust 标准的借用规则(独占可变引用)无法直接满足需求。

项目历史上出现过三种方案:

| 阶段 | 方案 | 时间线 |
|------|------|--------|
| A | 直接 `unsafe` 强转 `&self → *mut Storage` | 最初实现 → `330cd46` 之前 |
| B | `UnsafeCell` 包装 + `run_post_kill` 拆分 | `330cd46` 引入 |
| C | 方案 B + nightly `mutable-noalias=no` | 当前状态 |

本文档对比三种方案的 UB 风险、性能和可维护性。

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

1. **性能**: UnsafeCell 是 `#[repr(transparent)]`,编译后与裸类型完全相同。`run_post_kill` 多执行两次 `std::mem::replace`(仅在有 kill 回调时),可忽略不计。
2. **正确性**: 方案 A 的 provenance 违规更严重。当前无实际影响(因 noalias=no),但未来 Rust 编译器可能收紧对 provenance 的检查。
3. **代码量**: 方案 A 少 ~140 行,但全是机械性的 accessor 对,理解负担很低。
4. **维护安全**: 方案 B 的 `UnsafeCell` 在语义上正确传达了"这个字段会被内部可变地修改"的意图。方案 A 的 `*const → *mut` 强转没有任何编译器层面的保护。

---

## 7. "彻底消除 UB" 的可行路径

当前方案 C 仍有 `on_damage` 回调中的 `&mut Player` 别名 UB(约 8 个技能实现),只是被 `mutable-noalias=no` 掩盖。

### 7.1 方案 D: 延迟 on_damage 回调 (推荐的增量改进)

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

### 7.2 方案 E: Arena + token 模式 (大规模重构)

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

### 7.3 方案 F: RefCell 运行时检查 (调试辅助)

用 `RefCell<Player>` 替代 `UnsafeCell<Player>`,在 debug 模式下检测别名:

```rust
players: UnsafeCell<FastHashMap<PlrId, RefCell<Player>>>,
```

**优点**: debug 模式下 RefCell 会 panic 报告重复借用。
**缺点**: release 模式下 RefCell 的 borrow flag 检查有 ~2-3% 开销。可通过 feature flag 在 release 模式下切回 UnsafeCell。

---

## 8. 推荐路线

```
当前状态 (方案 C: UnsafeCell + noalias=no)
    │
    ├─ 短期: 维持现状,无需任何改动
    │         零性能损失,release 正确性已验证
    │
    ├─ 中期 (可选): 实施方案 D (延迟 on_damage)
    │         消除最后的 &mut 别名 UB
    │         移除 nightly 依赖和 mutable-noalias=no
    │         回到 stable toolchain
    │
    └─ 不推荐: 回退到方案 A
              零收益,更差的 provenance 安全性
              丢失 run_post_kill 的别名修复
```

---

## 9. 性能基准参考

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
