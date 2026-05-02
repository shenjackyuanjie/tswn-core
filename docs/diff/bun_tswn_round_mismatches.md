# Bun / tswn 逐局分叉报告

## 方法

- bun: 对官方 md5.js::ProfileWinChance 做最小补丁，只把 callback 粒度从 100 场改成 1 场，不改胜负逻辑。
- tswn: 使用 `bench auto --buckets-step 1` 输出每局累计胜场。
- 对比方式: 逐 round 比较 team1 是否获胜，得到真实分叉 round。
- seed 规则: 第 1 局无 seed；第 N 局 (N>1) 对应 `seed:` + (33554431 + N - 1)。

## 汇总

- case 数: 14
- 总分叉 round 数: 98

## QF9iBQK4HfUAE+Mmt0ZrdGnlpjwB

- 时间: 2026/04/20 1776657980000 / shenjack的bot
- 旧消息显示: bun=46.42% / tswn(old)=46.33% / diff=-0.09
- 本次精确结果: bun=4642/10000 / tswn=4646/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-42640-1777657298502.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42640-1777657298502.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42640-1777657298502.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42640-1777657298502.js:10661:16)

- 队 1:

```text
三猴一体 vFz1cu21MCaW@TigerStar
涵虚不等式 PFVKEUPBU@TigerStar
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 22
- 真实分叉 round: 476, 848, 1074, 2693, 3204, 3912, 4144, 4864, 5000, 6101, 6591, 6603, 7057, 7174, 8806, 9146, 9158, 9397, 9411, 9690, 9735, 9750
- 对应 seed: r476=3355906, r848=3356278, r1074=3356504, r2693=3358123, r3204=3358634, r3912=3359342, r4144=3359574, r4864=3360294, r5000=3360430, r6101=3361531, r6591=3362021, r6603=3362033, r7057=3362487, r7174=3362604, r8806=3364236, r9146=3364576, r9158=3364588, r9397=3364827, r9411=3364841, r9690=3365120, r9735=3365165, r9750=3365180

### 1000 场分段

- 1-1000: bun 0->494, tswn 0->492, 累计差 +0->-2, 净变化 -2, 分叉 2 场
  rounds: 476, 848
- 1001-2000: bun 494->979, tswn 492->976, 累计差 -2->-3, 净变化 -1, 分叉 1 场
  rounds: 1074
- 2001-3000: bun 979->1441, tswn 976->1439, 累计差 -3->-2, 净变化 +1, 分叉 1 场
  rounds: 2693
- 3001-4000: bun 1441->1897, tswn 1439->1895, 累计差 -2->-2, 净变化 +0, 分叉 2 场
  rounds: 3204, 3912
- 4001-5000: bun 1897->2332, tswn 1895->2331, 累计差 -2->-1, 净变化 +1, 分叉 3 场
  rounds: 4144, 4864, 5000
- 5001-6000: bun 2332->2786, tswn 2331->2785, 累计差 -1->-1, 净变化 +0, 分叉 0 场
- 6001-7000: bun 2786->3230, tswn 2785->3230, 累计差 -1->+0, 净变化 +1, 分叉 3 场
  rounds: 6101, 6591, 6603
- 7001-8000: bun 3230->3718, tswn 3230->3718, 累计差 +0->+0, 净变化 +0, 分叉 2 场
  rounds: 7057, 7174
- 8001-9000: bun 3718->4193, tswn 3718->4194, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 8806
- 9001-10000: bun 4193->4642, tswn 4194->4646, 累计差 +1->+4, 净变化 +3, 分叉 7 场
  rounds: 9146, 9158, 9397, 9411, 9690, 9735, 9750

## QF9iBQK4HfUAE+MpsB4DH2nlptYB

- 时间: 2026/04/20 1776658134000 / shenjack的bot
- 旧消息显示: bun=47.85% / tswn(old)=47.88% / diff=+0.03
- 本次精确结果: bun=4785/10000 / tswn=4783/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-49996-1777657307143.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-49996-1777657307143.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-49996-1777657307143.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-49996-1777657307143.js:10661:16)

- 队 1:

```text
湖心 SHVPEMAPV@TigerStar
涵伯发威 TGDPANBQM@TigerStar
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 12
- 真实分叉 round: 2115, 2273, 2878, 3535, 4662, 5097, 5833, 7257, 7747, 8139, 8215, 8343
- 对应 seed: r2115=3357545, r2273=3357703, r2878=3358308, r3535=3358965, r4662=3360092, r5097=3360527, r5833=3361263, r7257=3362687, r7747=3363177, r8139=3363569, r8215=3363645, r8343=3363773

