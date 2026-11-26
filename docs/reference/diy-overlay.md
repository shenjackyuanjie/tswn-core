# DIY / OL Overlay 说明

本文档描述当前 `tswn-core` 中已经落地的 DIY/OL overlay 行为。它面向两类使用场景：

- 手写 `+diy[...]` / `+ol:{...}` 覆盖玩家、幻影、使魔、丧尸的属性和技能。
- 使用 `tswn-cli to-diy` 把普通名字导出为可回读的 overlay，再继续调整。

实现入口主要在：

- [overlay.rs](../../crates/tswn_core/src/namerena/overlay.rs)：overlay 解析、编码与 minion overlay 结构。
- [input.rs](../../crates/tswn_core/src/namerena/input.rs)：从名字行里切出 `+diy[...]` / `+ol:{...}` 段。
- [build.rs](../../crates/tswn_core/src/namerena/build.rs)：把 overlay 应用到本体与幻影／使魔／丧尸的构造结果。
- [stats.rs](../../crates/tswn_core/src/namerena/stats.rs)、[skill.rs](../../crates/tswn_core/src/namerena/skill.rs)：属性与技能构成的最终值。
- [minions.rs](../../crates/tswn_core/src/runtime/handlers/minions.rs)、[skills_summon.rs](../../crates/tswn_core/src/runtime/combat/skills_summon.rs)：使魔／召唤物的运行期行为。
- [skills_control.rs](../../crates/tswn_core/src/runtime/combat/skills_control.rs)、[skill_loadout.rs](../../crates/tswn_core/src/runtime/entity/skill_loadout.rs)：幻影等分身技能的构造与技能表重建。

（旧 `crates/tswn_core/src/player/**` 是 0.x 时期布局，已随旧执行器一并删除。）

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
| `effective_skills` | `true` 根据原技能组构造条件抽样，也可填写手工权重对象，默认关闭 |
| `effective_skill_adjustment` | 最多十种双参数主动技的普通技能占用率，范围为 0 到 1 |
| `effective_skill_redistribution` | 条件抽样统一为 1；手工权重对象未指定动态参数时可取 0 到 1，默认 0 |
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

## 按等效熟练度抽取主动技

在普通玩家的 OL 或紧凑 DIY 中加入 `effective_skills: true`，可以从原技能组直接构造条件抽样配置。省略该字段或填写 `false` 时使用传统技能选择。

```text
PlayerName@Team+ol:{"effective_skills":true,"effective_skill_redistribution":1.0}
probe+ol:{"skills":{"sklclone":64,"sklfire":64},"effective_skills":true}
```

普通玩家的训练特征上限为 45 维。`effective_skills` 有 35 个主值（技能 ID 0 到 34）。自动构造的 `effective_skill_adjustment` 有十个固定位置，按技能 ID 排列为 `sklabsorb`、`sklhalf`、`sklexchange`、`sklheal`、`sklrevive`、`skliron`、`sklcharge`、`sklassassinate`、`sklclone`、`sklshadow`。缺失的技能和动态项填 0。两张表按技能名索引，字段重排不改变同一配置的抽样。原名字的整数结算等级、武器与分身加成仍正常构建。

- 单参数主动技沿用 SP2 的形式。原技能组从前到后构造，当前剩余量为 `R`、熟练度为 `L` 时，写入 `R × min(L, 128)`，再令 `R *= 1 - a_i × min(L, 128) / 128`。`a_i` 按传统扫描中技能实际占用行动机会的比例估计，并对十个评分环境等权平均。例如净化取 1，聚气约为 0.583。被动技保留原等级。
- 双参数主动技独立于上述递推，保存初始熟练度和它前面的普通技能占用率。模拟时用当前等级与发动门槛调整自身尝试概率，并用占用率估计前方普通技能的抢占概率。普通主动技按更新后的单参数权重共享剩余抽样区间。分身技能被扫到并命中时会占用行动，其位置仍会影响衰减前后的发动机会。分身实体沿用基准权重并按自己的当前等级缩放。
- 状态门槛挡住的技能在抽样前移除，`effective_skill_redistribution` 统一为 1。每次技能选择只取一个 24 位随机数，目标查找失败时复用它在命中区间里的相对位置。选目标与结算沿用各自的随机数。普攻使用剩余概率，训练向量不另设普攻维度。

### 从原始名单生成

