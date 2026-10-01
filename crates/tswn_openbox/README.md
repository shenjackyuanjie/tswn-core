# tswn_openbox

当前版本：`0.4.6`

`tswn_openbox` 是一个带 GUI 的本地交互面板，把常用 `tswn-cli` 工作流做成点击即用的界面。目标是能跑、无使用门槛、界面简洁。

同时提供 `openbox-cli` 无头命令行：与 GUI 共用同一套后端与配置，保留稳定的文本及文件输出，用于把面板工作流脚本化。

当前支持：

- `to-diy`
- `namer-pf`
- `cqd/cqp`，对应原 `bench batch-rate`
- `pair`
- `DS4`：工作目录中的评分、增量配对与实战筛选

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

## openbox-cli（无头 CLI）

```powershell
cargo run -p tswn_openbox --bin openbox-cli -- --help
```

`openbox-cli` 复用 GUI 的后端与预设（`tswn_openbox_backend` crate），
因此与面板共用解析、执行、文件输出格式、输出排序、标签清洗，
以及同样的配置约定——靶子/队友预设读 `./setting/settings.toml`（缺失时
自动释放内嵌默认资源），技能榜阈值默认读 `./setting/score_now.toml`。
数据行走 stdout，进度与状态走 stderr，方便管道与重定向。

以下四个子命令对应原有 GUI 面板；DS4 保留独立的统一 `tswn_ds4` CLI 出口：

```powershell
# to-diy：-r 单号默认追加详情日志；--no-details 关闭；-f 批量不输出详情；--old / --minions 同 GUI
openbox-cli to-diy -r "mario@team+fire"
openbox-cli to-diy -r "mario@team+fire" --no-details
openbox-cli to-diy -f names.txt --minions -o diy.txt

# namer-pf：默认五项上屏；--metric 逐项阈值/文件；--skill-board 技能榜
openbox-cli namer-pf -f names.txt --metric sum --metric pp:8000
openbox-cli namer-pf -f names.txt --metric pp:8000:pp.txt:7500 --no-screen
openbox-cli namer-pf -f names.txt --skill-board --skill-board-out board.txt
openbox-cli namer-pf -f names.txt --skill-board custom-thresholds.toml

# cqd/cqp：靶子用预设（--target-preset ID）或手动文件；默认输出每组胜率明细
openbox-cli cqd --target-preset 2 -p players.txt -n 10000
openbox-cli cqp -l targets.txt -p players.txt --no-show-matchups --min-screen 60.5

# pair：默认 id=2 靶子预设 + 第一个队友预设（head/factor 一并生效）
openbox-cli pair -p players.txt --teammate-preset 刺评 --head 4
openbox-cli pair -l targets.txt -p players.txt --teammates mates.toml --detail top
```

与 GUI 的刻意差异（均为 CLI 合理性考虑）：高亮颜色降级为普通行；
未实现“停止”按钮（Ctrl+C 直接终止进程）。预设系统、带权靶子/队友、
技能榜、mirror 50%、重名跳过、输出排序等行为与 GUI 完全一致。

## 界面

左侧是设置区，右侧展示实时日志与结构化结果。常用设置直接展示，低频设置放在“更多设置”弹窗中。顶部主题按钮支持浅色、深色和跟随系统；“关于”弹窗会显示 Openbox 与 Core 的当前版本，并提供 GitHub 项目链接。

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

## 实时日志与结果视图

右侧提供三种可随时切换的展示方式，选项顺序保持“纯文本、卡片、表格”。
`to-diy` 与 `pair` 默认卡片，便于查看属性、技能或队友明细；`namer-pf` 与 `cqd/cqp` 默认表格，便于比较评分与胜率。
各页独立保存视图选择，关闭再打开应用也会恢复。

- **纯文本**：沿用 `9473188ca61068a605ecd64075aebf196052c264` 的结果格式：DIY 导出行和「原始信息」详情块，评分的 `名字 指标:分数`，胜率/配队的 `分数 名字` 和缩进明细；不再逐项插入「预览」「完成」前缀。完整结果块完成后追加，空行和复制内容保留。
- **卡片**：每个输入组一张卡片，点击标题展开明细；默认左对齐，可在「排版设置」切换左、中、右对齐。DIY 详情按成员分组，八项属性合并为两行，技能每行最多四项，不再逐项重复名字。长导出行截断为单行，悬停查看完整内容，不撑坏虚拟列表行高。
- **表格**：展示名字、状态与评分列，点击名字查看明细。DIY 不显示无意义的空分数列；少量结果时详情紧接表格，避免半屏空白。`namer-pf` 分别展示五项指标；可在「排版设置」逐列调整宽度和左/中/右对齐，也可拖动表头右边界调整列宽。表头与数据共享横向滚动，避免错列。

