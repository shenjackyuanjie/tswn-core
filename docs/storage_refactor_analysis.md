# Storage 内部可变性方案对比分析（深入版）

> 创建: 2026-03-28
> 更新: 2026-05-06
> 代码基线: 当前 HEAD（仅引用当前仓库内可验证代码；历史原型只保留结论，不再直链仓库外目录）

---

## 1. 摘要与修正后的结论

重新审阅当前代码后，原先“当前主线只剩 `on_damage` 一类别名 UB”的结论需要修正。

当前主线仍然是方案 C：

- `Storage` 全字段 `UnsafeCell`
- `post_kill` 通过 [run_post_kill](../crates/tswn_core/src/player/skill/store.rs#L653) 分阶段执行
- toolchain 依赖 [nightly](../rust-toolchain.toml#L2) 与 [mutable-noalias=no](../.cargo/config.toml#L7)

但更深入地沿着当前调用链和 trait 签名向下看，会得到更准确的现状判断：

1. 已经明确修掉的，是 provenance 问题和 `post_kill` 的同实体别名。
2. `update_state` 主路径也已经局部去别名，因为 [update_states](../crates/tswn_core/src/player/impl_attr.rs#L211) 现在走的是 [update_state_inline](../crates/tswn_core/src/player/skill.rs#L215)，而不是运行期再次从 `Storage` 把 owner 借出来。
3. 仍然残留的别名面，不止 `on_damage`。当前至少还有两大类热路径会在持有 `&mut self` 时重新通过 `Storage` 取回同一个 `Player`：
   - owner/self phase：`act` / `pre_action` / `post_action` / `pre_defend` / `clear_positive_runtime`
   - target/caster phase：`on_damage`
4. 这些剩余路径现在都依赖 `mutable-noalias=no` 避免 LLVM 基于 `&mut` 独占假设做错误优化；也就是说，当前主线的“行为正确”是建立在编译器旗标上的，而不是已经从 Rust 语义上彻底闭合。

因此，推荐路线也要跟着修正：

- **短期**：维持方案 C，不回退。
- **中期第一步**：实施方案 D，把 `damage/on_damage` 拆成显式阶段。它仍然是高性价比改造，但**不应再被视为单独即可清空全部 UB 的方案**。
- **中期第二步**：实施本文新增的方案 J，把 owner-phase 的 trait 上下文改成阶段化能力上下文，进一步清理 `act/pre_action/post_action/pre_defend` 中的同实体重借。
- **长期**：若准备做更大规模架构重写，再考虑本文新增的方案 I（split components），或原有的方案 G / E（全局 EventQueue / Arena+token）。

一句话概括当前状态：

> 方案 C 已经把最危险、最容易在 release 中爆炸的两块问题压住了，但它不是“只剩最后 8 个回调”的状态；更准确地说，是“已经有两类局部去别名先例，但战斗主循环里仍残留多类同实体重借，统一由 `mutable-noalias=no` 掩护”。

---

## 2. 代码现状地图

### 2.1 `Storage` 是运行期内部可变性的总入口

当前 `Storage` 的核心结构在 [storage.rs](../crates/tswn_core/src/engine/storage.rs#L64) 到 [storage.rs](../crates/tswn_core/src/engine/storage.rs#L93)，关键点是：

- 玩家表是 `UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>`
- 运行期还有一个 `in_post_damage_player` 标记，专门用于使魔分摊伤害时校正死亡顺序

摘自 [storage.rs](../crates/tswn_core/src/engine/storage.rs#L64-L93)：

```rust
pub struct Storage {
    skills: UnsafeCell<FastHashMap<usize, Skill>>,
    groups: UnsafeCell<FastHashMap<usize, Vec<PlrId>>>,
    alive_groups: UnsafeCell<Vec<Vec<PlrId>>>,
    players: UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>,
    player_group: UnsafeCell<FastHashMap<PlrId, usize>>,
    pending_spawns: UnsafeCell<Vec<PendingSpawn>>,
    pending_remove_players: UnsafeCell<Vec<PlrId>>,
    death_queue: UnsafeCell<Vec<PlrId>>,
    pending_revivals: UnsafeCell<Vec<PlrId>>,
    needs_sync: AtomicBool,
    player_id_counter: AtomicU64,
    eval_rq: f64,
    in_post_damage_player: UnsafeCell<Option<PlrId>>,
}
```

摘自 [just_get_player_mut](../crates/tswn_core/src/engine/storage.rs#L336)：

```rust
pub fn just_get_player_mut(&self, ptr: PlrId) -> Option<&mut Player> {
    self.players_ref().get(&ptr).map(|player| unsafe { &mut *player.get() })
}
```

这段实现有两个效果：

1. 相比历史上的 `&self -> *mut Storage` 裸强转，当前 provenance 合法性已经明显更好，因为指针来自 `UnsafeCell::get()`。
2. 但别名规则不再由编译器保证，而是完全变成运行期 discipline：谁在什么阶段调用 `just_get_player_mut()`，就决定了是否会出现同实体重借。

### 2.2 真正把风险放大的，不只是 `Storage`，而是回调签名本身

当前战斗主路径把 `Arc<Storage>` 直接暴露给了多个层级：

- `OnDamageFunc`：见 [player/mod.rs](../crates/tswn_core/src/player/mod.rs#L109)
- `SkillArgs`：见 [player/skill.rs](../crates/tswn_core/src/player/skill.rs#L163)
- `StateTrait::on_pre_defend` / `on_post_damage`：见 [state.rs](../crates/tswn_core/src/player/state.rs#L79) 与 [state.rs](../crates/tswn_core/src/player/state.rs#L119)

摘自 [player/mod.rs](../crates/tswn_core/src/player/mod.rs#L109) 与 [player/skill.rs](../crates/tswn_core/src/player/skill.rs#L163)：

```rust
pub type OnDamageFunc = fn(PlrId, PlrId, i32, &mut RC4, &mut RunUpdates, &Arc<Storage>);

pub type SkillArgs<'d> = (PlrId, &'d mut RC4, &'d mut RunUpdates, &'d Arc<Storage>);
```

这意味着：

- 任何 `on_damage` 回调都可以在目标仍然以 `&mut self` 存活时，再从 `storage` 把 target 或 caster 借回来。
- 任何 `SkillTrait` / `StateTrait` 的运行期回调，只要拿到了 `args.0 + storage`，理论上也都能在 owner 自己的 `&mut self` 还没结束时，再借一次 owner。

从设计层面看，真正的根因不是“某几个技能写得危险”，而是**当前 trait 上下文把整个 `Storage` 能力无差别地暴露给了所有阶段**。

### 2.3 当前热路径的真实调用栈

伤害主路径集中在以下几个入口：

- [pre_defend](../crates/tswn_core/src/player/impl_runtime.rs#L1710)
- [post_defend](../crates/tswn_core/src/player/impl_runtime.rs#L1845)
- [attacked](../crates/tswn_core/src/player/impl_runtime.rs#L1900)
- [defned](../crates/tswn_core/src/player/impl_runtime.rs#L1936)
- [damage](../crates/tswn_core/src/player/impl_runtime.rs#L1962)
- [on_damaged](../crates/tswn_core/src/player/impl_runtime.rs#L2040)
- [on_die_impl](../crates/tswn_core/src/player/impl_runtime.rs#L2096)

`damage()` 的关键片段如下，摘自 [impl_runtime.rs](../crates/tswn_core/src/player/impl_runtime.rs#L1962-L2017)：

```rust
pub fn damage(
    &mut self,
    dmg: i32,
    caster: PlrId,
    on_damage: OnDamageFunc,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) -> i32 {
    // ... 省略 heal / zero-dmg 分支
    let old_hp = self.status.hp;
    self.status.hp -= dmg;
    if self.status.hp < 0 {
        self.status.hp = 0;
    }
    let mut msg = "[1]受到[2]点伤害".to_string();
    if dmg >= 160 {
        msg.push_str("[s_dmg160]");
    } else if dmg >= 120 {
        msg.push_str("[s_dmg120]");
    }
    updates.emit(|| {
        let mut update = RunUpdate::new(msg, caster, self.as_ptr(), dmg as u32);
        update.delay0 = if dmg > 250 { 1500 } else { 1000 + dmg * 2 };
        update
    });
    on_damage(caster, self.as_ptr(), dmg, randomer, updates, storage);
    self.on_damaged(dmg, old_hp, caster, randomer, updates, storage)
}
```

这段代码的结构决定了一个核心事实：

```text
attacked/defned
  -> damage(&mut target)
    -> on_damage(..., &Arc<Storage>)
      -> callback 内可 just_get_player_mut(...)
    -> on_damaged(&mut target)
```

只要 `on_damage` 回调去拿 `target` 或 `caster`，就会出现同实体重借。后文会列出当前仍在这么做的具体实现。

---

## 3. 当前主线已经修掉了什么

### 3.1 provenance 与跨实体 split borrow

相比最早的“裸强转 `&self -> *mut Storage`”思路，当前 [Storage](../crates/tswn_core/src/engine/storage.rs#L64) 的 `UnsafeCell` 设计至少把 provenance 问题从“显式无效的可写来源”变成了“显式声明的内部可变性”。

同时，玩家表从：

```rust
HashMap<PlrId, Player>
```

变成：

```rust
UnsafeCell<FastHashMap<PlrId, UnsafeCell<Player>>>
```

以后，不同玩家之间的 split borrow 至少有了明确的实现意图。也就是说，`A` 攻击 `B` 时重新从 `storage` 取 `B`，这件事本身不再需要通过 provenance 不合法的裸强转来达成。

### 3.2 `post_kill` 的 killer 同实体别名已经被局部消除

当前 `post_kill` 已经不是“拿着 `&mut killer` 直接递归调用 kill 回调”，而是先把技能实现取出来，释放 killer，再调用回调，最后放回。

摘自 [run_post_kill](../crates/tswn_core/src/player/skill/store.rs#L653)：

```rust
pub fn run_post_kill(...) {
    for skill_key in keys {
        let (mut skill_type, level) = {
            let killer = storage.just_get_player_mut(caster).expect("killer not found in storage");
            let skill = killer.skills.store.get_mut(&skill_key).expect("skill not found in store");
            (skill.take_skill_type(), skill.level())
        };

        let triggered = skill_type.kill_with_level(level, target, (caster, randomer, updates, storage));

        {
            let killer = storage.just_get_player_mut(caster).expect("killer not found in storage");
            let skill = killer.skills.store.get_mut(&skill_key).expect("skill not found in store");
            skill.put_skill_type(skill_type);
        }

        if triggered {
            break;
        }
    }
}
```

主路径对它的调用在 [impl_runtime.rs](../crates/tswn_core/src/player/impl_runtime.rs#L2136) 与 [impl_runtime.rs](../crates/tswn_core/src/player/impl_runtime.rs#L2246)。

这不是纸面修复，而是有行为锚点的：

- [merge_kill_applies_owner_growth](../crates/tswn_core/src/player/test.rs#L910) 覆盖了 kill 后 owner 增长逻辑
- [perf/ub_fix_no_debug.md](perf/ub_fix_no_debug.md) 记录了 `330cd46` 后消除了 27 个 release-only diff failures
- 同一文档也说明 UB 导致的 release-only failure 在修复后全部消失

### 3.3 `update_state` 主路径已经有一条成功的“内联化”先例

这一点非常关键，因为它说明本项目并不是非得所有回调都拿 `Storage` 才能工作。

当前 `Player::update_states()` 在 [impl_attr.rs](../crates/tswn_core/src/player/impl_attr.rs#L211) 中已经改为：

```rust
pub fn update_states(&mut self) {
    // ... 初始化 status
    self.apply_update_state_effects();
    self.skills.update_state_inline(&mut self.status);
}
```

而 trait 本身也明确给了一个 inline 入口，见 [skill.rs](../crates/tswn_core/src/player/skill.rs#L215)：

```rust
fn update_state_inline(&mut self, _level: u32, _status: &mut super::PlayerStatus) {}
```

这条旁路的意义是：

- 主循环里的 `update_state` 已经不再需要把 owner 重新从 `Storage` 里借出来
- 也就是说，“把阶段内真正需要的最小能力直接传进回调”这件事，在当前代码里已经有成功落地案例

需要注意的是，`SkillStorage::update_state()` 这个旧接口仍然还在，见 [skill/store.rs](../crates/tswn_core/src/player/skill/store.rs#L332)，测试里也仍有直接调用，例如 [player/test.rs](../crates/tswn_core/src/player/test.rs#L930-L931) 与 [player/test.rs](../crates/tswn_core/src/player/test.rs#L959-L960)。但它已经不再是战斗主循环的主通道。

### 3.4 使魔分摊伤害已经使用“局部阶段标记”修顺序

`Storage` 里的 [in_post_damage_player](../crates/tswn_core/src/engine/storage.rs#L93) 与辅助方法 [set_in_post_damage](../crates/tswn_core/src/engine/storage.rs#L231)、[clear_in_post_damage](../crates/tswn_core/src/engine/storage.rs#L234)、[is_in_post_damage](../crates/tswn_core/src/engine/storage.rs#L238) 是另一条很重要的先例：

- 它不是全局 EventQueue
- 但它已经承认“有些路径必须分阶段标记，否则死亡顺序会错”

对应的使用点在 [summon.rs](../crates/tswn_core/src/player/skill/act/summon.rs#L245-L268)：

```rust
struct SummonShareDamageSkill;

fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
    let owner_id = args
        .3
        .get_player(&args.0)
        .and_then(|player| player.get_state::<MinionRuntimeState>())
        .and_then(|state| state.owner);
    let Some(owner_id) = owner_id else {
        return;
    };
    args.3.set_in_post_damage(args.0);
    if let Some(owner) = args.3.just_get_player_mut(owner_id) {
        owner.damage(dmg / 2, caster, on_summon_share_damage as OnDamageFunc, args.1, args.2, args.3);
    }
    args.3.clear_in_post_damage();
}
```

而顺序语义有测试覆盖：

- [owner_death_marks_linked_minion_for_cleanup](../crates/tswn_core/src/player/test.rs#L1268)
- [owner_death_removes_linked_minions_in_roster_order](../crates/tswn_core/src/player/test.rs#L1302)

这说明“局部 staging / phase marker”在本项目语义上是成立的，不是纸上设计。

---

## 4. 剩余风险清单

重新盘点后，当前残留风险至少可以分成两大类。

### 4.1 类别 A：owner/self phase 的同实体重借

这一类问题在原文档里基本没有展开，但在当前代码里其实很广泛。

关键调用点是：owner 自己的 `&mut self` 仍然活着时，把 `Storage` 传进了技能回调；而多个技能实现又通过 `just_get_player_mut(args.0)` 把 owner 再借了一次。

主路径入口见：

- [self.skills.pre_action(...)](../crates/tswn_core/src/player/impl_runtime.rs#L218)
- [skill.act(...)](../crates/tswn_core/src/player/impl_runtime.rs#L459)
- [run_post_action_chain(...)](../crates/tswn_core/src/player/impl_runtime.rs#L537) 与它的调用点 [impl_runtime.rs#L510](../crates/tswn_core/src/player/impl_runtime.rs#L510)
- [self.skills.pre_defend(...)](../crates/tswn_core/src/player/impl_runtime.rs#L1710)
- [clear_positive_runtime(...)](../crates/tswn_core/src/player/skill/store.rs#L593)

代表性热点如下：

| 阶段 | 外层持有的引用 | 代表实现 | 重新借回的是谁 | 说明 |
| ------ | ---------------- | ---------- | ---------------- | ------ |
| `act` | owner 的 `&mut self` | [ChargeSkill](../crates/tswn_core/src/player/skill/act/charge.rs#L51) | owner | `skill.act(...)` 在 [impl_runtime.rs](../crates/tswn_core/src/player/impl_runtime.rs#L459) 期间执行，`charge` 又 `just_get_player_mut(args.0)` |
| `act` | owner 的 `&mut self` | [AccumulateSkill](../crates/tswn_core/src/player/skill/act/accumulate.rs#L62) | owner | 与 `charge` 同类，owner 仍存活时再次取 owner |
| `pre_action` / `post_action` | owner 的 `&mut self` | [HideSkill](../crates/tswn_core/src/player/skill/skl/hide.rs#L92) / [hide.rs#L121-L135](../crates/tswn_core/src/player/skill/skl/hide.rs#L121-L135) | owner | `hide` 在 `pre_action` 与 `update_state` 路径都会再次借 owner |
| `pre_defend` | target 的 `&mut self` | [ReflectSkill](../crates/tswn_core/src/player/skill/skl/reflect.rs#L23) | target/owner | `reflect` 的 owner 就是当前 defender，自借最明显 |
| `clear_positive_runtime` | owner 的 `&mut self` | [ChargeSkill](../crates/tswn_core/src/player/skill/act/charge.rs#L100) / [AccumulateSkill](../crates/tswn_core/src/player/skill/act/accumulate.rs#L103) | owner | 清状态时又回头借 owner |

以当前 HEAD 粗搜，光是 `player/skill` 目录里就能找到 49 处 owner/target 级别的 `just_get_player_mut(...)` 命中；其中有一部分只是初始化或测试辅助，但上表这些都处在当前主循环的真实热路径内。

最直观的例子是 [ChargeSkill::act_with_level](../crates/tswn_core/src/player/skill/act/charge.rs#L51-L59)：

```rust
fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
    self.step += 2;
    self.on_post_action = Some(());
    self.on_update_state = Some(());
    args.2.add(RunUpdate::new("[0]开始[蓄力]", args.0, args.0, 1));
    let owner = args.3.just_get_player_mut(args.0).expect("cannot get charge owner from storage");
    owner.update_states();
    owner.set_magic_point(owner.magic_point() + 32);
}
```

而它的外层调用点正是 [Player::action](../crates/tswn_core/src/player/impl_runtime.rs#L213)。也就是说，这一类问题并不是“理论上 trait 太宽”，而是已经真正在主战斗路径里发生。

### 4.2 类别 B：`on_damage` 的 target/caster 重借

这是原文档已经识别到的一类，但可以列得更精确。

当前把 `OnDamageFunc` 传进 `attacked/defned/damage` 的调用点很多，但真正危险的 `on_damage` 实现并不是全部。下面这张表只列**会在回调内部重新取回 target/caster 可变引用**的实现：

| 回调 | 入口调用点 | 回调位置 | 重借对象 | 当前副作用 |
| ------ | ------------ | ---------- | ---------- | ------------ |
| `on_absorb` | [absorb.rs#L54](../crates/tswn_core/src/player/skill/act/absorb.rs#L54) | [absorb.rs#L58](../crates/tswn_core/src/player/skill/act/absorb.rs#L58) | caster | 吸血后给 caster 回血 |
| `on_berserk` | [berserk.rs#L86](../crates/tswn_core/src/player/skill/act/berserk.rs#L86) | [berserk.rs#L137](../crates/tswn_core/src/player/skill/act/berserk.rs#L137) | target | 给 target 叠狂暴状态 |
| `on_curse` | [curse.rs#L94](../crates/tswn_core/src/player/skill/act/curse.rs#L94) | [curse.rs#L166](../crates/tswn_core/src/player/skill/act/curse.rs#L166) | target | 给 target 叠诅咒状态 |
| `on_disperse` | [disperse.rs#L84](../crates/tswn_core/src/player/skill/act/disperse.rs#L84) | [disperse.rs#L92](../crates/tswn_core/src/player/skill/act/disperse.rs#L92) | target | 清 positive runtime/state，并扣 MP |
| `on_fire` | [fire.rs#L71](../crates/tswn_core/src/player/skill/act/fire.rs#L71) | [fire.rs#L75](../crates/tswn_core/src/player/skill/act/fire.rs#L75) | target | 给 target 增加火焰状态 |
| `on_fire`（复用） | [summon.rs#L236](../crates/tswn_core/src/player/skill/act/summon.rs#L236) | [fire.rs#L75](../crates/tswn_core/src/player/skill/act/fire.rs#L75) | target | 召唤技能复用了火球的 on_damage |
| `on_ice` | [ice.rs#L104](../crates/tswn_core/src/player/skill/act/ice.rs#L104) | [ice.rs#L163](../crates/tswn_core/src/player/skill/act/ice.rs#L163) | target | 冻结状态写入，且有二次重借 |
| `on_poison` | [poison.rs#L44](../crates/tswn_core/src/player/skill/act/poison.rs#L44) | [poison.rs#L113](../crates/tswn_core/src/player/skill/act/poison.rs#L113) | target | 中毒状态写入，且有二次重借 |
| `lazy_attack_on_damage -> lazy_infect` | [lazy.rs#L118](../crates/tswn_core/src/player/boss/lazy.rs#L118) | [lazy.rs#L132](../crates/tswn_core/src/player/boss/lazy.rs#L132) -> [lazy.rs#L186](../crates/tswn_core/src/player/boss/lazy.rs#L186) | target | 懒癌感染 |
| `covid_spread_on_damage -> covid_infect` | [covid.rs#L341](../crates/tswn_core/src/player/boss/covid.rs#L341) | [covid.rs#L349](../crates/tswn_core/src/player/boss/covid.rs#L349) -> [covid.rs#L370](../crates/tswn_core/src/player/boss/covid.rs#L370) | target | 新冠感染，并对全场存活玩家调移动点 |

这里有两个值得强调的细节：

1. 原文里写“约 8 个技能实现”，如果只按**唯一危险实现**来数，数量接近；但如果按**真实危险调用点**来数，已经不止 8 个。
2. 不是所有 `OnDamageFunc` 都危险。像 `on_assassinate`、`on_counter`、`on_critical`、`on_quake`、`on_rapid`、`on_thunder`、`on_poison_tick`、`on_summon_share_damage` 都是空/近空回调，它们本身不是别名来源。

### 4.3 EventQueue 原型为什么没有直接成为主线解法

历史原型在方向上是对的，但没有走完。

需要先说明一点：本文最早引用的 `../../tswn-core-old` / `../../tswn-core-63b81dd` 目录不在当前仓库里，已经不能作为当前仓库内可点击校验的证据。因此这一节只保留当时对原型的结论摘要，不再保留失效链接。

原型当时已经做了两件关键尝试：

- 引入 `Event` / `EventQueue` 一类显式事件层
- 把 `SkillArgs` 与 `OnDamageFunc` 从直接携带 `Storage` 改成携带事件队列或更窄的上下文

这说明 old 原型已经意识到：

- callback 不应该直接持有全局容器的可变能力
- 跨实体副作用应该先表达成事件，再由 runner 统一处理

但是，它没有彻底完成两件事：

1. `damage()` 当时仍然是同步栈，没有真正把伤害链彻底阶段化
2. 当前主线里需要的那批真实跨实体副作用（吸血、冰冻、中毒、净化、boss 感染、使魔分摊）并没有被完整重写成事件语义

因此，EventQueue 原型更像是“方向性证明”，而不是一个可以直接 cherry-pick 回主线的完整解法。

---

## 5. 方案总览（更新版）

下表把“当前代码真正修了什么、还剩什么”拆开比较：

| 方案 | 核心思想 | provenance | `post_kill` 同实体别名 | `on_damage` 别名 | owner-phase 别名 | 能否脱离 nightly/noalias | 改造规模 |
| ------ | ---------- | ------------ | ------------------------ | ------------------ | ------------------ | -------------------------- | ---------- |
| **A** | 裸 `unsafe` 强转 | ❌ | ❌ | ❌ | ❌ | ❌ | 最小 |
| **B** | `UnsafeCell` + `run_post_kill` | ✅ | ✅ | ❌ | ❌ | ❌ | 中等 |
| **C** | B + `mutable-noalias=no` | ✅ | ✅ | ⚠️ 被掩盖 | ⚠️ 被掩盖 | ❌ | 中等 |
| **D** | staged damage / delayed `on_damage` | ✅ | ✅ | ✅ | ❌ | ❌ | 中等 |
| **J** | 阶段化上下文 + 局部 effect buffer | ✅ | ✅ | ✅ | ✅ | ✅ | 中大 |
| **I** | split components / split field | ✅ | ✅ | ✅ | ✅ | ✅ | 大 |
| **G** | 全局 EventQueue | ✅ | ✅ | ✅ | ✅ | ✅ | 很大 |
| **E** | Arena + token | ✅ | ✅ | ✅ | ✅ | ✅ | 很大 |

这里最重要的修正是：

> 方案 D 不再是“单独即可回到 stable”的完整终局，它只能清掉 `damage/on_damage` 子系统的 alias。若要彻底移除 `mutable-noalias=no`，至少还需要 J 这类 owner-phase 去别名方案，或者更大规模的 I / G / E。

---

## 6. 各方案详解

### 6.1 方案 C：当前主线为什么还能工作

当前主线的工具链配置非常明确：

- [../rust-toolchain.toml#L2](../rust-toolchain.toml#L2) 选择 `nightly`
- [../.cargo/config.toml#L7](../.cargo/config.toml#L7) 开启 `-Z mutable-noalias=no`

而配置文件本身也已经把原因写得很清楚，见 [../.cargo/config.toml](../.cargo/config.toml#L2-L6)：

```toml
# 禁用 LLVM 对 &mut 引用的 noalias 优化。
# 本项目中 Storage（共享 Arc<Storage>）的设计允许多个 &mut Player 引用同时存在
# （通过 UnsafeCell），技能回调经常通过 storage.just_get_player_mut(owner_id)
# 访问与调用者 &mut self 相同的 Player，违反 Rust 的 noalias 约束。
rustflags = ["-Z", "mutable-noalias=no"]
```

从工程角度看，方案 C 的优点非常现实：

- 行为已经稳定
- 性能损失可忽略
- 已有一部分局部去别名修复（`run_post_kill`、`update_state_inline`、`in_post_damage`）

性能数据也支持“短期保留 C”这个判断：

- [perf/ub_fix_no_debug.md](perf/ub_fix_no_debug.md) 记录了 `330cd46` 后 UB 修复本身没有可测量性能退化
- 当前文档旧版中的 `mario vs luigi` 数据也显示 `mutable-noalias=no` 的 fight 部分差异接近噪声

但它的缺点同样明确：

- 正确性建立在编译器开关上，而不是类型边界上
- 一旦未来想回 stable，就必须继续清理剩余 alias 面

### 6.2 方案 D：staged damage / delayed `on_damage`

这是最应该先做、但需要重新界定边界的一步。

#### 6.2.1 它为什么仍然值得做

因为 `damage()` 的结构太明确了，拆开收益很高，风险也低：

- 现在的 `damage()` 在 [impl_runtime.rs#L1962-L2017](../crates/tswn_core/src/player/impl_runtime.rs#L1962-L2017)
- `on_damage` 之后的后续逻辑基本都在 [on_damaged](../crates/tswn_core/src/player/impl_runtime.rs#L2040)
- 也就是说，天然就有一个“core damage / callback / after damage”三段式边界

更重要的是，当前代码里已经存在“先改 HP，再独立调用 `on_damaged()`”的先例：

- [exchange.rs#L149](../crates/tswn_core/src/player/skill/act/exchange.rs#L149)
- [half.rs#L123](../crates/tswn_core/src/player/skill/act/half.rs#L123)

这说明 staged damage 不是往系统里强行塞一个全新执行模型，而是把已经存在的局部风格正规化。

#### 6.2.2 建议的 API 草图

下面是一个可编译方向明确、但细节仍可调整的草图：

```rust
pub struct DamageCoreResult {
    pub actual_dmg: i32,
    pub old_hp: i32,
    pub target: PlrId,
    pub needs_on_damage: bool,
}

impl Player {
    pub fn damage_core(
        &mut self,
        dmg: i32,
        caster: PlrId,
        updates: &mut RunUpdates,
    ) -> DamageCoreResult {
        // 只做数值更新与文案，不调用 on_damage / on_damaged
    }

    pub fn finish_damage(
        &mut self,
        result: DamageCoreResult,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        self.on_damaged(result.actual_dmg, result.old_hp, caster, randomer, updates, storage)
    }
}
```

外层调用变成：

```rust
let result = {
    let target = storage.just_get_player_mut(target_id).unwrap();
    target.damage_core(dmg, caster, updates)
};

if result.needs_on_damage {
    on_damage(caster, target_id, result.actual_dmg, randomer, updates, storage);
}

let ret = {
    let target = storage.just_get_player_mut(target_id).unwrap();
    target.finish_damage(result, caster, randomer, updates, storage)
};
```

#### 6.2.3 它能修什么，不能修什么

**能修的**：

- `on_damage` 里重借 target / caster 的那一大类 alias
- 也能让 boss 的 `lazy_attack_on_damage` / `covid_spread_on_damage` 这种二级跳转回调变得安全得多

**不能单独修的**：

- `skill.act(...)` 期间 owner 自借
- `pre_action/post_action/pre_defend/clear_positive_runtime` 期间 owner 或 target 自借

因此，方案 D 的准确定位应该是：

> **优先级最高的第一步子方案**，而不是完整终局。

#### 6.2.4 预估工作量

- 中央改动集中在 [impl_runtime.rs](../crates/tswn_core/src/player/impl_runtime.rs)
- 需要逐个过一遍表 4.2 中的危险 `on_damage` 实现
- 预估实际代码改动在 200 到 300 行之间，比原文估计略大，但仍属“可控的局部改动”

### 6.3 新方案 J：阶段化上下文 + 局部 effect buffer

这是本文新增、也是我认为最适合作为 D 之后第二步的方案。

#### 6.3.1 它解决的根因是什么

从当前代码看，真正放大风险的是两件事：

1. owner/target 明明已经以 `&mut self` 形式存在，trait 回调却还只能拿 `PlrId + Arc<Storage>`
2. `SkillArgs` / `OnDamageFunc` 暴露的是“完整 `Storage` 能力”，而不是阶段内真正需要的最小能力

这和 `update_state_inline` 的思路正好相反。`update_state_inline` 成功的原因就在于：

- 它直接把 `&mut PlayerStatus` 传给技能
- 技能不需要再去 `Storage` 里把 owner 借一遍

方案 J 要做的，就是把这种思路从 `update_state` 扩展到其他阶段。

#### 6.3.2 一个更贴近当前代码的草图

下面的草图故意不走“全局 EventQueue”，而是走**局部 buffer**，这样更贴近当前架构：

```rust,ignore
pub struct ActionCtx<'a> {
    pub owner: &'a mut Player,
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
    pub world: &'a Arc<Storage>,
    pub effects: SmallVec<[Effect; 4]>,
}

pub enum Effect {
    Attack { target: PlrId, atp: f64, is_mag: bool, on_damage: OnDamageFunc },
    Heal { target: PlrId, amount: i32 },
    AddMovePoint { target: PlrId, delta: i32 },
    ClearPositive { target: PlrId },
    // 以及按现有 on_damage 行为继续扩展的状态类 effect
}
```

核心思想是：

- owner-centric 阶段直接给 `&mut owner`
- 只要是跨实体动作，就先变成 `Effect`
- 阶段结束后统一 flush `effects`

这样一来，像下面这些当前很别扭的代码就都可以消失：

- [ChargeSkill::act_with_level](../crates/tswn_core/src/player/skill/act/charge.rs#L51)
- [AccumulateSkill::act_with_level](../crates/tswn_core/src/player/skill/act/accumulate.rs#L62)
- [HideSkill::pre_action / update_state](../crates/tswn_core/src/player/skill/skl/hide.rs#L92-L135)
- [ReflectSkill::pre_defend_with_level](../crates/tswn_core/src/player/skill/skl/reflect.rs#L23-L80)

因为这些实现里，大量 `just_get_player_mut(args.0)` 的真实目的都只是“我想改 owner 自己”，那就应该把 owner 直接传进去，而不是让它绕一圈 `Storage`。

#### 6.3.3 迁移顺序建议

这套方案可以增量落地，不必一次重写全部 trait：

1. **先迁 self-only 主动技能**：`charge`、`accumulate`、`clone` 这类 owner 自身技能
2. **再迁 owner-centric 被动技能**：`hide`、`upgrade`、`shield` 等
3. **再迁 pre_defend / post_action**：`reflect`、部分 `protect` 分支
4. **最后把 D 的 staged damage 接进来**：`on_damage` 也统一走局部 `Effect`

#### 6.3.4 成本与收益

**收益**：

- 直接打到当前真正的根因：trait 上下文能力过宽
- 不需要上升到全局 EventQueue
- 保持当前 `Player + SkillStorage + StateStore` 架构

**成本**：

- trait 签名会改得比较多
- 需要设计一层 `Effect` 表达跨实体副作用
- 某些复杂状态写入（如 `covid_infect`）需要专门封装 effect 载荷

经验上，我会把它估成 **400 到 800 行量级的中大改动**。它明显比 D 大，但远小于 G / E。

### 6.4 新方案 I：split components / split field

原始文档里已经有“字段完全分离”的想法，这里把它进一步具体化。

#### 6.4.1 更贴近当前代码的拆法

不是必须一步走到完整 ECS，完全可以先从 `Player` 内部最容易分离的部分开始：

```rust,ignore
pub struct Storage {
    player_core: UnsafeCell<FastHashMap<PlrId, PlayerCore>>,
    player_status: UnsafeCell<FastHashMap<PlrId, PlayerStatus>>,
    player_states: UnsafeCell<FastHashMap<PlrId, PlayerStateStore>>,
    player_skills: UnsafeCell<FastHashMap<PlrId, SkillStorage>>,
    // 其余 world / queue 字段保持不变
}
```

这样做的直接好处是：

- `damage_core` 主要操作 `player_status`
- 很多 `on_damage` 主要操作 `player_states`
- 很多 AI / scoring 路径只读 `status + group`，根本不需要整块 `Player`

例如 [IceSkill::score_target](../crates/tswn_core/src/player/skill/act/ice.rs#L29) 的大部分逻辑只是在读：

- target 的 `status`
- group / alive group
- 是否有 `IceState`

这和“必须借整个 `Player` 才能工作”是两回事。

#### 6.4.2 为什么这个方案在当前代码风格里不是空想

已经有两个现成先例支撑它：

1. [update_state_inline](../crates/tswn_core/src/player/skill.rs#L215) 说明把技能作用直接投到 `PlayerStatus` 是可行的
2. [IceSkill::score_target](../crates/tswn_core/src/player/skill/act/ice.rs#L29) 说明一大类只读逻辑本来就更适合基于“拆开的数据视图”表达

#### 6.4.3 成本与收益

**收益**：

- 一旦拆到位，同玩家的“状态层 / 技能层 / 数值层”就能更自然地分借用
- AI、打分、状态刷新等逻辑会更 Rust-native
- 为将来做更强的并行或 replay/logging 打基础

**成本**：

- `Player` 方法会大量拆成自由函数或多段访问器
- `set_state` / `update_states` / `clear_positive_runtime` 等横跨多个子组件的操作会变复杂
- 改动面接近架构重写

我更倾向把它视为 **长期演进方向**，或者做成 `I-lite`：先只拆 `PlayerStatus`，再看是否继续拆 `states/skills`。

### 6.5 方案 G：全局 EventQueue

历史原型已经证明这个方向理论可行，但完整落地成本极高。

优点：

- 概念上最干净
- 事件边界天然切断借用作用域
- 易于做 replay / deterministic log

缺点：

- 需要把当前大量“直接摸 player”的技能逻辑重新编码成命令/事件
- 现有 `SkillTrait` / `StateTrait` 几乎都得重写一遍
- 当前主线已经从局部上吸收了它的低成本好处：`run_post_kill`、`pending_spawns`、`death_queue`、`pending_revivals`

所以从现实角度，它更像长期大重构方案，不适合用来解决“现在剩余 alias 怎么收尾”。

### 6.6 方案 E：Arena + token

这是最纯粹的 Rust 解法之一，但对当前代码基座太重。

问题不在于它不能做，而在于：

- 当前 API 已经深度绑定 `Player` 方法风格
- `PlrId + &mut Storage` 风格会把整个实现面都翻一遍
- 实际上它和 G 一样，都属于“如果要大改架构，可以一起考虑；否则没必要只为现在这批 alias 做这么大手术”

---

## 7. 辅助与验证方案

### 7.1 Debug borrow tracker / `RefCell` 只适合作为辅助，不是终局

原文档里的 `RefCell` 调试辅助仍然有价值，但更准确的定位应当是：

- **用于暴露 owner-phase 与 on_damage 重借**
- **不用于作为最终运行时方案**

如果要做，我更建议是 debug-only borrow tracker，而不是 release 中全面上 `RefCell<Player>`。

一个更轻量的思路如下：

```rust,ignore
#[cfg(debug_assertions)]
thread_local! {
    static ACTIVE_PLAYER_BORROWS: RefCell<FxHashSet<PlrId>> = Default::default();
}

pub fn just_get_player_mut_checked(&self, ptr: PlrId) -> BorrowGuard<'_> {
    // 进入时登记 ptr，离开时 guard 自动清理
    // 若同线程同阶段重复借同一 ptr，则 panic
}
```

它解决不了 release 正确性，但非常适合作为 D / J 迁移过程中的“地雷探测器”。

### 7.2 现有测试覆盖到的内容

当前和本文主题直接相关的测试锚点包括：

- [on_damaged_triggers_on_die](../crates/tswn_core/src/player/test.rs#L299)
- [merge_and_zombie_kill_write_target_states](../crates/tswn_core/src/player/test.rs#L522)
- [merge_kill_applies_owner_growth](../crates/tswn_core/src/player/test.rs#L910)
- [zombie_kill_marks_corpse_and_queues_minion_spawn](../crates/tswn_core/src/player/test.rs#L1215)
- [owner_death_marks_linked_minion_for_cleanup](../crates/tswn_core/src/player/test.rs#L1268)
- [owner_death_removes_linked_minions_in_roster_order](../crates/tswn_core/src/player/test.rs#L1302)
- [ice_score_halves_already_frozen_targets](../crates/tswn_core/src/player/test.rs#L1356)

这些测试已经很好地覆盖了：

- `post_kill` 行为正确性
- 使魔死亡顺序
- 一部分技能/状态写回行为

### 7.3 现有测试还没覆盖到的内容

当前仍然缺两类非常重要的回归用例：

1. **owner-phase alias probe**
   - 例如 `ChargeSkill::act_with_level` / `ReflectSkill::pre_defend_with_level`
   - 目标是专门验证“如果去掉 `mutable-noalias=no`，这些路径是否会出现 release-only 行为分叉”
2. **`on_damage` release probe**
   - 例如 `Absorb`、`Fire`、`Ice`、`Poison`、`Covid`、`Lazy`
   - 目标是把“当前剩余问题”从文档论证变成可自动化探针

从验证策略看，后续最有价值的不是再补一堆普通功能测试，而是专门为 D / J 准备几组 release/noalias 探针。

---

## 8. 性能参考

这里保留两个结论，不再重复展开所有表格：

1. [perf/ub_fix_no_debug.md](perf/ub_fix_no_debug.md) 已经说明 `330cd46` 引入的 `run_post_kill` 没有可测量性能退化。
2. 当前主线下 `mutable-noalias=no` 的额外性能影响，在已有基准里接近噪声。

因此，当前路线选择不应该被“会不会慢很多”主导，而应该主要由：

- 是否想摆脱 nightly 依赖
- 是否准备接受多大规模的 trait / 架构改动

来决定。

---

## 9. 推荐路线（更新版）

### 9.1 短期：继续维持方案 C

原因很简单：

- 行为已经稳定
- `post_kill` / `update_state_inline` / `in_post_damage` 等局部修复已经落地
- 当前没有证据表明继续维持 C 会带来性能或功能风险

### 9.2 中期第一步：先做方案 D，但把它定位成“第一阶段收敛”

优先级最高，因为：

- `damage()` 的结构天然适合拆
- 有 [exchange.rs#L149](../crates/tswn_core/src/player/skill/act/exchange.rs#L149) 和 [half.rs#L123](../crates/tswn_core/src/player/skill/act/half.rs#L123) 这种现成先例
- 能立刻减少一整批危险 `on_damage` 回调

但要接受一个修正后的事实：

> **D 不是终局，它是“把剩余风险从多类压缩成 owner-phase 一类”的第一步。**

### 9.3 中期第二步：用方案 J 清掉 owner-phase 重借

这是我现在最推荐的后续方向，因为它：

- 正中当前根因：trait 上下文能力过宽
- 有 [update_state_inline](../crates/tswn_core/src/player/skill.rs#L215) 作为现成成功范式
- 不需要立刻做全局 EventQueue 或 Arena 重写

如果 D + J 都落地，再考虑去掉 `mutable-noalias=no`、回到 stable，路径就会比“直接冲 G/E”清晰很多。

### 9.4 长期：I / G / E 只在要重写架构时再考虑

这三者都属于“从根上重塑系统”的方案：

- **I**：最贴近当前代码风格的长期演进版
- **G**：最强的命令/事件架构纯度
- **E**：最符合传统 Rust 借用哲学

但它们都不适合作为当前这轮“收尾 alias 风险”的第一选择。

### 9.5 不推荐：回退到方案 A

原因比原文档里还更明确：

- 当前并不是只有 `on_damage` 残留，而是整套 owner-phase / target-phase 都还依赖 `mutable-noalias=no`
- 回退到裸强转并不会减少这些问题，只会让 provenance 语义更差
- 在已经有 `run_post_kill`、`update_state_inline`、`in_post_damage` 这些成功局部修复的前提下，回退没有任何工程收益

---

## 10. 最终判断

如果只从“今天要不要改主线”回答，答案仍然是：

- **不用急着改，继续维持方案 C。**

如果从“要不要把文档里的技术判断修正准确”回答，答案则是：

- **要。当前主线并不是“只剩 on_damage”，而是“已经修掉两大块风险，并在 update_state 主路径上完成了一次成功内联化，但 action/pre_action/post_action/pre_defend/on_damage 里仍有多类同实体重借”。**

如果从“未来要怎么走回 stable”回答，答案是：

1. **先做 D**，因为 `damage()` 最好拆，且已有现成先例。
2. **再做 J**，因为当前真正需要收口的是 trait 上下文能力，而不是先把整个引擎改成 ECS/EventQueue。
3. **只有在准备接受架构重写时，才考虑 I / G / E。**
