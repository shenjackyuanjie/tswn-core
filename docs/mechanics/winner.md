# 判胜语义

[返回机制索引](README.md)

本文固定“一场对局何时分出胜负”的语义，以及 `alive_group_count` 在这个问题上的边界。
结论同时适用于 Rust 主 Runtime 与 legacy `md5.js`，并给出两者的对照证据。

结论先行：

- 只有**恰好一个队伍仍有存活实体**才算分出胜负；两个及以上队伍存活、或全部阵亡，都是“尚无胜者”。
- 判胜只在**某队最后一个成员离场**的时刻发生，判据是精确的“存活者是否同属一队”，与 `alive_group_count` 无关。
- `alive_group_count`（legacy `Engine.y.a.Q`）是**粘性计数**：队伍被清空后复活不会补回。它只服务于 `> 2` 的智能选目标，
  **不能用于判胜**。批量热路径曾误用它，已在 `0.5.3` 之后修正。

---

## 1. Rust 侧定义

`WorldArena::sync_winner`（`crates/tswn_core/src/runtime/world.rs`）扫描 `EntityArena` 的 `runtime.alive`：

```rust
pub fn sync_winner(&mut self, entities: &EntityArena) -> Option<usize> {
    let mut alive_team = None;
    for (_, entity) in entities.iter() {
        if !entity.runtime.alive {
            continue;
        }
        match alive_team {
            None => alive_team = Some(entity.runtime.team),
            Some(team) if team == entity.runtime.team => {}
            Some(_) => {
                self.winner_team = None;
                return None;
            }
        }
    }
    self.winner_team = alive_team;
    self.winner_team
}
```

- 恰好一个队伍存活 → 该队伍编号；出现第二个队伍 → `None`；一个存活实体都没有 → `None`。
- 判胜结果落在 `WorldArena.winner_team`，对外只暴露 `winner_team()` 与
  `RuntimeRunner::input_group_won`（把 runtime 队伍映射回输入分组，因此可能对应多个输入分组，
  数据集生成器要求胜者唯一）。

两条调用路径：

| 路径 | 入口 | 判胜实现 |
| --- | --- | --- |
| 可交互 / 数据集 | `run_minimal_round_with_capture::<true>`、`RuntimeRunner::run_until_winner` | `sync_winner`（全量扫描实体表） |
| 批量胜率评分 | `run_minimal_round_with_capture::<false>`、`RuntimeRunner::run_to_completion_prevalidated` | `sync_winner_from_alive_views`（只读存活视图，不扫实体表） |

可交互路径在每个子回合开始前（`combat/round.rs` 的 `run_minimal_round_once_with_capture`）和回合结束时
（`finish_round_with_capture`）各判一次。批量路径为了热路径性能省掉实体表扫描，但**必须给出与全量扫描完全相同的结论**；
`winner_team` 是唯一对外结果，两条路径分歧就等于“同一局对出两个胜者”。

`sync_winner_from_alive_views` 现在的实现按 `team_alive` 的非空槽位计数，复杂度 O(队伍数)：

```rust
let mut winner = None;
let mut alive_groups = 0usize;
for (team, alive) in self.team_alive.iter().enumerate() {
    if alive.is_empty() {
        continue;
    }
    alive_groups += 1;
    if alive_groups == 1 {
        winner = Some(team);
    }
}
self.winner_team = if alive_groups == 1 { winner } else { None };
```

`team_alive` / `flat_alive` 由 `add_spawned_alive`、`revive_alive`、`remove_alive` 统一维护，
`runtime/tests/winner_path_divergence_tests.rs` 会逐回合核对它们与实体表 `runtime.alive` 是否一致。

## 2. legacy 侧定义（`md5.js`）

legacy 只在 `T.Grp.prototype.dj`（把实体移出存活视图）里判胜（行内注释为本文所加）：

