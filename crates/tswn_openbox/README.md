# tswn_openbox

当前版本：`0.4.2`

`tswn_openbox` 是一个带 GUI 的本地交互面板，把常用 `tswn-cli` 工作流做成点击即用的界面。目标是能跑、无使用门槛、界面简洁。

当前支持：

- `to-diy`
- `namer-pf`
- `cqd/cqp`，对应原 `bench batch-rate`
- `pair`

## 运行

在 workspace 根目录运行：

```powershell
cargo run -p tswn_openbox
```

推荐的 release 构建命令：

```powershell
cargo build -p tswn_openbox --release --features "no_debug,mimalloc_alloc"
```

构建完成后可直接双击：

```text
target\release\tswn_openbox.exe
```

Windows GUI 构建启用 `windows_subsystem = "windows"`，双击启动时不会额外挂出控制台窗口。

## 界面

左侧是设置区，右侧只显示运行日志。常用设置直接展示，低频设置放在“更多设置”弹窗中。顶部主题按钮支持浅色、深色和跟随系统；“关于”弹窗会显示 Openbox 与 Core 的当前版本，并提供 GitHub 项目链接。

设置旁的圆形 `i` 是上下文帮助：鼠标悬浮会显示简要说明，点击后会把说明固定在独立小窗口中，方便对照设置。

普通设置包括：

- 精确度：`1%` / `10%` / `100%`，分别对应 `100` / `1000` / `10000` 场。
- `cqd/cqp` 的“每组胜率”。
- `pair` 的“每组 cqp”和“有效 cqp”。
- 输出文件、输出格式、日志阈值和文件阈值。

更多设置包括：

- 场数和精确度二选一。
- 线程数；默认“系统线程 * 1.5”，显示上等价线程 `0`。
- “不低估短号”，对应 `--keep-rq`。
- `JSONL (--log)`、输出小数位、手动靶子、手动队友等高级选项。
- “高亮超强名字”，用于把超过阈值的屏幕输出行标红。

运行时进度条会显示进度、速度和预计剩余时间。正在运行的任务可以点击“停止”取消。

## 输入

文本输入支持两种方式：

- 直接在面板中输入。
- 勾选“从文件中读取”后选择文件。

从文件读取时，界面只预览前 10 行，运行时会读取完整文件。所有多行显示框不自动换行，内容过长时使用横向滚动。

## 输出

`cqd/cqp` 和 `pair` 需要先选择输出文件。输出格式与 CLI 选项对应：

- `分数 名字`：默认格式。
- `JSONL (--log)`：对应 `--log`，在更多设置中展示。
- `名字 (--pure)`：对应 `--pure`。

阈值对应关系：

- `日志阈值` 对应 `--min-screen`，控制右侧日志显示。
- `文件阈值` 对应 `--min-file`，控制写入输出文件的结果。

## 功能说明

### to-diy

支持普通导出、旧 `+diy` 导出和“召唤物diy”（对应 `--minions`）。单名详情只在更多设置中展示。

### namer-pf

支持 `pp`、`pd`、`qp`、`qd`、`sum` 五项输出。每项可以分别配置：

- 是否输出到屏幕。
- 屏幕阈值。
- 是否输出到文件。
- 文件阈值。
- 高亮超强名字阈值。

“不低估短号”默认关闭，对应 `--keep-rq`。“保留小数点后 X 位”在更多设置中配置，默认值为 `0`，对应 CLI `namer-pf --precision`。开启后会保留计算得到的真实小数分数，再统一格式化到普通评分和技能榜的屏幕/文件输出；不会只在整数后补零。

#### 技能榜

`namer-pf` 支持“技能榜”输出。开启后，程序会把名字转为 `+diy` 形式，找到熟练度最高的技能，并按 `setting\score_now.toml` 的阈值筛选输出。若待评名字或组合的全部技能熟练度均小于 30，还会按 `[lessskl]`（白板号）阈值筛选。

屏幕输出为蓝字，格式为：

```text
技能名指标 分数 名字
```

例如：

```text
冰冻qp 6647 某个名字
冰冻全能 32721 某个名字
```

