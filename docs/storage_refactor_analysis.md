# Storage 内部可变性方案对比分析

> 创建: 2026-03-28 | 对应 commit: `330cd46` (UnsafeCell 切换) + 当前 HEAD

---

## 1. 摘要与现状结论

**当前主线状态**  
采用 **方案 C**：`UnsafeCell` 包装所有字段 + `run_post_kill` 延迟结算 + nightly 的 `mutable-noalias=no` 标志。  
该方案已修复 provenance 违规和 kill 回调中的同实体别名 UB，但 `on_damage` 重入路径仍存在 `&mut Player` 别名（约 8 个技能实现），已被 `mutable-noalias=no` 掩盖，未触发实际错误。

**推荐路线**  
- **短期**：维持方案 C，零改动。  
- **中期**：实施方案 D（延迟 `on_damage` / staged damage），彻底消除最后一类别名 UB，可移除 `mutable-noalias=no` 并回归 stable toolchain。  
- **长期**（架构重写时）：可评估全局 EventQueue 或 Arena+token，但成本高，非当前必要。

---

## 2. 背景与问题定义

`Storage` 是引擎的核心实体容器，以 `Arc<Storage>` 在整个战斗推进中共享。由于技能回调、伤害链、死亡处理等路径需要同时持有多个 `&mut Player` 引用，Rust 标准的借用规则（独占可变引用）无法直接满足需求。

项目历史上主线实际落地过三种方案：

| 阶段 | 方案 | 时间线 |
|------|------|--------|
| A | 直接 `unsafe` 强转 `&self → *mut Storage` | 最初实现 → `330cd46` 之前 |
| B | `UnsafeCell` 包装 + `run_post_kill` 拆分 | `330cd46` 引入 |
| C | 方案 B + nightly `mutable-noalias=no` | 当前状态 |

此外，old 分支在 `63b81ddf87732475ba5b11e77ae098cb9c20e7eb` → `2456131` 一度试验过一个未完成的全局 `EventQueue` 原型。由于它没有进入主线，不计入上表，但下文会单独分析其可行性。

本文档对比三种主线方案的 UB 风险、性能和可维护性，并补充评估 event queue 旁支方案，以及 Dart async / async-like 改造的可行性。

---

## 3. 方案总览

### 3.1 各方案核心对比

| 方案 | 核心思想 | 需要 nightly | provenance UB 修复 | &mut 别名 UB 修复 | 改造规模 |
|------|----------|--------------|-------------------|-------------------|----------|
| **A** | 裸 unsafe 强转 | ❌ | ❌ 无效 | ❌ 存在 | 最小 |
| **B** | UnsafeCell + `run_post_kill` | ❌ | ✅ 修复 | ❌ on_damage 仍存在 | 中等 |
| **C** | B + `mutable-noalias=no` | ✅ | ✅ 修复 | ⚠️ 存在但 noalias=no 掩盖 | 中等 |
| **D** | 延迟 on_damage | ❌ | ✅ 修复 | ✅ 彻底修复 | 中等（~200 行） |
| **E** | Arena + token | ❌ | ✅ 修复 | ✅ 彻底修复 | 大规模（1000+ 行） |
| **G** | 全局 EventQueue | ❌ | ✅ 修复 | ✅ 彻底修复 | 大规模（架构重写） |
| **H** | async-like 分阶段 | ❌ | ✅ 修复 | ✅ 彻底修复 | 中等（类似 D） |

> 注：方案 G 为全局 EventQueue（旁支原型），方案 H 为 async-like 分阶段思想（与 D 本质相同）。

### 3.2 扩展性与维护性对比

| 方案 | 扩展单个技能/状态 | 扩展新流程点 | 修改核心伤害/死亡顺序 | Dart 读者上手 | 长期维护 |
|------|-------------------|--------------|----------------------|--------------|----------|
| **A** | 🟢 高 | 🟡 中 | 🔴 低 | 🟢 高 | 🔴 低 |
| **B** | 🟢 高 | 🟡 中 | 🟡 中低 | 🟡 中高 | 🟡 中 |
| **C** | 🟢 高 | 🟡 中 | 🟡 中 | 🟡 中高 | 🟡 中 |
| **D** | 🟢 高 | 🟡 中高 | 🟡 中高 | 🟡 中高 | 🟢 高 |
| **G** | 🟡 中 | 🟢 高 | 🟢 高 | 🟡 中 | 🟢 高 |
| **E** | 🟡 中 | 🟡 中高 | 🟡 中 | 🟡 中 | 🟢 高 |

