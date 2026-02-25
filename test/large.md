# fight_large_replay_should_match 专项失败分析

> 本文只分析 `fight_large_replay_should_match`（`tswn-core/src/engine/test.rs:2254`），不再汇总 sampled case 01-10。

## 复现摘要

- 测试定义：`tswn-core/src/engine/test.rs:2254`
- 复现命令：
  - `cargo test fight_large_replay_should_match`
- 当前结果：失败，首分叉 `idx=26`
  - 期望：`wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到62点伤害`
  - 实际：`wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到45点伤害`
  - 附加现象：`actual_len=709`, `expected_len=677`

## cargo test 原文（摘录）

```text
---- engine::test::runner::fight_large_replay_should_match stdout ----
fight_large mismatch context [21..31):
  idx=21: actual=Some("#念-GP8LKM21D4JZ发起攻击, DianmuYKFMWRPXIMCQ受到62点伤害") | expected=Some("#念-GP8LKM21D4JZ发起攻击, DianmuYKFMWRPXIMCQ受到62点伤害")
  idx=22: actual=Some("Tachibana_akira#BydbIMidbs发起攻击, 无惨不等式#YMGTFCOPE受到22点伤害") | expected=Some("Tachibana_akira#BydbIMidbs发起攻击, 无惨不等式#YMGTFCOPE受到22点伤害")
  idx=23: actual=Some("Hypochondriac#TtwN3jZ发起攻击, <ζο>-2ny1o5sk受到46点伤害") | expected=Some("Hypochondriac#TtwN3jZ发起攻击, <ζο>-2ny1o5sk受到46点伤害")
  idx=24: actual=Some("DianmuYKFMWRPXIMCQ发起攻击, tCtrVweRgshV受到87点伤害") | expected=Some("DianmuYKFMWRPXIMCQ发起攻击, tCtrVweRgshV受到87点伤害")
  idx=25: actual=Some("血谣染硫决使用幻术, 召唤出幻影") | expected=Some("血谣染硫决使用幻术, 召唤出幻影")
  idx=26: actual=Some("wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到45点伤害") | expected=Some("wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到62点伤害")
  idx=27: actual=Some("泠珞VmRtntb使用生命之轮, 权计WN13vmJnn受到155点伤害") | expected=Some("泠珞VmRtntb发起攻击, 愞㢯老海受到53点伤害")
  idx=28: actual=Some("态度jX2HoULfsFU9发起攻击, Reality#ke10TrY受到74点伤害") | expected=Some("tCtrVweRgshV发起攻击, Reality#ke10TrY受到99点伤害")
  idx=29: actual=Some("「OS」#c1#bFc71OCDuO35发动会心一击, Obsession#EYNIZRX回避了攻击") | expected=Some("「OS」#c1#bFc71OCDuO35发动会心一击, Obsession#EYNIZRX回避了攻击")
  idx=30: actual=Some("氯化钠8UJMGcZ投毒, Wakaba_mutsumi#pjFhEhSbjy回避了攻击") | expected=Some("氯化钠8UJMGcZ投毒, Wakaba_mutsumi#pjFhEhSbjy回避了攻击")

thread 'engine::test::runner::fight_large_replay_should_match' (23564) panicked at src\engine\test.rs:3726:13:
fight_large mismatch at idx=26, actual_len=709, expected_len=677, actual=Some("wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到45点伤害"), expected=Some("wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze受到62点伤害")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

## 关键现象

1. `idx=21..25` 仍对齐，`idx=26` 开始先出现“同动作伤害值偏差”，随后立刻升级为行动类型偏差（`生命之轮` vs `普攻`）。
2. 分叉点附近刚出现 `幻术召唤幻影`（`idx=25`），属于高敏感窗口。
3. 后续总长度差达到 32（709 vs 677），说明后续很长一段都在累计偏移。

## Rust 侧关键链路（参考）

- 轮转与行动执行：`src/engine/runners.rs:374-410`
- `on_update_end` 与实体同步：`src/engine/runners.rs:304-315`, `328-372`
- 行动决策（smart、req_mp、技能扫描、fallback）：`src/player/impl_runtime.rs:29-87`
- 技能目标选择与打分：`src/player/impl_runtime.rs:140-189`
- 受击与伤害落地：`src/player/impl_runtime.rs:535-585`

## JS 侧对照（参考）

- `Plr.step/action`：`fast-namerena/branch/latest/md5.js:18223-18279`
- 通用 `Skill.aa`：`.../md5.js:18618-18647`
- `SklShadow.aa`：`.../md5.js:15673-15675`
- `pickSkipRange`：`.../md5.js:19867-19877`
- 受击入口 `a3/bN/aF`：`.../md5.js:18351-18390`

## 原因判断（按置信度）

### 高置信
- 该 large 分叉是“中前段轻微随机漂移”在 `idx=25~28` 被集中放大的体现，不是单一技能文案问题。

### 中高置信
- 分叉窗口含“幻影加入 + 高密度行动切换”，对 `target pick / score / dodge / damage` 任一消费差异都很敏感。

### 中置信
- `idx=26` 首先体现为同动作伤害差值（45 vs 62），可能来自 `getAt/getDf` 采样链路已经不同步；随后才表现为技能分支变化。

## 建议验证顺序

1. 以 `idx=25..28` 为核心窗口，记录每条动作的 RC4 消费点、目标集合、伤害中间值。
2. 先确保 “shadow 相关动作” 不额外引入随机消费，再验证 `pickSkipRange` 与 `score_target` 次数一致性。
3. 窗口对齐后再全跑 `fight_large`，观察 `actual_len` 是否回落到 677 附近。

