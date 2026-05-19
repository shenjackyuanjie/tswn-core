# DIY 往返验证 — 工作流与执行计划

这份文档描述 DIY overlay 系统的验证工作流：将普通对局中的每个玩家转换为 DIY 格式，然后验证 DIY 版本的**初始状态**（八围、技能等级、name_factor）是否与原始玩家一致。

## 核心概念

```
原始玩家:  PlayerA @Team  --build-->  attrs=[47,60,48,...], skills={fire:5,...}
              ↓ to-diy
DIY 玩家:   PlayerA+diy[83,96,84,...]{...} @Team  --build-->  attrs=[47,60,48,...], skills={fire:5,...}
              ↓ 对比
预期结果:   初始八围、技能等级、name_factor 完全一致
                  ↓ 对战
对战日志:   fight --out-raw (orig vs diy)
                  ↓ 对比
预期结果:   对战过程一致
```

## 新增：对战过程比对

现在会额外对比对战过程：

- 通过 `tswn-cli fight --out-raw` 输出标准化日志
- 比较原始对战与 DIY 对战日志是否一致
- 脚本默认使用 `ol:` 格式，技能按顺序排列（顺序即行动顺序），不再使用独立的 `skill_order` 字段
- to-diy 输出包含全部 40 个技能槽位（含零级技能），完整保留行动顺序，同时编码 boost 类型（`"2*base"` / `"base+boost"`）

如果你只关心 build 后的初始状态，或预期存在已知差异（如武器禁用），可使用 `--skip-fight` 跳过对战过程比对。

## 执行原则

1. **目标是初始状态一致**：原始玩家和 DIY 玩家 build 后的八围、技能等级、name_factor 应当匹配。
2. **默认对战过程一致**：对战日志应与原始对局一致；如需跳过，用 `--skip-fight`。
3. **先小样本验证**：默认用少量 case（64~200）快速验证，确认无问题后再扩大规模。
4. **差异分级处理**：
   - 八围不匹配 → `parse_diy` 或 `to_diy_compact` 的 ±36 逻辑有误
   - 技能等级不匹配 → `apply_diy_skill_levels` 或 boost 逻辑有误
   - name_factor 不匹配 → `name_factor_enabled` 处理有误
    - 对战过程不一致 → 可能是技能槽顺序、武器禁用或其它逻辑差异
    - 崩溃/无输出 → DIY 名字解析失败

## 快速开始

### 1. 编译

```bash
cargo build --bin tswn-cli
```

### 2. 运行验证

```bash
# 从号库生成 64 个 case 并验证
python track_diy_roundtrip.py --library tests/sqp6000.txt --max-cases 64

# 带偏移量
python track_diy_roundtrip.py --library tests/sqp6000.txt --max-cases 64 --case-offset 64

# 安静模式
python track_diy_roundtrip.py -q --max-cases 64

# 只比初始状态，跳过对战过程
python track_diy_roundtrip.py -q --max-cases 64 --skip-fight
```

### 3. 查看结果

```
target/diy_roundtrip/
├── summary.json              # 总体统计
└── <case-id>/
    ├── input_orig.txt         # 原始输入
    ├── input_diy.txt          # DIY 输入
    ├── status_orig.json       # 原始玩家初始状态
    ├── status_diy.json        # DIY 玩家初始状态
    ├── fight_orig.txt          # 原始对战日志
    ├── fight_diy.txt           # DIY 对战日志
    ├── fight_diff.txt          # 对战差异摘要（失败时）
    └── diff.txt               # 状态差异（失败时）
```

## 测试规模建议

| 阶段 | case 数 | 目标 |
|------|---------|------|
| 冒烟测试 | 16 | 快速验证基本功能 |
| 小样本 | 200 | 发现常见差异模式 |
| 中样本 | 2000 | 确认修复覆盖主流场景 |
| 大样本 | 12000 | 最终验证 |

## 已知差异排查指南

### 1. DIY 名字生成失败

**现象**：`[WARN] 无法转换: ...`，或 DIY 玩家 build 失败。

**原因**：`tswn-cli to-diy` 未能解析玩家名字（如包含特殊字符）。

### 2. 紧凑格式八围偏移

**现象**：DIY 玩家初始八围与原始玩家不同。

**原因**：`parse_diy` 或 `to_diy_compact` 的前七围 ±36 逻辑有误，或 HP 被错误处理。

**排查**：对比 `diff.txt` 中的 `attr` 字段，检查是否恰好差 36 或差某个固定偏移。

### 3. 武器差异

**现象**：原始玩家有武器，DIY 版本 weapon_state=None。

**原因**：当前 DIY 模式禁用了武器。`to_diy_compact` 不导出武器信息。

**这是已知限制**：暂不支持武器还原。

### 4. 技能 boost 信息丢失 —— ✅ 已修复

技能 boost 类型现已通过 `Skill.diy_boost` 在 build 阶段自动记录（`boost_if_not()` → `LastBoost`，`boost_level()` → `SlotBoost`）。to-diy 导出时正确编码为 `"2*base"` / `"base+boost"` 格式。

### 5. name_factor 差异

**现象**：DIY 玩家的 name_factor 与原始不同。

**原因**：`to_ol_json` 输出 `name_factor_enabled:true`，但 DIY 名字计算的 name_factor 与原始不同。

**排查**：对比两个玩家的 `系数` 值。

## 修复判定标准

| 情况 | 判定 |
|------|------|
| 初始八围完全一致 | ✅ 通过 |
| 初始技能等级完全一致 | ✅ 通过 |
| 所有 case 通过 | ✅ 全部通过 |
| 八围/技能出现差异 | ❌ 需修复 |
| 出现崩溃 | ❌ 需修复 |

## 当前验证状态

| 项目 | 状态 |
|------|:---:|
| 初始八围、技能等级、name_factor 一致 | ✅ 64/64 通过 |
| 对战过程一致（含 clone、boost 等） | ✅ 61/64 通过 (95.3%) |
| 完整 action order 往返（40 槽位） | ✅ 已实现 |
| Boost 类型编码（LastBoost/SlotBoost） | ✅ 已实现 |
| Clone 衰减下限统一（普通=DIY） | ✅ 已实现 |

剩余 3 个 case 涉及分身+加成技能的复杂交互，正在排查中。

## 已知限制

1. **武器不支持**：DIY 模式下武器不计入（weapon_state = None）。
2. **大型回归测试**：clone 行为修正（clamp→boost）影响了 45 个 large 测试，需逐一更新基线。

## 最终目标

- 初始状态一致（八围、技能等级、name_factor）。
- 对战过程一致（64/64 通过）。

## 与 sby_test.md 的关系

| 维度 | sby_test.md | diy_validation.md |
|------|-------------|-------------------|
| 比对对象 | TS(JS) vs Rust | 原始 vs DIY(均为 Rust) |
| 比对内容 | 对局 round-by-round | build 后初始状态 |
| 工具 | bun + out_md5.ts | tswn-cli to-diy |
| 目标 | Rust 对齐 JS 行为 | DIY 还原初始状态 |
| RNG 是否一致 | 是（同 seed） | 否（不同 name_base） |
