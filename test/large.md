# fight_large_replay_should_match 失败分析

## 复现摘要

- 复现命令：`cargo test fight_large_replay_should_match -- --nocapture`
- 当前结果：失败，首分叉 `idx=42`
  - 期望：`BZoPIow发起狂暴攻击, 兔蛙智仁$0a0LD4Dh受到115点伤害`
  - 实际：`BZoPIow发起攻击, 七七#EUEMIGPI受到110点伤害`
  - 附加现象：`actual_len=737`, `expected_len=677`

## cargo test 原文（摘录）

```text
fight_large mismatch context [37..47):
  idx=37: actual=Some("稗田阿求OQL68NN8发起攻击, 泠珞VmRtntb防御, 泠珞VmRtntb受到25点伤害") | expected=Same
  idx=38: actual=Some("权计WN13vmJnn使用诅咒, 三田一重TxtrdTN4l8nT受到61点伤害, 三田一重TxtrdTN4l8nT被诅咒了") | expected=Same
  idx=39: actual=Some("子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, Tik_Tok#IBxWzGZtr受到46点伤害") | expected=Same
  idx=40: actual=Some("「OS」#c1#E7WGTekQTugF发起攻击, 东乡幻翎#BCBNRCXFX受到29点伤害") | expected=Same
  idx=41: actual=Some("封魔宣夜8uW56ll发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到108点伤害") | expected=Same
  idx=42: actual=Some("BZoPIow发起攻击, 七七#EUEMIGPI受到110点伤害") | expected=Some("BZoPIow发起狂暴攻击, 兔蛙智仁$0a0LD4Dh受到115点伤害")
  idx=43: actual=Some("BZoPIow从狂暴中解除") | expected=Same
  idx=44: actual=Some("仇决clFJZCMHS发起攻击, DianmuYKFMWRPXIMCQ受到99点伤害") | expected=Some("仇决clFJZCMHS发起攻击, 石之自由jV3zf35受到69点伤害")
  idx=45: actual=Some("兔蛙智仁$0a0LD4Dh使用雷击术") | expected=Some("「OS」#H1#YoRmfG4zW9发起攻击, MeltelabRC3P3Go7受到43点伤害")
  idx=46: actual=Some("ImmutableZYsdlabOOz受到17点伤害") | expected=Some("针刀霜|U/T)h8J\"发起攻击, 七七#EUEMIGPI回避了攻击")
```

## 关键现象

1. `idx=0..41` 完全对齐，属于中后段分叉。
2. 分叉点：应发起**狂暴攻击**但实际执行了**普通攻击**，且目标不同（兔蛙智仁 vs 七七）。
3. 注意 `idx=43` 仍有"从狂暴中解除"，与 case_03 表现一致：角色处于狂暴状态但未正确触发狂暴攻击。
4. 总长度差异大（737 vs 677），后续完全发散。

## 问题分类

**狂暴攻击触发 + 目标选择** — 与 case_03 属同类问题。角色处于狂暴状态但攻击动作和目标选择均与期望不同。
