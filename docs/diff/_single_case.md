# Bun / tswn 逐局分叉报告

## 方法

- bun: 对官方 md5.js::ProfileWinChance 做最小补丁，只把 callback 粒度从 100 场改成 1 场，不改胜负逻辑。
- tswn: 使用 `bench auto --buckets-step 1` 输出每局累计胜场。
- 对比方式: 逐 round 比较 team1 是否获胜，得到真实分叉 round。
- seed 规则: 第 1 局无 seed；第 N 局 (N>1) 对应 `seed:` + (33554431 + N - 1)。

## 汇总

- case 数: 1
- 总分叉 round 数: 6

## Jm0MGgK4HfUAAQ5yKW59XGnlp0EB

- 时间: 2026/04/20 1776658241000 / rua！
- 旧消息显示: bun=41.53% / tswn(old)=41.54% / diff=+0.01
- 本次精确结果: bun=4153/10000 / tswn=4153/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-51000-1777657271202.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-51000-1777657271202.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-51000-1777657271202.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-51000-1777657271202.js:10661:16)

- 队 1:

```text
Tik_Tok #IBxWzGZtr@Shabby_fish
青霉素 #gRybM8geD@Shabby_fish
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 6
- 真实分叉 round: 2128, 3120, 5769, 5922, 7238, 9691
- 对应 seed: r2128=3357558, r3120=3358550, r5769=3361199, r5922=3361352, r7238=3362668, r9691=3365121

### 1000 场分段

- 1-1000: bun 0->410, tswn 0->410, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 410->810, tswn 410->810, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 810->1223, tswn 810->1224, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 2128
- 3001-4000: bun 1223->1650, tswn 1224->1650, 累计差 +1->+0, 净变化 -1, 分叉 1 场
  rounds: 3120
- 4001-5000: bun 1650->2082, tswn 1650->2082, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 5001-6000: bun 2082->2470, tswn 2082->2470, 累计差 +0->+0, 净变化 +0, 分叉 2 场
  rounds: 5769, 5922
- 6001-7000: bun 2470->2895, tswn 2470->2895, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 7001-8000: bun 2895->3324, tswn 2895->3325, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 7238
- 8001-9000: bun 3324->3732, tswn 3325->3733, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 9001-10000: bun 3732->4153, tswn 3733->4153, 累计差 +1->+0, 净变化 -1, 分叉 1 场
  rounds: 9691
