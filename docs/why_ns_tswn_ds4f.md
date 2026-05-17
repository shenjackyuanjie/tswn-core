
## `why_ns` 在 `tswn-core` 里的实际情况

### 一句话结论

`tswn-core` 没有实现 `why_ns`，当前也不需要它来排查问题。

`why_ns` 这个概念指的是"某一局里第几个被构造出来的玩家/召唤物"，它会影响玩家 RC4 的初始状态，进而影响 `name_base`、技能顺序、八围等一切由 RC4 驱动的结果。但目前代码里根本没这东西。

---

### 当前玩家构造流程

所有玩家（正常玩家、minion）最终都收敛到这个函数：

**`crates/tswn_core/src/player/impl_ctor.rs` → `Player::new_and_init_inner()`**

流程如下：

```
1. 校验队名/玩家名长度
2. 确定 PlayerType（Normal / Test1 / Test2 / TestEx / Boss / Boost / Seed）
3. RC4::new(&team_bytes, 1)   ← team 初始化 RC4，skip=1
4. rand.update(&name_bytes, 2) ← name 混入，skip=2
5. 从 RC4 生成 name_base（128 bytes）
6. Test1/Test2/TestEx 特殊改写 name_base
7. RC4 洗牌技能顺序（40个技能）
8. 计算 name_factor
9. storage.new_plr_id()        ← 分配运行期 ID（此时 name_base 已确定）
10. 解析武器，创建武器状态
```

关键点：**`new_plr_id()` 在第 9 步才调用**，而此时 `name_base` 已经生成完毕。所以 `player_id_counter` 不影响任何玩家属性，跟 `why_ns` 完全不是一回事。

---

### `player_id_counter` 不是 `why_ns`

`Storage.player_id_counter`（`crates/tswn_core/src/engine/storage.rs:95`）是 `AtomicU64`，纯粹用于分配运行期实体 ID。它的递增时机：

- 普通玩家：`new_and_init_inner()` 末尾（`name_base` 已确定之后）
- minion：同上，走 `new_minion_and_init()`
- `PreparedRunner` 预分配：`new_from_prepared_groups_with_seed()` 开头会预先调用 `new_plr_id()` 多次（仅为了对齐 JS 的 ID 分配顺序，不影响构造结果）

所以不管怎么折腾这个计数器，它都改不了玩家的 `name_base`、属性、技能顺序。

---

### Minion 的处理方式（比 `why_ns` 更值得关注）

三个 runtime minion 的创建入口：

| minion | 代码路径 | 构造入口 |
|--------|----------|----------|
| 幻影 (shadow) | `crates/tswn_core/src/player/skill/act/shadow.rs` | `Player::new_minion_and_init()` |
| 使魔 (summon) | `crates/tswn_core/src/player/skill/act/summon.rs` | `Player::new_minion_and_init()` |
| 丧尸 (zombie) | `crates/tswn_core/src/player/skill/skl/zombie.rs` | `Player::new_minion_and_init()` |

三者都走 `new_and_init_inner(..., force_normal_type = true)`，强制按 `PlayerType::Normal` 生成 `name_base`。然后各自在外部覆写：

```rust
// 以 shadow 为例（shadow.rs:71-88）：
prepare_combat_minion(&mut shadow);
shadow.build();
shadow.attr[7] /= 2;
shadow.init_values();
shadow.set_display_name_override(Some("幻影".to_string()));
shadow.player_type = PlayerType::Clone;
shadow.set_state(MinionRuntimeState { owner, kind: MinionKind::Shadow });
```

这意味着 owner 就算队名是 `!` / `\u0002` / `\u0003`，minion 也**不会**误走 TestEx/Test1/Test2 的 `name_base` 变换。这个保护机制比 `why_ns` 更容易在日常对局中产生实际影响，排查时优先级更高。

---

### `PreparedRunner` 的缓存机制

`crates/tswn_core/src/engine/runners.rs` 里有：

```rust
struct PreparedRunnerTemplate {
    groups: Vec<Vec<Player>>,    // 已构造 + build 完成的 Player 克隆
    base_names_sorted: Vec<String>,
    eval_rq: f64,
}
```

缓存 key 只依赖 `(players, eval_rq)`：

```rust
fn groups_cache_key(players: &[Vec<String>], eval_rq: f64) -> u64 {
    // hash 所有 group 的字符串 + eval_rq
}
```

如果未来要补 `why_ns`，这里的缓存 key 就必须加上构造序号信息，否则不同局里相同 roster 的玩家会被错误复用同一个模板。不过现在不需要考虑这个，因为 `why_ns` 还没实现。

---

### 如果以后要补 `why_ns`，应该怎么插

代码位置很明确。在 `impl_ctor.rs` 的 `new_and_init_inner()` 里：

```rust
let mut rand = RC4::new(&team_bytes, 1);
// ← 这里插入 why_ns 的构造序号影响
//    比如：rand.update(&[construction_slot], 2);
rand.update(&name_bytes, 2);
```

要求：
- 太早不行（还没拿到 team 初始化的 RC4 状态）
- 太晚不行（`name_base` 已经算完了）
- 不能复用 `player_id_counter`（语义不对，时机也不对）
- 不建议做成全局 static，应该挂在 `Runner` 或 `Storage` 上，局间隔离

minion 也要占构造序号，但它们在 `new_minion_and_init()` → `build()` 之后还会被技能代码大幅改写（属性覆盖、技能覆盖、血量修正等），所以 `why_ns` 对 minion 的影响可能在后续覆写中被冲掉。真要补的时候需要仔细对一下原版行为。

---

### 排查优先级（实际有用版）

如果你现在 score / benchmark 对不上，别先怀疑 `why_ns`。按这个顺序查：

1. **`PlayerType` 判定对不对** → `impl_ctor.rs:48-72`，特别是 `!` 队名的分支逻辑
2. **`name_base` 特殊变换对不对** → `impl_ctor.rs:96-130`，Test1/Test2/TestEx 的三种变换
3. **minion 有没有误走特殊类型** → `force_normal_type = true` 是否传到位了
4. **score/benchmark 的输入构造** → roster 字符串拆分、`eval_rq` 的值、是否走了正确的 API 路径
5. **`PreparedRunner` 缓存是否命中** → 重复对局用了同一个缓存模板，但 seed 不同导致 `sort_int` 等 seed 相关属性错位

排查时只需要关注这三个文件：

```
crates/tswn_core/src/player/impl_ctor.rs     ← 玩家构造 + name_base
crates/tswn_core/src/player/impl_attr.rs     ← 八围/技能展开
crates/tswn_core/src/engine/runners.rs       ← PreparedRunner 缓存 + 分组
```

---

### 阅读顺序（如果以后需要深入代码）

```
impl_ctor.rs    → 构造流程 + name_base 生成
impl_attr.rs    → name_base → 属性/技能
shadow.rs       → minion 创建示例
summon.rs       → minion 创建示例（最复杂的一个）
zombie.rs       → minion 创建示例
runners.rs      → PreparedRunner 缓存 + 分组逻辑
storage.rs      → 需要时查，不需要不用看
engine_core.rs  → 需要时查，不需要不用看
```

不需要先读 `md5.js` 也能看懂这一侧。