### 1000 场分段

- 1-1000: bun 0->468, tswn 0->468, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 468->946, tswn 468->946, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 946->1436, tswn 946->1433, 累计差 +0->-3, 净变化 -3, 分叉 3 场
  rounds: 2115, 2273, 2878
- 3001-4000: bun 1436->1925, tswn 1433->1921, 累计差 -3->-4, 净变化 -1, 分叉 1 场
  rounds: 3535
- 4001-5000: bun 1925->2406, tswn 1921->2403, 累计差 -4->-3, 净变化 +1, 分叉 1 场
  rounds: 4662
- 5001-6000: bun 2406->2872, tswn 2403->2869, 累计差 -3->-3, 净变化 +0, 分叉 2 场
  rounds: 5097, 5833
- 6001-7000: bun 2872->3343, tswn 2869->3340, 累计差 -3->-3, 净变化 +0, 分叉 0 场
- 7001-8000: bun 3343->3808, tswn 3340->3807, 累计差 -3->-1, 净变化 +2, 分叉 2 场
  rounds: 7257, 7747
- 8001-9000: bun 3808->4289, tswn 3807->4287, 累计差 -1->-2, 净变化 -1, 分叉 3 场
  rounds: 8139, 8215, 8343
- 9001-10000: bun 4289->4785, tswn 4287->4783, 累计差 -2->-2, 净变化 +0, 分叉 0 场

## Jm0MGgK4HfUAAQ5u8M44wWnlpzQB

- 时间: 2026/04/20 1776658228000 / rua！
- 旧消息显示: bun=53.59% / tswn(old)=53.61% / diff=+0.02
- 本次精确结果: bun=5359/10000 / tswn=5362/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-42168-1777657312362.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42168-1777657312362.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42168-1777657312362.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-42168-1777657312362.js:10661:16)

- 队 1:

```text
Beijing_Beijing #buBfAkjNf@Shabby_fish
luogu.com.cn/paste/q9h8sdzk@Shabby_fish
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 5
- 真实分叉 round: 3062, 4833, 6641, 7979, 8623
- 对应 seed: r3062=3358492, r4833=3360263, r6641=3362071, r7979=3363409, r8623=3364053

### 1000 场分段

- 1-1000: bun 0->530, tswn 0->530, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 530->1066, tswn 530->1066, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 1066->1597, tswn 1066->1597, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1597->2135, tswn 1597->2136, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 3062
- 4001-5000: bun 2135->2692, tswn 2136->2694, 累计差 +1->+2, 净变化 +1, 分叉 1 场
  rounds: 4833
- 5001-6000: bun 2692->3203, tswn 2694->3205, 累计差 +2->+2, 净变化 +0, 分叉 0 场
- 6001-7000: bun 3203->3718, tswn 3205->3721, 累计差 +2->+3, 净变化 +1, 分叉 1 场
  rounds: 6641
- 7001-8000: bun 3718->4288, tswn 3721->4290, 累计差 +3->+2, 净变化 -1, 分叉 1 场
  rounds: 7979
- 8001-9000: bun 4288->4822, tswn 4290->4825, 累计差 +2->+3, 净变化 +1, 分叉 1 场
  rounds: 8623
- 9001-10000: bun 4822->5359, tswn 4825->5362, 累计差 +3->+3, 净变化 +0, 分叉 0 场

## Jm0MGgK4HfUAAQ5yKW59XGnlp0EB

- 时间: 2026/04/20 1776658241000 / rua！
- 旧消息显示: bun=41.53% / tswn(old)=41.54% / diff=+0.01
- 本次精确结果: bun=4153/10000 / tswn=4153/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-30444-1777657321453.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-30444-1777657321453.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-30444-1777657321453.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-30444-1777657321453.js:10661:16)

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

## Jm0MGgK4HfUAAQ565d9J3mnlp7UB

- 时间: 2026/04/20 1776658357001 / rua！
- 旧消息显示: bun=51.99% / tswn(old)=52.00% / diff=+0.01
- 本次精确结果: bun=5199/10000 / tswn=5199/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-27168-1777657326743.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-27168-1777657326743.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-27168-1777657326743.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-27168-1777657326743.js:10661:16)

- 队 1:

```text
Tik_Tok #IBxWzGZtr@Shabby_fish
Mythic YK!,o,dT@Shabby_fish
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 4
- 真实分叉 round: 4795, 4844, 5738, 7246
- 对应 seed: r4795=3360225, r4844=3360274, r5738=3361168, r7246=3362676

