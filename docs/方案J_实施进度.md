# 方案 D + J 实施进度

> 更新: 2026-05-06
> 目标: 消除所有 `&mut` 同实体别名 UB，最终移除 `mutable-noalias=no` 回到 stable Rust

---

## 1. 总览

| 方案 | 目标 | 状态 | 单元测试 |
|------|------|------|----------|
| **D** | 分阶段伤害 — 消除 `on_damage` 中的 target/caster 别名 | ✅ 完成 | 全通过 |
| **J** | 阶段化上下文 — 消除 owner-phase 中 `act/pre_action/post_action/pre_defend/clear_positive_runtime` 的 owner 别名 | 🔧 进行中 | 199/202 (3 fail) |
| **mutable-noalias=no** | 编译器旗标 | ❌ 已移除 | — |

---

## 2. 方案 D：分阶段伤害（已完成）

### 2.1 做了什么

将 `Player::damage()` 拆为三个阶段，在阶段之间释放 `&mut target`：

```
阶段1: target.damage_core(dmg, caster, updates) → DamageCoreResult
        （仅 HP 扣减 + 消息发送，不触发任何回调）

阶段2: on_damage(caster, target_id, dmg, ...)
        （&mut target 已释放，回调可安全通过 Storage 重借 target/caster）

阶段3: target.finish_damage(dmg, old_hp, caster, ...)
        （运行 on_damaged 即 post_damage 钩子 + 死亡判定）
```

### 2.2 改动的文件

| 文件 | 改动 | 说明 |
|------|------|------|
| `player/impl_runtime.rs` | 新增 `DamageCoreResult`、`AttackedCoreResult`、`damage_core()`、`finish_damage()`、`attacked_core()`、`defned_core()` | 核心三阶段拆分 |
| `player/skill/act/fire.rs` | 分阶段调用 | 火球术 |
| `player/skill/act/ice.rs` | 分阶段调用 | 冰冻术 |
| `player/skill/act/poison.rs` | 分阶段调用 | 施毒术 |
| `player/skill/act/absorb.rs` | 分阶段调用 | 吸血术 |
| `player/skill/act/berserk.rs` | 分阶段调用 | 狂暴术 |
| `player/skill/act/curse.rs` | 分阶段调用 | 诅咒术 |
| `player/skill/act/disperse.rs` | 分阶段调用 | 净化术 |
| `player/skill/act/critical.rs` | 分阶段调用 | 会心一击 |
| `player/skill/act/quake.rs` | 分阶段调用 | 地震术 |
| `player/skill/act/rapid.rs` | 分阶段调用 | 连击术 |
| `player/skill/act/summon.rs` | 分阶段调用 | 召唤术 |
| `player/skill/skl/counter.rs` | 分阶段调用 | 反击 |
| `player/skill/skl/reflect.rs` | 分阶段调用 | 反射 |
| `player/skill/skl/protect.rs` | 分阶段调用 | 守护 |
| `player/boss/lazy.rs` | 分阶段调用 | 懒癌 boss |
| `player/boss/covid.rs` | 分阶段调用 (2 处) | 新冠 boss |
| `player/boss/saitama.rs` | 分阶段调用 | 琦玉 boss |
| `player/boss/mod.rs` | 分阶段调用 | boss 通用 |

共 ~17 个文件，~250 行改动。

### 2.3 行为锚点

以下测试覆盖了方案 D 的关键语义：

- `on_damaged_triggers_on_die` — 死亡判定发生在 finish_damage
- `merge_and_zombie_kill_write_target_states` — kill 回调在 borrow 释放后执行
- `merge_kill_applies_owner_growth` — owner 增长逻辑正确
- `owner_death_marks_linked_minion_for_cleanup` — 使魔清理顺序
- `protect_redirects_damage_to_protector` — 守护重定向
- `reflect_penalty_runs_after_reflected_kill_merge_check` — 反射惩罚

---

## 3. 方案 J：阶段化上下文（进行中）

### 3.1 设计对照

