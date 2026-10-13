# 分身机制详解（CloneSkill）

[返回机制索引](README.md)

分身（普通技能 ID 23）创建的是一个重新派生属性与技能的新 Runtime 实体，不是本体的镜像。当前入口为 [drain_plain_clone_skill_into](../../crates/tswn_core/src/runtime/combat/skills_control.rs#L226-L440)，构造数据保存在 `CloneBuildData` 中。

## 1. 本体与克隆体的属性

### 本体

只有本体未处于蓄力状态（`at_boost_millionths < 3_000_000`）时，才执行这一步属性衰减：

| 项目 | 当前行为 |
| --- | --- |
| 当前 HP | `ceil(hp / 2)`，截断到 `1..=旧 max_hp` |
| 原始 HP 构造值 | `ceil(raw_hp / 2)`，再派生新的最大 HP |
| 原始七围 | 各项 `ceil(raw_attr * CLONE_ATTR_DECAY)` |
| 派生战斗属性 | 从衰减后的构造值重新计算名字系数缩放、速度 +160 和保存的属性修正 |
| MP | 这个分身处理步骤不重新初始化本体 MP |
| 技能 | 分身技能自身衰减；其他技能保留本体的当前等级 |

`CLONE_ATTR_DECAY` 保留旧实现 f32 常量转为 f64 后的值，约为 `0.7799999713897705`。因此“七围 ×0.78”只是便于理解的近似，不能直接对展示出的攻击、速度等数值乘 0.78。精确操作见 [CloneBuildData::decay_owner / derive_stats](../../crates/tswn_core/src/runtime/entity.rs#L149-L201)。蓄力状态跳过本体 HP 与原始八围衰减，但不跳过后面的分身技能等级处理。

### 克隆体

| 项目 | 当前行为 |
| --- | --- |
| 当前 HP | 继承上述处理后的本体当前 HP，最低为 1 |
| 最大 HP / 七围 | 由本体的 `CloneBuildData.child()` 重新派生，不直接复制当前战斗属性 |
| 武器属性 | `child()` 在构造八围上再次加入保存的武器加成 |
| 名字系数 | 切换到已准备好的 child 名字系数；特殊编号玩家的本体与 child 系数可能不同 |
| MP | `max(child_wisdom >> 1, 0)` |
| 行动点 | 随机初始化为 `r255() * 4 + 256` |
| 召唤物蓝图 | 继承已准备好的 shadow / summon / zombie 模板 |

属性派生见 [CloneBuildData::child](../../crates/tswn_core/src/runtime/entity.rs#L156-L164)，实体初始化见 [skills_control.rs](../../crates/tswn_core/src/runtime/combat/skills_control.rs#L378-L439)。当前模型没有旧 `Player::init_values()` 调用，MP 规则直接写在 Runtime 构造链中。

## 2. 分身技能自身的衰减

本体分身技能与克隆体分身技能的等级不同：

```text
f = (next_u8() & 63) + 64                         // 64..127
owner_level = ceil(old_level * f / 128)
如果处理后的 owner.hp + owner.magic < r255():
    owner_level = floor(owner_level / 2) + 1
owner_level = max(owner_level, 1)
child_clone_level = max(ceil(sqrt(owner_level)), 1)
```

条件读取的是**魔法属性 `magic`**，不是 MP `magic_point`。克隆体分身技能的开根号等级覆盖下一节的一般重建结果；其他技能不使用这条开根号规则。完整的五种技能衰减见 [技能衰减机制](skill-decay.md)。

## 3. 其他技能：基底截断后重放加成

当前普通 / DIY 路径使用 [SkillLoadout::rebuilt_for_clone](../../crates/tswn_core/src/runtime/entity/skill_loadout.rs#L501-L518)：复制技能布局和构造元数据，以本体当前等级截断保存的加成前构造等级，再重放 `SkillBoost`。

```text
base = min(build_level, owner_current_level)
无加成 / Normal : child_level = base
LastBoost       : child_level = base * 2
SlotBoost       : child_level = base + min(saved_boost, base)
```

这里没有重新解析克隆体名字、重新随机生成技能，也没有把战斗衰减写回 `name_base`。无加成技能通常保留已经发生的等级下降；带加成技能可能在基底截断后恢复一部分等级，因此也不能概括为“所有技能保留衰减”或“全部恢复初始值”。

### 3.1 末尾主动技 LastBoost

假设技能构造基底为 46，LastBoost 后初始等级为 92：

```text
本体当前等级 50 -> min(46, 50) * 2 = 92
本体当前等级 40 -> min(46, 40) * 2 = 80
```

第二种情况不会完全恢复到 92。所谓“衰减下限”描述的是重建时可恢复的范围，不代表本体使用技能时等级不能继续下降。

### 3.2 末端座位 SlotBoost

假设技能基底为 34，保存的加成量为 32，初始等级为 66：

```text
本体当前等级 40 -> base=34 -> 34 + min(32, 34) = 66
本体当前等级 25 -> base=25 -> 25 + min(32, 25) = 50
```

后一项是 **50，而不是 25+32=57**。SlotBoost 的实际加成不能超过截断后的基底。

另一个基底 41、加成 41 的例子：本体衰减到 21 时，child 为 `21 + min(41,21) = 42`，不是 62。DIY 的 `"base+boost"` 编码与普通技能构造都要保留这个上限。

### 3.3 score profile 的特殊路径

数字 score profile 如果带有 `ScoreCloneSkillBoostPlan`，会使用 [rebuilt_for_score_clone](../../crates/tswn_core/src/runtime/entity/skill_loadout.rs#L520-L555)：先截断等级、恢复初始 boosted 标记，再按当前可用主动技能顺序选择末尾加成并重放座位加成。它与普通路径的“按已保存 boost 类型重放”不同，目的是保留评分 profile 的历史行为。

## 4. 对 DIY 拟合与往返验证的影响

最终等级相同不代表分身行为相同。等级 40 可能来自：

- 无加成的基底 40；
- LastBoost 的基底 20；
- SlotBoost 的基底 20 + 加成 20；
- SlotBoost 的基底 25 + 加成 15。

当本体继续衰减时，这些构成会产生不同的重建结果。只导出最终熟练度会丢失这一差异，当前导出器用 `"2*base"` / `"base+boost"` 保存加成类型，见 [DIY / OL 参考](../reference/diy-overlay.md#分身与-diy-技能加成)。

验证时还需比较输入队名、name_factor、技能行动顺序、武器限制和召唤物模板。具体工作流见 [DIY 往返验证](../guides/diy-validation.md)。旧名字样本中的等级恢复现象可用于解释机制，但本页数值例子是按当前重建公式计算的，不是本轮重新执行的样本统计。

## 5. 验证入口

[entity/tests.rs](../../crates/tswn_core/src/runtime/entity/tests.rs#L234) 覆盖构造基底、当前等级与 boost 重建；[plain_clone_skill_tests.rs](../../crates/tswn_core/src/runtime/tests/plain_clone_skill_tests.rs) 覆盖蓝图继承、score profile 的苏生术等级与自定义幻术克隆。

```powershell
cargo test -p tswn_core --lib skill_loadout_rebuilds_clone_levels_from_build_baseline_before_boosts
cargo test -p tswn_core --lib plain_clone_skill_tests
cargo test -p tswn_core --lib raw_import_minion_blueprints_keep_clone_build_metadata
```