界面偏好自动保存到 eframe 的用户应用存储，正常退出时保存，运行期间每 5 秒自动保存一次。
保存范围为主题、当前工具、各工具视图模式、跟随开关、卡片对齐、表格列宽与对齐，以及窗口位置/尺寸、输入面板宽度等 egui 布局状态。
不保存输入内容、计算参数、日志或运行状态；清空日志、重新运行都不会重置排版。「恢复默认排版」只重置当前视图的排版。

视图选项旁的“说明”可固定查看预览、并发顺序、筛选与日志复制规则。
状态使用蓝色预览、绿色完成、琥珀色未完成；红色表示高亮结果或明确标注的错误，技能榜使用蓝色。
深浅主题分别调整颜色对比度，始终保留状态文字；工具输入、阈值及操作按钮也提供简短说明和悬停提示。

卡片和表格中，`to-diy` 每行完成即显示导出及已启用的属性／技能详情；`namer-pf` 每项指标完成即显示；
`cqd/cqp` 每个靶子的完整胜率计算结束即显示；`pair` 每个队友组合的所有靶子结束后即显示 cqp。
这些更新不等待整批或整个计算窗口结束，但不会逐场战斗刷新。

“每组胜率”和“每组 cqp”的开关及明细阈值仍生效。“有效 cqp”在运行中展示**当前 Top**，
全部队友算完后才确定最终排名和总分。同分按原队友输入顺序排列；加权计算不受线程完成顺序影响。

设置日志阈值时先在卡片／表格预览明细，总分完成后移除未达标名字；纯文本只追加通过筛选的完整结果块。
文件仍严格按文件阈值输出。停止时结构化视图保留已完成明细，未完成组标为
“已停止 · 结果不完整”，不会把部分靶子或部分队友的得分显示为最终分数。

常态约每 100ms 获取一批更新，繁忙时分帧处理，以计算吞吐为优先。三种视图共享结果，
只绘制可见行；纯文本保留最近约 4MiB，结构化结果及后台待消费队列分别限制为 8MiB。
达到上限时裁剪较早内容并显示提示；输出文件不受展示裁剪影响。
“复制日志”复制当前保留的文本流水，“清空日志”同时清空三种视图。
“跟随最新”控制自动滚动，向上滚动查看历史时会关闭跟随。

GUI 的纯文本结果格式与 CLI 保持一致；并发任务可能交错到达，显示顺序不承诺与 CLI 相同。
结构化实时预览与文本结果互不混排，输出文件保持原有约定。

### 界面截图校验（可选 feature）

`ui_capture` 默认关闭，不进入普通 GUI 构建。需要复现截图时显式启用：

```powershell
cargo run -p tswn_openbox --bin tswn_openbox --features ui_capture -- --capture-dir target/openbox-captures
```

该命令打开校验窗口，使用内置 pair 输入运行真实任务，自动切换视图、请求停止并退出，
向指定目录写入 `cards.png`、`table.png`、`text.png`、`stopped.png`。

追加 `--capture-diy` 使用 DIY 组合样例校验三种视图及旧版纯文本详情格式。
追加 `--capture-light` 可使用浅色主题检查同样的四种状态；默认使用深色主题。
追加 `--capture-align center` 或 `--capture-align right` 可检查非纯文本视图的中／右对齐。DIY 样例关闭自动跟随，从第一个组合块开始显示，便于核对标题及原始信息格式。
截图来自 egui 渲染回传，只包含应用自身画面，不截取桌面或操作其他窗口。
需要本机可用的图形环境；启用 feature 后不传 `--capture-dir` 仍正常启动 GUI。

界面示例：[卡片](../../docs/images/openbox-live/cards.png)、[表格](../../docs/images/openbox-live/table.png)、
[纯文本](../../docs/images/openbox-live/text.png)、[停止状态](../../docs/images/openbox-live/stopped.png)。

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

支持普通导出（默认 `+ol`）、旧 `+diy` 导出（`--old`）和“召唤物diy”（`--minions`，
与 `--old` 互斥）。

