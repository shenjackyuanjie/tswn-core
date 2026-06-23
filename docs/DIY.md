# DIY / OL Overlay 说明

本文档描述当前 `tswn-core` 中已经落地的 DIY/OL overlay 行为。它面向两类使用场景：

- 手写 `+diy[...]` / `+ol:{...}` 覆盖玩家、幻影、使魔、丧尸的属性和技能。
- 使用 `tswn-cli to-diy` 把普通名字导出为可回读的 overlay，再继续调整。

实现入口主要在：

- `crates/tswn_core/src/player/overlay.rs`
- `crates/tswn_core/src/player/impl_ctor.rs`
- `crates/tswn_core/src/player/impl_attr.rs`
- `crates/tswn_core/src/player/skill.rs`
- `crates/tswn_core/src/player/skill/act/minion.rs`
- `crates/tswn_core/src/player/skill/act/summon.rs`
- `crates/tswn_core/src/player/skill/act/clone.rs`

## 基本格式

### 紧凑 DIY 格式

```text
PlayerName+diy[72,39,69,76,67,66,0,84]{"sklfire":5}
```

`diy[...]` 中必须有 8 个整数，顺序为：

```text
[atk, def, spd, agi, mag, res, wis, maxhp]
```

前七围使用 JS 兼容编码：文本中的值会在解析时 `-36` 后取非负；HP 原样保留。因此上例解析后的内部属性为：

```text
[36, 3, 33, 40, 31, 30, 0, 84]
```

### OL JSON 格式

```text
PlayerName+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklfire":5},"name_factor_enabled":true}
```

`ol.attrs` 也使用同一套属性编码：前七围解析时 `-36`，HP 原样保留。也就是说：

```json
"attrs":[86,86,86,86,86,86,86,300]
```

代表内部属性：

```text
[50, 50, 50, 50, 50, 50, 50, 300]
```

`ol:{...}` 支持的顶层字段：

| 字段 | 含义 |
| ---- | ---- |
| `attrs` | 玩家八围覆盖，使用前七围 +36 编码 |
| `skills` | 玩家技能覆盖，有序 object，顺序就是行动尝试顺序 |
| `weapon` | 记录武器名；当玩家级 `attrs` 或 `skills` 存在时不计入武器效果 |
| `name_factor_enabled` | 是否启用名字系数，默认 `true` |
| `shadow` / `phantom` / `幻影` | 幻影模板 |
| `summon` / `familiar` / `使魔` | 使魔模板 |
| `zombie` / `丧尸` / `僵尸` | 丧尸模板 |

解析时如果同一个名字里同时有武器段和 overlay 段，overlay 通过 `+` 分隔：

```text
PlayerName@Team+weapon+ol:{"attrs":[86,86,86,86,86,86,86,300]}
```

当 overlay 提供玩家级 `attrs` 或 `skills` 时，`weapon_state` 强制为 `None`，武器不参与属性和技能构建。只有纯召唤物模板 overlay 不会单独禁用武器。

## 技能值格式

`skills` 是有序 JSON object。字段顺序会保留为行动时的技能尝试顺序；未列出的技能按固定槽位顺序追加到末尾。

技能值支持三种写法：

| 写法 | 内部含义 | 最终等级 |
| ---- | -------- | -------- |
| `5` | `SkillBoost::Normal(5)` | `5` |
| `"40+30"` | `SkillBoost::SlotBoost { base: 40, boost: 30 }` | `70` |
| `"2*46"` | `SkillBoost::LastBoost(46)` | `92` |

示例：

```text
PlayerName+ol:{
  "attrs":[86,86,86,86,86,86,86,300],
  "skills":{
    "sklfire":5,
    "sklheal":"40+30",
    "sklshadow":"2*46"
  }
}
```

## 普通玩家技能

普通玩家实际技能 ID 为 `0..34`；`35` 是 `NoneSkill` 占位，`36..39` 是保留槽位。技能名大小写不敏感，可写 `fire`、`sklfire` 或 `skillfire`。

| ID | 名称 | ID | 名称 | ID | 名称 |
| -- | ---- | -- | ---- | -- | ---- |
| 0 | `sklfire` | 1 | `sklice` | 2 | `sklthunder` |
| 3 | `sklquake` | 4 | `sklabsorb` | 5 | `sklpoison` |
| 6 | `sklrapid` | 7 | `sklcritical` | 8 | `sklhalf` |
| 9 | `sklexchange` | 10 | `sklberserk` | 11 | `sklcharm` |
| 12 | `sklhaste` | 13 | `sklslow` | 14 | `sklcurse` |
| 15 | `sklheal` | 16 | `sklrevive` | 17 | `skldisperse` |
| 18 | `skliron` | 19 | `sklcharge` | 20 | `sklaccumulate` |
| 21 | `sklassassinate` | 22 | `sklsummon` | 23 | `sklclone` |
| 24 | `sklshadow` | 25 | `skldefend` | 26 | `sklprotect` |
| 27 | `sklreflect` | 28 | `sklreraise` | 29 | `sklshield` |
| 30 | `sklcounter` | 31 | `sklmerge` | 32 | `sklzombie` |
| 33 | `sklupgrade` | 34 | `sklhide` | 35 | `sklnone` |

