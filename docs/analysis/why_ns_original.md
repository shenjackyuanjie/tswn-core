# md5.js `why_ns` 变量分析（2026-05-17）

## 结论摘要

`why_ns` 是 `fast-namerena/md5.js` 顶层的一个全局玩家构造计数器。它会在每次 `Plr.a1()` 构造玩家时递增，在战斗胜利输出时归零，并在运行期把召唤物加入战斗列表时做一次回退。

最容易误判的一点是：`why_ns` 不是“只要非 0 就影响属性”。`md5.js` 中的 `C.JsInt.P(a, b)` 是整除，不是取余。因此当前代码的实际触发条件是：

```text
abs(why_ns_before_ctor) >= 2048
```

只有达到这个阈值时，`why_ns` 才会改变玩家个人 RC4 的初始排列，进而影响 `name_base`、技能顺序和属性。常规对战、`ProfileMain` 评分局、`!test!` benchmark 的单局玩家数量远小于 2048，所以在这些场景里 `why_ns` 通常只是计数，不会改变属性。

因此，当前 `!test!\n\n` / `!test!\n!\n\n` 分数不一致问题，不能简单归因于 `why_ns` 起始值不同。已有 probe 显示，ProfileMain 第一轮确实会因为预览目标而让真实对战玩家的构造序号后移，但这些序号仍远小于 2048，不会触发玩家 RC4 的交换分支。

换句话说，`why_ns` 能解释“为什么 benchmark 路径和普通 fight 路径的构造序号看起来不同”，但它不能解释常规 `!test!` 分数偏差。后续排查应优先确认 score 路径是否使用了 md5.js ProfileMain 的评分参数，例如 `$.vr = 6` / `WIN_RATE_EVAL_RQ = 6.0`，而不是把 `why_ns != 0` 当成属性差异根因。

## 相关代码位置

`md5.js` 中和 `why_ns` 直接相关的位置：

- 顶层定义：`fast-namerena/md5.js:669`
- 胜利 / reset：`fast-namerena/md5.js:17421`、`fast-namerena/md5.js:17549`
- 运行期加入成员时回退：`fast-namerena/md5.js:17712`
- 玩家构造时读取并递增：`fast-namerena/md5.js:17961`、`fast-namerena/md5.js:17962`
- 玩家构造时可能扰动 RC4：`fast-namerena/md5.js:17969`
- `C.JsInt.P` 是整除：`fast-namerena/md5.js:9218`

`tswn-core` 当前相关位置：

- 玩家构造与 `name_base`：`crates/tswn_core/src/player/impl_ctor.rs`
- Test1/Test2/TestEx 的 `name_base` 变换：`crates/tswn_core/src/player/impl_ctor.rs`
- runtime minion 构造入口：`Player::new_minion_and_init`

## 变量生命周期

### 1. 初始化

`why_ns` 在模块顶层初始化为 0：

```js
let why_ns = 0;
```

它不是每个 Engine 的字段，而是 `md5.js` 模块级全局变量。

### 2. 玩家构造时递增

在 `T.Plr.prototype.a1()` 内，玩家个人 RC4 用 team 初始化后，代码会读取 `why_ns` 并递增：

```js
q = why_ns;
why_ns += 1;
q = C.JsInt.P(Math.abs(q), 2048);
q = Math.abs(q) / 2048;
if (q > 0) {
  q = rc4.c;
  m = q[0];
  q[0] = q[1];
  q[1] = m;
}
rc4.dB(0, LangData.fZ(name), 2);
```

关键点：

- `why_ns` 读取发生在 `rc4.bd(team, 1)` 之后。
- 可能的 RC4 扰动发生在 `rc4.dB(name, 2)` 之前。
- 所以如果真的触发，它会影响后续 name 参与初始化后生成的 `name_base` 和技能洗牌。
- 但因为 `C.JsInt.P` 是整除，`why_ns = 1..2047` 时 `q` 会变成 0，不会触发交换。

### 3. 胜利时归零