输入的一行既是一条导出结果，也可能是一个队伍：行内用 `+` 分隔同队成员。
`单名详情`（更多设置里的复选框，CLI 是 `--no-details` 反向开关）只影响**日志**，
不会写进输出文件，也不会改变导出行本身；它出现与不出现的规则是：

| 场景 | 日志 | 输出文件 |
| --- | --- | --- |
| 勾选详情，未选输出文件 | 导出行 + 该行各成员的详情块 | — |
| 未勾选详情 | 只有导出行，与旧版逐字节一致 | 同左 |
| 选择了输出文件 | 只有导出行 | 只有导出行 |
| `openbox-cli -f` 批量读文件 | 只有导出行 | 只有导出行 |

勾选详情时，每一行的结果是「导出行 + 该行每个成员的详情块」，
行与行之间、成员块与成员块之间都空一行（空行只属于日志，导出行本身不变）：

```text
1@team+ol:{"attrs":[78,78,58,64,72,60,77,245],"skills":{...},"name_factor_enabled":true}+2@team+ol:{"attrs":[81,83,59,70,71,61,61,271],"skills":{...},"name_factor_enabled":true}

=== 原始信息 ===
1@team
HP 245(+2) 攻 78 防 78 速 58 敏 64 魔 72 抗 60 智 77 八围 568.7 嘲讽281
  守护 19
  加速 14
  诅咒 29
  分身 4
  聚气 2
  反弹 1
  护符 4

=== 原始信息 ===
2@team
HP 271 攻 81 防 83 速 59 敏 70 魔 71 抗 61 智 61 八围 576.3 嘲讽265
  地裂 12
  ...

test+ol:{"attrs":[71,80,59,70,75,56,67,259],"skills":{...},"name_factor_enabled":true}

=== 原始信息 ===
test
HP 259 攻 71 防 80 速 59 敏 70 魔 75 抗 56 智 67 八围 564.3 嘲讽275
  聚气 21
  治愈 4
  魅惑 25
  苏生 1
  血祭 12
  蓄力 1
  铁壁 12
```

- 名字行写该成员的 `名字@队伍`（无队伍时只有名字），与导出行一致。
- **`(+N)` / `(-N)` 标出“组队后与原值的差额”**：详情块按**整行整队**构建（所以拿到的
  属性和技能与导出行完全一致），每个成员再用**单独构建**（去掉队友、保留自己的
  overlay / 武器等后缀）的结果做差，只在该项真的变化时标出。**属性和技能行都会标**：
  - 属性：同队的 `1@team` 单独构队 HP 是 243、同队后 245，于是显示 `HP 245(+2)`；
    `2@team` 没有变化，所以只显示 `271`。
  - 技能：`光 jKLA6V5mirfs@Afterglow` 的护符单独构队是 84、同队后 98，于是显示
    `护符 98(+14)`；没有变化的技能只显示熟练度。
- 八围为 `七围之和 + HP / 3`，四舍五入到一位小数；`1@team` 是 `487 + 81.7 = 568.7`。
- 嘲讽为 `防*2 + 抗*2 - 攻*2 - 魔*2 - 速*2 - 敏 - 智` 的绝对值（公式本身给出负数，
  面板按数值大小显示）；`1@team` 是 `-281`。
- 技能按行动顺序列出 `中文名 熟练度`（与技能榜同一张名字表），跳过 0 熟练度技能；
  `"2*6"` 这类 boost 只体现在熟练度数值里，不会打印成字符串。
- 属性前七围与导出行一样是 DIY/OL 的 +36 编码，HP 原样，所以数值与导出行一致。

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

## DS4 页面

1. 选择包含 `input/` 的工作目录。已有 `config.json` 会自动读取；新目录载入默认配置。
2. 填写输入中 `@` 后的队伍名，设置线程数、二人组类型及阈值。
3. 按需开启 ABCP5、三人组和 Openbox 实战筛选；基础分类、SP1、八类三人阈值及归档选项在“更多设置”中。
4. 点击运行。界面先保存配置，再在后台调用 DS4 Rust API；右侧显示阶段日志和本轮统计，底部按钮可打开结果目录。

ABCP5 与三人流程需要工作目录的 `abcp5/` 模型和运行库，实战筛选直接使用内置 Rust 后端。运行按整轮完成，暂不提供中途取消；请等待历史状态归档后再关闭窗口。阶段进度不代表耗时比例。

