# fight_large_replay_should_match 失败分析（2026-02-28 更新）

## 复现摘要

- 复现命令：`cargo test fight_large_replay_should_match -- --nocapture`
- 当前结果：失败，首分叉 `idx=59`
  - 期望：`湖心SHVPEMAPV使用生命之轮, 锋利ⅤEGZPVQMY的体力值与湖心SHVPEMAPV互换`
  - 实际：`湖心SHVPEMAPV使用交换, 湖心SHVPEMAPV回复体力56点, 锋利ⅤEGZPVQMY受到56点伤害`
  - 附加现象：`actual_len=696`, `expected_len=677`

## cargo test 原文（摘录）

```text
fight_large mismatch context [54..64):
  idx=54: actual=Some("PHKBUUPNHGMI发起攻击, 缇亚卡#WOVLHAESD受到70点伤害") | expected=Same
  idx=55: actual=Some("力气30#zxQ4y6发起攻击, 湖心SHVPEMAPV受到39点伤害") | expected=Same
  idx=56: actual=Some("态度jX2HoULfsFU9发起攻击, 冥河WyO8MUZPPtKH受到85点伤害") | expected=Same
  idx=57: actual=Some("SDPC#AZLZJQUPN开始聚气, SDPC#AZLZJQUPN攻击力上升") | expected=Same
  idx=58: actual=Some("PraykxtsMobhMzey发起攻击, <ζε>-fhepgq2n受到77点伤害") | expected=Same
  idx=59: actual=Some("湖心SHVPEMAPV使用交换, 湖心SHVPEMAPV回复体力56点, 锋利ⅤEGZPVQMY受到56点伤害") | expected=Some("湖心SHVPEMAPV使用生命之轮, 锋利ⅤEGZPVQMY的体力值与湖心SHVPEMAPV互换")
  idx=60: actual=Some("\"铁胆\"哈拉文领主-ksbGnquBbq-发起攻击, 可可萝#EZBAOSOOV回避了攻击") | expected=Same
  idx=61: actual=Some("<ζε>-fhepgq2n发起攻击, 力气30#zxQ4y6受到122点伤害") | expected=Same
  idx=62: actual=Some("看到这个号说明你要豹了Xa2Zuiqj发起攻击, 可可萝#EZBAOSOOV受到111点伤害") | expected=Same
  idx=63: actual=Some("冥河WyO8MUZPPtKH发起攻击, <ζο>-2ny1o5sk受到33点伤害") | expected=Same
```

## 关键现象

1. `idx=0..58` 完全对齐，属于中段分叉。
2. 首分叉：技能选择错误——应使用“生命之轮”（体力值互换）却使用了“交换”（造成伤害并回复自身）。
3. 分叉后 `idx=60..63` 的事件依然对齐，但事件长度变化（696 vs 677），说明分叉点之后的部分事件被跳过或新增，但此处提供的上下文有限，后续仍有较长序列出现偏移。
4. 该分叉与 `sampled_large_case_10` 非常相似，均为生命之轮与交换的混淆，但影响范围更大（导致总事件数变化）。

## 问题分类

**技能选择** — 在特定时机下，角色应使用生命之轮但实际使用了交换，可能是技能优先级、目标状态判定或随机选择机制与预期不符。由于战斗规模较大，该分叉引发了后续连锁反应，导致整体事件长度变化。
