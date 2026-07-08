# runtime v2 custom 迁移审计

> 状态：阶段 B 审计草案
> 范围：`github/main..github/custom` 中与 custom 产品线相关的行为差异
> 目标：把 custom 分支的行为改动逐项落到 runtime v2 extension / policy / renderer / fixture 验收面

---

## 1. 审计来源

审计基准：

```powershell
git diff --stat github/main..github/custom
git diff --name-status github/main..github/custom
```

关键差异文件：

| 类型 | custom 分支文件 |
| --- | --- |
| player kind / overlay | `crates/tswn_core/src/player/impl_attr.rs`, `impl_ctor.rs`, `impl_runtime.rs`, `player/mod.rs` |
| summon / minion | `player/skill/act/summon.rs`, `player/skill/act/minion.rs`, `player/test/minions.rs` |
| merge | `player/skill/skl/merge.rs` |
| replay / show | `crates/tswn_core/src/replay_view.rs`, `crates/tswn_wasm/examples/show-utils.js` |
| runner fixture | `crates/tswn_core/src/engine/test/**`, moved from `crates/tswn_test/src/suite/**` |

---

## 2. custom 行为映射

| custom 改动点 | 证据锚点 | v2 落点 | 当前 v2 状态 | 验收 case |
| --- | --- | --- | --- | --- |
| bed2 player type | `DEFAULT_BED2_HP = 3000`; `PlayerType::Bed2`; `bed2[...]` / `@bed2` marker | `PlayerKindSpec` + `PlayerKindPolicies` + template/entity slot | 已有 bed2 registry/template fixture 覆盖 kind、policy、3000 HP 与 marker slot；仍缺完整 parser/import fixture | bed2 构造后固定 HP、技能槽、summon 模板 strict diff |
| bed2 固定 summon 技能 | custom 将 bed2 overlay 设为 `[0,99,0,0,0,99,0,hp]` 并只保留 `sklsummon=255` | `PlayerTemplate::with_kind(...).with_skills([summon])` + policy | v2 fixture 已覆盖固定 summon skill loadout；缺内置 summon 技能迁移 | bed2 只尝试 summon，不扫描普通技能 |
| bed2 summon template 导出 | `summon_overlay_from_player_template`; `overlay_from_built_minion` | template slot 保存 summon/minion 模板；effect handler 生成实体 | v2 fixture 已覆盖 summon template slot；缺真实 template payload 与 custom parser | bed2 summon 的 attr/skills 与 custom branch 一致 |
| summon recast 复用技能 | `reuse_skills_on_recast: is_summon` | summon policy + effect handler | 已有 custom summon 复合 fixture 覆盖 spawn 后 SkillLoadout 保留；仍缺真实 recast handler | summon recast 后技能继承/复用顺序不漂移 |
| summon 继承 owner 防御/魔防 | `inherit_owner_def_res: is_summon` | `PlayerKindPolicies::inherit_owner_def_res` + template/runtime def/res | 已有 v2 custom summon fixture 覆盖 spawn 时继承 owner defense/resistance；仍缺真实 summon handler | summon 出场后的防御/魔防展示与 custom 一致 |
| summon/root-owner 伤害路由 | summon clone damage route to root owner | `OwnerResolutionPolicy::RootOwner` | 已接入并在 custom summon 复合 fixture 中覆盖 | root owner 承伤、致死 hook 目标一致 |
| summon 伤害共享 owner | child/summon damage share owner | `DamageSharePolicy::ShareToOwner` | 已接入并测试 owner 共享致死 hook；复合 fixture 覆盖 summon policy 注册 | 子实体受伤同步扣 owner，owner 死亡 hook 顺序一致 |
| owner 伤害共享 summon | owner damage share alive summons | `DamageSharePolicy::ShareToSummons` | 已接入并在 custom summon 复合 fixture 中覆盖按实体顺序共享 | owner 受伤同步扣存活 summon，顺序稳定 |
| minion heal sharing 移除/调整 | custom minion 行为集中在 `act/minion.rs` 与 `player/test/minions.rs` | player kind policy 或 damage/share policy | 已有 v2 custom minion heal fixture 固化 damage 仍共享、heal 只作用目标实体；仍缺真实 minion handler | minion 相关 heal 不再产生 custom 分支禁止的共享 |
| merge 固定槽继承 | custom 保留 `slot_skill` 固定槽语义以避免 merge 错位 | `MergePolicy::FixedLane` | 已接入并测试 fixed lane 合并 | 同槽位技能覆盖，未映射技能 append |
| merge 丢弃未映射技能 | custom 分支支持 drop unmapped 语义 | `MergePolicy::DropUnmappedSkills` | 已接入并测试 drop unmapped | 未映射来源技能不进入 caster loadout |
| merge replay | custom replay 使用吞噬/属性上升展示 | `QueuedEffect::Merge` replay update | 已输出 `[0][吞噬]了[1]` 与 `[0]属性上升` | merge frame 顺序、score 分别为 60/0 |
| HP report replay | custom 新增 `"[0]还剩[2]点血"` 作为 HP marker | replay/show renderer + entity slot | 已用 v2 core replay/show golden 固化 payload 和 `[2]` param，并补 HP bar show renderer fixture；仍缺 wasm show adapter | HP marker 强制显示 HP bar，`[2]` 作为 data |
| show 数字高亮 | `show-utils.js` 把 `点血` 纳入数字高亮 | show renderer / wasm show adapter | v2 core show golden 已覆盖 `还剩87点血` 文本；缺 wasm show adapter | `还剩87点血` 中 87 被识别为数值 |
| runner fixture 内置化 | `crates/tswn_test/src/suite/**` moved into `crates/tswn_core/src/engine/test/**` | repo 内 extension fixture + strict diff runner | 已有最小 v2 custom runner strict-diff golden 覆盖 spawn、owner def/res、damage share、heal 与 HP marker；仍缺 large/fight_multi 样例 | bed2/summon/merge/minion/custom replay golden 可稳定复跑 |

