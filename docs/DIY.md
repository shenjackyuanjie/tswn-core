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

| 文件 | 职责 | 与 DIY 的关系 |
|------|------|--------------|
| `player/mod.rs` | `Player` 结构体、`PlayerType` | 需加 `overlay` 字段 |
| `player/impl_ctor.rs` | `new_and_init`、`new_from_namerena_raw` | 需接收/解析 overlay |
| `player/impl_attr.rs` | `build_inner` 构建流程 | 需插入 DIY 覆盖逻辑 |
| `player/skill.rs` | `Skill`、`SkillNames`、注册表 | 需加 `skill_name_to_id` 映射 |
| `player/skill/store.rs` | `SkillStorage`、`slot_skill`/`skill` 顺序管理 | DIY 技能需操作排序 |
| `player/skill/act/mod.rs` | 27 种主动技能 | 为名称映射提供源 |
| `player/skill/skl/mod.rs` | 13 种被动技能 | 同上 |
| `player/weapons.rs` | 武器系统 | DIY 模式 weapon_state = None |
| `player/status.rs` | `PlayerStatus` | build 最终状态更新 |
| `engine/runners.rs` | `Runner` 对局构造 | 需感知 overlay 以构建玩家 |

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

| | Plan A (字符串扩展) | Plan B (serde 数据驱动) |
|---|---|---|
| 数据存储 | 名字字符串中编码 | `Option<Box<PlayerOverlay>>` 结构体 |
| 解析时机 | build 时再解析 | 构造时解析一次 |
| normal 玩家开销 | 无 (skip) | 1 次 `is_none()` |
| 扩展性 | 字符串编码 → 难扩展 | serde JSON → 任意字段 |
| API 友好度 | 差 (需字符串拼接) | 好 (直接传结构体/JSON) |
| C/Python FFI | 需字符串编解码 | JSON 字符串透明传递 |

**选定方案: Plan B**

## 设计方案

### 数据模型

```rust
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PlayerOverlay {
    pub attrs: Option<[i32; 8]>,
    pub skills: Option<HashMap<String, u32>>,
    pub weapon: Option<String>,
}
```

### Player 结构体变更

```rust
pub struct Player {
    // ... 现有字段不变 ...
    pub overlay: Option<Box<PlayerOverlay>>,  // None == 普通玩家，零额外内存
}
```

`Option<Box<T>>` 在 64 位平台占 8 字节，`None` 时利用 niche 优化不额外分配。

### 输入方式（3 种，从优到劣）

**1. API 模式 (推荐 — 结构化)**
```rust
let overlay = PlayerOverlay {
    attrs: Some([72, 39, 69, 76, 67, 66, 0, 84]),
    skills: Some(HashMap::from([("fire".into(), 5)])),
    ..Default::default()
};
Player::new_and_init(team, name, weapon, Some(overlay), storage)?;
```

**2. 内联格式 (兼容 — 名字中编码)**
```
PlayerName+ol:{"attrs":[72,39,69,76,67,66,0,84],"skills":{"fire":5}}
```
`new_from_namerena_raw` 在解析 weapon 时检测 `diy[...]{...}` 和 `ol:{json}` 格式。

**3. 批量配置 (大数据场景)**
```json
{
  "overlays": {
    "Bob":  {"attrs": [72,39,69,76,67,66,0,84], "skills": {"fire": 5}},
    "Alice": {"attrs": [50,50,50,50,50,50,50,200]}
  },
  "groups": [["Bob", "Alice"]]
}
```
Runner 接受外部 overlay 映射，按 name 匹配。

### API 变更清单

| 函数 | 当前 | 变更 |
|------|------|------|
| `Player::new_and_init` | `(team, name, weapon, storage)` | `(team, name, weapon, overlay, storage)` |
| `Player::new_from_namerena_raw` | `(raw_name, storage)` | 不变；内部解析 `diy`/`ol:` 为 overlay |
| `Runner::new_from_groups_with_seed` | `(groups, seed)` | 不变；但允许 group 字符串含 `ol:` |
| `Runner::prepare_groups` | `(players)` | 新增 overlay 参数？或拆为两步 |

## 实施步骤

### Step 1: 定义 PlayerOverlay 结构体
- 文件: `player/overlay.rs` (新建)
- 内容: `PlayerOverlay` 结构体 + serde 派生 + 工具方法

### Step 2: Player 结构体加 overlay 字段
- 文件: `player/mod.rs`
- 加 `pub overlay: Option<Box<PlayerOverlay>>`
- 确保 `Default`, `Clone` 等 trait 正确派生

### Step 3: 修改构造函数
- 文件: `player/impl_ctor.rs`
- `new_and_init`: 加 `overlay: Option<PlayerOverlay>` 参数
- `new_from_namerena_raw`: 解析 `diy[...]{...}` / `ol:{json}` → `PlayerOverlay`

### Step 4: 修改 build_inner
- 文件: `player/impl_attr.rs`
- 在 `init_raw_attr` 后，若 `overlay.attrs.is_some()`，覆盖 base_attr
- 在 init skills 阶段，若 `overlay.skills.is_some()`，跳原有 `dm()`，执行 DIY 技能设置

### Step 5: 技能名称映射
- 文件: `player/skill.rs`
- 加 `skill_name_to_id(name: &str) -> Option<usize>`
- 大小写不敏感匹配 `constructor.name`（JS 语义）
- 映射表: act 0–26 + skl 0–12 = 共计 40 技能

### Step 6: DIY 技能应用逻辑
- 在 `skill.rs` 或 `impl_attr.rs` 实现 `apply_diy_skills`
- 对每个 overlay skill: `find_id → set_level → 重排 slot_skill` 使 1–3 为主动、4+ 为被动

### Step 7: C/Python API 更新
- C API: 新增 `PlayerOverlay` 参数，通过 JSON 字符串透传
- Python: `new_and_init` 接受 `overlay: Optional[Dict]`

### Step 8: 测试
- normal player: overlay = None, 零额外开销 (build 路径 trace)
- DIY 玩家: 八围覆盖正确 (自动减 36)
- DIY 技能: 等级设置 + 顺序重排 (主动在前)
- 旧格式兼容: `diy[72,...,84]{"sklfire":5}` 解析正确
- Clone 不应用 overlay
- 边界: 技能名不存在时静默忽略

## 未完成/待确认

- CI 跑通 — 需先有可编译的 Rust 代码
- 和 JS 输出做 diff 验证数值一致性
- 确认八围第 8 项（幸运）是否也在 JS 中 `-= 36`（第 8 项索引为 7，JS 只循环了 0–6，需确认）
- `PlayerOverlay.weapon` 的语义：覆盖武器还是追加武器？