```powershell
cargo build --profile release-fast -p tswn_core --features no_debug --bin tswn-cli
python scripts/construct_effective_skills.py names.txt effective-names.txt --features effective-features.jsonl
```

名单每行一个名字。`effective-names.txt` 可直接用于对局。`--features` 只调用 `to-diy` 构建并导出两张特征表，不运行对局或读取评分。若需将自动构造的特征展平成向量，按技能 ID 输出 35 项，再按上述固定顺序输出 10 项。`name` 和固定的 `effective_skill_redistribution` 为元数据。手工指定双参数时可为任意普通主动技添加占用率，解析器最多接受十项。

用同一组训练特征跑多项评分时，先逐号冻结成显式 DIY，例如 `target/release-fast/tswn-cli to-diy -f effective-names.txt -o frozen-names.txt --no-details`。显式 DIY 与训练特征来自单名字构建。把带 `effective_skills:true` 的输入直接加入队伍时，引擎从当局构建出的技能组计算权重，同公会队友的组队加成可能改变结果。

### 复测折减系数和评分环境

采样、系数分布、线性特征的备选方案和逐项评分结论见 [等效熟练度实验报告](../design/effective-skill-experiment-2026-11-19.md)。

`scripts/profile_effective_skills.py` 的 `observe` 阶段统计原版扫描，将按当前等级累计的门槛通过比例乘以命中后成功选目标的比例。门槛失败和目标失败分别记录继续选中技能与进入普攻分支的次数。采样可按本体、分身和智能状态拆分。`focus` 阶段加密指定名字的采样。`compare` 和 `confirm` 使用不同名册分片、不同种子，分别测试缩小队友集合和完整 Openbox 队友、靶子集合。系数估计只用扫描计数，评分用于比较构造方案。

```powershell
cargo build --profile release-fast -p tswn_core --features no_debug,effective-skill-stats --bin tswn-cli --example effective_skill_scores
python scripts/profile_effective_skills.py calibration.txt holdout.txt target/effective-skills-profile observe
python scripts/profile_effective_skills.py calibration.txt holdout.txt target/effective-skills-profile confirm
```

### 手工指定静态权重

也可用 `effective_skills` 对象直接指定静态权重。以下规则适用于未指定 `effective_skill_adjustment` 的权重对象。

```text
probe+ol:{"attrs":[86,86,86,86,86,86,116,300],"skills":{"sklclone":64,"sklfire":64,"sklshield":48},"effective_skills":{"sklclone":64,"sklfire":44.8}}
probe+diy[86,86,86,86,86,86,116,300]{"sklclone":64,"sklfire":64,"sklshield":48,"effective_skills":{"sklclone":64,"sklfire":44.8}}
```

- 权重接受非负小数，按技能固定槽位划分抽样区间。同优先级的技能钩子也按固定槽位排序，因此重排 `skills` 或 `effective_skills` 的字段会得到相同的战斗行为。
- 主动技权重和至多为 128 时，单项抽中概率为 `权重 / 128`，剩余概率用于普攻。权重和超过 128 时，按 `权重 / 权重和` 分配全部区间。未列出的主动技权重为零。
- 蓝量门槛通过后抽取一个 24 位随机数。抽中的技能仍检查状态和目标，无法施放时走普攻，技能结算及选目标继续使用各自的随机数。被动技能按各自的触发条件和整数等级结算。
- 可选的 `effective_skill_redistribution` 只在等效模式开启时生效。大于零时，抽样前先判定可直接检查的状态门槛，将被挡住的技能从候选区间移除，并从抽样总区间减去其权重乘以该比例。被挡住权重的其余部分留给普攻。技能目标仍在抽中后选择。紧凑 DIY 可将该字段和 `effective_skills` 并列写在同一个对象里。
- 若未另写某技能的原熟练度，初始结算等级取其权重向上取整。设置了原熟练度时，保留原来的技能效果和加成构成。对局中使用的权重为 `初始权重 × 当前等级 / 初始等级`，因此使用衰减、分身的等级变化和克隆体的加成重建都会反映到抽取概率上。
- 该模式适用于玩家及其分身，`to-diy` 可以往返保留配置。现有神经网络状态编码格式尚无抽样权重字段，因此这类对局应使用普通战斗输出或胜率评分采集结果。

### 使用评分校准静态权重的实验

