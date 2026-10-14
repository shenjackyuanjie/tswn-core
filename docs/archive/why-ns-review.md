# why_ns：JS 机制与兼容排查复盘

[返回历史索引](README.md)

本文整理 2026-05-18 的 JS/profile 兼容分析，集中保留构造序号机制和该批差异的定位经验。原分析发生在旧 Rust 对象模型阶段；现行角色构建与实体生命周期见 [角色构建](../core-guide/name-to-player.md) 和 [状态与召唤物](../core-guide/statuses-and-minions.md)。

## JS 构造序号的生命周期

`why_ns` 是当时 `md5.js` 的构造序号计数器。分析记录的入口如下：

| JS 入口 | 对计数器的操作 | 意义 |
| --- | --- | --- |
| `Plr.a1()` | 读取构造前的值，然后加 1 | 为这次构造取得 slot |
| `Grp.aZ()` | 首次挂入 roster 时减 1 | 与构造配对的“退款” |
| `Engine.O()` / `bE()` | 胜负输出时归零 | 对局结束的重置 |
| `ProfileMain.dZ()` / `ProfileWinChance.dY()` | 真正开打前预构造角色 | 预构造影响首场实际构造的起点 |

名字混入玩家 RC4 前，JS 使用以下门控：

```text
slot = why_ns
why_ns += 1
若 abs(slot) >= 2048：交换玩家 RC4 排列的前两个字节
随后混入名字
```

实际影响取决于**构造前 slot、2048 阈值和完整生命周期**。正常 roster 的构造/退款会相互抵消，分析时普通对局与少量 profile 预构造保持在低值范围。稳定复现阈值路径时，应连同预构造、首次挂入、复用和终局重置一起记录。

实体编号与构造序号分别服务于运行时引用和名字构建随机协议。排查时沿两条数据流独立跟踪；PreparedRunner 的模板复用也要与原始构造上下文一起理解。

## 当时真正闭环的差异

该批 profile / namer-pf 分歧通过具体语义修正得到解决。保留下表可帮助识别相似症状的不同来源：

| 差异来源 | 当时修正的行为 | 现行阅读入口 |
| --- | --- | --- |
| score 计分 | 目标队任一赢家都计入该队胜场 | [胜率与评分](../core-guide/datasets-and-extension.md#胜率profile-评分和组合评估)、[判胜语义](../mechanics/winner.md) |
| ProfileMain 输入形状 | 单目标是 1 vs 3，重复双名 collapse 为 1 vs 1 | [评分入口](../../crates/tswn_core/src/cli_api/bench.rs) |
| TestEx 同队升级 | 对齐 JS `PlrEx.cA()` 的 no-op 路径 | [构建链](../../crates/tswn_core/src/namerena/build.rs) |
| clone 原始底板 | 保存普通构造底板，再重放相应变换与加成 | [分身机制](../mechanics/clone.md) |
| summon 重施 | 复用死亡使魔时重置运行状态，并复活原实体 | [plain_summon.rs](../../crates/tswn_core/src/runtime/plain_summon.rs) |
| Iron 清理时机 | 防御链中击破后即时清理，使后续 hook 读取更新后的状态 | [受击结算](../core-guide/battle-flow.md)、[defense.rs](../../crates/tswn_core/src/runtime/combat/defense.rs) |

分析时 Rust 修复采用了这些输入、构建、复用和结算语义。判断构造序号是否需要额外建模，应以能够命中其门控和生命周期的具体差分样例为依据。

## 排查方法

1. 保存完整输入、队伍边界、rq、seed，以及 profile 是否经过预构造。
2. 从第一处分叉判断阶段：构建属性、行动随机、目标选择、伤害、状态清理或计分。
3. 对召唤物分别记录首次构造、死亡重施、复用和重新挂入场上的过程。
4. 针对命中的机制补实际行为回归，使用相同输入继续对拍。

现行复现与源码定位方法见 [随机与兼容](../core-guide/randomness-and-compatibility.md) 和 [开发与排查](../core-guide/contributing-and-debugging.md)。

## 历史来源

本页合并原机制概览、早期分析、Rust 分析和修复复盘。完整推理过程与旧源码定位可在 [整理前的 Git 快照](https://github.com/shenjackyuanjie/tswn-core/tree/821e3d47439983099e256bc2a07898a8b999bef8/docs/archive) 查阅。