`score_now.toml` 中每个技能可配置：

```toml
[sklice]
pp = 8650
qp = 6647
qd = 8210
all = 32721
```

低熟练度白板号使用单独的 `[lessskl]` 配置：

```toml
[lessskl]
pp = 8542
qp = 6357
qd = 6819
```

字段含义：

- `pp`：普评阈值。
- `qp`：强评阈值。
- `qd`：强单阈值。
- `all`：全能总分阈值。

“全能”除满足对应 `all` 外，还需要同时满足 `pp >= 8000`、`pd >= 9000`、`qp >= 6000`、`qd >= 7000`。
`[lessskl]` 可以只配置需要筛选的指标；默认白板号阈值暂不配置 `all`。

### cqd/cqp

对应原 `bench batch-rate`。普通设置中保留常用选项，更多设置中可以切换手动靶子、`DIYcqp（++分割名字）`、线程数、场数和输出细节。

靶子预设可通过 `factor_enabled = true` 启用带权模式。带权靶子使用 TOML 文件，每个 `[[targets]]` 项包含一个有限正数权重 `factor` 和玩家数组 `players`。最终平均胜率按有效对局的
`sum(胜率 * factor) / sum(factor)` 计算。

带权模式下，如果选手组和靶子组包含完全相同的玩家（不要求顺序相同），该组直接记为 `50%` 并参与加权；只有部分玩家相同时仍正常计算，不会按重名跳过。未启用带权模式时保持原有重名跳过行为。

不勾选“每组胜率”时输出：

```text
平均胜率 名字
```

勾选“每组胜率”时输出：

```text
平均胜率 名字
  胜率 名字
```

### pair

普通设置里不显示靶子选择；默认使用 `settings.toml` 中 `id = 2` 的靶子。靶子选择和手动靶子只在更多设置中展示。

队友默认从 `settings.toml` 的 `teammate` 字段导入。普通设置只显示导入的队友选项，不显示预览；手动输入和从文件读取队友只在更多设置中展示。

`pair` 的 cqp 详情有三个模式：

- 不显示 cqp。
- 每组 cqp：可设置 `cqp阈值`，只显示超过阈值的队友组合。
- 有效 cqp：只显示该名字最高 `head` 个队友组合后的 cqp。

勾选 cqp 详情时输出形如：

```text
最终分数 名字
  cqp 队友名字
```

`pair` 支持带权靶子 TOML，靶子权重会用于每个队友组合的加权平均。选手和队友都支持一行多个玩家；更多设置中可分别勾选 `++` 分割。默认选手按单个 `+` 分割，队友按 `++` 分割。

输入按“每行一个组合”处理。未启用 `++` 分割时，行内使用单个 `+` 连接成员；启用后使用 `++` 连接成员。两个开关相互独立：选手输入和队友输入可以使用不同的分隔符。例如，选手输入（默认单个 `+`）可以写成：

```text
a@team+b@team
```

队友输入（默认 `++`）可以写成：

```text
c@team++d@team
```

每个组合会把选手组和队友组拼接后，与每个靶子组进行计算。普通文本靶子仍按每行一个靶子组解析；带权 TOML 则按每个 `[[targets]]` 项的 `players` 作为靶子组，靶子 `factor` 只影响靶子组之间的平均值。手动靶子不启用带权 TOML，选择手动靶子时会忽略预设的 `factor_enabled`。

队友预设启用 `factor_enabled` 时，计算顺序是：先按靶子权重得到该队友组合的平均胜率，再乘以队友组的 `factor`，然后按乘权后的分数降序取前 `head` 个并求和。因此，队友权重会影响排名和最终分数，而不是只影响展示的平均胜率。手动输入队友时不会读取队友 TOML 权重。

## 配置文件

面板会从当前目录的 `setting\settings.toml` 读取靶子和队友预设。路径相对于 `setting` 目录解析。

如果文件不存在，启动时会从内嵌资源自动写入默认 `settings.toml` 和默认预设文本；如果文件存在但格式损坏，会在界面中显示警告。

示例：