```js
dj(a) {
    C.Array.U(this.f, a)          // this.f = 本队存活成员
    s = this.a                    // s = 世界/引擎
    r = s.e                       // r = 全场存活列表
    C.Array.U(r, a)
    ...
    q = this.f.length             // 本队剩余存活
    p = 0
    if (q === p) {                // 本队被清空
        s.Q = s.Q - 1
        q = r[p].y                // 全场第一个存活玩家所在的队
        if (q.f.length === r.length) {   // 该队存活人数 == 全场存活人数
            s.cy = q              // 判胜
            H.throw_expression(q) // 抛出以展开回合循环
        }
    }
}
```

- 判据是“首个存活玩家所在队伍的存活人数 == 全场存活人数”，等价于“所有存活者同属一队”，**精确且不需要遍历队伍表**。
- 只在队伍被清空的瞬间检查。对局从“多队存活”变成“只剩一队”的那一步必然是某队清空，所以不会漏判。
- **全程不读 `Q`。**
- 全员阵亡时 `r[0]` 为 `undefined`，`r[p].y` 抛异常，被 `T.Engine.prototype.O` 的 `try/catch` 直接忽略
  （`md5.js` 17566-17572），于是 `cy` 保持 `null` → 无胜者。与 Rust 返回 `None` 一致。
- 胜者通过 `s.cy = q` 传递，异常只用来跳出 `while (this_.cy == null)` 循环；随后 `O()` 输出
  `RunUpdateWin`。

## 3. `alive_group_count` 的语义与边界

legacy `Engine.y.a.Q` 与 Rust `WorldArena.alive_group_count` 语义一致：

| 时机 | 行为 |
| --- | --- |
| 初始化 | `md5.js` 17267 `this_.Q = i.length`，只数非空队伍；Rust 在 `WorldArena::from_entities` 同样按非空队伍计数 |
| 某队被清空 | `md5.js` 17742 / Rust `remove_alive` 递减 |
| 复活或入场 | `md5.js` `aZ` 17707-17725 与 `SklRevive` 15731-15741 都不碰 `Q`；Rust `revive_alive` 只在队伍槽位是新建时才 `+1` |
| 唯一用途 | `md5.js` 18883 `if (this.gap().y.a.Q > 2)`；Rust 侧为多处 `world.alive_group_count() > 2` 的智能选目标 |

因此它是“曾经存在过的队伍数”而不是“当前存活队伍数”。这一点在
[0.3.3 更新说明](../releases/0.3.3.md) 中已经写死，并明确警告过：

> 如果下游代码把它当成实时非空队伍数使用，在 revive / addNew 场景下可能会看到不同结果；这是为对齐 JS 而做的修正。

**判胜不属于它的合法用途。** 需要“当前存活队伍”时，用 `team_alive` 的非空槽位或实体表 `runtime.alive`。

另有一处相关用法：`combat/round.rs` 的 post-action 状态循环用 `alive_group_count() <= 1` 判断“行动者已死亡且战斗同时结束”
从而提前跳出。那是对 legacy 行为的有意对齐，与判胜无关，本文不改动；修改它需要先拿到能区分 legacy 行为的用例。

## 4. 历史缺陷与修复

批量胜率路径曾用 `alive_group_count == 1` 判胜，于是一个可达局面会出错：

```text
3 队 → A 队被清空（计数 3→2）→ A 队成员被魅惑的敌人用苏生术复活（计数仍是 2）
     → B 队被清空（计数 2→1）→ 此时 A、C 两队都还活着
```

- 全量扫描（数据集/可交互路径）：两队存活 → 无胜者，继续打。
- `alive_group_count == 1`（批量路径）：宣布第一个非空队伍获胜 → **错判**，且该局会被提前结束。

反向表现也存在：计数只减不增，因此还可能“已经分出胜负却判不出胜者”，批量路径空转到 `max_rounds`
并把标签记成未完成。修复（`fix(runtime): 修正批量判胜在队伍复活后误判胜者`）把判据换成 `team_alive` 非空槽位计数，
`alive_group_count` 的 legacy 语义保持不动（技能目标选择依赖它）。

实测证据（名字池 `tests/sqp5900.txt` 的 2v2v2 抽样，3 队）：

