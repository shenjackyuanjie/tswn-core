# 技能衰减机制

对局中，以下 5 个主动技能在使用后会发生**熟练度衰减**（即技能等级下降）：

| #   | 技能                     | ID  | 衰减方式                            |
| --- | ------------------------ | --- | ----------------------------------- |
| 1   | 生命之轮 (ExchangeSkill) | 9   | 半衰                                |
| 2   | 治愈魔法 (HealSkill)     | 15  | 高等级线性衰减                      |
| 3   | 苏生术 (ReviveSkill)     | 16  | 半衰                                |
| 4   | 分身 (CloneSkill)        | 23  | 随机衰减（本体） / 开根号（克隆体） |
| 5   | 幻术 (ShadowSkill)       | 24  | 乘 0.75 衰减                        |

[返回机制索引](README.md)

本文按当前 Runtime 的技能处理函数说明等级变化。可用合法 overlay（例如 `probe+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklheal":128,"sklfire":128}}`）观察行动顺序变化；发动频率还受选目标与发动条件影响，不能仅凭频率推导衰减公式。

---

## 1. 生命之轮（ExchangeSkill, ID = 9）

### 衰减公式

```text
new_level = (level + 1) / 2     （整数除法，向下取整）
```

### 衰减条件

`level >= 2` 时触发。

### 对照表

| 原等级 | 衰减后 |
| ------ | ------ |
| 1      | 1      |
| 2      | 1      |
| 3      | 2      |
| 4      | 2      |
| 5      | 3      |
| 6      | 3      |
| 7      | 4      |
| 8      | 4      |
| …      | …      |

**性质**：半衰减，每次使用后等级大致减半。

---

## 2. 治愈魔法（HealSkill, ID = 15）

### 衰减公式

```text
if level > 8 {
    new_level = level - 1
} else {
    new_level = level        // 不变
}
```

### 衰减条件

`level >= 9` 时触发。

### 对照表

| 原等级 | 衰减后 |
| ------ | ------ |
| 1 ~ 8  | 不变   |
| 9      | 8      |
| 10     | 9      |
| 11     | 10     |
| 12     | 11     |
| …      | …      |

**性质**：高等级线性衰减。低级（≤8）时完全不受影响，超过 8 后每次使用 -1。

---

## 3. 苏生术（ReviveSkill, ID = 16）

### 衰减公式

```text
new_level = (level + 1) / 2     （整数除法，向下取整）
```

### 衰减条件

`level >= 2` 时触发。

### 对照表

| 原等级 | 衰减后 |
| ------ | ------ |
| 1      | 1      |
| 2      | 1      |
| 3      | 2      |
| 4      | 2      |
| 5      | 3      |
| …      | …      |

**性质**：与生命之轮完全相同的半衰减公式。

---

## 4. 分身（CloneSkill, ID = 23）

分身的衰减分两部分：**本体衰减**和**克隆体继承**。

### 4.1 第一阶段：随机衰减

```text
f = rand(0, 63) + 64          // f ∈ [64, 127]
new_level = ceil(level * f / 128)
```

- `f` 是均匀分布在 [64, 127] 的随机数
- 衰减比例在 50% ~ 99.2% 之间波动
- **不一定每次都降**，但整体趋势是下降的

### 4.2 第二阶段：条件减半

当本体满足以下条件时，额外衰减：

```text
owner.hp + owner.magic < r255()
```

这里读取处理后的本体当前 HP 与**魔法属性 `magic`**，不是 MP `magic_point`。若条件成立：

```text
new_level = (new_level / 2) + 1
```

即在上一步结果基础上再减半。

### 4.3 克隆体继承

克隆体的**分身技能**不是直接继承，而是基于本体的衰减后等级开根号；其他技能按 [分身机制](clone.md#3-其他技能基底截断后重放加成) 的基底与 boost 重建：

```text
cloned_level = ceil(sqrt(owner_current_level))
```

保证至少为 1。

---

## 5. 幻术（ShadowSkill, ID = 24）

### 衰减公式

```text
new_level = max(ceil(level * 3 / 4), 1)    // 向上取整，最低为 1
```

### 衰减条件

每次使用都计算公式；`level >= 4` 时等级才实际下降，1、2、3 经向上取整后保持原值。

### 对照表

| 原等级 | 衰减后 |
| ------ | ------ |
| 1      | 1      |
| 2      | 2      |
| 3      | 3      |
| 4      | 3      |
| 5      | 4      |
| 6      | 5      |
| 7      | 6      |
| 8      | 6      |
| 9      | 7      |
| 10     | 8      |
| 11     | 9      |
| 12     | 9      |
| …      | …      |

**性质**：乘 0.75 衰减，每次使用丢失约 25% 的等级。

---

## 汇总

| 技能         | ID  | 衰减类型 | 衰减门槛  | 公式                                          |
| ------------ | --- | -------- | --------- | --------------------------------------------- |
| 生命之轮     | 9   | 半衰     | level ≥ 2 | `(level + 1) / 2`                             |
| 治愈魔法     | 15  | 线性     | level ≥ 9 | `level - 1`                                   |
| 苏生术       | 16  | 半衰     | level ≥ 2 | `(level + 1) / 2`                             |
| 分身（本体）   | 23  | 随机     | 每次使用  | `ceil(level * rand[64,127] / 128)` + 条件减半 |
| 分身（克隆体） | 23  | 开根号   | 创建时    | `ceil(sqrt(owner_level))`                     |
| 幻术         | 24  | 等比     | level ≥ 4 | `ceil(level * 3 / 4)`, 最低 1                 |

这些衰减机制解释了为什么在高技能等级下某些技能的实际发动次数会受限——每次使用后熟练度下降，可能导致后续回合不再满足发动条件。

## 当前源码依据

- 生命之轮与分身：[skills_control.rs](../../crates/tswn_core/src/runtime/combat/skills_control.rs)，分别为 `drain_plain_exchange_skill_into` 与 `drain_plain_clone_skill_into`。
- 治愈魔法：[skills_team.rs](../../crates/tswn_core/src/runtime/combat/skills_team.rs)，`drain_plain_heal_skill_into` 在当前等级大于 8 时减 1。
- 苏生术与幻术：[skills_lifecycle.rs](../../crates/tswn_core/src/runtime/combat/skills_lifecycle.rs)，幻术使用整数 `saturating_mul(3).div_ceil(4).max(1)`。
- 分身技能基底与 boost 重建：[entity/tests.rs](../../crates/tswn_core/src/runtime/entity/tests.rs#L234)、[plain_clone_skill_tests.rs](../../crates/tswn_core/src/runtime/tests/plain_clone_skill_tests.rs)。