普通玩家 overlay 中如果只出现普通技能名，技能槽顺序固定为 `0..39`，行动顺序按 `skills` 字段顺序优先。

## 分类技能通道

DIY/OL 里有三类技能需要分开存放：

1. 普通玩家技能：`sklfire`、`sklsummon`、`sklclone` 等。
2. 使魔固定技能：火球 1、火球 2、自爆。
3. 幻影技能：附体。

为了避免吞噬时把不同类别按同一编号串槽，当前实现会在需要时进入“分类技能通道”：

| 类别 | 玩家 overlay key | 玩家内部 key | 使魔模板内部 key |
| ---- | ---------------- | ------------ | ---------------- |
| 普通技能 | `sklfire` 或 `normal:sklfire` | `0..39` | `80..119` |
| 使魔火球 1 | `summon:sklfire1` 或 `sklfire1` | `40` | `40` |
| 使魔火球 2 | `summon:sklfire2` 或 `sklfire2` | `41` | `41` |
| 使魔自爆 | `summon:sklexplode` 或 `sklexplode` | `42` | `42` |
| 幻影附体 | `phantom:sklpossess` 或 `sklpossess` | `43` | `43` |

例子：

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,300],
  "skills":{
    "sklfire":3,
    "summon:sklfire1":5,
    "summon:sklfire2":7,
    "summon:sklexplode":11,
    "phantom:sklpossess":13
  }
}
```

吞噬仍按固定槽位逐位抬等级，这是 JS 行为；区别是这些特殊技能会占用独立槽位，因此普通 `sklfire` 不会和使魔火球 1 互相覆盖。

## 召唤物模板

`shadow`、`summon`、`zombie` 都使用 `MinionOverlay`：

```json
{
  "attrs": [86,86,86,86,86,86,86,300],
  "skills": {"sklrapid":7},
  "reuse_skills_on_recast": true,
  "inherit_owner_def_res": true,
  "shadow": {...},
  "summon": {...},
  "zombie": {...}
}
```

字段说明：

| 字段 | 适用对象 | 含义 |
| ---- | -------- | ---- |
| `attrs` | 全部召唤物 | 召唤物八围覆盖，仍使用前七围 +36 编码 |
| `skills` | 全部召唤物 | 召唤物技能覆盖，有序 object |
| `reuse_skills_on_recast` | 使魔 | 血祭重施复用已死亡使魔时，是否复用现有技能对象 |
| `inherit_owner_def_res` | 使魔 | 在 `attrs` 覆盖后，是否把防御和魔抗替换为施法者当前值 |
| `shadow` / `summon` / `zombie` | 全部召唤物 | 传给该召唤物未来生成的子召唤物模板 |

`reuse_skills_on_recast` 默认 `false`。`to-diy --minions` 导出的使魔模板会写 `true`，用于贴近原始血祭重施时复用同一个使魔对象的行为。手写模板时，如果希望每次重施都重新套用 `summon.skills`，可以省略或设为 `false`。

`inherit_owner_def_res` 默认 `false`。普通血祭派生的默认属性本来就继承施法者防御和魔抗；当你手写 `summon.attrs` 后，只有显式设为 `true` 才会在覆盖属性后再次继承这两项。`to-diy --minions` 在需要保持原始行为时会导出该字段。

### 幻影模板

幻影默认只有附体技能。手写时可以直接写：

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,300],
  "shadow":{
    "attrs":[46,47,48,49,50,51,52,200],
    "skills":{"sklpossess":9}
  }
}
```

如果给幻影模板写普通技能，未加前缀的普通技能名也可以识别：

```json
"shadow":{"skills":{"sklrapid":7,"sklpossess":9}}
```

当模板中出现 `normal:` / `summon:` / `phantom:` 等前缀时，幻影也会进入分类技能通道。

### 使魔模板

未使用分类前缀时，`summon.skills` 只接受三个固定槽位名：

| 槽位名 | 含义 |
| ------ | ---- |
| `sklfire1` | 第一个固定火球槽位 |
| `sklfire2` | 第二个固定火球槽位 |
| `sklexplode` | 自爆槽位 |

旧别名如 `sklfire`、`fire1`、`explode` 会被忽略。0 熟练度技能导出时会省略，但内部固定槽位仍保留为 `[sklfire1, sklfire2, sklexplode]`，吞噬使魔时仍按固定槽位继承等级。

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,300],
  "skills":{"sklsummon":10},
  "summon":{
    "attrs":[36,86,56,55,36,89,88,89],
    "skills":{"sklfire2":4,"sklfire1":"2*14"},
    "reuse_skills_on_recast":true,
    "inherit_owner_def_res":true
  }
}
```

要给使魔配置普通玩家技能，必须给普通技能加 `normal:` 前缀。只要 `summon.skills` 中出现分类前缀，就会使用分类使魔通道：

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,400],
  "summon":{
    "attrs":[60,60,60,60,60,60,60,240],
    "skills":{
      "normal:sklsummon":255,
      "normal:sklclone":120,
      "normal:sklrapid":9,
      "sklfire1":5,
      "summon:sklexplode":3,
      "phantom:sklpossess":13
    }
  }
}
```

