# DIY 捏人系统 — 移植分析与设计方案

## 目标

将 JS `md5.js` 中的 DIY 捏人功能移植到 Rust `tswn-core`，支持自定义八围属性和技能等级，同时为未来扩展（自定义状态、AI 配置等）提供数据驱动的可扩展架构。

## 已完成分析

### 1. JS 侧 DIY 解析流程 (`md5.js`)

- **格式**: `PlayerName+diy[72,39,69,76,67,66,0,84]{"sklfire":5}`
  - `+` 之后是 weapon/DIY 字段
  - `diy[...]` — 前 8 个数字为八围覆盖值（解析后每一项 `-= 36`）
  - `{...}` — JSON 对象，key 为技能名（不区分大小写），value 为等级
- **关键函数**:
  - `d(b, P)` (constructor): 检测 `b[0]==='diy'`，初始化 `this.q = attrs`，设置 `state=0`
  - `az(c, P)` (build): 若 `state===0` 走 DIY 分支 → 跳过武器/职业，直接设 `this.q = cappedAttrs`，调用 `diy_skills`
  - `diy_skills(c)`: 按 `constructor.name` 大小写不敏感匹配技能名 → `set_level(lv)` → 交换技能顺序确保 1–3 为主动、4 起为被动
  - `bP()`: 初始化技能列表时，如果 `state===0`（DIY）则 `return` 跳过

### 2. Rust 侧结构分析 (`tswn-core`)

| 文件                      | 职责                                          | 与 DIY 的关系                |
| ------------------------- | --------------------------------------------- | ---------------------------- |
| `player/mod.rs`           | `Player` 结构体、`PlayerType`                 | 需加 `overlay` 字段          |
| `player/impl_ctor.rs`     | `new_and_init`、`new_from_namerena_raw`       | 需接收/解析 overlay          |
| `player/impl_attr.rs`     | `build_inner` 构建流程                        | 需插入 DIY 覆盖逻辑          |
| `player/skill.rs`         | `Skill`、`SkillNames`、注册表                 | 需加 `skill_name_to_id` 映射 |
| `player/skill/store.rs`   | `SkillStorage`、`slot_skill`/`skill` 顺序管理 | DIY 技能需操作排序           |
| `player/skill/act/mod.rs` | 27 种主动技能                                 | 为名称映射提供源             |
| `player/skill/skl/mod.rs` | 13 种被动技能                                 | 同上                         |
| `player/weapons.rs`       | 武器系统                                      | DIY 模式 weapon_state = None |
| `player/status.rs`        | `PlayerStatus`                                | build 最终状态更新           |
| `engine/runners.rs`       | `Runner` 对局构造                             | 需感知 overlay 以构建玩家    |

### 3. 构建流程比对

```
JS:                          Rust (build_inner):
  bP() [init lists]          pre_upgrade_input
  d() [constructor]          init_raw_attr (calc_base_attr)
    → parse diy              boss_additions
    → set this.q             upgrade (同队)
  az() [build]               init skills (dm)
    → if state===0:          post_upgrade_clamp
      cap attrs              attr_boost
      diy_skills             init_values
    → else: normal path
```

### 4. 方案对比

|                 | Plan A (字符串扩展) | Plan B (serde 数据驱动)             |
| --------------- | ------------------- | ----------------------------------- |
| 数据存储        | 名字字符串中编码    | `Option<Box<PlayerOverlay>>` 结构体 |
| 解析时机        | build 时再解析      | 构造时解析一次                      |
| normal 玩家开销 | 无 (skip)           | 1 次 `is_none()`                    |
| 扩展性          | 字符串编码 → 难扩展 | serde JSON → 任意字段               |
| API 友好度      | 差 (需字符串拼接)   | 好 (直接传结构体/JSON)              |
| C/Python FFI    | 需字符串编解码      | JSON 字符串透明传递                 |

**选定方案: Plan B**

---

## 设计方案（当前实现）

### 数据模型

#### PlayerOverlay

