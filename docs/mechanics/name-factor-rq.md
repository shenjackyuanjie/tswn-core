# name_factor 与 rq：对打和胜率评估的属性差异

[返回机制索引](README.md)

## 简要结论

当前普通对打默认使用 `rq=4.0`，胜率与评分入口默认使用 `rq=6.0`。这里的 `rq` 是名字评估参数，不是运行期伤害倍率：它在构造玩家时影响 `name_factor`，再影响由原始七围派生的战斗属性。

| 场景 | 默认 rq | 当前入口 |
| --- | --- | --- |
| 普通对打 | 4.0 | `BattleOptions::default()`、`Runner::new_from_namerena_raw`、CLI `fight` |
| 胜率 / 评分 | 6.0 | 胜率与评分绑定的默认参数、CLI `bench`、`namer-pf` |
| 已准备的对局 | 构造时选定 | `PreparedRunner` 复用已构造的 roster；应在准备阶段指定要比较的 `eval_rq` |

对负系数短名，rq=6 通常使系数更接近 0，因此属性加成小于 rq=4；这并不能单独推出某场战斗或某个胜率统计一定降低。队名、技能、对手和 seed 都参与最终结果。需要严格比较时，应显式对齐 `eval_rq`、输入分组与 seed。

## 1. 当前构造链

实现位于 [namerena/build.rs](../../crates/tswn_core/src/namerena/build.rs#L255-L263)，不是旧对象模型的 `impl_ctor.rs` / `Storage`：

```text
Test1 / Test2 / TestEx                           -> name_factor = 0
带 overlay 且 name_factor_enabled=false          -> name_factor = 0
其余玩家:
    factor_name = eval_str_common_with_rq(name, true, eval_rq)
    factor_team = eval_str_common_with_rq(clan_name, true, eval_rq)
    name_factor = max(factor_name, factor_team - 6)
```

因此仅计算名字字符串的分值，还不一定得到玩家的最终 `name_factor`：队名的 `factor_team - 6` 可能覆盖名字分值。DIY / OL 还可以关闭名字系数；特殊测试类型则始终为 0。

## 2. rq 改变了什么

[namerena/eval_name.rs](../../crates/tswn_core/src/namerena/eval_name.rs#L65-L224) 先按 UTF-16 码元统计字符类别、类型变化和长度，再计算类别权重乘积的自然对数。对数值经过分段压缩并减去 32，得到中间量 `x`。rq 只进入最后的归一化：

```text
x > 0       : name_factor = x / (32768 - 3500)
x < -rq     : name_factor = (x + rq) / (rq - 3)
其他         : name_factor = 0
```

比较默认的 rq=4 与 rq=6：

- `x >= -4`：两者结果相同；正分支是很小的正值，并非恒等于 0。
- `-6 <= x < -4`：rq=4 进入负分支，rq=6 为 0。
- `x < -6`：两者都进入负分支，但**分子和分母都不同**，结果仍有差异。

所以差异不仅发生在 -6 到 -4 之间，也不只限于某两种字符类型。超过 8 个字符会触发额外类别计数调整，不能简化为“超过 8 字符强制为 0”。

### 可手算的短名示例

下表计算的是**单个字符串**的评估值，尚未应用队名的 `max`。这些名字只有一种字符类别且长度不超过 8：数字使用权重 16，小写字母使用权重 32，故 `x = 字符数 × ln(权重) / 2 - 20`。

| 字符串 | x（约） | rq=4 的分值（约） | rq=6 的分值（约） |
| --- | ---: | ---: | ---: |
| `1` | -18.613706 | -14.613706 | -4.204569 |
| `11` | -17.227411 | -13.227411 | -3.742470 |
| `111111` | -11.682234 | -7.682234 | -1.894078 |
| `a` | -18.267132 | -14.267132 | -4.089044 |
| `abc` | -14.801396 | -10.801396 | -2.933799 |

既有 `win_rate_rq_changes_negative_threshold` 测试验证 rq=6 的负分值更接近 0；具体数值应按上述完整公式计算，不要把 `x` 与最终系数混用。

## 3. 对属性的影响

普通构造的缩放见 [namerena/build.rs](../../crates/tswn_core/src/namerena/build.rs#L412-L439)；分身使用同类公式重新派生，见 [runtime/entity.rs](../../crates/tswn_core/src/runtime/entity.rs#L216-L230)。

```text
攻击 / 防御 / 基础速度 / 敏捷 / 魔法 / 抗性:
    round(raw_attr * (1 - name_factor / 128))
智慧:
    round(raw_attr * (1 - name_factor / 80))
速度:
    缩放后的基础速度 + 160
HP:
    不做 name_factor 缩放
```

负系数令缩放倍率大于 1。以下是只展示该缩放步骤的算例，原始八围为 `[100,80,90,70,85,75,60,200]`，采用 `111111` 的字符串分值且假定队名不覆盖它，不叠加武器或其他修正：

| 属性 | 原始值 | rq=4（nf≈-7.682234） | rq=6（nf≈-1.894078） |
| --- | ---: | ---: | ---: |
| 攻击 | 100 | 106 | 101 |
| 防御 | 80 | 85 | 81 |
| 速度 | 90 | 255 | 251 |
| 敏捷 | 70 | 74 | 71 |
| 魔法 | 85 | 90 | 86 |
| 抗性 | 75 | 80 | 76 |
| 智慧 | 60 | 66 | 61 |
| HP | 200 | 200 | 200 |

这些是属性缩放算例，不是重新跑过的名字池胜率结果。

## 4. seed 与 rq 是两个独立维度

当前胜率批处理的 [profile_seed_for_round](../../crates/tswn_core/src/runtime/batch.rs#L340-L348) 不按 rq 分支：

```text
round == 0  -> 不附加 profile seed
round > 0   -> seed:{PROFILE_START + round}@!
```

这些 seed 扰动同一组输入的战斗初始化，不是据 rq 生成另一批“假想对手”。评分的对手 profile 由独立评分构造链生成，也不能从上述 seed 规则推断。旧文档中引用 `win_rate.rs`、按 rq 选择两种 seed 格式的代码属于旧实现，不是当前行为。

## 5. 如何对齐比较

- Rust / Python / WASM / C 调用时使用对应入口的显式 `eval_rq` 参数；普通战斗选项与进阶 runner 的位置见 [公共 API](../reference/public-api.md)。
- CLI 的 `bench win-rate`、`bench group-win-rate`、`bench batch-rate`、`bench pair` 与 `namer-pf` 支持 `--keep-rq`，用于保留普通对打的 rq=4；`bench auto` 没有这个开关。当前参数以 [CLI 定义](../../crates/tswn_core/src/bin/tswn_cli/args/cli.rs) 为准。
- `PreparedRunner` 已固定构造属性；比较不同 rq 时分别准备 roster，而不是只替换运行期 seed。
- 对照输出时同时记录名字、队名、overlay 的 `name_factor_enabled`、rq、输入分组与 seed。

验证名字评估单元测试：

```powershell
cargo test -p tswn_core --lib namerena::eval_name::test
```
