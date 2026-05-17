# `why_ns` 在 `tswn-core` 里的简单分析（2026-05-18）

## TL;DR

先说最重要的结论：

- `tswn-core` 现在**没有实现** `why_ns`，也没有等价的“玩家构造序号”状态。
- 当前玩家生成逻辑的核心在 `crates/tswn_core/src/player/impl_ctor.rs`：
  先按 `team` 初始化 RC4，再混入 `name`，生成 `name_base`，然后按 `Test1 / Test2 / TestEx` 做特殊改写。
- `name_base` 生成完以后，`build()` 才会在 `crates/tswn_core/src/player/impl_attr.rs` 里把它展开成八围、技能等级和部分技能参数。
- 所以从 `tswn` 侧排查问题时，优先看的是：
  1. `PlayerType` 判定对不对；
  2. `name_base` 的特殊变换对不对；
  3. runtime minion 有没有误走特殊类型；
  4. score / benchmark 路径是不是用了正确的输入形状和 `eval_rq`。
- 如果以后要补 `why_ns`，正确插点应该在 `RC4::new(&team_bytes, 1)` 和 `rand.update(&name_bytes, 2)` 之间，**不是**在 `build()` 后面打补丁。

---

## 1. 先把 `why_ns` 说人话

对只看 `tswn-core` 的读者，可以把 `why_ns` 理解成一句话：

> “这一局里，第几个被构造出来的玩家/召唤物。”

它如果要影响结果，本质上影响的是：

- 玩家个人 RC4 的初始状态；
- 然后连带影响 `name_base`；
- 再继续影响技能顺序、八围、部分技能参数。

换句话说，`why_ns` 不是一个“战斗中途的 buff/debuff”，而是一个**构造阶段**的兼容细节。

---

## 2. `tswn-core` 现在的玩家构造流程

现在 `tswn-core` 的主要流程其实很直白：

```text
Runner::new_from_namerena_raw
  -> Runner::split_namerena_into_groups
  -> Player::new_from_namerena_raw
  -> Player::new_and_init_inner
       1. 判定 PlayerType
       2. 用 team 初始化 RC4
       3. 用 name 更新 RC4
       4. 生成 name_base / raw_name_base
       5. 按 Test1 / Test2 / TestEx 改写 name_base
       6. 洗技能顺序，计算 name_factor
  -> Player::build
       7. 用 name_base 计算属性、技能等级、部分技能参数
```

最关键的代码点：

- 入口：`crates/tswn_core/src/engine/runners.rs`
- 玩家构造：`crates/tswn_core/src/player/impl_ctor.rs`
- 属性/技能展开：`crates/tswn_core/src/player/impl_attr.rs`

这也是为什么排查时，通常先盯这三处最有效。

---

## 3. 当前代码里“像计数器的东西”，并不是 `why_ns`

`tswn-core` 里最容易让人误会的是 `Storage.player_id_counter`（见 `crates/tswn_core/src/engine/storage.rs`）。

但它**不是** `why_ns`，原因很简单：

- 它只是给实体分配运行期 `id`；
- `Player::new_and_init_inner()` 里，`id = storage.new_plr_id()` 发生在 `name_base` 生成之后；
- 也就是说，它根本不参与 RC4 初始化，更不会影响 `name_base`；
- `PreparedRunner` 甚至还会提前预分配一批 `id`，这和“实际第几个被构造的玩家”更不是一回事。

所以：

> `player_id_counter` 是运行期实体编号，不是 `why_ns` 的替代品。

---

## 4. `tswn-core` 已经处理好的 minion 关键点

这部分比 `why_ns` 更值得读者优先理解。

runtime minion（`shadow / summon / zombie`）不是从输入 roster 里直接来的，而是在技能执行时动态创建的：

- `shadow`：`crates/tswn_core/src/player/skill/act/shadow.rs`
- `summon`：`crates/tswn_core/src/player/skill/act/summon.rs`
- `zombie`：`crates/tswn_core/src/player/skill/skl/zombie.rs`

它们都会走：

- `Player::new_minion_and_init(...)`

而这个入口会进入：

- `Player::new_and_init_inner(..., force_normal_type = true)`

意思很明确：

> minion 在生成 `name_base` 的阶段，先强制按普通玩家处理。

这样做的作用是：

- owner 就算是 `@!`、`@\u0002`、`@\u0003`；
- 新生成的 minion 也**不会**误走 `TestEx / Test1 / Test2` 的特殊 `name_base` 变换。

之后，技能代码再补上 minion 自己的运行期身份：

- 显示名（幻影 / 使魔 / 丧尸）
- 血量修正
- 技能修正
- `MinionRuntimeState`
- 入队 `pending_spawns`，再由 `EngineCore::sync_runtime_entities()` 挂进 world

