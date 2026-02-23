# tswn-core State / act 重写方案（2026-02 重写版）

## 1. 文档目标

本文档是 `tswn-core\plan.md` 的重写版本，目标是给出一套可以在当前代码结构上直接落地的方案，解决：

1. 技能 `act()` 没有形成完整执行链；
2. State 结构已零散存在，但生命周期和触发机制未打通；
3. 与 Dart 行为对齐时缺乏统一迁移规则。

> 本文档优先级高于旧方案。旧内容中的实验性设计（尤其是全量 `dyn Trait + Any` 方案）不作为当前实施基线。

## 1.1 当前进度（2026-02-23）

1. ✅ 已完成 `PlayerStateStore` 纯 `Box<dyn StateTrait>` 改造，并移除 `StateValue` 路径。
2. ✅ 已打通 `SkillStorage` 的 `update_state/pre_step/pre_action/post_action` 分发。
3. ✅ 已接入 `Player::step/action` 调用链（含状态 pre/post 处理）。
4. ✅ 已迁移 `Poison/Haste/Slow/Ice` 的状态写入与核心生命周期逻辑。
5. ✅ `src/player/skill` 已按 Dart 拆分为 `act/` 与 `skl/` 子目录，并保留旧路径 re-export 兼容。
6. ✅ 已补齐 `Berserk/Charm/Curse/Merge/Shield/Zombie` 的 boxed-state 接入与基础行为。
7. ✅ 已补充回归测试（当前 `cargo test`：29 通过）。

---

## 2. 约束与原则

### 2.1 项目约束

- 目标项目：`tswn-core`（Rust edition 2024）
- 迁移来源：`namer-src`（Dart，代码不完整且版本较旧）
- 可兜底来源：`fast-namerena/branch/latest/md5.js`（非常大，仅按需局部检索）

### 2.2 设计原则

1. **性能优先**：避免不必要的动态分发与频繁堆分配。
2. **最小依赖**：不引入额外框架式状态系统。
3. **扩展性优先**：状态可按技能逐步迁移，不要求一次性重构全局。
4. **贴合现状**：基于当前 `PlayerStateStore + SkillStorage` 结构演进。

---

## 3. 当前代码现状（as-is）

### 3.1 已有基础

1. `PlayerStateStore`：`FastHashMap<StateTag, Box<dyn StateTrait>>`，当前已稳定接入 `FireState`。
2. `SkillTrait`：已定义 `act / pre_step / pre_action / post_action / pre_defend / post_defend / post_damage / die / kill`。
3. `SkillStorage`：已实现按 `proc_kinds()` 的索引缓存，部分 proc 分发已存在（如 `pre_defend/post_defend/post_damage/die/kill`）。
4. 多个状态结构体已定义在技能文件中（如 `PoisonState/HasteState/SlowState/IceState/CurseState/CharmState`），但大多未接入运行链路。

### 3.2 主要缺口

1. 目前仅 `FireState` 已接入统一存储，其他状态仍未迁入 `PlayerStateStore`。
2. `SkillStorage` 已接入 `pre_step/pre_action/post_action/update_state` 统一分发，但对应技能实现仍较少。
3. `Player::update_states()` 内尚未直接承载技能状态分发（当前在 `step()` 前段统一触发）。
4. `Player::action()` 仍是 TODO，`act()` 未形成完整闭环。
5. 状态过期移除和日志记录缺少统一语义。

---

## 4. 最终架构决策

### 4.1 采用方案

采用“**Box 状态存储 + 技能 proc 行为驱动**”的两层模型：

1. **数据层**：`Player.state` (`PlayerStateStore`) 只负责状态数据读写。
2. **行为层**：`SkillTrait` proc 负责状态触发、结算、过期和日志。

### 4.2 明确不采用（当前阶段）

1. 不采用 `StateValue` 的全量枚举扩展路线；
2. 不采用在业务逻辑中散落 `Any` 下转型；
3. 不新建第二套“状态专用 proc 注册系统”（避免与 `SkillStorage` 重叠）。

原因：

- 枚举集中维护会随状态数增长而膨胀；
- 与当前 `Box<dyn SkillTrait>` 的扩展路径不一致；
- downcast 只应收敛在 `PlayerStateStore` helper，而不是散落在技能逻辑中。

---

## 5. 数据模型重写方案

## 5.1 定义 StateTrait 并接入 Box 存储

将状态存储切到 `Box<dyn StateTrait>`：

```rust
pub trait StateTrait: Debug + Any {
    fn meta_type(&self) -> i32 { 0 }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn StateTrait>;
}

pub struct PlayerStateStore {
    states: FastHashMap<StateTag, Box<dyn StateTrait>>,
}
```

> 各状态结构体继续放在各技能文件中；新增状态不再要求改中心化 `StateValue` 枚举。

### 5.2 PlayerStateStore helper 统一化

为状态容器提供一致接口：

1. 泛型访问：`get<T>() / get_mut<T>() / set<T>() / clear<T>() / has<T>()`；
2. 热路径 helper：如 `fire_mag()/add_fire_mag()`；
3. 统一 tag 语义，避免依赖枚举 discriminant。

并提供：

- `meta_type(tag) -> i32`：负面/正面/中立判定；
- `clear_negative_states()`：统一清负面状态入口。

### 5.3 生命周期字段规范

状态结构体中的生命周期字段统一约定：