```rust
#[derive(Debug, Clone)]
pub struct PlayerOverlay {
    /// 八围覆盖值（`[atk, def, spd, agi, mag, res, wis, maxhp]`）。
    /// `None` 表示不覆盖。
    pub attrs: Option<[i32; 8]>,

    /// 有序技能列表：`(技能名, 加成类型和等级)`。
    ///
    /// 列表中的顺序决定行动时的技能尝试顺序（排在前面的先尝试）。
    /// 未列出的技能按默认固定顺序排在末尾。
    /// `None` 表示不覆盖。
    pub skills: Option<Vec<(String, SkillBoost)>>,

    /// 武器名（DIY 模式下 weapon_state 强制为 None，此字段仅记录）。
    pub weapon: Option<String>,

    /// 是否启用 name_factor 缩放（默认 true）。
    /// 设为 false 时强制 name_factor = 0，八围不缩放。
    pub name_factor_enabled: bool,
}
```

#### SkillBoost（技能加成类型）

用于精确描述技能最终等级的内部构成，在分身后克隆体重建时正确计算衰减下限。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillBoost {
    /// 普通技能：最终等级 = 指定值。
    Normal(u32),

    /// 末尾座位加成：最终等级 = base + boost。
    SlotBoost { base: u32, boost: u32 },

    /// 末尾主动技翻倍：最终等级 = base × 2。
    LastBoost(u32),
}
```

| 变体 | 内联格式示例 | 最终等级 | 说明 |
|------|-------------|---------|------|
| `Normal(lv)` | `"sklfire":5` | `lv` | 无特殊加成 |
| `SlotBoost { base, boost }` | `"sklfire":"40+30"` | `base + boost` | 末尾座位 passive boost |
| `LastBoost(base)` | `"sklshadow":"2*46"` | `base × 2` | 末尾主动技 boost_last 翻倍 |

**辅助方法**：

| 方法 | 说明 |
|------|------|
| `final_level()` | 加成后的最终展示等级 |
| `base_level()` | 加成前的原始基础等级 |
| `decayed_base_from_level(current_level)` | 由衰减后最终等级反推衰减后基础 |
| `final_level_from_decayed_base(decayed_base)` | 由衰减后基础重新计算加成后等级 |
| `parse(raw: &str)` | 从字符串解析（`"5"` / `"40+30"` / `"2*46"`） |

#### Skill 结构体扩展

```rust
pub struct Skill {
    pub boosted: bool,
    level: u32,
    skill_type: Box<dyn SkillTrait>,
    pub target: Option<PlrId>,
    /// DIY 技能加成信息，None 表示非 DIY 技能。
    /// clone 重建时用于计算衰减下限。
    pub diy_boost: Option<SkillBoost>,
}
```

#### SkillStorage 扩展

```rust
pub struct SkillStorage {
    // ... 原有字段 ...
    /// 标记此 SkillStorage 是否由 DIY overlay 构建。
    /// clone 重建时用于判断是否走 DIY 衰减下限逻辑。
    pub is_diy: bool,
}
```

### Player 结构体

```rust
pub struct Player {
    // ... 现有字段不变 ...
    pub overlay: Option<Box<PlayerOverlay>>,  // None == 普通玩家，零额外内存
}
```

`Option<Box<T>>` 在 64 位平台占 8 字节，`None` 时利用 niche 优化不额外分配。

### 输入方式（3 种）

#### 1. API 模式（推荐 — 结构化）

```rust
use std::collections::HashMap;
use tswn_core::player::{PlayerOverlay, SkillBoost};

// 普通技能
let overlay = PlayerOverlay {
    attrs: Some([72, 39, 69, 76, 67, 66, 0, 84]),
    skills: Some(vec![
        ("fire".into(), SkillBoost::Normal(5)),
    ]),
    ..Default::default()
};

// 末尾座位加成：基础 40 + 加成 30 = 70
let overlay = PlayerOverlay {
    attrs: Some([50, 50, 50, 50, 50, 50, 50, 200]),
    skills: Some(vec![
        ("heal".into(), SkillBoost::SlotBoost { base: 40, boost: 30 }),
    ]),
    ..Default::default()
};

// 末尾主动技翻倍：基础 46 × 2 = 92
let overlay = PlayerOverlay {
    skills: Some(vec![
        ("shadow".into(), SkillBoost::LastBoost(46)),
    ]),
    ..Default::default()
};

