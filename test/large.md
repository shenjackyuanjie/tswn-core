# fight_large_replay_should_match 失败分析（2026-03-01 更新）

## 复现摘要

- 复现命令：`cargo test fight_large_replay_should_match -- --nocapture`
- 当前结果：失败，首分叉 `idx=135`
  - 期望：`泠珞VmRtntb使用瘟疫, 都江堰00217109183087体力减少64%`
  - 实际：`泠珞VmRtntb使用瘟疫, 都江堰00217109183087回避了攻击`
  - 附加现象：`actual_len=696`, `expected_len=677`

## cargo test 原文（摘录）

```text
fight_large mismatch context [130..140):
  idx=130: actual=Some("咲夜bJjbFYez使用净化, 运松翁nkJspy1Oh54A受到57点伤害") | expected=Same
  idx=131: actual=Some("虚空咦aubHluOMPElA发起攻击, 涵虚不等式PFVKEUPBU受到70点伤害") | expected=Same
  idx=132: actual=Some("<ζο>-2ny1o5sk发起攻击, 权计WN13vmJnn受到41点伤害") | expected=Same
  idx=133: actual=Some("灀瑈篆狓鵃发起攻击, Reku_Mochizuki#494460162188受到67点伤害") | expected=Same
  idx=134: actual=Some("oWmjI_$'4Z#GK,,BX2发动会心一击, 神谷紫苑#EUKSOXAA受到85点伤害") | expected=Same
  idx=135: actual=Some("泠珞VmRtntb使用瘟疫, 都江堰00217109183087回避了攻击") | expected=Some("泠珞VmRtntb使用瘟疫, 都江堰00217109183087体力减少64%")
  idx=136: actual=Some("稗田阿求OQL68NN8发起攻击, 看到这个号说明你要豹了Xa2Zuiqj受到88点伤害") | expected=Same
  idx=137: actual=Some("GordonALYJDXORPTER发起攻击, 东乡幻翎#BCBNRCXFX受到91点伤害") | expected=Same
  idx=138: actual=Some("tCtrVweRgshV发起攻击, <ζο>-2ny1o5sk受到45点伤害") | expected=Same
  idx=139: actual=Some("<ζο>-2ny1o5sk被击倒了") | expected=Same
```

## 关键现象

1. `idx=0..134` 完全对齐，属于后段分叉（战斗进行到中后期）。
2. 首分叉：技能名称相同（瘟疫），但效果不同——实际为“回避了攻击”（未命中），期望为“体力减少64%”（命中并造成百分比伤害）。
3. 分叉后事件序列从 `idx=136` 开始重新对齐，说明该分叉未引发后续连锁偏移，仅改变了该技能的效果描述。
4. 事件长度差异（696 vs 677）表明分叉点之后的事件数量有变化，但上下文显示后续事件仍然对齐，可能是分叉前后的事件计数有细微调整。

## 问题分类

**命中判定/技能效果** — 相同技能（瘟疫）的命中判定与实际效果不一致：期望命中并造成百分比伤害，实际被回避。可能是目标的回避率计算、技能命中率或状态免疫判定与预期不符。由于战斗规模较大，该分叉虽未引发后续连锁反应，但导致了整体事件长度变化。