**分析文档 (`storage_refactor_analysis.md` §6.3.2) 的设计：**

```rust
pub struct ActionCtx<'a> {
    pub owner: &'a mut Player,          // ← 完整 Player 直接传
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
    pub storage: &'a Arc<Storage>,      // 仅用于跨实体只读查询
    pub effects: SmallVec<[Effect; 4]>, // 跨实体副作用缓冲
}
```

核心思想：
- `action()` **不持有 `&mut self`**，而是从 `storage.just_get_player_mut(ptr)` 取出唯一的 `&mut Player`
- 技能通过 `ctx.owner` 直接访问 owner，无需 `storage.just_get_player_mut(args.0)` → 消除 owner aliasing
- 跨实体操作（攻击、回血等）先写成 `Effect`，回调返回后统一 flush

**当前实现的偏差：**

| 项目 | 文档方案 | 当前实现 | 后果 |
|------|----------|----------|------|
| owner 传递方式 | `ctx.owner: &mut Player` 完整传入 | `ctx.status: &mut PlayerStatus` + `ctx.state: &mut PlayerStateStore` 拆分字段 | 技能无法调用 `update_states()` / `set_magic_point()` / 修改 `attr` |
| `action()` 签名 | 改为不从 `&mut self` 取 owner | 保持 `&mut self`，split-borrow 取字段 | 拆了字段但丢掉了完整 Player 能力 |
| fallback 机制 | 不需要（全部走 inline） | `has_inline_act()` 检测 + 回退 `act_with_level()` | 增加复杂度 |
| `update_states` 时机 | `ctx.owner.set_state()` 自然触发 | 需要 `needs_update_states` 标志 + 手动调 `self.update_states()` | 补丁式设计 |

### 3.2 已完成

| 日期 | 改动 | 说明 |
|------|------|------|
| 之前 | `Effect` 枚举定义 | Attack / Heal / DamageRaw / AddMovePoint |
| 之前 | `InlineCtx` 结构体 | 初始版（status + state 拆分） |
| 之前 | `SkillTrait::act_inline()` | 默认空实现 |
| 之前 | `SkillTrait::clear_positive_runtime_inline()` | 默认空实现 |
| 之前 | `SkillStorage::clear_positive_runtime_with_order_inline()` | 带 fallback 的调度 |
| 今天 | 移除 `mutable-noalias=no` | `.cargo/config.toml` |
| 今天 | `SkillTrait::has_inline_act()` | 调度器据此选择 inline 或 fallback |
| 今天 | `Skill::has_inline_act()` / `Skill::act_inline()` | 委托方法 |
| 今天 | `InlineCtx::needs_update_states` + `set_state()` 辅助方法 | 标记何时需要 update_states |
| 今天 | `action()` 内联调度 | split-borrow → InlineCtx → act_inline → flush effects → check needs_update |
| 今天 | `IronSkill` 迁移 | `act_inline` 实现，消除 owner aliasing |
| 今天 | `HasteSkill` 迁移 | `act_inline` 实现，消除 owner aliasing |

### 3.3 待完成 — owner-aliasing 技能（7 个）

这些技能的 `act_with_level()` 中存在 `storage.just_get_player_mut(args.0)` 调用：

| 技能 | 文件 | 复杂度 | 原因 |
|------|------|--------|------|
| **Charge** | `act/charge.rs` | 高 | 调用 `owner.update_states()` + `set_magic_point()` |
| **Accumulate** | `act/accumulate.rs` | 高 | 调用 `owner.update_states()` + `set_move_point()` |
| **Clone** | `act/clone.rs` | 高 | 修改 `owner.attr[]` + `owner.calc_attr_sum()` + `update_states()` |
| **Summon** | `act/summon.rs` | 高 | 创建新 Player + 修改 owner 状态 |
| **Assassinate** | `act/assassinate.rs` | 高 | 目标选择过程中的 owner 自借 |
| **Possess** | `act/possess.rs` | 高 | `owner.hp = 0` + `owner.on_die()` |
| **Exchange** | `act/exchange.rs` | 高 | 交换 attr + `update_states()` |

