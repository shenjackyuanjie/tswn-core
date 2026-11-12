# fix/Rinick-feature 补丁分支说明

这是一个**补丁/参考分支**：不新增接口、不改 DTO 形状，只修正两处战斗语义。分支从 `main` 的 `5237f2b3` 切出，两个提交互相独立，可以按需单独取用。

**不建议直接合入 main**：两处修正都会改变战斗结果，冻结语料 `runtime_stress/golden.json` 因此有 15 条基线不一致（见[语料影响](#对冻结语料的影响)），需要维护者先决定是否接受新语义。

## 分支与提交

```
9c82dc5f fix(core): 诅咒的蓄力加成改按施法者判定
da678255 fix(core): 隐匿不再吞掉狂暴强制行动
5237f2b3 chore(wasm): 递增到 0.5.12 并封板更新日志   ← main
```

| 提交 | 改动文件 | 规模 |
| --- | --- | --- |
| `da678255` | [`runtime/plain_assassinate.rs`](../../crates/tswn_core/src/runtime/plain_assassinate.rs)、[`runtime/tests/plain_action_scheduler_tests.rs`](../../crates/tswn_core/src/runtime/tests/plain_action_scheduler_tests.rs) | +46 −5 |
| `9c82dc5f` | [`runtime/combat/attack.rs`](../../crates/tswn_core/src/runtime/combat/attack.rs)、[`runtime/tests/plain_probability_tests.rs`](../../crates/tswn_core/src/runtime/tests/plain_probability_tests.rs) | +47 −12 |

两个提交都只动 `tswn_core` 的机制层与单测，不涉及 `cli_api`、WASM、C ABI、Python 绑定，也不改变任何公开类型。

## 补丁一：隐匿不再吞掉狂暴强制行动

### 现象

带「隐匿」技能的单位被狂暴命中后：

- 状态栏会显示「狂暴 (n)」，看起来已经进入狂暴；
- 但该单位此后**一次狂暴攻击都不执行**，永远按正常技能逻辑行动；
- 更糟的是 `step` 不会减少（只有强制攻击后才会递减），所以**狂暴状态永久挂在身上**。

### 根因

隐匿的 pre-action 在解除隐匿后，顺手把「本手不要强制行动」的旗子置上：

```rust
// runtime/plain_assassinate.rs（修正前，第 36-42 行）
DEFAULT_CORE_HIDE_SKILL_EXPORT => {
    self.clear_plain_hide_before_action(actor);
    if self.has_plain_berserk_state(actor) {
        outcome.clear_forced_action = true;   // ← 问题所在
    }
    outcome.forced_skill = None;
}
```

而这个旗子唯一被消费的地方就是狂暴分支：

```rust
// runtime/combat/round.rs:359（修正前后都未变）
if !pre_action.clear_forced_action && self.has_plain_berserk_state(actor) {
    // 执行狂暴强制攻击
}
```

于是：

1. 行动开始时先跑 pre-action，隐匿槽位把 `clear_forced_action` 置真；
2. `prepare_plain_action` 的狂暴分支被跳过，这一手改成普通技能扫描；
3. `prepared_plain_action` 不是 `ForcedAttack`，`forced_plain_action` 为假（`round.rs:148`）；
4. `drain_plain_berserk_forced_action_state_into`（`round.rs:199`）不执行，`step` 不减。

判定依据还有一个容易误解的地方：它看的是**技能表里装备了隐匿这个槽位**，而不是「当前是否处于隐匿」；而且判断之前 `clear_plain_hide_before_action` 已经把隐匿清掉了。所以只要这个号学了隐匿，被狂暴命中后就再也不会有狂暴行为。

### 改动

删除那三行，pre-action 只负责解除隐匿：

```rust
DEFAULT_CORE_HIDE_SKILL_EXPORT => {
    // 隐匿的 pre-action 只负责解除隐匿；不再顺手掐掉狂暴强制行动。
    self.clear_plain_hide_before_action(actor);
    outcome.forced_skill = None;
}
```

潜行/背刺那条「pending 的锁目标已失效」分支保持原样（仍在 `plain_assassinate.rs:65` 置旗），所以下面这种组合仍然是「先正常 1 回合」：

> A 无隐匿，潜行至 B；B 死亡；A 被附体挂 4 回合狂暴

B 死亡不会立即清掉 A 的 pending，只有 A 行动时（发现目标已死）、背刺命中、或被识破这三个时机才会清。因此 A 的下一手命中死目标分支：清 pending + 移除 pre-action 槽位 + 置旗 → 正常行动 1 回合（`step` 保持 4）；再下一手槽位已移除，才进入 4 回合狂暴。

### 测试

- `plain_merge_restores_newly_enabled_hide_pre_action_lane`、`plain_merge_restores_hide_pre_action_for_minion_lane`：断言从 `clear_forced_action == true` 改为 `false`；
- 新增 `plain_hide_does_not_swallow_berserk_forced_attack`：构造「只有隐匿 + 狂暴 4」的单位，断言 pre-action 不置旗，且 `prepare_plain_action` 返回 `ForcedAttack`。

### 影响范围

所有「带隐匿技能 + 中狂暴」的对局。语料中 14 个 case 的积分发生变化。

## 补丁二：诅咒的蓄力加成改按施法者判定

### 现象

- 蓄力中施放诅咒 → 得到普通诅咒（42 prob / 2 multiply）；
- 诅咒命中一个正在蓄力的目标 → 反而得到「蓄力诅咒」（52 / 3 起）。

判定对象反了：加成应该来自**施法者**是否蓄力。

### 根因

```rust
// runtime/combat/attack.rs:643（修正前）
let (target_hp, target_flags, charge_active, existing) = {
    let target_entity = self.entities.get(target)...;
    (
        target_entity.runtime.hp,
        target_entity.runtime.flags,
        target_entity.runtime.at_boost_millionths >= 3_000_000,  // ← 读的是目标
        existing,
    )
};
```

同批受击效果里，狂暴（`attack.rs:587`）和冰冻（`defense.rs:603`）都按**施法者**的 `at_boost >= 3.0` 判断蓄力，只有诅咒读成了目标。

### 改动

蓄力标记改从 `caster` 读取，数值与叠加规则一概不动：

```rust
// 蓄力加成取施法者：和狂暴、冰冻等其它受击效果保持同一口径。
let charge_active = self
    .entities
    .get(caster)
    .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);
```

叠加后的数值：

| 情况 | 首次命中 | 再叠一次 |
| --- | --- | --- |
| 施法者未蓄力 | 42 / 2 | 52 / 3 |
| 施法者蓄力 | 52 / 3 | 72 / 5 |

目标是否蓄力不再影响诅咒强度。

### 测试

- `plain_curse_on_damage_stacks_caster_charge_bonus_without_reapplying_atk_sum`：把 `at_boost = 3.0` 从目标移到施法者，连叠两次仍断言 `Curse { prob: 72, multiply: 5 }`（证明数值语义未变）；
- 新增 `plain_curse_on_damage_ignores_target_charge_bonus`：目标蓄力、施法者不蓄力，叠两次断言 `52 / 3`。

### 影响范围

所有「蓄力中施放诅咒」与「诅咒命中蓄力目标」的对局。语料中新增 1 个 case（`fight_multi_3`）发生变化。

## 对冻结语料的影响

```powershell
cargo test -p tswn_test --features runtime-corpus
```

结果：**109 passed / 15 failed**，失败全部是 `score mismatch`：

```
large_29  large_34  large_58  large_59  large_63  large_72  large_full
fight_multi_1  fight_multi_3  fight_multi_4  fight_multi_5
stress_3v3v3_db3944e8cf92d8d9
stress_ffa_8_3d155c15a9d6ec4b  stress_ffa_8_cce83cebc69761de
stress_ffa_6_3ae9dd4b7b788849
```

其中前 14 个来自补丁一，`fight_multi_3` 来自补丁二。

基线没有更新：`crates/tswn_test/src/golden.rs` 明确只断言、不生成或覆写基线，仓库也没有 bless/重算工具。合入 main 前需要维护者决定是否接受新语义；接受的话应写一次性的重算工具，把 15 条记录连同 diff 一起提交审阅，而不是手工改 JSON。

这些失败是语义更新的预期结果，不是回归。

## 如何取用

- 只要隐匿修复：`git cherry-pick da678255`
- 只要诅咒修复：`git cherry-pick 9c82dc5f`
- 只需要对照行为：`git revert` 单个提交即可回到旧语义，两个补丁互不依赖
- 与 `instant` 分支的关系：`instant` 的脚本注入代码复制了同一个 `!pre_action.clear_forced_action` 条件，所以补丁一合入后，脚本模式也自动一致，不需要额外改动

## 验证记录

| 命令 | 结果 |
| --- | --- |
| `cargo test -p tswn_core --lib` | 617 passed |
| `cargo test -p tswn_test`（默认 feature） | 3 passed |
| `cargo check --workspace --all-targets` | 通过 |
| `cargo +nightly fmt --check` | 通过 |
| `cargo test -p tswn_test --features runtime-corpus` | 109 passed / 15 failed（见上节） |

## 已知边界

- 只修正了这两处判定；同类「受击效果判定对象错位」的其它技能（吸血、投毒、会心、瘟疫等）尚未逐一审计。
- 冰冻、新冠/摆烂拦截、潜行目标死亡同样会让狂暴「顺延」（不消耗 `step`），这是既有设计，本次未改。
- 诅咒只改蓄力判定的对象，不涉及数值、叠加规则或目标选择。
- 补丁分支不更新 CHANGELOG 与版本号；需要发布时由维护者统一处理。
