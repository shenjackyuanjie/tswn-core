# `why_ns` 复盘：修复完成后的当前结论（2026-05-18）

这版文档直接基于**当前仓库代码**重写，不再沿用上一版“`tswn-core` 还没修到这里”的判断。

## TL;DR

先说结论：

1. **`md5.js` 里的 `why_ns` 机制本身没有变。**
   它仍然是：
   - `Plr.a1()` 构造时读取并 `+= 1`
   - `Grp.aZ()` 首次把成员挂进 roster 时 `-= 1`
   - 胜利输出时 reset 为 `0`
   - `ProfileMain.dZ()` / `ProfileWinChance.dY()` 的预构造会先吃掉一轮 `a1()`

2. **但检查当前 `tswn-core` 后，这次 profile / `namer-pf` 对齐并不是通过“在 Rust 里显式实现一个 `why_ns` 计数器”完成的。**
   `crates/tswn_core/src/player/impl_ctor.rs` 当前仍然是：

   ```rust
   let mut rand = RC4::new(&team_bytes, 1);
   rand.update(&name_bytes, 2);
   ```

   也就是说：**现在 Rust 里仍然没有显式的 `why_ns` / `constructor_slot` 建模。**

3. **当前真正修掉的，是几处之前很容易被误判成 `why_ns` 的 JS 语义差异。**
   这次对齐主要落在下面几块：
   - `score` / `raw score` / `namer-pf` 的**计分语义**：按“目标队任一赢家”计胜，不是只看 `winner[0]`
   - JS `ProfileMain` 的**输入形状**：单目标时是 `1 vs 3`，重复双名时会 collapse 成 `1 vs 1`
   - JS `PlrEx.cA()` 是 **no-op**：`@!` TestEx 不应做同队 upgrade
   - JS `init_PlrClone()` 的 `raw_name_base` 保持**普通构造底板**，不能直接沿用 owner 的变换后底板
   - JS `SklSummon.v()` 重复施放会复用旧 summon，并重新跑 `bP()/bs()/cn()`
   - `Iron` 在 merged `post_defend` 链里被击破时，状态必须**当场清掉**，不能拖到链路结束后一起清

4. **所以当前更准确的判断是：`why_ns` 仍然是 md5.js 里的真实机制，也确实会在 profile 路径里碰到；但这次剩余 diff 的根因，已经被证明主要不是“Rust 还缺一个 why_ns 计数器”。**

5. **如果以后又出现新的 profile 边角 case，再考虑显式实现 `why_ns`。**
   但应该把它当成“边界兼容层”，而不是继续把所有 profile 分歧先归罪给它。

---

## 1. 仍然成立的 `md5.js` 侧结论

这一部分和旧文档相比，核心判断没变，只是现在要更明确地说：

> `why_ns` 是真的存在，也真的会被 profile 路径碰到；只是它不是这次最终修复的主战场。

### 1.1 `why_ns` 的生命周期

`md5.js` 里和 `why_ns` 直接相关的点仍然是：

- 顶层定义：`md5.js`
- `T.Plr.prototype.a1()`：读取当前值并递增
- `T.Grp.prototype.aZ()`：首次挂进 roster 时减回去
- `T.Engine.prototype.O()` / `bE()`：winner 输出时 reset
- `V.ProfileMain.prototype.dZ()` / `L.ProfileWinChance.prototype.dY()`：profile 开打前先 `choose_boss()` 预构造

### 1.2 `why_ns` 在 `Plr.a1()` 里怎么参与

当前 `md5.js` 的逻辑仍然等价于：

```text
slot = why_ns
why_ns += 1
if abs(slot) >= 2048:
    swap(player_rc4.state[0], player_rc4.state[1])
rc4.mix(name)
```

也就是说：

- `why_ns` **不是**“非 0 就生效”
- 当前门控仍然是：

```text
abs(slot_before_ctor) >= 2048
```

### 1.3 为什么 profile 确实会碰到 `why_ns`

因为：

- `ProfileMain.dZ()` 会先 `choose_boss()` 预构造展示/计分用玩家
- `ProfileWinChance.dY()` 也会做同类预构造
- 这些预构造会走 `Plr.a1()`
- 但它们**不会**立刻走 `Grp.aZ()` 退款

所以 profile 路径确实会改动第一场真实 fight 的构造起点。

### 1.4 但这次修复并没有在 Rust 里显式复刻它

这是这版文档最重要的更新点。

虽然 `md5.js` 有 `why_ns`，但当前 `tswn-core` 里：