### 1000 场分段

- 1-1000: bun 0->527, tswn 0->527, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 527->1061, tswn 527->1061, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 1061->1589, tswn 1061->1589, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1589->2108, tswn 1589->2108, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 4001-5000: bun 2108->2627, tswn 2108->2625, 累计差 +0->-2, 净变化 -2, 分叉 2 场
  rounds: 4795, 4844
- 5001-6000: bun 2627->3136, tswn 2625->3135, 累计差 -2->-1, 净变化 +1, 分叉 1 场
  rounds: 5738
- 6001-7000: bun 3136->3647, tswn 3135->3646, 累计差 -1->-1, 净变化 +0, 分叉 0 场
- 7001-8000: bun 3647->4173, tswn 3646->4173, 累计差 -1->+0, 净变化 +1, 分叉 1 场
  rounds: 7246
- 8001-9000: bun 4173->4683, tswn 4173->4683, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 9001-10000: bun 4683->5199, tswn 4683->5199, 累计差 +0->+0, 净变化 +0, 分叉 0 场

## Jm0MGgK4HfUAAQ59YmDV0WnlqCcB

- 时间: 2026/04/20 1776658471001 / rua！
- 旧消息显示: bun=47.44% / tswn(old)=47.45% / diff=+0.01
- 本次精确结果: bun=4744/10000 / tswn=4743/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-19440-1777657331904.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-19440-1777657331904.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-19440-1777657331904.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-19440-1777657331904.js:10661:16)

- 队 1:

```text
Italian_Love #5Agn8kVYl@Shabby_fish
我会回来的 #yTneTj00J@Shabby_fish
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 7
- 真实分叉 round: 641, 947, 4437, 5106, 7536, 8039, 9616
- 对应 seed: r641=3356071, r947=3356377, r4437=3359867, r5106=3360536, r7536=3362966, r8039=3363469, r9616=3365046

### 1000 场分段

- 1-1000: bun 0->487, tswn 0->487, 累计差 +0->+0, 净变化 +0, 分叉 2 场
  rounds: 641, 947
- 1001-2000: bun 487->951, tswn 487->951, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 951->1447, tswn 951->1447, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1447->1920, tswn 1447->1920, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 4001-5000: bun 1920->2404, tswn 1920->2405, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 4437
- 5001-6000: bun 2404->2865, tswn 2405->2865, 累计差 +1->+0, 净变化 -1, 分叉 1 场
  rounds: 5106
- 6001-7000: bun 2865->3351, tswn 2865->3351, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 7001-8000: bun 3351->3791, tswn 3351->3790, 累计差 +0->-1, 净变化 -1, 分叉 1 场
  rounds: 7536
- 8001-9000: bun 3791->4254, tswn 3790->4252, 累计差 -1->-2, 净变化 -1, 分叉 1 场
  rounds: 8039
- 9001-10000: bun 4254->4744, tswn 4252->4743, 累计差 -2->-1, 净变化 +1, 分叉 1 场
  rounds: 9616

## Jm0MGgK4HfUAAQ6FZnIJEGnlqR8B

- 时间: 2026/04/20 1776658719001 / rua！
- 旧消息显示: bun=44.78% / tswn(old)=44.77% / diff=-0.01
- 本次精确结果: bun=4478/10000 / tswn=4481/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-47936-1777657336896.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-47936-1777657336896.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-47936-1777657336896.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-47936-1777657336896.js:10661:16)

- 队 1:

```text
Instantaneous #LxiPNA8jV@Shabby_fish
Diffindo #3WzDmhsqL@Shabby_fish
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 3
- 真实分叉 round: 3362, 7461, 8977
- 对应 seed: r3362=3358792, r7461=3362891, r8977=3364407