---

## 3. v2 fixture 切分顺序

1. **bed2 registry fixture**：已注册 `custom.bed2` kind、固定 summon skill、HP marker slot；后续补真实 parser/template payload。
2. **summon policy fixture**：已覆盖 root-owner 路由、owner/summon 伤害共享、spawn 后技能保留、owner defense/resistance 继承；后续补真实 recast handler。
3. **minion heal fixture**：已覆盖 owner damage share 仍生效、minion heal 不向 owner 或 sibling minion 共享；后续补真实 minion handler。
4. **merge fixture**：使用 `FixedLane` 与 `DropUnmappedSkills` 两组 golden 覆盖 replay 与 loadout。
5. **HP marker renderer fixture**：已用 core replay/show payload 固化 `还剩[2]点血` 展示与数值 data，并补 HP bar show renderer payload；后续补 wasm show adapter。
6. **runner fixture**：已新增最小 v2 strict-diff golden；后续把 custom 分支 large / fight_multi 的关键样例缩成更多 runner golden。

---

## 4. 已落地能力

- `PlayerKindSpec` / `PlayerKindPolicies` 可表达 custom kind 与行为策略。
- `OwnerResolutionPolicy::RootOwner` 已覆盖 summon/root-owner 伤害路由。
- `DamageSharePolicy::ShareToOwner` / `ShareToSummons` 已覆盖 owner 与 summon 伤害共享。
- `PlayerKindPolicies::inherit_owner_def_res` 已覆盖 custom summon 继承 owner 防御/魔防的数据面。
- `QueuedEffect::Heal` 已用 custom minion fixture 固化不触发 owner/summon damage share。
- `MergePolicy::FixedLane` / `DropUnmappedSkills` 已覆盖 custom merge 数据面。
- `RuntimeFrame::render_core_replay` / `render_core_show` 已提供 show 迁移前的最小 golden 面。
- HP marker show renderer fixture 已固化 `hp-bar` payload，保留 `[2]` HP 数值给展示层使用。
- 最小 custom runner strict-diff golden 已把 spawn、share、heal、HP marker 和 world 派生视图接入同一验收面。

---

## 5. 未完成项

- bed2 的 parser/import 与真实 summon template payload fixture。
- summon 真实 recast handler、真实 minion handler。
- HP marker wasm show adapter。
- custom large / fight_multi runner 归一化 golden 扩展。
- 将审计表中的每个验收 case 接入 strict diff 或稳定单测。