- 没有 `why_ns` 字段
- 没有 `constructor_slot` 字段
- 没有在 `RC4::new(team, 1)` 与 `update(name, 2)` 之间插任何 slot 扰动

所以现在不能再写成：

> “当前 tswn 的对齐修复 = 已经把 why_ns 实现进 Rust。”

这不符合当前代码。

---

## 2. 当前代码真正修掉了什么

如果把当前改动按“真正影响 profile / `namer-pf` 结果”的角度梳理，重点是下面 5 类。

---

### 2.1 `score` / `raw score` / `namer-pf` 的胜负统计语义修正了

这次一个很关键的改动在：

- `crates/tswn_core/src/bin/tswn_cli/bench.rs`
- `crates/tswn_core/src/bin/tswn_cli/fight.rs`

当前逻辑已经从：

```rust
winners.first().is_some_and(|winner| team0_targets.contains(winner))
```

改成：

```rust
winners.iter().any(|winner| target_team.contains(winner))
```

这意味着：

> 只要目标队中有任一成员属于赢家集合，这一场就算目标队赢。

这是更符合 JS / profile 语义的做法。

#### 为什么它重要

在多人组、多人同胜、召唤物/克隆也在 winner 集合里的场景里：

- 只看 `winner[0]` 会把一些本应算胜的局错算成负
- 症状会表现成“某一场翻转”，非常像 profile 内部构造序号差一位导致的结果偏移

但现在代码已经说明：

> 这类分歧的根因之一，其实是**计分语义**，不是 `why_ns`。

#### 对应到 `namer-pf`

当前还新增了：

- `crates/tswn_core/src/bin/tswn_cli.rs`
- `crates/tswn_core/src/bin/tswn_cli/args.rs`
- `crates/tswn_core/src/bin/tswn_cli/bench.rs`

里的 `namer-pf` 命令，直接按 plugin 的四项评分语义跑：

- `pp`
- `pd`
- `qp`
- `qd`

并且支持每行一个组、组内 `+` 分隔的输入形式。

---

### 2.2 JS `ProfileMain` 的 score 输入形状已经明确对齐

这部分在：

- `crates/tswn_core/src/bin/tswn_cli/bench.rs`
- `crates/tswn_core/src/bin/tswn_cli/fight.rs`

的 `build_js_score_match_input()` 里。

当前逻辑明确对齐了 JS 的两条关键规则：

#### 单目标：`1 vs 3`

单目标组时，输入会构造成：

```text
target
profile_base@modifier

profile_base+1@modifier
profile_base+2@modifier
```

也就是：

```text
1 个目标 vs 3 个 profile 靶子
```

#### 重复双名：collapse 成 `1 vs 1`

当目标组是：

```text
[name, name]
```

时，JS 会把它 collapse 成单目标语义。Rust 当前也已经显式复刻。

#### 为什么这块也曾经像 `why_ns`

因为 profile 的输入 shape 一旦错：

- 每轮参与 fight 的人数会错
- `PROFILE_START + round * profile_count` 的推进也会错
- 最终会表现成 profile 某一场结果翻转

这类症状表面上也很像“某个 constructor slot 偏了一点”，但当前代码表明：

> 输入 shape 本身也是一类真实根因，而且现在已经被单测钉住了。

---

### 2.3 `PlrEx.cA()` 在 JS 是 no-op，`@!` TestEx 不应做同队 upgrade

这部分当前修在：

- `crates/tswn_core/src/player/impl_attr.rs`

`upgrade()` 现在会对 `PlayerType::TestEx` 直接 return。

这对应的是 `md5.js` 里：

- `T.PlrEx.prototype.cA(a) { }`

也就是：

> JS 的 `@!` TestEx 玩家不会参与普通同队 upgrade 逻辑。

#### 为什么它会和 `why_ns` 混淆

因为 `upgrade()` 会改 `name_base` 的后半段，继而影响：

- 属性
- 技能等级
- 后续战斗结果

而这些又正好是很多人直觉上会先归因给 `why_ns` 的部分。

但这次代码已经说明：

> 至少有一部分过去看起来像“构造阶段随机底板偏了”的现象，实际是 TestEx 错误吃了同队 upgrade。

---

### 2.4 `init_PlrClone()` 的 `raw_name_base` 语义补对了

这部分当前修在：

- `crates/tswn_core/src/player/impl_ctor.rs`
  - `Player::normal_raw_name_base()`
- `crates/tswn_core/src/player/skill/act/clone.rs`

当前 clone 逻辑会显式做：