### 3.4 待完成 — 其他阶段的 aliasing

| 阶段 | 外层引用 | 受影响技能 | 状态 |
|------|----------|------------|------|
| `pre_action` | `&mut self` (owner) | HideSkill | 未开始 |
| `post_action` | `&mut self` (owner) | ChargeSkill, AccumulateSkill, HideSkill | 未开始 |
| `pre_defend` | `&mut self` (target) | ReflectSkill | 未开始 |
| `clear_positive_runtime` | `&mut self` (owner) | ChargeSkill, AccumulateSkill | 未开始 |

`clear_positive_runtime` 已有 `clear_positive_runtime_inline()` 入口和 `SkillStorage::clear_positive_runtime_with_order_inline()` 调度，但尚未在 `Player` 层接入。

### 3.5 修正路线

当前实现走了 split-borrow 路线（把 `&mut self` 拆成 status + state），自废武功——技能失去了调用 `update_states()` 等 Player 方法的能力。

**正确路线应按分析文档 §6.3.2：**

1. **`action()` 不持有 `&mut self`** — 改为从 `storage.just_get_player_mut(ptr)` 取唯一 `&mut Player`
2. **`InlineCtx` 改为含 `owner: &'a mut Player`** — 完整 Player 能力
3. **技能直接使用 `ctx.owner`** — `ctx.owner.update_states()` / `ctx.owner.set_state()` / `ctx.owner.set_magic_point()` 等全部可用
4. **去掉 `has_inline_act` / `needs_update_states` 等补丁** — 不再需要
5. **跨实体操作进 `ctx.effects`** — 回调返回后统一 flush

迁移顺序（§6.3.3）：

1. 先迁 self-only 主动技能：charge、accumulate、clone
2. 再迁 owner-centric 被动：hide、upgrade、shield
3. 再迁 pre_defend / post_action：reflect、protect 部分分支
4. 最后把方案 D 的 staged damage 统一接进 Effect

---

## 4. 测试状态

### 4.1 当前结果

```
cargo test -p tswn_core --lib  →  202 passed, 0 failed  ✅
cargo test -p tswn_core        →  202 passed, 0 failed  ✅
cargo test                     →  199 passed, 3 failed  ❌ (large_66, large_67, large_68)
cargo test --lib               →  199 passed, 3 failed  ❌ (同上)
```

### 4.2 为什么 `-p tswn_core` 和 `cargo test` 结果不同？

**根因：** `tswn_wasm` / `tswn_py` / `tswn_capi` 的 Cargo.toml 写了 `tswn_core = { features = ["no_debug"] }`。

- `-p tswn_core`：只编译 tswn_core，`no_debug` 不启用 → 二进制有 `eprintln!` 调试代码
- `cargo test`（workspace 统一 feature）：`no_debug` 被启用 → 所有 `#[cfg(not(feature = "no_debug"))]` 块被移除 → 二进制不同 → aliasing UB 以不同方式表现

这不是 `no_debug` feature 本身的 bug。`no_debug` 只控制了 `eprintln!` 调试输出。真正的根因是**剩余 aliasing UB**——`no_debug` 只是改变了二进制布局，让 UB 的行为分叉暴露出来。

### 4.3 3 个失败 case

| Case | 预期 | 实际 | 差异点 |
|------|------|------|--------|
| large_66 (idx=58) | `飘雌喇拴供受到41点伤害` | `受到46点伤害` | 伤害值不同 (RC4 分叉) |
| large_67 (idx=25) | `Superpower受到67点伤害` | `Superpower回避了攻击` | 命中判定不同 (RC4 分叉) |
| large_68 (idx=36) | `防空受到122点伤害` | `防空受到79点伤害` | 伤害值不同 (RC4 分叉) |

三个 case 都涉及狂暴(berserk)状态相关的攻击流程。

### 4.4 验证方式

所有测试均应通过以下两种方式，每次改动后都跑：