这部分已经是一个很重要的 `tswn` 侧兼容点，而且比 `why_ns` 更容易在日常对局里造成真实差异。

---

## 5. 为什么当前排查不该优先怀疑 `why_ns`

原因其实很朴素：

### 5.1 `tswn-core` 里现在根本没有它

在 `crates/tswn_core/src` 下全局搜索，找不到 `why_ns`，也找不到现成的 `constructor_slot` 一类实现。

也就是说，当前 `tswn-core` 不存在“某个隐藏的 `why_ns` 把属性改坏了”这种情况。

### 5.2 当前更可能出问题的是这些地方

如果是对局结果或 score 对不上，`tswn` 侧更应该先看：

1. `PlayerType` 判定有没有偏；
2. `Test1 / Test2 / TestEx` 的 `name_base` 变换有没有偏；
3. minion 是否被误当成特殊玩家；
4. score / benchmark 的输入构造是否真的复刻了目标流程；
5. `eval_rq` 是否用了对的值（尤其是 win-rate / score 路径）。

### 5.3 就算以后补了，也不是“非 0 就生效”

这个点最好也顺手记住：

> `why_ns` 不是“只要不为 0 就会改属性”的东西。

它属于低频兼容细节，不是一个“任何普通小对局都会触发”的开关。

所以在常规规模的 score 偏差排查里，优先级通常不会太高。

---

## 6. 如果以后要在 `tswn-core` 里补 `why_ns`，应该怎么放

从 `tswn` 结构来看，比较合理的落点是：

### 6.1 不要做成全局 static

`tswn-core` 的对局状态天然是按 `Runner` / `Storage` 隔离的。

所以如果以后要补，应该挂在：

- `Runner`
- `Storage`
- 或一个专门的“构造上下文”

总之应该是**单局内状态**，不是进程级全局变量。

### 6.2 真正的插点在 `impl_ctor.rs`

最自然的位置就是：

```rust
let mut rand = RC4::new(&team_bytes, 1);
// 这里才是 why_ns 应该插入的位置
rand.update(&name_bytes, 2);
```

原因很简单：

- 太早插，拿不到玩家自己的 team 初始化结果；
- 太晚插，`name_base` 都已经生成完了，补也补不回来了。

### 6.3 不要复用 `player_id_counter`

前面已经说过，那个计数器只负责发实体 `id`，语义完全不同。

### 6.4 minion 也要算进去，但处理点要谨慎

如果要继续贴近原逻辑，那么 runtime minion：

- 构造时也会占一个“构造序号”；
- 但在真正挂进 world 以后，可能还需要做一次回退/补偿。

在 `tswn-core` 里，这类逻辑最可能要放在：

- `Storage::queue_spawn()` 周边；
- 或 `EngineCore::sync_runtime_entities()` 把 `pending_spawns` 落地的时候。

### 6.5 `PreparedRunner` 也要重新审视

现在 `PreparedRunner` 会缓存“已经构造并 build 好的玩家模板”。

如果未来 `why_ns` 真的能影响玩家构造结果，那么缓存键就不能只看：

- `players`
- `eval_rq`

还得考虑构造序号相关信息；否则就会把本来应该不同的玩家模板错误复用。

---

## 7. 给读者的最短结论

如果你现在只是想判断“该从哪排查”，可以直接记这几句：

1. `tswn-core` 当前**没有实现** `why_ns`。
2. 当前玩家生成的核心代码在 `crates/tswn_core/src/player/impl_ctor.rs`。
3. 当前更重要的兼容点，是 `PlayerType` 判定、`name_base` 特殊变换、以及 minion 强制走普通类型。
4. 如果是 score / benchmark 对不上，先查 score 路径本身，不要先把锅甩给 `why_ns`。
5. 如果以后要补 `why_ns`，应当在 RC4 的 `team -> name` 中间插入，而不是后处理。

---

## 建议的阅读顺序

如果读者只想快速看代码，建议按这个顺序：

1. `crates/tswn_core/src/player/impl_ctor.rs`
2. `crates/tswn_core/src/player/impl_attr.rs`
3. `crates/tswn_core/src/player/skill/act/minion.rs`
4. `crates/tswn_core/src/player/skill/act/shadow.rs`
5. `crates/tswn_core/src/player/skill/act/summon.rs`
6. `crates/tswn_core/src/player/skill/skl/zombie.rs`
7. `crates/tswn_core/src/engine/storage.rs`
8. `crates/tswn_core/src/engine/engine_core.rs`

这样读，基本不需要先懂 `md5.js` 的细节，也能把 `tswn` 这一侧的问题看明白。