上例中：

- `normal:sklsummon` 让使魔可以继续血祭生成子使魔。
- `normal:sklclone` 让使魔可以分身。
- `sklfire1` 和 `summon:sklexplode` 仍是使魔固定技能槽。
- `phantom:sklpossess` 是独立的幻影附体槽，不会和普通技能串槽。

### 丧尸模板

丧尸模板和幻影模板一样，未加前缀时可直接写普通技能：

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,300],
  "skills":{"sklzombie":255},
  "zombie":{
    "attrs":[40,41,42,43,44,45,46,90],
    "skills":{"sklrapid":7}
  }
}
```

## 召唤物的召唤物

召唤物模板可以继续嵌套 `shadow` / `summon` / `zombie`，用于配置“召唤物的召唤物”。

```text
owner+ol:{
  "attrs":[86,86,86,86,86,86,86,400],
  "summon":{
    "attrs":[60,60,60,60,60,60,60,240],
    "skills":{"normal:sklsummon":255},
    "summon":{
      "attrs":[70,71,72,73,74,75,76,333],
      "skills":{"normal:sklrapid":11,"sklfire1":9}
    }
  }
}
```

行为要点：

- 父使魔生成时应用外层 `summon.attrs` / `summon.skills`。
- 父使魔获得的运行时 overlay 只保留外层模板里的子模板字段。
- 父使魔之后再释放血祭时，子使魔会应用内层 `summon` 模板。

## 伤害传导与分身

血祭生成的使魔在未处于蓄力状态时，会带一个内部伤害分摊技能：

- 使魔受到伤害后，会把本次伤害的一半传给 `MinionRuntimeState.owner`。
- 使魔召唤出的子使魔按直接来源链路传导：子使魔传给父使魔，父使魔再传给主名字。
- 处于蓄力状态时，血祭使魔的伤害分摊技能会被关闭。

使魔分身有一条特殊规则：

- 使魔的克隆体仍按 root owner 命名，并随 root owner 体系清理。
- 如果来源使魔拥有伤害分摊技能，克隆体受到伤害时会直接把一半伤害传给主名字，而不是先传给使魔本体。

也就是说，血祭使魔分身后：

```text
使魔克隆受伤 -> 主名字受一半伤害
```

而不是：

```text
使魔克隆受伤 -> 使魔本体受一半伤害 -> 主名字再受四分之一伤害
```

## 分身与 DIY 技能加成

DIY 玩家分身时，克隆体不会简单复制初始 overlay 等级。当前流程是：

1. 按克隆体自己的名字重新 build。
2. 应用本体 overlay 技能配置。
3. 将克隆体技能截断到本体当前战斗中的技能等级。
4. 根据 `SkillBoost` 元数据重新执行 `"2*base"` 或 `"base+boost"` 加成。

这保证了 `sklshadow`、`sklclone`、`sklheal`、`sklrevive` 等会在战斗中衰减的技能，不会因为分身而刷新回初始熟练度。

## Boss 与特殊玩家

Overlay 不会改变玩家类型：

- `name@!` 中的已知 Boss 仍是 `PlayerType::Boss`，Boss 免疫和专属运行时逻辑仍按 Boss 类型判断。
- 如果 Boss overlay 提供 `attrs`，这些属性会直接覆盖普通构建结果，不再额外叠加 Boss `appendAttr`。
- 如果 Boss overlay 提供 `skills`，会走 DIY 技能覆盖；这会绕过 Boss 默认“普通技能等级全为 0”的构建分支。
- `Test1` / `Test2` / `TestEx` 的 `name_factor` 仍强制为 0；`name_factor_enabled:false` 也会强制为 0。

## 导出命令

默认导出 `+ol:{...}`：

```bash
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire"
```

导出旧紧凑格式：

```bash
cargo run -p tswn_core --bin tswn-cli -- to-diy -r "mario@team+fire" --old
```

批量导出：

```bash
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt -o diy.txt
```

连同召唤物模板一起导出：

```bash
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt --minions -o diy.txt
```

`--minions` 只支持 `+ol` 输出，不能和 `--old` 同时使用。生成的模板会省略 0 熟练度技能；回读时缺失技能会按固定槽位补齐为 0。

## 验证

相关测试集中在：

- `crates/tswn_core/src/player/test/basic.rs`
- `crates/tswn_core/src/player/test/minions.rs`

常用验证命令：

```bash
cargo test -p tswn_core player::test::basic::player_raw_new_parses_diy_overlay --lib -- --test-threads=1
cargo test -p tswn_core player::test::minions --lib -- --test-threads=1
cargo test -p tswn_core --lib -- --test-threads=1
```

往返和对战过程验证见：

- `docs/howto/diy_validation.md`
- `track_diy_roundtrip.py`
- `crates/tswn_core/src/bin/track_diy_roundtrip.rs`