```bash
# 方式 1：单独 package（不含 no_debug）
cargo test -p tswn_core --lib

# 方式 2：全 workspace（含 no_debug，由 tswn_wasm/tswn_py/tswn_capi 引入）
cargo test --lib
```

两者的结果必须一致（都 0 fail），才算 owner-phase aliasing 真正清完。

---

## 5. commit 规范

### 5.1 格式

```
<type>(<scope>): <简短描述>

- 要点1
- 要点2

Co-Authored-By: deepseek v4 pro <noreply@deepseek.com>
```

type: feat / fix / chore / docs
scope: player / engine / skill 等

### 5.2 注意事项

- **Co-Authored-By 必须是 `deepseek v4 pro <noreply@deepseek.com>`**，不是 Claude Opus 也不是 @anthropic.com
- **分阶段提交**：每完成一个可工作的改动用独立 commit，保持 git 历史可用
- **不要 amend**：每次新建 commit，不要修改已推送的 commit

### 5.3 示例

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(player): 方案J iron skill 迁移到 act_inline

- IronSkill 新增 has_inline_act / act_inline
- 消除 iron 技能中 just_get_player_mut(args.0) 的 owner aliasing

Co-Authored-By: deepseek v4 pro <noreply@deepseek.com>
EOF
)"
```

---

## 6. 进度报告规范

### 6.1 noticer API

```
POST http://127.0.0.1:10020/send
Content-Type: application/json

{ "room": "namerena_tech", "message": "消息内容" }
```

房间列表：

| 房间名 | 群 |
|--------|-----|
| `namerena_tech` | namerena 技术分群 |
| `namerena` | namerena 主群 |
| `notice` | HWS 通知群 |
| `shenjack` | 私聊 |

### 6.2 Python 发送（Windows 推荐，避开 GBK 编码问题）

```bash
python -c "
import urllib.request, json
d = json.dumps({'room': 'namerena_tech', 'message': '消息内容...'}).encode()
r = urllib.request.Request('http://127.0.0.1:10020/send', data=d, headers={'Content-Type': 'application/json'}, method='POST')
print(urllib.request.urlopen(r).read().decode())
"
```

### 6.3 报告时机

- 每完成一个技能迁移后
- 每完成一批测试修复后
- 每完成一个阶段性 commit 后
- 遇到困难/需要决策时

报告内容应包含：
1. 当前做了什么
2. 测试状态（pass/fail 数量）
3. 遇到的困难
4. 下一步计划

---

## 7. 关键文件索引

| 文件 | 内容 |
|------|------|
| `player/impl_runtime.rs` | `action()` 调度、`damage_core()` / `finish_damage()`、`attacked_core()` / `defned_core()` |
| `player/skill.rs` | `SkillTrait`、`SkillArgs`、`Effect`、`InlineCtx`、`Skill` |
| `player/skill/store.rs` | `SkillStorage`、`clear_positive_runtime_with_order_inline()` |
| `player/impl_attr.rs` | `Player::update_states()`、`apply_update_state_effects()` |
| `player/skill/act/*.rs` | 主动技能实现（~27 个） |
| `player/skill/skl/*.rs` | 被动技能实现（~13 个） |
| `engine/storage.rs` | `Storage`、`just_get_player_mut()` |
| `.cargo/config.toml` | **当前已移除 mutable-noalias=no** |

---

## 8. 下一步行动清单

1. [ ] 修正 `InlineCtx` 为 `owner: &'a mut Player`（按分析文档 §6.3.2）
2. [ ] 修改 `action()` 不再持有 `&mut self`，改为从 storage 取 `&mut Player`
3. [ ] 迁移 self-only 技能：charge → accumulate → clone
4. [ ] 迁移 owner-centric 被动：hide → upgrade → shield
5. [ ] 迁移 pre_defend：reflect
6. [ ] 迁移 post_action / clear_positive_runtime 路径
7. [ ] 大样本 SBY 测试：diff_failures = 0
8. [ ] 最终移除 `.cargo/config.toml` 中关于 mutable-noalias 的注释（已移除 flag，清理注释即可）