```rust
cloned.raw_name_base = Player::normal_raw_name_base(Some(owner_clan_name.as_str()), owner_base_name.as_str());
```

这对应 JS 的 `init_PlrClone()` 语义：

- 先跑一遍普通 `PlrClone` 构造
- 再把 clone 的 `name_base` / 变换底板改成 owner 的那套
- 但 `raw_name_base` 保留普通构造出来的原始底板

#### 为什么它重要

`raw_name_base` 会继续影响：

- upgrade / merge 等后续逻辑
- boost 判定
- 某些技能等级与 proc 行为

所以 clone 这一层如果错了，也会表现成“某一局 profile 特别像 constructor order 出了偏差”。

但当前代码已经把它单独补齐了。

---

### 2.5 `SklSummon.v()` 的 reuse 语义补对了

这部分当前修在：

- `crates/tswn_core/src/player/skill/act/summon.rs`

当前代码在 summon 再次施放、复用旧 summon 且该 summon 已死亡时，会：

- `summoned.state = PlayerStateStore::default()`
- 重新挂回 `MinionRuntimeState { owner, kind: Summon }`
- 重新处理 share-damage skill / proc 队列
- `init_values()`
- `queue_revival(summoned_id)`

这正对应 JS `T.SklSummon.v()` 的 reuse 分支：

```js
s.bP();
s.bs();
s.cn();
```

Rust 里的注释也已经写明了：

> JS 首次构造之后，后续重复施放复用的是同一个死掉的 summon 对象，并重新跑 `bP()/bs()/cn()`。

#### 为什么它会被误判成 `why_ns`

因为 summon reuse 同时具备两种“很像 why_ns”的特征：

1. JS 这条路径**不新构造对象**
2. 但它又会把旧对象重新挂回场上

这和 `Grp.aZ()` 的“只退款、不重新构造”的生命周期非常像，所以很容易让人把结果偏差直接联想到 `why_ns`。

但当前代码表明：

> 这里真正需要补的是**对象复用后的 runtime 状态重置**，不是在 Rust 里平地加一个 `why_ns` 计数器。

---

### 2.6 `Iron` 在 merged `post_defend` 链里要立即清状态

这部分当前修在：

- `crates/tswn_core/src/player/impl_runtime.rs`
- `crates/tswn_core/src/player/state.rs`
- 相关状态定义：`crates/tswn_core/src/player/skill/act/iron.rs`

当前 `post_defend` 已经明确按 JS 的共享 `y2` 链模型做：

- skill 和 state 统一 merge 到一条优先级链里
- 某个 state 在 `run_one_post_defend(...)` 返回需要清除后，**立即清 tag**
- 若该 tag 会改状态面板，就**立即 `update_states()`**

而不是像旧逻辑那样：

- 先把整条链跑完
- 最后统一清理

#### 为什么它会影响 profile 胜负

`Iron` 被击破时：

- 防御状态是否还“残留半拍”
- 后续同一链上的其它 skill/state 是否看到它还存在

都可能直接改变该次 damage resolve 的结果。

所以它也会表现成：

> “某一场 benchmark fight 的胜负翻转了。”

这类现象和之前用户描述的 profile/namer-pf 差 1 的症状完全同型，因此也很容易被先怀疑成 `why_ns`。

但当前代码已经说明：

> 这里真正的根因是 `post_defend` 清理时机，而不是 `why_ns`。

---

## 3. 为什么这些问题之前会被看成 `why_ns`

这是这次复盘里最值得记住的一点。

`why_ns` 容易成为“兜底背锅位”，是因为它确实满足 3 个条件：

1. 它只在 `md5.js` 里有，Rust 之前没显式实现
2. 它确实和 profile 预构造、spawn/reuse 生命周期有关
3. 它理论上又会影响 `name_base` 这种非常底层的随机底板

所以一旦出现下面这些症状：

- `profile` 某一轮翻转
- `namer-pf` 总分差 1
- clone / summon / TestEx / state clear 的边缘场景不对

很自然就会先怀疑：

```text
是不是 why_ns 起点不对？
```

但当前代码修完后，实际给出的答案更接近：

```text
这些症状很多都发生在“构造 / 复用 / 计分 / 后处理”边界上；
它们看起来像 why_ns，但真正缺的是更具体的 JS 语义。
```

换句话说：

> `why_ns` 是一个正确的怀疑方向，但不是一个足够具体的 root cause。

---

## 4. 现在应该怎么理解 `why_ns`

### 4.1 还要不要继续研究它？

要，但定位要变。