### 3.3 Dart 读者迁移成本

| Dart 概念 | tswn 对位 | 迁移难度 |
|-----------|-----------|----------|
| `Plr` 对象直接互调 | `PlrId + Arc<Storage> + just_get_player_mut()` | 🔴 高 |
| `proc.dart` 的 `PreActionProc / PostDamageProc / KillProc ...` | `SkillTrait` 各默认方法 + `ProcKind` 注册 | 🟢 低 |
| `meta / updatestates / presteps` | `StateTrait` + 状态容器 + `update_state / on_pre_step` | 🟡 中 |
| `RunUpdates.onUpdateEnd` | `on_update_end()` + `run_update_end()` | 🟡 中 |
| `onDie / kill` 的即时递归 | `run_post_kill()` + `pending_*` 队列 | 🟡 中高 |
| 全局流程注入 | `HookPipeline` | 🟡 中 |

| 任务类型 | 看过 Dart 源码的人在 tswn 中的上手速度 |
|----------|----------------------------------------|
| 阅读已有技能实现 | 🟢 快 |
| 新增/改写单个技能 | 🟢 较快 |
| 新增状态或 target scoring | 🟡 中等偏快 |
| 修改 `damage / on_die / post_kill` 核心路径 | 🟡 中等偏慢 |
| 改动整个引擎推进模型 | 🔴 慢 |

---

## 4. 各方案详解

### 4.1 方案 A：裸 unsafe 强转（原始实现）

#### 实现方式

```rust
pub struct Storage {
    players: FastHashMap<PlrId, Player>,  // 普通字段
    skills: FastHashMap<usize, Skill>,
    // ...
}

impl Storage {
    pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
        unsafe {
            let mut_slf = self as *const Storage as *mut Storage;
            (*mut_slf).players.get_mut(&ptr)
        }
    }
    // 所有需要 &self → 可变的方法都使用相同的强转
}
```

**kill 回调路径（旧版）**  
```rust
if let Some(killer) = storage.just_get_player_mut(caster)
    && killer.get_status().hp > 0
{
    killer.skills.kill(target, (caster, randomer, updates, storage));
    // kill 回调内再次调用 just_get_player_mut(caster) → 第二个 &mut Player
}
```

#### UB 分析

| UB 类型 | 严重程度 | 说明 |
|---------|---------|------|
| **provenance 违规** | 🔴 严重 | `&self` 的 provenance 是只读的，强转后写入违反 provenance 模型。 |
| **`&mut` 别名** | 🔴 严重 | 两个 `&mut Player` 指向同一玩家，LLVM noalias 优化可能导致写入不可见。 |
| **kill 回调别名** | 🔴 **已触发** | 实际出现 `set_level(98)` 写入对后续读取不可见（仍读到 80）。 |
| **跨实体 split borrow** | 🟡 中等 | 形式上 UB，但因 HashMap 稳定性实际未出错。 |

#### 实际被 LLVM 利用的 UB

`330cd46` commit 记录：优化导致 `set_level(98)` 的写入对后续 `action()` 中的读取不可见（仍读到旧值 80）。这是 LLVM 基于 noalias 假设将读取提前/缓存的结果。

---

### 4.2 方案 B：UnsafeCell 包装 + run_post_kill（`330cd46`）

#### 实现方式