Engine 输出胜利 update 时会执行：

```js
why_ns = 0;
```

这意味着正常一场 fight 结束后，下一场 fight 的初始 `why_ns` 会回到 0。

### 4. 运行期加入成员时回退

`T.Grp.prototype.aZ()` 把新成员加入 engine 的全局玩家列表时，如果该成员之前不在列表内，会执行：

```js
why_ns -= 1;
```

这主要影响 shadow / summon / zombie 这类运行期实体。它们构造时同样会让 `why_ns += 1`；随后 `aZ()` 再 `-= 1`，使全局计数不会因为运行期 minion 无限增长。

这个回退不会改变已经构造好的 minion 本身。它只是在 minion 加入 world 后，把全局计数退回构造前的水平。

## 实际算法

把 `md5.js` 的逻辑翻成伪代码，大致是：

```text
global why_ns = 0

function construct_player(name, team):
    rc4 = SuperRC4()
    rc4.seed(team, round = 1)

    slot = why_ns
    why_ns += 1

    gate = floor(abs(slot) / 2048)
    gate = abs(gate) / 2048

    if gate > 0:
        swap(rc4.state[0], rc4.state[1])

    rc4.mix(name, round = 2)
    build name_base / skills / attrs from rc4
```

由于 `gate > 0` 等价于 `abs(slot) >= 2048`，常见输入下不会进入 `swap` 分支。

## 已观测到的构造序号

对普通 raw 输入：

```text
aaaaaa
33554464@\u0002

33554465@\u0002
33554466@\u0002
```

probe 到的构造序号是：

```text
aaaaaa              q=0
33554464@\u0002     q=1
33554465@\u0002     q=2
33554466@\u0002     q=3
33554466?shadow     q=4
33554466?shadow     q=4
```

注意：这些 `q=1..4` 仍然不会触发 RC4 交换，因为没有达到 2048。

对 `ProfileMain` 评分输入 `!test!\n\naaaaaa`，第一轮前会先为了展示目标构造一次 `aaaaaa`：

```text
preview aaaaaa      q=0
round1 aaaaaa       q=1
round1 profile A    q=2
round1 profile B    q=3
round1 profile C    q=4
```

第一轮结束输出 winner 后 `why_ns = 0`，所以第二轮开始又会变成：

```text
round2 aaaaaa       q=0
round2 profile A    q=1
round2 profile B    q=2
round2 profile C    q=3
```

这解释了为什么 `ProfileMain` 路径的 `why_ns` 序号确实和普通 raw 首轮不同，但也解释了为什么这通常不改变属性：序号没有达到 2048。

## 对属性和 TestEx/Test1 的影响

如果 `why_ns` 触发了 RC4 交换，它发生在 `name_base` 生成之前。因此它会间接影响：

- `name_base`
- `raw_name_base`
- 技能顺序 / 技能等级来源
- 八围属性
- 依赖 `name_base` 的技能参数

对 `@!` / `@\u0002` / `@\u0003` 这类特殊玩家也一样：先生成基础 `name_base`，再做 TestEx/Test1/Test2 的特殊变换。因此只有当 `why_ns` 已经触发 RC4 交换时，特殊玩家最终属性才会跟着变化。

在当前 benchmark 的常规规模下，`why_ns` 没达到阈值，所以它不是 `@!` 或 `@\u0002` 属性不一致的主要嫌疑点。已经单独验证过：

- `33554431@\u0002` 的 Test1 属性与 md5.js 对齐。
- `33554431@!` 的 TestEx 属性与 md5.js 对齐。

### 与 `textEx` / `type` 的关系

`textEx` 更接近“玩家文本被解析出来后属于哪一种特殊类型”的结果，不是 `why_ns` 本身控制的状态。两者的先后关系可以理解为：