// name_factor 不缩放
let overlay = PlayerOverlay {
    attrs: Some([50, 50, 50, 50, 50, 50, 50, 200]),
    name_factor_enabled: false,
    ..Default::default()
};

Player::new_and_init_with_overlay(team, name, weapon, Some(overlay), storage)?;
```

#### 2. 内联格式（兼容 — 名字中编码）

**紧凑格式** (`diy[...]{...}`)：

```
# 普通技能
PlayerName+diy[72,39,69,76,67,66,0,84]{"sklfire":5}

# 末尾座位加成
PlayerName+diy[72,39,69,76,67,66,0,84]{"sklheal":"40+30"}

# 末尾主动技翻倍
PlayerName+diy[72,39,69,76,67,66,0,84]{"sklshadow":"2*46"}

# 混合使用
PlayerName+diy[72,39,69,76,67,66,0,84]{"sklfire":5,"sklheal":"40+30","sklshadow":"2*46"}
```

属性值会自动 `-36` 后取非负（兼容 JS 侧 36~127 范围）。

**JSON 对象格式** (`ol:{...}`)：

技能按顺序排列，顺序决定行动时的尝试顺序。不再使用 `skill_order` 字段。

```
# 普通技能（属性值原样使用，不 -36）
PlayerName+ol:{"attrs":[1,2,3,4,5,6,7,8],"skills":{"fire":4},"name_factor_enabled":true}

# 带加成类型
PlayerName+ol:{"attrs":[50,50,50,50,50,50,50,200],"skills":{"heal":"40+30","shadow":"2*46"},"name_factor_enabled":true}

# 禁用 name_factor
PlayerName+ol:{"attrs":[50,50,50,50,50,50,50,200],"name_factor_enabled":false}
```

`new_from_namerena_raw` 自动解析 `diy[...]{...}` 和 `ol:{json}` 格式。

#### 3. 批量配置（大数据场景）

```json
{
  "overlays": {
    "Bob": { "attrs": [72, 39, 69, 76, 67, 66, 0, 84], "skills": { "fire": 5 } },
    "Alice": { "attrs": [50, 50, 50, 50, 50, 50, 50, 200],
               "skills": { "shadow": "2*46", "heal": "40+30" } }
  },
  "groups": [["Bob", "Alice"]]
}
```

Runner 接受外部 overlay 映射，按 name 匹配。

### 武器行为

DIY 模式下（`attrs` 或 `skills` 不为 `None` 时），`weapon_state` 强制为 `None`。即**武器不计入**。

`PlayerOverlay.weapon` 字段仅记录武器名，不会实际生效。

### name_factor 覆盖

| `name_factor_enabled` | 行为 |
|----------------------|------|
| `true`（默认） | 八围按正常 name_factor 缩放 |
| `false` | `name_factor` 强制为 0，八围使用原始值 |

### 分身后 clone 行为

Clone 构建过程按 boost 来源分为两条路径：

**Step A — 普通号**：从 `name_base` 检测 boost 候选 → 记录 `diy_boost` 元数据 → 执行 boost（翻倍/加值）。boost 在 clamp 之前执行。

**Step B — DIY 号**：`apply_diy_skill_levels` 将等级初始化为 `base_level`（未加成），存储 `diy_boost` 元数据。随后在 build 流程中基于当前等级执行 boost（`current * 2` 或 `current + boost`）。

两条路径的 `diy_boost` 元数据均用于 `to-diy` 导出，保证往返时 boost 类型不丢失。

| 加成类型 | overlay 中的 base | Step B 执行 | 最终等级 |
|---------|------------------|------------|---------|
| `LastBoost(46)` | 46 (`"2*46"`) | 46 × 2 | 92 |
| `SlotBoost{40,30}` | 40 (`"40+30"`) | 40 + 30 | 70 |
| `Normal(5)` | 5 (`"5"`) | 不变 | 5 |

### 名字 → DIY/OL 转换

使用 CLI 子命令 `to-diy` 将任意名字转换为 DIY overlay 格式：

```bash
# 基本用法
tswn-cli to-diy help
tswn-cli to-diy "mario@team+fire"