```toml
[[targets]]
id = 1
name = "默认靶子"
file = "targets/default.txt"

[[targets]]
id = 2
name = "pair默认靶子"
file = "targets/pair-default.txt"

[[targets]]
id = 3
name = "带权二人组"
file = "targets/weighted-pairs.toml"
factor_enabled = true

[[teammate]]
head = 3
name = "默认队友"
file = "teammates/default.txt"

[[teammate]]
head = 3
name = "带权队友"
file = "teammates/weighted.toml"
factor_enabled = true
```

说明：

- `targets[].id` 期望是数字。
- `targets[].file` 是靶子列表文件。
- `targets[].factor_enabled` 可以省略，默认为 `false`；设为 `true` 时，`file` 必须使用下述带权 TOML 格式。
- `teammate[].head` 是 `pair` 的“保留前几”。
- `teammate[].file` 是队友列表文件。
- `teammate[].factor_enabled` 可省略，默认为 `false`；设为 `true` 时，`file` 使用与带权靶子相同的 `[[targets]]` TOML 格式，并按每组 `factor` 调整队友组合分数后再取 `head`。
- 选择“手动队友”后，预设中的 `factor_enabled` 和队友文件权重均不会生效；手动队友始终按文本列表解析。
- `pair` 默认优先选择 `targets` 中 `id = 2` 的靶子；如果不存在，则退回第一个靶子。
- `pair` 的选手和队友分组开关只影响行内分隔，不会改变换行分组；每行仍对应一个待评分的组合。

带权靶子文件示例：

```toml
[[targets]]
factor = 1.5
players = ["mario", "luigi"]

[[targets]]
factor = 0.75
players = ["peach", "fire"]
```

队友预设开启 `factor_enabled = true` 时，队友文件也使用上述 `[[targets]]` TOML 结构；每个队友组的平均胜率先乘以该组 `factor`，再按 `head` 取最高组合求和。

`factor` 必须是大于 `0` 的有限数值，`players` 不得为空或包含空名字。带权 TOML 用于选中的 `cqd/cqp` 或 `pair` 靶子预设；手动靶子仍使用原文本格式。

`setting\score_now.toml` 用于 `namer-pf` 技能榜阈值。仓库中提供了一份示例/当前阈值文件。

## 字体

界面字体内嵌使用：

```text
crates\tswn_openbox\src\SarasaMonoSC-Regular.ttf
```

## 实现说明

源码按职责拆分：

- `src/app.rs` 和 `src/app/`：UI 状态、控件和任务启动。
- `src/backend.rs` 和 `src/backend/`：解析、执行、输出格式化和文件写入。

这样后续继续对齐 `tswn-cli` 能力时，可以把 UI 和业务逻辑分开维护。

## 0.3.9 说明

`0.3.9` 重点修复 `cqd/cqp` 大批量双人组运行时的内存增长问题。Openbox 后端的批量胜率路径现在使用 uncached prepared runner，不再把 `tests\allCO3pure.txt` 这类高唯一度 matchup 写入全局 prepared 缓存。

同时，`cqd/cqp` 的“每组胜率”和 `pair` 的“每组 cqp / 有效 cqp”屏幕输出统一为块状格式：

```text
分数 名字
  分数 名字
  分数 名字
  分数 名字

分数 名字
  分数 名字
  分数 名字
  分数 名字
```

为了避免长任务继续积压内存，GUI 事件通道和后端 worker 事件通道都改为有界队列，主日志也会保留最近一段内容而不是无限增长。输出文件不受日志裁剪影响。

## 内存排查

仓库提供 `openbox_mem_probe` 调试入口，用于复现 Openbox 后端 `cqd/cqp` 路径并采样 RSS：

```powershell
cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox/assets/targets/target2.txt --limit 10000 --target-limit all --count 1 --threads 8 --report-ms 2000
```

`0.3.9` 修复后，`allCO3pure.txt` 取 10000 组、`target2.txt` 全 41 个靶子、共 410000 个 matchup 的测试中，RSS 运行中稳定在约 `15-16 MB`，结束约 `9.1 MB`。