配置读写保留额外字段；手动改换目录后，应先读取配置，防止把另一个目录的设置覆盖进来。默认增量去重会复用 `file/` 中的历史，重新评估模型或阈值时应使用新工作目录。

页面按目录、基础分类、配对、后续筛选和结果使用不同强调色。悬停字段查看对应上游参数说明；点击 ⓘ 可固定流程、阈值、历史、三人组合、模型诊断或实战筛选帮助。基础分类明确按“评分或潜力任一达标”保留，技能参数标为“技能容差”（越大越宽松），避免误当作最低分数。

使用真实 DS4 样例校验深浅主题界面，每次生成主页面、更多设置和固定帮助三张截图：

```powershell
cargo run -p tswn_openbox --bin tswn_openbox --features ui_capture -- --capture-dir target/openbox-ds4-dark --capture-ds4
cargo run -p tswn_openbox --bin tswn_openbox --features ui_capture -- --capture-dir target/openbox-ds4-light --capture-ds4 --capture-light
```

## 实现说明

源码按职责拆分，GUI 与 `openbox-cli` 共用同一套后端与预设：

- `src/lib.rs`、`src/app.rs`、`src/app/`：GUI 的状态、控件与任务启动。
  - `state.rs` 面板状态、`view.rs` 布局与控件、`actions.rs` 启动任务、`widgets.rs` 复用控件、
    `task.rs` 后台任务生命周期、`help.rs` 上下文帮助、`style.rs` 语义配色、`log.rs` 日志缓存、
    `results.rs` 结果模型、`results/view.rs` 结果渲染、`source.rs` 文本输入来源、`ds4.rs` DS4 页面与任务入口、`ds4_help.rs` DS4 参数及流程帮助。
- `../tswn_openbox_backend/src/backend.rs`、`backend/`：解析、执行、输出格式化与文件写入。
  - `to_diy.rs` 导出及属性详情、`namer_pf.rs` 评分入口、`pair.rs` 配队入口、`batch.rs` 批量胜率及有界窗口执行，
    `parse.rs` 输入解析、`format.rs` 输出格式、`output.rs` 文件创建与排序、`score.rs` 评分、
    `pair/matrix.rs` 队友×靶子窗口矩阵、`live.rs` 实时收件箱、`skill_board.rs` 技能榜、`types.rs` 事件与输入类型。
- `../tswn_openbox_backend/src/presets.rs`：靶子/队友预设与默认资源释放，GUI、DS4 与 CLI 共用。
- `src/bin/openbox_cli/`：无头入口（`main.rs` 排空事件通道并按 stdout/stderr 分流、
  `args.rs` 参数解析与执行计划、`input.rs` 输入读取与校验、`plan.rs` 计划类型、`tools.rs` 分发）。

这样后续继续对齐 `tswn-cli` 能力时，可以把 UI 和业务逻辑分开维护。

性能检查、窗口化内存对照与验证范围见 [Openbox 代码检查报告](../../docs/perf/reports/openbox-review-2026-09-29.md)。

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
cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox_backend/assets/targets/target2.txt --limit 10000 --target-limit all --count 1 --threads 8 --report-ms 2000
```

`0.3.9` 修复后，`allCO3pure.txt` 取 10000 组、`target2.txt` 全 41 个靶子、共 410000 个 matchup 的测试中，RSS 运行中稳定在约 `15-16 MB`，结束约 `9.1 MB`。

## pair 并行基准

`openbox_pair_probe` 是 `pair` 后端的 headless 基准入口，用于在没有 GUI 的条件下比较两个提交的
pair 路径：

```powershell
cargo run --release -p tswn_openbox --bin openbox_pair_probe -- `
  --players docs/perf/cqp/sqp6000_first20.txt `
  --teammates crates/tswn_openbox_backend/assets/teammates/teammate_fz.txt `
  --targets crates/tswn_openbox_backend/assets/targets/target2.txt `
  --count 100 --threads auto --head 5
```

stdout 是该路径的日志行（可用于新旧输出哈希对账），stderr 是 `elapsed_s` 等摘要。
`--count` 对应 1% / 10% / 100%（100 / 1000 / 10000），`--threads auto` 为自动线程。
参数、输出约定与同机交替 A/B 的完整口径见
[`docs/perf/guides/openbox-pair-probe.md`](../../docs/perf/guides/openbox-pair-probe.md)。