| 样本 | 规模 | 复活 | 计数残留 | 判胜分歧 |
| --- | --- | --- | --- | --- |
| Rust，10000 局（修复前） | 409126 回合 | 30975 | 2 局 | **2 局** |
| Rust，10000 局（修复后） | 409129 回合 | 30975 | 2 局 | **0** |
| legacy `md5.js`，10000 局 | 20001 次清空 | 28512 | 2 次（同一局） | **0（胜者 10000/10000 正确）** |
| Rust，35 个 `runtime_stress` 用例 | 2055 回合 | 218 | 0 | 0 |

legacy 侧同样出现计数残留，甚至出现一次“`Q == 1` 但两队存活”的现场（`Q` 2→1 时真实存活队伍为 2），
但由于它不拿 `Q` 判胜，胜者仍然全部正确。**修复方向与 legacy 一致，不是重新定义语义。**

## 5. 复活与“获胜资格”

复活是内置主动技能，不是自定义扩展：

- legacy：`T.SklRevive.prototype.v`（`md5.js` 15702-15748），技能 ID 16，等级半衰。
- Rust：`BuiltinActiveSkill::Revive` / `core.skill.revive`，`skills_lifecycle.rs` 的
  `select_plain_revive_targets` / `drain_plain_revive_skill_into`。
- 召唤、僵尸化、幻影等也会通过 `add_spawned_alive` / `revive_alive` 把实体放回存活视图。

所以“某一帧该队伍 0 存活”**不等于**“该队伍已经出局”，判胜也因此不能退化成“按本体是否存活屏蔽队伍”。
数据集侧实测：100k 数据集前 5 个分片、4 万个样本中，标签队伍在采样帧上 0 存活的比例是 `0 / 40000`
——这份池子里从未发生，但这是经验事实，不是引擎保证。

给张量化的直接结论：

- 不要用 `runtime.alive > 0` 当 `team_mask`，也不要用 `alive_group_count` 当任何 mask。
- 需要表达“获胜资格”时，应显式定义“该队伍是否仍可能被复活/召唤”，或在 encoder 规格中声明
  “假设不可复活”并引用上面的实测比例作为分布依据。
- 训练标签用 `winner_team_index`，它来自本文第 1 节的判胜结果；截断局标签为空（`null`）。

## 6. 复现与验证

```powershell
# Rust 侧两条判胜路径的逐回合一致性
cargo test -p tswn_core --lib winner_path_divergence

# 计数残留 + 复活残局的定点回归
cargo test -p tswn_core --lib world_alive_view_winner

# 金标基线（含 winner_team_index）
cargo test -p tswn_test --features runtime-corpus

# 名字池上的频率扫描（本地诊断，依赖未纳入版本库的 tests/sqp5900.txt）
$env:TSWN_DIVERGENCE_BATTLES=10000
cargo test -p tswn_core --lib pool_matchups -- --ignored --nocapture

# legacy 对照（需要外部 md5.js）
node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --case-dir crates/tswn_test/cases/runtime_stress
node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --names tests/sqp5900.txt --battles 10000
```

`scripts/md5_winner_probe.cjs` 会在 `Grp.dj` / `Grp.aZ` 里插桩，统计清空次数、复活次数、
`Q` 残留次数、`Q == 1` 但两队存活的次数，以及判胜时的比较值，用于核对本文第 2、3 节的结论。

## 7. 源码定位

| 主题 | Rust | legacy `md5.js` |
| --- | --- | --- |
| 判胜 | `runtime/world.rs` `sync_winner` / `sync_winner_from_alive_views` | `T.Grp.prototype.dj`（17726-17749） |
| 存活视图维护 | `world.rs` `add_spawned_alive` / `revive_alive` / `remove_alive` | `T.Grp.prototype.aZ`（17707-17725）、`dj` |
| 队伍计数 | `world.rs` `alive_group_count` | `Engine.y.a.Q`（初始化 17267、递减 17742、使用 18883） |
| 回合循环 | `combat/round.rs` `run_minimal_round_with_capture` | `T.Engine.prototype.O`（17514-17588） |
| 复活技能 | `combat/skills_lifecycle.rs` | `T.SklRevive.prototype.v`（15702-15748） |
| 回归测试 | `runtime/tests/winner_path_divergence_tests.rs`、`world.rs` 单测 | `scripts/md5_winner_probe.cjs` |