### 1000 场分段

- 1-1000: bun 0->442, tswn 0->442, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 442->894, tswn 442->894, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 894->1340, tswn 894->1340, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1340->1798, tswn 1340->1799, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 3362
- 4001-5000: bun 1798->2264, tswn 1799->2265, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 5001-6000: bun 2264->2712, tswn 2265->2713, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 6001-7000: bun 2712->3139, tswn 2713->3140, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 7001-8000: bun 3139->3606, tswn 3140->3608, 累计差 +1->+2, 净变化 +1, 分叉 1 场
  rounds: 7461
- 8001-9000: bun 3606->4036, tswn 3608->4039, 累计差 +2->+3, 净变化 +1, 分叉 1 场
  rounds: 8977
- 9001-10000: bun 4036->4478, tswn 4039->4481, 累计差 +3->+3, 净变化 +0, 分叉 0 场

## QF9iBQK4HfUAE+M1GSOv6Wnlq/oB

- 时间: 2026/04/20 1776659450000 / shenjack的bot
- 旧消息显示: bun=49.34% / tswn(old)=49.27% / diff=-0.07
- 本次精确结果: bun=4934/10000 / tswn=4941/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-40844-1777657342686.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-40844-1777657342686.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-40844-1777657342686.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-40844-1777657342686.js:10661:16)

- 队 1:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 队 2:

```text
Tachibana_akira #BydbIMidbs@🥒
𝚐𝚒𝚛𝚕𝚜.𝚎𝚡𝚎 #JgLghHnHgP@🥒
```
- 真实分叉 round 数: 9
- 真实分叉 round: 98, 2312, 2443, 3275, 4620, 6020, 8321, 8638, 8907
- 对应 seed: r98=3355528, r2312=3357742, r2443=3357873, r3275=3358705, r4620=3360050, r6020=3361450, r8321=3363751, r8638=3364068, r8907=3364337

### 1000 场分段

- 1-1000: bun 0->484, tswn 0->485, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 98
- 1001-2000: bun 484->941, tswn 485->942, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 2001-3000: bun 941->1420, tswn 942->1421, 累计差 +1->+1, 净变化 +0, 分叉 2 场
  rounds: 2312, 2443
- 3001-4000: bun 1420->1938, tswn 1421->1940, 累计差 +1->+2, 净变化 +1, 分叉 1 场
  rounds: 3275
- 4001-5000: bun 1938->2432, tswn 1940->2435, 累计差 +2->+3, 净变化 +1, 分叉 1 场
  rounds: 4620
- 5001-6000: bun 2432->2927, tswn 2435->2930, 累计差 +3->+3, 净变化 +0, 分叉 0 场
- 6001-7000: bun 2927->3443, tswn 2930->3447, 累计差 +3->+4, 净变化 +1, 分叉 1 场
  rounds: 6020
- 7001-8000: bun 3443->3959, tswn 3447->3963, 累计差 +4->+4, 净变化 +0, 分叉 0 场
- 8001-9000: bun 3959->4443, tswn 3963->4450, 累计差 +4->+7, 净变化 +3, 分叉 3 场
  rounds: 8321, 8638, 8907
- 9001-10000: bun 4443->4934, tswn 4450->4941, 累计差 +7->+7, 净变化 +0, 分叉 0 场

## Jm0MGgK4HfUAAQ6MYo7rXGnlxeYB

