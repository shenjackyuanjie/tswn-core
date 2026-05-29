# tswn_openbox

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

左侧是设置区，右侧只显示运行日志。常用设置直接展示，低频设置放在“更多设置”弹窗中。顶部主题按钮支持浅色、深色和跟随系统。

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

“不低估短号”默认关闭，对应 `--keep-rq`。

#### 技能榜

`namer-pf` 支持“技能榜”输出。开启后，程序会把名字转为 `+diy` 形式，找到熟练度最高的技能，并按 `setting\score_now.toml` 的阈值筛选输出。

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

字段含义：

- `pp`：普评阈值。
- `qp`：强评阈值。
- `qd`：强单阈值。
- `all`：全能总分阈值。

“全能”除满足对应 `all` 外，还需要同时满足 `pp >= 8000`、`pd >= 9000`、`qp >= 6000`、`qd >= 7000`。

### cqd/cqp

对应原 `bench batch-rate`。普通设置中保留常用选项，更多设置中可以切换手动靶子、`DIYcqp（++分割名字）`、线程数、场数和输出细节。

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

[[teammate]]
head = 3
name = "默认队友"
file = "teammates/default.txt"
```

说明：

- `targets[].id` 期望是数字。
- `targets[].file` 是靶子列表文件。
- `teammate[].head` 是 `pair` 的“保留前几”。
- `teammate[].file` 是队友列表文件。
- `pair` 默认优先选择 `targets` 中 `id = 2` 的靶子；如果不存在，则退回第一个靶子。

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