```text
raw name/team/weapon text
    -> 解析出普通玩家、TestEx、Test1、Test2 等类型
    -> 用 team 初始化玩家个人 RC4
    -> 读取 why_ns，并在达到 2048 阈值时扰动 RC4
    -> 用 name 混入 RC4，生成基础 name_base
    -> 按 textEx / Test1 / Test2 规则改写 name_base
    -> 由最终 name_base 派生属性、技能和部分技能参数
```

所以如果 `player.textEx`、特殊 team 标记或 TestEx/Test1 分支本身和 md5.js 对不上，属性当然会错；但那是类型解析或特殊变换的问题，不是 `why_ns` 的问题。`why_ns` 只在 `name_base` 生成前提供一个极低频的 RC4 扰动入口。

## 对 minion 的影响

shadow / summon / zombie 的构造有两层容易混在一起的问题：

1. 它们会走 `Plr.a1()`，所以也会读取并递增 `why_ns`。
2. 它们不是 roster 里的普通 `@!` / `@\u0002` 玩家，不应该因为 owner 的 team 是 `!` 或 `\u0002` 就套用 TestEx/Test1 的 `name_base` 变换。

第二点是 tswn-core 已经修过的 minion 构造问题：`Player::new_minion_and_init()` 强制 minion 走普通类型，避免 shadow/summon/zombie 被误当成 TestEx/Test1。

第一点则是 `why_ns` 问题。按 md5.js 的现有代码，minion 构造序号通常等于初始 roster 的构造数量，例如 4 人局里 minion 常见 `q=4`。但因为 `q=4` 不到 2048，所以仍然不会触发 RC4 交换。`Grp.aZ()` 的 `why_ns -= 1` 主要是防止长局里 minion 反复生成导致全局计数持续增长。

## 对 benchmark 的影响

`why_ns` 会让 ProfileMain 与普通 raw 路径在“构造序号”上出现差异，特别是：

- ProfileMain 会先调用 `dZ()` 构造展示用目标。
- 第一轮评分局的真实玩家构造序号会因此后移。
- 每场战斗胜利输出后又会 reset 到 0。

但在当前 `!test!` benchmark 的规模里，这些序号差异没有达到 2048，不会改变属性。因此：

- `why_ns` 可以解释“构造序号为什么不同”。
- `why_ns` 不能解释“为什么常规 `!test!` score 和 md5.js ProfileMain score 不同”。
- 对 score benchmark 来说，更关键的是复刻 ProfileMain 的 score 构造语义，包括目标玩家预览、profile 种子生成、以及评分参数 `WIN_RATE_EVAL_RQ = 6.0`。

这里要特别区分两种现象：

```text
现象 A：ProfileMain 第一轮真实玩家 q 从 1 开始
原因：预览目标先构造了一次，why_ns 被加到 1
影响：q < 2048，不改属性

现象 B：score benchmark 中同名玩家属性和普通 raw fight 不同
原因：更可能是 eval_rq / ProfileMain score 参数不同
影响：会真实改变属性，进而改变胜负和总分
```

## 对 tswn-core 的实现建议

如果只是修当前常规 `!test!` benchmark，不应该把 `why_ns != 0` 简化成“交换 RC4”。这是错误的，会把绝大多数玩家属性改坏。

如果要完整兼容 md5.js 的边界行为，建议这样建模：

1. 引入一个 Engine / Runner 级别的构造计数器，避免全局 static，保证并行 benchmark 线程安全。
2. 玩家构造接口接收 `constructor_slot`，按 md5.js 的阈值逻辑判断是否交换玩家个人 RC4 的 `state[0]` / `state[1]`。
3. 胜利输出时把计数器 reset 为 0。
4. runtime minion 构造后，在加入 world 时回退一次计数器，复刻 `Grp.aZ()`。
5. prepared runner 缓存如果要覆盖 `why_ns >= 2048` 的边界场景，需要把构造序号纳入 cache key，或者只缓存不受 `why_ns` 影响的场景。

当前更现实的判断是：在 namerena 常规人数限制和 `ProfileMain` 评分逻辑下，`why_ns` 基本是一个兼容边界行为的计数器，而不是这次分数偏差的根因。