- 时间: 2026/04/20 1776666086004 / rua！
- 旧消息显示: bun=42.05% / tswn(old)=42.08% / diff=+0.03
- 本次精确结果: bun=4205/10000 / tswn=4206/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-28512-1777657352160.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-28512-1777657352160.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-28512-1777657352160.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-28512-1777657352160.js:10661:16)

- 队 1:

```text
killer YFgziuYJGUOW93Ryni2X@czr2012
huk Fkua7Pj7AzG9CW4QZiGJ@czr2012
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 3
- 真实分叉 round: 4242, 7971, 8414
- 对应 seed: r4242=3359672, r7971=3363401, r8414=3363844

### 1000 场分段

- 1-1000: bun 0->400, tswn 0->400, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 400->825, tswn 400->825, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 825->1215, tswn 825->1215, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1215->1612, tswn 1215->1612, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 4001-5000: bun 1612->2049, tswn 1612->2050, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 4242
- 5001-6000: bun 2049->2504, tswn 2050->2505, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 6001-7000: bun 2504->2908, tswn 2505->2909, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 7001-8000: bun 2908->3351, tswn 2909->3351, 累计差 +1->+0, 净变化 -1, 分叉 1 场
  rounds: 7971
- 8001-9000: bun 3351->3767, tswn 3351->3768, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 8414
- 9001-10000: bun 3767->4205, tswn 3768->4206, 累计差 +1->+1, 净变化 +0, 分叉 0 场

## Jm0MGgK4HfUAAQ6V8AOp8GnlxmkB

- 时间: 2026/04/20 1776666217000 / rua！
- 旧消息显示: bun=39.71% / tswn(old)=39.68% / diff=-0.03
- 本次精确结果: bun=3971/10000 / tswn=3975/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-24804-1777657359598.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-24804-1777657359598.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-24804-1777657359598.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-24804-1777657359598.js:10661:16)

- 队 1:

```text
killer YFgziuYJGUOW93Ryni2X@czr2012
dust lmHylvLqY4hn0QIMnCia@czr2012
```
- 队 2:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 真实分叉 round 数: 6
- 真实分叉 round: 2476, 3805, 3816, 4299, 5080, 8825
- 对应 seed: r2476=3357906, r3805=3359235, r3816=3359246, r4299=3359729, r5080=3360510, r8825=3364255

### 1000 场分段

- 1-1000: bun 0->398, tswn 0->398, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 398->784, tswn 398->784, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 784->1196, tswn 784->1197, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 2476
- 3001-4000: bun 1196->1596, tswn 1197->1599, 累计差 +1->+3, 净变化 +2, 分叉 2 场
  rounds: 3805, 3816
- 4001-5000: bun 1596->2010, tswn 1599->2014, 累计差 +3->+4, 净变化 +1, 分叉 1 场
  rounds: 4299
- 5001-6000: bun 2010->2368, tswn 2014->2371, 累计差 +4->+3, 净变化 -1, 分叉 1 场
  rounds: 5080
- 6001-7000: bun 2368->2782, tswn 2371->2785, 累计差 +3->+3, 净变化 +0, 分叉 0 场
- 7001-8000: bun 2782->3189, tswn 2785->3192, 累计差 +3->+3, 净变化 +0, 分叉 0 场
- 8001-9000: bun 3189->3594, tswn 3192->3598, 累计差 +3->+4, 净变化 +1, 分叉 1 场
  rounds: 8825
- 9001-10000: bun 3594->3971, tswn 3598->3975, 累计差 +4->+4, 净变化 +0, 分叉 0 场

## QF9iBQK4HfUAE/HjYZR2WGnt5foB

- 时间: 2026/04/26 1777198586000 / shenjack的bot
- 旧消息显示: bun=42.87% / tswn(old)=42.88% / diff=+0.01
- 本次精确结果: bun=4287/10000 / tswn=4288/10000
- bun md5.js: D:\githubs\namer\fast-namerena\branch\latest\md5.js
- 队 1:

```text
随之任之 #kNPyDNhppy@🥒
```
- 队 2:

```text
Ⅹ q5H2HO5@新纪元
```
- 真实分叉 round 数: 3
- 真实分叉 round: 913, 4405, 5478
- 对应 seed: r913=3356343, r4405=3359835, r5478=3360908

### 1000 场分段

- 1-1000: bun 0->408, tswn 0->409, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 913
- 1001-2000: bun 408->854, tswn 409->855, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 2001-3000: bun 854->1295, tswn 855->1296, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1295->1719, tswn 1296->1720, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 4001-5000: bun 1719->2150, tswn 1720->2152, 累计差 +1->+2, 净变化 +1, 分叉 1 场
  rounds: 4405
- 5001-6000: bun 2150->2565, tswn 2152->2566, 累计差 +2->+1, 净变化 -1, 分叉 1 场
  rounds: 5478
- 6001-7000: bun 2565->3014, tswn 2566->3015, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 7001-8000: bun 3014->3445, tswn 3015->3446, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 8001-9000: bun 3445->3868, tswn 3446->3869, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 9001-10000: bun 3868->4287, tswn 3869->4288, 累计差 +1->+1, 净变化 +0, 分叉 0 场

## QF9iBQK4HfUAE/QeiKV6MGnu3OIB

- 时间: 2026/04/27 1777261794000 / shenjack的bot
- 旧消息显示: bun=57.30% / tswn(old)=57.31% / diff=+0.01
- 本次精确结果: bun=5730/10000 / tswn=5731/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-44972-1777657369600.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-44972-1777657369600.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-44972-1777657369600.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-44972-1777657369600.js:10661:16)

- 队 1:

```text
Cosmo #CLjFsRj@Arcadia
Wispy #6uttTRg@Arcadia
```
- 队 2:

```text
预言 yyDH8z1LkOOU@芒萁
硫 xL8rdIlimAPy@芒萁
```
- 真实分叉 round 数: 1
- 真实分叉 round: 2827
- 对应 seed: r2827=3358257

### 1000 场分段

- 1-1000: bun 0->563, tswn 0->563, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 1001-2000: bun 563->1167, tswn 563->1167, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 2001-3000: bun 1167->1751, tswn 1167->1752, 累计差 +0->+1, 净变化 +1, 分叉 1 场
  rounds: 2827
- 3001-4000: bun 1751->2330, tswn 1752->2331, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 4001-5000: bun 2330->2902, tswn 2331->2903, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 5001-6000: bun 2902->3428, tswn 2903->3429, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 6001-7000: bun 3428->4001, tswn 3429->4002, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 7001-8000: bun 4001->4600, tswn 4002->4601, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 8001-9000: bun 4600->5177, tswn 4601->5178, 累计差 +1->+1, 净变化 +0, 分叉 0 场
- 9001-10000: bun 5177->5730, tswn 5178->5731, 累计差 +1->+1, 净变化 +0, 分叉 0 场

## QF9iBQK4HfUAE/lWjx76DmnyAhYB

- 时间: 2026/04/29 1777467926000 / shenjack的bot
- 旧消息显示: bun=62.82% / tswn(old)=62.80% / diff=-0.02
- 本次精确结果: bun=6282/10000 / tswn=6279/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-55756-1777657374375.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-55756-1777657374375.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-55756-1777657374375.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-55756-1777657374375.js:10661:16)

- 队 1:

```text
Orbital #FdIt7oZLt@tyakasha
Orbital #uToMNkhhZ@tyakasha
```
- 队 2:

```text
镜湖TVWAFPFHB@涵虚
田柤钿坥畑租鉏组佃怚钿组@涵虚
```
- 真实分叉 round 数: 7
- 真实分叉 round: 66, 168, 389, 3187, 3843, 5234, 8228
- 对应 seed: r66=3355496, r168=3355598, r389=3355819, r3187=3358617, r3843=3359273, r5234=3360664, r8228=3363658

### 1000 场分段

- 1-1000: bun 0->630, tswn 0->629, 累计差 +0->-1, 净变化 -1, 分叉 3 场
  rounds: 66, 168, 389
- 1001-2000: bun 630->1260, tswn 629->1259, 累计差 -1->-1, 净变化 +0, 分叉 0 场
- 2001-3000: bun 1260->1871, tswn 1259->1870, 累计差 -1->-1, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1871->2503, tswn 1870->2502, 累计差 -1->-1, 净变化 +0, 分叉 2 场
  rounds: 3187, 3843
- 4001-5000: bun 2503->3140, tswn 2502->3139, 累计差 -1->-1, 净变化 +0, 分叉 0 场
- 5001-6000: bun 3140->3746, tswn 3139->3744, 累计差 -1->-2, 净变化 -1, 分叉 1 场
  rounds: 5234
- 6001-7000: bun 3746->4374, tswn 3744->4372, 累计差 -2->-2, 净变化 +0, 分叉 0 场
- 7001-8000: bun 4374->4989, tswn 4372->4987, 累计差 -2->-2, 净变化 +0, 分叉 0 场
- 8001-9000: bun 4989->5630, tswn 4987->5627, 累计差 -2->-3, 净变化 -1, 分叉 1 场
  rounds: 8228
- 9001-10000: bun 5630->6282, tswn 5627->6279, 累计差 -3->-3, 净变化 +0, 分叉 0 场

## QF9iBQK4HfUAE/lZ+dgrkGnyAkAB

- 时间: 2026/04/29 1777467968000 / shenjack的bot
- 旧消息显示: bun=45.63% / tswn(old)=45.62% / diff=-0.01
- 本次精确结果: bun=4563/10000 / tswn=4559/10000
- bun md5.js: D:\githubs\namer\fast-namerena\md5.js
- branch/latest 失败后 fallback: bun trace produced empty stdout for D:\githubs\namer\fast-namerena\branch\latest\md5.js: h\latest\.tswn-md5-trace-16676-1777657388192.js:10728:27)
    at _Future__propagateToListeners (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-16676-1777657388192.js:4008:103)
    at c2 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-16676-1777657388192.js:10586:11)
    at $0 (D:\githubs\namer\fast-namerena\branch\latest\.tswn-md5-trace-16676-1777657388192.js:10661:16)

- 队 1:

```text
H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
```
- 队 2:

```text
镜湖TVWAFPFHB@涵虚
田柤钿坥畑租鉏组佃怚钿组@涵虚
```
- 真实分叉 round 数: 10
- 真实分叉 round: 288, 1374, 5736, 6016, 7280, 7445, 7605, 8506, 8603, 9116
- 对应 seed: r288=3355718, r1374=3356804, r5736=3361166, r6016=3361446, r7280=3362710, r7445=3362875, r7605=3363035, r8506=3363936, r8603=3364033, r9116=3364546

### 1000 场分段

- 1-1000: bun 0->434, tswn 0->433, 累计差 +0->-1, 净变化 -1, 分叉 1 场
  rounds: 288
- 1001-2000: bun 434->897, tswn 433->897, 累计差 -1->+0, 净变化 +1, 分叉 1 场
  rounds: 1374
- 2001-3000: bun 897->1379, tswn 897->1379, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 3001-4000: bun 1379->1820, tswn 1379->1820, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 4001-5000: bun 1820->2278, tswn 1820->2278, 累计差 +0->+0, 净变化 +0, 分叉 0 场
- 5001-6000: bun 2278->2742, tswn 2278->2741, 累计差 +0->-1, 净变化 -1, 分叉 1 场
  rounds: 5736
- 6001-7000: bun 2742->3202, tswn 2741->3200, 累计差 -1->-2, 净变化 -1, 分叉 1 场
  rounds: 6016
- 7001-8000: bun 3202->3659, tswn 3200->3656, 累计差 -2->-3, 净变化 -1, 分叉 3 场
  rounds: 7280, 7445, 7605
- 8001-9000: bun 3659->4122, tswn 3656->4119, 累计差 -3->-3, 净变化 +0, 分叉 2 场
  rounds: 8506, 8603
- 9001-10000: bun 4122->4563, tswn 4119->4559, 累计差 -3->-4, 净变化 -1, 分叉 1 场
  rounds: 9116