实验脚本读取 Openbox 的单人靶、双人靶和五套带权队友预设，以及指定的扩展名单。候选公式在构造时读取原技能顺序，输出为每个名字的一组无序技能权重。各阶段统一写入 `effective_skill_redistribution: 1.0`，模型只需拟合技能权重。逐号选择以五项评分的最大误差为准，`--refine` 对偏差较大的名字追加十项联合校准。脚本为拟合和复核使用不同的轮次区间。

```powershell
cargo build --profile release-fast -p tswn_core --features no_debug --bin tswn-cli --example effective_skill_scores
python scripts/experiment_effective_skills.py D:/namerena/tswn-core/tests/sqp6000.txt --pair-n 12 --global-pair --refine --final-check
```

`target/effective-skills-redistribution/` 中包含每批的 `*-fitted.txt`、固定公式的 `*-global.txt`、评分明细和复核统计。新名单或不同预设用 `--out` 指定独立结果目录。

要将十项评分的逐号误差收紧到 2 个百分点，可以继续运行下面的脚本。它使用 NumPy 和 SciPy，先实测各权重的局部变化，再搜索使十项最大误差尽量小的权重组合。搜索阶段使用候选队友缩小配队矩阵，复核阶段用独立轮次和完整靶池及队友预设。CQD 和 SQP 中的同名样本共用一组权重。

```powershell
python scripts/calibrate_effective_skills.py
```

用 `--previous` 可以指定已有的 `*-refined.txt` 或 `*-final.txt` 作为初始权重。脚本先把所有输入的重分配比例设为 1，再复核十项评分并调整超差名字的权重。结果写入 `target/effective-skills-unified/`，包括 `*-final.txt`、记录复核轮次和场数的 `final-scores.csv`、超差名单 `final-over-2.csv` 和技能重排检查 `order-check.json`。达到校准轮数上限仍有超差时，脚本保留报告并返回失败。

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

1. 复制本体的技能布局与保存的构造元数据，不重新解析名字或随机 build。
2. 对每个技能取 `min(build_level, owner_current_level)` 作为重建基底。
3. 根据 `SkillBoost` 元数据重放加成：LastBoost 为基底 ×2；SlotBoost 为基底 + `min(saved_boost, 基底)`。
4. 分身技能自身另以本体本次衰减后等级的 `ceil(sqrt(level))` 覆盖。

无加成技能保留等级下降；带加成技能可能恢复一部分等级，但不会无条件重置到初始值。实现见 [SkillLoadout::rebuilt_for_clone](../../crates/tswn_core/src/runtime/entity/skill_loadout.rs#L508-L525)，score profile 的特殊重建与数值示例见 [分身机制](../mechanics/clone.md)。

## Boss 与特殊玩家

Overlay 不会改变玩家类型：

- `name@!` 中的已知 Boss 仍是 `PlayerClass::Boss`，Boss 免疫和专属运行时逻辑仍按 Boss 类型判断。
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

机器回读应使用 `-o/--out-file` 保存转换正文：`to-diy` 的 stdout 仍有 CLI 欢迎 banner，`--no-details` 只关闭额外属性详情，不关闭 banner。文件输出只写转换结果，见 [to_diy.rs](../../crates/tswn_core/src/bin/tswn_cli/to_diy.rs#L10-L28)。

## 验证

相关测试集中在：

- [input.rs](../../crates/tswn_core/src/namerena/input.rs)（`parses_identity_weapon_and_overlay_without_splitting_quoted_plus`）
- [build.rs](../../crates/tswn_core/src/namerena/build.rs)（`native_minion_blueprints_apply_overlay_data`）
- [parse.rs](../../crates/tswn_core/src/cli_api/parse.rs)（`+diy` / `+ol` 段解析与导出）
- [custom_runner_import_tests.rs](../../crates/tswn_core/src/runtime/tests/custom_runner_import_tests.rs)、[plain_raw_import_tests.rs](../../crates/tswn_core/src/runtime/tests/plain_raw_import_tests.rs)

常用验证命令：

```bash
cargo test -p tswn_core namerena::input::tests::parses_identity_weapon_and_overlay_without_splitting_quoted_plus --lib -- --test-threads=1
cargo test -p tswn_core namerena::build::tests::native_minion_blueprints_apply_overlay_data --lib -- --test-threads=1
cargo test -p tswn_core runtime::tests::custom_runner_import_tests --lib -- --test-threads=1
cargo test -p tswn_core --lib -- --test-threads=1
```

往返和对战过程见 [DIY 往返验证](../guides/diy-validation.md)：集中维护 JSONL 采样、差异分级、历史工具状态与运行记录。
