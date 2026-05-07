# 方案J 剩余工作清单

> 创建: 2026-05-07
> 状态: 核心架构已落地，此为渐进式收尾

---

## 1. 当前状态摘要

方案J 的核心调度框架已就位：

- `InlineCtx { owner: &mut Player, effects: SmallVec<[Effect; 4]>, ... }` — 替代 `PlrId + Arc<Storage>` 的 owner-centric 上下文
- `Effect` 枚举 — Attack / Heal / DamageRaw / AddMovePoint / OwnerDie
- 全部 **act 技能已迁移**到 `act_inline` + take/put 模式（8 个技能）
- `pre_defend` 已用 `run_pre_defend_skill_range` 替代旧调度
- `pre_action` / `post_action` / `post_damage` 内联调度器已写入 SkillStorage，impl_runtime 主循环已接入

本次未提交修改（6 文件，461 行增/168 行删）在补完这些调度接入和 Hide/Charge/Possess 的内联实现。

---

## 2. 待完成项

### 2.1 `post_damage` 内联化（优先级：高）

当前仅 `HideSkill` 实现了 `post_damage_inline`。以下技能仍然走旧路径 `post_damage(args: SkillArgs)`，需要通过 `args.3` (Storage) 拿回 owner/target：

| 技能 | 文件 | 说明 |
|------|------|------|
| `UpgradeSkill` | `skl/upgrade.rs:23` | owner-centric，`post_damage_with_level` 修改自身属性 |
| `CounterSkill` | `skl/counter.rs:32` | owner-centric，`post_damage_with_level` 修改自身状态 |
| `AssassinateSkill` | `act/assassinate.rs:344` | `post_damage` — 暂为空回调，可跳过 |
| `SummonShareDamageSkill` | `act/summon.rs:266` | 使魔分摊伤害，访问 owner 和 minion |

**改造方式**：按 `HideSkill` 的模式，为 `UpgradeSkill` 和 `CounterSkill` 添加 `has_inline_post_damage() -> true` 和 `post_damage_inline(level, ctx)`。

`SummonShareDamageSkill` 情况特殊——它需要从 `ctx.storage` 拿 minion 再拿 owner，但不再需要 `just_get_player_mut(args.0)` 拿自己。可以给 `InlineCtx` 加一个访问非 owner 实体的 helper，或者继续用 `ctx.storage.just_get_player_mut(owner_id)`（跨实体访问是安全的）。

### 2.2 `clear_positive_runtime` 调度接入（优先级：中）

`SkillStorage::clear_positive_runtime_with_order_inline` 已写好，会优先调用 `clear_positive_runtime_inline`，fallback 到旧路径。但 **impl_runtime.rs 中没有调用这个 inline 版本**。

当前 `clear_positive_runtime` 的唯一调用点在 `disperse.rs:112`：
```rust
let mut clear_messages = target.skills.clear_positive_runtime_with_order((target_id, r, updates, storage));
```

这是 cross-entity 调用（caster 清 target 的状态），不是 self-only 的。需要确认 `clear_positive_runtime_with_order_inline` 的 ctx 构造是否适合 cross-entity 场景（ctx.owner 是 target 还是 caster？）。如果设计上只支持 self-clear，那这条可以暂时跳过。

### 2.3 旧 `pre_defend_range_inline` 死代码清理（优先级：低）

`store.rs:587` 的 `pre_defend_range_inline` 方法仍存在，但 `impl_runtime.rs` 已改用 `Player::run_pre_defend_skill_range`。确认无其他调用点后可直接删除。

### 2.4 旧路径 `post_action_early` / `post_action_late` 死代码检查（优先级：低）

`store.rs` 中的 `post_action_early` 和 `post_action_late` 旧方法：
- `post_action_early` ↔ `post_action_early_inline`
- `post_action_late` ↔ `post_action_late_inline`

如果旧路径调用点（`impl_runtime.rs` 的 `run_post_action_chain`）已全部切换到 inline 版，可删除旧的 `SkillArgs` 版方法。

### 2.5 方案D 完整接入（优先级：中）

当前 `attacked_core` + `damage_core` + `finish_damage` 模式已存在，但仅在 `Effect` dispatch 中使用。旧的 `attacked` → `defned` → `damage`（包含同步 `on_damage` 回调）路径仍在被非内联 act 技能使用。

要彻底清掉 `on_damage` 回调路径的 target/caster 重借（文档 Table 4.2 列出的 10+ 个危险回调），需要：
- 把 `attacked` 的调用方逐个改为 `attacked_core` + 独立 `on_damage` + `finish_damage` 三段式
- 或者把剩余 act 技能（disperse/half/heal/quake/rapid/revive/summon/thunder）迁移到 `act_inline`，自然走 `Effect::Attack` 路径

第二种方案（继续推进 act_inline 迁移）更一致，因为迁移后技能自动获得 staged damage 保护。

### 2.6 测试覆盖（优先级：中）

文档 §7.3 提到的两类探针仍未实现：
1. **owner-phase alias probe** — 验证 ChargeSkill/ReflectSkill 路径在 `mutable-noalias=yes` 下的行为
2. **`on_damage` release probe** — 验证 Absorb/Fire/Ice/Poison 回调的 release-only 行为分叉

建议在方案J 内联化全部完成后、尝试去掉 `mutable-noalias=no` 之前，先把这两类探针加上。

---

## 3. 推荐执行顺序

1. **先提交当前修改** — 6 个文件的内联调度接入和 Hide/Charge/Possess 实现是功能完整的增量
2. **`post_damage` 收尾** — UpgradeSkill / CounterSkill 加 `post_damage_inline`
3. **act_inline 续迁** — 把剩余 9 个 act 技能逐步迁移到 `act_inline`（顺便获得 staged damage 保护）
4. **死代码清理** — 删 `pre_defend_range_inline`、旧 post_action 方法
5. **`clear_positive_runtime` 接入** — 确认 cross-entity 语义后决定是否接入
6. **加 alias probe 测试**
7. **尝试去掉 `mutable-noalias=no`** — 跑全量测试 + SBY 大样本对比

---

## 4. 不需要做的事

- **不需要重写 trait 签名**：`InlineCtx` 作为增量旁路已经和旧 `SkillArgs` 路径并存，兼容性良好
- **不需要做全局 EventQueue（方案G）或 Arena+token（方案E）**：当前架构增量式改进足够覆盖已知风险
- **不需要动 `on_damage` 回调签名**：方案D 的分段 damage 模式已经在 Effect dispatch 中落地，只要剩余 act 技能迁移到 inline 即可逐步收缩危险面