# 输出示例（紧凑格式）
help+diy[64,87,57,68,61,79,76,297]{"sklthunder":21,"skliron":7,...}

# 输出示例（JSON 格式，不再包含 skill_order 字段）
help+ol:{"attrs":[28,51,21,32,25,43,40,261],"skills":{...},"name_factor_enabled":true}
```

也可以通过 Rust API 直接调用：

```rust
let mut player = Player::new_from_namerena_raw("help".to_string(), storage)?;
player.build();

// 紧凑格式（attrs +36，兼容 JS）
let diy_str = player.to_diy_compact();

// JSON 格式（attrs 原样）
let ol_str = player.to_ol_json();
```

### API 一览

| 函数 / 类型 | 说明 |
|------------|------|
| `PlayerOverlay` | overlay 数据结构 |
| `PlayerOverlay::parse_inline(segment)` | 解析 `diy[...]` / `ol:{...}` 段 |
| `PlayerOverlay::default()` | 默认值（`name_factor_enabled = true`） |
| `SkillBoost` | 技能加成类型枚举 |
| `SkillBoost::parse(raw)` | 从字符串解析（`"5"` / `"40+30"` / `"2*46"`） |
| `SkillBoost::final_level()` | 计算最终等级 |
| `SkillBoost::base_level()` | 计算基础等级 |
| `skill_name_to_id(name)` | 技能名 → Rust 技能 ID（大小写不敏感） |
| `skill_name_for_export(id)` | 技能 ID → overlay 技能名（如 `sklfire`） |
| `diy_skill_order()` | DIY 模式固定技能槽顺序 |
| `apply_diy_skill_levels(storage, skill_levels)` | 将有序技能列表写入 SkillStorage，顺序决定行动顺序 |
| `Player::new_and_init_with_overlay(...)` | 带 overlay 的构造函数 |
| `Player::new_from_namerena_raw(...)` | 从名字字符串解析（含 overlay 检测） |
| `Player::to_diy_compact()` | 导出为紧凑 DIY 格式字符串 |
| `Player::to_ol_json()` | 导出为 ol: JSON 格式字符串 |
| `Skill::diy_boost` | 技能上存储的 SkillBoost 信息 |
| `SkillStorage::is_diy` | 标记是否为 DIY 构建的技能集 |

---

## 实施状态

### 已完成 ✅

| 步骤 | 内容 | 状态 |
|------|------|:---:|
| Step 1 | 定义 `PlayerOverlay` + `SkillBoost` | ✅ |
| Step 2 | `Player` 结构体加 `overlay` 字段 | ✅ |
| Step 3 | 修改构造函数（API + 内联解析） | ✅ |
| Step 4 | `build_inner` DIY 覆盖逻辑 | ✅ |
| Step 5 | `skill_name_to_id` 技能名映射 | ✅ |
| Step 6 | `apply_diy_skill_levels` + 技能排序 | ✅ |
| Step 7 | C/Python API 更新 | ⬜ |
| Step 8 | 测试（基本覆盖 + 回归） | ✅ |
| — | `name_factor_enabled` 覆盖 | ✅ |
| — | DIY 模式武器不计入 | ✅ |
| — | `SkillBoost` 三种格式解析 | ✅ |
| — | DIY clone 衰减下限 + 加成重新执行 | ✅ |
| — | `split_by_plus_outside_quotes` 修复 JSON 内 `+` | ✅ |
| — | `to_diy_compact()` / `to_ol_json()` 导出方法 | ✅ |
| — | `tswn-cli to-diy` 子命令 | ✅ |
| — | 测试（基本覆盖 + 回归） | ✅ |

### 待完成 ⬜

- C API: 新增 `PlayerOverlay` 参数透传（JSON 字符串）
- Python API: `new_and_init_with_overlay` 接受 `Optional[Dict]`
- 和 JS 输出做 diff 验证数值一致性
- 衰减下限的完整对局级验证（DIY 玩家分身对局 trace）

---

## 相关文档

- [技能衰减机制](./analysis/skill_decay.md) — 5 种衰减技能的详细公式
- [分身机制详解](./analysis/clone_mechanism.md) — clone 重建与衰减下限分析