- `step`：按行动次数衰减；
- `count`：按触发次数衰减；
- `owner/caster/target`：明确来源与归属；
- `on_xxx: Option<()>` 这类占位字段可逐步替换为显式注释或删除，避免语义不清。

---

## 6. 行为执行链重写方案

## 6.1 SkillTrait 最小必要调整

为了能真正作用到玩家状态，建议补齐/调整以下能力：

1. `update_state` 需要可操作玩家状态（当前 `fn update_state(&self)` 信息不足）；
2. `SkillStorage` 需要有完整分发方法：
   - `update_state(...)`
   - `pre_step(...) -> i32`
   - `pre_action(...)`
   - `post_action(...)`

可选落地方式（推荐其一并统一）：

- A. `update_state(&mut self, status: &mut PlayerStatus, state: &mut PlayerStateStore)`
- B. `update_state(&mut self, args: SkillArgs)`

建议优先 A：对 update_state 足够、借用更简单、开销更低。

### 6.2 Player.step/action 调用顺序

重写后的单次行动流程建议为：

1. `rebuild_base_status`（当前 `update_states` 基础部分）；
2. `skills.update_state(...)`（应用持续状态影响）；
3. 计算步进值；
4. `skills.pre_step(...)`（冻结/加速等前置处理）；
5. 达到行动阈值后：
   - `skills.pre_action(...)`
   - 执行主动作（普通攻击或技能 `act()`）
   - `skills.post_action(...)`
6. 最后处理延迟清理（如过期状态、clear_negative_states）。

### 6.3 避免借用冲突的统一做法

任何“遍历 + 删除”场景采用两段式：

```rust
let mut expired = Vec::new();
for tag in tags_to_check {
    if should_expire(tag) {
        expired.push(tag);
    }
}
for tag in expired {
    state_store.remove(tag);
}
```

不要在迭代容器时直接删除当前元素。

---

## 7. 状态语义模板（用于每个技能迁移）

每个状态按以下模板实现：

1. **set**：首次附加状态；
2. **refresh**：重复命中时刷新持续时间/叠层；
3. **tick**：在 `pre_step` 或 `post_action` 衰减；
4. **effect**：触发实际影响（伤害、属性变化、冻结等）；
5. **expire**：到期移除 + 写 `RunUpdate`。

### 7.1 Poison（首个样板）

- `act()`：命中后写入/刷新 `PoisonState { caster, target, atp, count }`
- `post_action()`：每行动后按公式结算毒伤，`count -= 1`
- `count == 0`：移除毒状态并写解除日志

### 7.2 Haste / Slow

- `update_state()`：影响速度相关数值
- `post_action()`：递减持续回合，到期移除

### 7.3 Ice

- `pre_step()`：冻结类限制（修改 step 或阻断行动）
- 到期后清理冻结标记并写日志

---

## 8. 分阶段实施计划

## Phase 1：基础设施打通（必须先做）

1. 定义 `StateTrait` 并重构 `PlayerStateStore`（boxed 存储 + helper）；
2. 为 `SkillStorage` 增加 `update_state/pre_step/pre_action/post_action` 分发；
3. 把 `Player::step/action` 接上这四个分发点。

**验收标准**：

- 无新依赖；
- 能在一个最小回合中稳定进入 pre/post action 流程；
- 状态写入与读取可用。

## Phase 2：Poison 全链路（首个行为状态）

1. 补全 `PoisonSkill::act`；
2. 补全 `PoisonSkill::proc_kinds`（至少 `PostAction`）；
3. 补全 `PoisonSkill::post_action` 与过期清理。

**验收标准**：

- 中毒可叠加或刷新（按规则选其一并固定）；
- 每次 post_action 有结算；
- 到期后可靠移除且无借用冲突。

## Phase 3：Haste/Slow/Ice

1. 迁移速度类与冻结类状态；
2. 统一生命周期处理；
3. 与已有 `check_immune`/死亡流程做兼容验证。

## Phase 4：一致性与清理

1. 增加 `clear_negative_states` 统一入口；
2. 消除重复状态访问代码；
3. 补充关键回归测试（状态触发顺序、日志、过期时机）。

---

## 9. 对齐策略（Dart / JS）

1. **优先 Dart**：先查 `namer-src` 可用实现。
2. **Dart 缺失时再查 JS**：只针对具体技能和触发点，局部检索 `fast-namerena/branch/latest/md5.js`。
3. **避免盲读大文件**：先明确“技能名 + 触发阶段 + 关键变量”再定位。
4. 若出现差异，不默认 Dart 一定正确，以“当前目标行为 +可验证样例”定最终语义。

---

## 10. 风险与回退策略

### 10.1 风险

1. 旧 TODO 较多，容易出现“流程接上但语义不完整”；
2. pre/post 调用顺序不一致会导致行为偏差；
3. 状态过期时机与日志时机可能与旧实现有细微差异。

### 10.2 回退策略

1. 每个状态迁移都独立提交，不做大爆改；
2. 新增行为先挂在单个技能上验证，再批量推广；
3. 一旦出现链路回归，优先回退最近状态模块，而不是推翻基础设施。

---

## 11. 最终结论

当前最可行路线是：

- 保留 `PlayerStateStore + Box<dyn StateTrait>` 作为状态数据层；
- 用 `SkillTrait + SkillStorage::proc_kinds` 做行为层；
- 先打通基础分发，再以 `Poison` 为样板逐个迁移状态。

这条路线改动面可控、性能风险低、与现有代码最兼容，并且能持续向 Dart 语义对齐。