当前更合理的定位是：

> `why_ns` 是 md5.js 里一个真实存在、需要理解的 profile/runtime 生命周期机制；
> 但在当前仓库已经补完的这些分歧里，它更像“帮助解释为什么当初会怀疑到这里”，而不是“这次最终修复直接落下去的代码点”。

### 4.2 当前还不能说“Rust 已实现 why_ns”

因为从当前代码看，Rust 里仍然没有：

- `why_ns`
- `constructor_slot`
- `profile preview constructor offset`
- `aZ()` 退款计数器

所以文档里不能再写成：

> “现在 `tswn-core` 已经完整实现了 `why_ns`。”

这和当前代码不符。

### 4.3 当前更准确的说法

当前更准确的说法应该是：

- `md5.js` 的 `why_ns` 机制仍然存在
- `ProfileMain` / `ProfileWinChance` 仍然会在预构造阶段碰到它
- 但当前 `tswn-core` 之所以已经能把这批 profile / `namer-pf` 分歧修掉，主要靠的是：
  - score 输入形状对齐
  - 计分语义对齐
  - TestEx upgrade 对齐
  - clone 原始底板对齐
  - summon reuse 生命周期对齐
  - post_defend 状态清理时机对齐

### 4.4 什么时候才需要真的在 Rust 里补它？

只有当后面还剩下这种 case 时，才应该重新把它提到最高优先级：

- profile 预构造 + 真实 fight 起点偏移仍能稳定复现 diff
- 上述修复都已经对齐，但某些局仍只在“constructor lifecycle”层面解释得通
- 明确出现和 `ProfileMain.dZ()` / `Grp.aZ()` / winner reset 有关、而无法被现有 score/clone/summon/state 逻辑解释的边界场景

如果真的要补，正确方向仍然是：

- 不要复用 `player_id_counter`
- 要同时建模：
  - `ProfileMain.dZ()` / `ProfileWinChance.dY()` 的预构造
  - `Grp.aZ()` 的 fresh-spawn / re-add 退款
  - winner reset
  - `PreparedRunner` 缓存上下文

但这是**以后可能需要的兼容层**，不是这次已经落地的修复点。

---

## 5. 当前仓库中最值得看的文件

如果要理解“现在到底是怎么修好的”，推荐按这个顺序读：

1. `crates/tswn_core/src/bin/tswn_cli/bench.rs`
   - `build_js_score_match_input()`
   - `run_bench_score_range()`
   - `run_bench_score_worker()`
   - `run_namer_pf()`
2. `crates/tswn_core/src/bin/tswn_cli/fight.rs`
   - `build_js_score_match_input()`
   - `run_raw_score_range()`
   - `run_raw_score_worker()`
3. `crates/tswn_core/src/player/impl_attr.rs`
   - `upgrade()` 里 `TestEx` 的 early return
4. `crates/tswn_core/src/player/skill/act/clone.rs`
   - `raw_name_base = Player::normal_raw_name_base(...)`
5. `crates/tswn_core/src/player/impl_ctor.rs`
   - `normal_raw_name_base()`
   - 顺手确认：当前依然没有显式 `why_ns` 注入
6. `crates/tswn_core/src/player/skill/act/summon.rs`
   - reuse dead summon 的 `queue_revival()` 分支
7. `crates/tswn_core/src/player/impl_runtime.rs`
   - merged `post_defend` 链里的即时 clear
8. `crates/tswn_core/src/player/state.rs`
   - `on_post_defend_states()` 改成返回待清 tag 列表
9. `crates/tswn_core/src/player/skill/act/iron.rs`
   - `IronState::on_post_defend()`

---

## 6. 这版文档替代旧版后的新结论

把旧版一句话浓缩成最新版，可以写成：

> `why_ns` 在 md5.js/profile 里当然是真实存在的，但当前 tswn/profile/namer-pf 对齐的关键，不是“Rust 终于也有了 why_ns”，而是把几处原先看起来像 why_ns 的 JS 生命周期语义逐个补齐了。

如果只想记最短版，就记下面这 5 句：

1. `why_ns` 仍然是 md5.js 真机制，profile 也确实会碰到它。
2. 当前 Rust 里**仍没有显式 `why_ns` 实现**。
3. 这次真正修好的，是 score 计分、TestEx upgrade、clone 原始底板、summon reuse、Iron 清理时机。
4. 这些问题之所以难排，是因为它们全都长得很像 `why_ns`。
5. 所以现在别再把“profile 还差一点”自动翻译成“肯定是 why_ns 没实现”。