```rust
pub struct Storage {
    players: UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>,
    skills: UnsafeCell<FastHashMap<usize, Skill>>,
    // ...
}

unsafe impl Send for Storage {}
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

**kill 回调路径（新版）**  
```rust
pub fn run_post_kill(keys, caster, target, randomer, updates, storage) {
    for skill_key in keys {
        // 1. 临时取出技能实现，释放 &mut killer
        let (mut skill_type, level) = {
            let killer = storage.just_get_player_mut(caster).unwrap();
            let skill = killer.skills.store.get_mut(&skill_key).unwrap();
            (skill.take_skill_type(), skill.level())
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

#### UB 分析

| UB 类型 | 状态 | 说明 |
|---------|------|------|
| provenance 违规 | 🟢 修复 | `UnsafeCell::get()` 返回具有合法可变 provenance 的指针。 |
| `&mut` 别名 | 🟡 仍存在 | 两个 `&mut Player` 同时指向同一玩家仍是 UB。 |
| kill 回调别名 | 🟢 修复 | `run_post_kill` 通过 `take/put` 确保回调执行时无重叠引用。 |
| 跨实体 split borrow | 🟢 修复 | 内层 `UnsafeCell<Player>` 允许从 `&HashMap` 获取不同玩家的 `&mut`。 |

#### 仍存在的 UB

在 `player.step(&mut self, ...)` 调用链中：

```
tick → storage.just_get_player_mut(actor) → &mut player_A
     → player_A.step(&mut self)
       → player_A.action(&mut self)
         → player_A.attacked(target_B)
           → storage.just_get_player_mut(B) → &mut player_B  // OK
             → player_B.damage()
               → on_damage 回调
                 → storage.just_get_player_mut(A) → &mut player_A  // UB! 与外层 &mut self 重叠
```

此类 UB 在约 8 个技能实现的 `on_damage` 回调中存在，但尚未被 LLVM 优化利用。

---

### 4.3 方案 C：B + mutable-noalias=no（当前主线）

在方案 B 基础上添加：
- `rust-toolchain.toml`: `channel = "nightly"`
- `.cargo/config.toml`: `rustflags = ["-Z", "mutable-noalias=no"]`

**效果**：完全禁用 LLVM 对 `&mut` 引用的 noalias 标注，即使存在 `&mut` 别名，LLVM 也不会进行基于独占假设的优化（值缓存、指令重排等）。

| UB 类型 | 状态 |
|---------|------|
| provenance 违规 | 🟢 由 UnsafeCell 修复 |
| kill 回调别名 | 🟢 由 run_post_kill 修复 |
| on_damage 回调别名 | 🟡 形式上仍 UB，但 noalias=no 防止 LLVM 利用 |
| 跨实体 split borrow | 🟢 由内层 UnsafeCell 修复 |

**性能影响**：实测差异 < 0.2%，在噪声范围内（详见第 7 章）。

---

### 4.4 方案 D：延迟 on_damage（推荐的 async-like 增量改进）

#### 实现思路

将 `damage()` 拆分为三个阶段：

1. **`damage_core`**：计算伤害、应用 HP 变更，返回结果但不调用回调。
2. **回调阶段**：释放 `&mut target`，执行 `on_damage` 回调（此时无借用冲突）。
3. **`on_damaged` 阶段**：重新获取 `&mut target`，执行后续逻辑。

```rust
// 当前：damage() 内部直接调用 on_damage
pub fn damage(&mut self, dmg: i32, caster: PlrId,
              on_damage: OnDamageFunc, ...) -> i32 {
    // ...
    on_damage(caster, self.as_ptr(), dmg, ...);  // ← UB 点
    self.on_damaged(...)
}

// 改为：
pub fn damage_core(&mut self, dmg: i32, ...) -> (i32, bool) {
    // 计算伤害并应用
    // 返回 (实际伤害, 是否需要 on_damage)
}
```

调用方（`attacked()` 或各技能 `action`）中：
- 获取 `&mut target` → 调用 `damage_core()`，记录结果
- 释放 `&mut target`
- 调用 `on_damage` 回调
- 重新获取 `&mut target` → 调用 `on_damaged()`

#### 影响面与工作量

- 约 8 个 `on_damage` 回调实现
- `attacked()`/`defned()` 等调用方 ~20 处
- 预估 150–200 行改动

#### 收益

- 消除最后一类 `&mut Player` 别名 UB
- 可移除 `mutable-noalias=no`，回归 stable toolchain
- 保持现有架构风格，无需大规模重构

---

### 4.5 方案 E：Arena + token 模式（大规模重构）

#### 实现思路

将 `Storage` 改为 arena 分配器，使用 `SlotMap<PlrId, Player>` 替代 `HashMap`，并将所有 `Player` 方法改为自由函数，通过 `storage` 和 `plr_id` 间接访问。

```rust
pub struct Storage {
    players: SlotMap<PlrId, Player>,
    // ...
}

// 所有方法签名从：
// fn step(&mut self, ... , storage: &Arc<Storage>)
// 改为：
fn step(plr_id: PlrId, storage: &mut Storage, ...)
```

#### 影响面与工作量

- 全项目 162 处 `just_get_player_mut` 调用点需重构
- 所有 `Player` 方法签名修改，涉及 ~50 个文件
- 预估 1000+ 行改动

#### 性能风险

- 每次访问需要额外的 HashMap 查找，hot path 可能下降 5–10%

#### 收益

- 完全消除 unsafe，由借用检查器保证安全性
- 无需 nightly 或特殊 flag

---

### 4.6 方案 G：全局 EventQueue（`63b81dd` 原型）

#### 实现方式

将跨实体副作用改写为命令队列，`Runner` 独占 `Storage` 和 `EventQueue`，在 `process_events()` 中统一处理。技能实现只能 `push` 事件，不能直接获取 `&mut Player`。

```rust
pub enum Event {
    TryAttack { caster: PlrId },
    Attack { caster: PlrId, target: PlrId, is_mag: bool },
    DealDamage { caster: PlrId, target: PlrId, dmg: i32 },
}
```

#### 理论优势

| 问题 | EventQueue 彻底化后的状态 |
|------|--------------------------|
| `&self → &mut Storage` provenance | 可避免，由 `Runner` 持有 `&mut Storage` |
| 跨实体 split borrow | 可避免，事件边界统一结算 |
| 回调重入拿到第二个 `&mut same_player` | 若严格禁止回调直接碰 Storage，可避免 |
| `mutable-noalias=no` 依赖 | 可移除 |

#### 原型不足

- 事件类型极少（仅三种），未覆盖 `pre_action`、`post_action`、`on_die`、`kill`、`revive`、`clear states`、`summon/remove`、`sync world` 等
- 伤害生命周期仍是同步栈，`Attack` 事件直接进入 `target.attacked()` → `damage()` → 回调，未命令化
- 技能回调拿不到 `Storage`，无法实现当前很多跨实体操作
- 事件负载表达力不足，缺乏顺序语义定义

#### 当前主线已吸收的部分

- 局部 command buffer：`run_post_kill()` 先释放引用再执行回调
- 延迟同步生命周期：`pending_spawns`、`death_queue`、`pending_revivals` 等
- 受控 drain loop：`run_update_end()`

#### 可行性评估

- 理论上可行，但需要一次完整的架构重写（与方案 E 相当甚至更大）
- 对当前“消除剩余 UB”的目标，性价比低
- 只有在项目明确想走“命令缓冲 / replayable event log / 强约束 ECS”路线时才值得作为长期重构方向

---

### 4.7 方案 H：Dart async / async-like 改造

#### Dart async 的真实作用

原版 namerena 在驱动层使用 `async`，主要用于：
- 外层驱动分帧（`nextUpdates()` 返回 `Future<RunUpdates>`）
- 让出浏览器事件循环（`Future.delayed` / `Timer`）
- 初始化阶段异步化

战斗内核 `round()` → `step()` → `action()` → `attacked()` → `damage()` 仍为同步调用，且 Dart 没有 Rust 的 `&mut` 独占规则。

#### 为什么 Rust async 不能直接修掉 UB

- `async fn` 不会放宽 `&mut` 独占规则
- 跨 `await` 持有 `&mut Player` 会将借用保存在状态机中，无法安全获取同一玩家
- 若在 `await` 前释放引用，本质已是手动分阶段
- 改造面大：`SkillTrait` 等需 async_trait 或装箱 Future，执行顺序语义需重定义

#### async-like 分阶段思想的价值

借鉴 async 的“暂停/恢复”边界，将同步栈改写为显式阶段：
1. 持有 `&mut target`，执行核心逻辑
2. 释放引用，执行回调
3. 重新获取引用，继续后续

这正是方案 D 的本质，比引入真实 async runtime 更轻量、更可控。

---

## 5. 回退到方案 A 的利弊分析

### 5.1 回退 + 保留 mutable-noalias=no

| 方面 | 影响 |
|------|------|
| 正确性 | ✅ noalias=no 阻止 LLVM 利用别名 UB，release 模式应与当前一致 |
| kill 回调 | ⚠️ `run_post_kill` 被移除，回退到内联调用。但因 noalias=no，set_level bug 不会触发 |
| provenance | 🔴 退化：`&self → *mut Storage` 的 provenance 无效。Miri 和未来编译器可能捕获 |
| 代码量 | ➖ 减少 ~140 行 |
| 性能 | ⚖️ 无变化 |
| 可维护性 | 🟢 略微简化 |
| Miri 兼容性 | 🔴 更差 |

### 5.2 回退 + 不用 mutable-noalias=no（回到最原始状态）

| 方面 | 影响 |
|------|------|
| 正确性 | 🔴 release 模式 38 test failures（已验证） |
| kill 回调 | 🔴 实际触发 UB（set_level 不可见） |
| toolchain | ✅ 可用 stable |
| 性能 | ⚠️ noalias 优化理论上有效，但实测差异 < 1% |

### 5.3 结论

回退到方案 A 无实质收益：
- **性能**：`UnsafeCell` 零开销，`run_post_kill` 开销可忽略
- **正确性**：方案 A 的 provenance 违规更严重，虽被 noalias=no 掩盖，但未来风险更高
- **代码量**：减少 140 行机械性代码，但牺牲了语义正确性
- **维护安全**：方案 B/C 的 `UnsafeCell` 明确传达“内部可变”意图，而方案 A 的强转无任何编译器保护

---

## 6. 彻底消除 UB 的可行路径

当前方案 C 仍有 `on_damage` 回调中的 `&mut Player` 别名 UB，只是被 `mutable-noalias=no` 掩盖。

### 6.1 方案 D：延迟 on_damage（推荐）

（详见 4.4）

- **工作量**：150–200 行
- **收益**：消除所有别名 UB，移除 nightly 依赖
- **风险**：🟢 低，已有 `run_post_kill` 成功先例

### 6.2 方案 E：Arena + token

（详见 4.5）

- **工作量**：1000+ 行
- **收益**：彻底消除 unsafe，借用检查器保证安全
- **风险**：🟡 中等性能下降，重构范围大

### 6.3 方案 F：RefCell 运行时检查（调试辅助）

```rust
players: UnsafeCell<FastHashMap<PlrId, RefCell<Player>>>,
```

- **优点**：debug 模式下 panic 报告重复借用
- **缺点**：release 模式仍有 borrow flag 开销（~2–3%），可通过 feature flag 在 release 切回 UnsafeCell
- **适用性**：可作为调试辅助，但非彻底消除 UB 的方案

---

## 7. 性能基准参考

已完成的基准测试（mario vs luigi，100000 场，single-thread，`--features no_debug`）：

### win_rate_st

| 版本 | Run 1 | Run 2 | Run 3 | 平均 |
|------|-------|-------|-------|------|
| Baseline (stable, noalias=yes) | 3.279s (30494/s) | 3.113s (32126/s) | 3.399s (29422/s) | ~3.264s |
| 当前 (nightly, noalias=no) | 3.136s (31884/s) | 3.155s (31691/s) | 3.113s (32125/s) | ~3.135s |

### --perf（含 init）

| 版本 | Run 1 | Run 2 | Run 3 | 平均 fight |
|------|-------|-------|-------|-----------|
| Baseline | 5.990s | 5.992s | 6.213s | ~3.037s |
| 当前 | 6.062s | 6.196s | 6.053s | ~3.041s |

**结论**：fight 部分差异 < 0.2%，在噪声范围内。`mutable-noalias=no` 对此项目无可测量的性能影响。

`UnsafeCell` 本身是 `#[repr(transparent)]` 的零开销抽象；`run_post_kill` 的 `take/put` 每次 kill 回调两次 `mem::replace`，在正常战斗中 kill 事件频率远低于主循环，开销不可测量。

---

## 8. 推荐路线与落地计划

| 时间尺度 | 建议 | 原因 |
|----------|------|------|
| **短期** | 维持方案 C，无需改动 | 零性能损失，release 正确性已验证 |
| **中期（可选）** | 实施方案 D（延迟 on_damage） | 消除最后的 `&mut` 别名 UB，移除 nightly 依赖，回归 stable toolchain |
| **长期（架构重写时）** | 全局 EventQueue 或 Arena+token | 仅在需要架构纯度、事件日志化或强约束 ECS 时考虑，成本高 |
| **不推荐** | 回退到方案 A | 零收益，更差的 provenance 安全性，丢失已修复的 kill 回调别名 |