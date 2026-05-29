# 更新日志

## [0.3.1] - 2026-05-29

### 新增

- `namer-pf` 新增“技能榜”输出。开启后会读取 `setting\score_now.toml`，根据名字的最高熟练度技能筛选 `pp`、`pd`、`qp`、`qd` 和“全能”结果。
- 新增 `setting\score_now.toml` 示例/当前阈值文件，字段来源约定为：`pp` 来自普评，`qp` 来自强评，`qd` 来自强单，`all` 来自全能总分。
- 技能榜屏幕日志使用蓝字显示，文件输出格式为 `技能项 分数 名字`。
- `namer-pf` 新增“保留小数点后 X 位”设置，默认值为 `0`，对应 CLI `namer-pf --precision`，作用于普通评分和技能榜的屏幕/文件输出。

### 调整

- 技能榜“全能”判定除满足 `score_now.toml` 中对应技能的 `all` 外，还要求 `pp >= 8000`、`pd >= 9000`、`qp >= 6000`、`qd >= 7000`。
- `namer-pf` 只计算当前实际启用的评分项；只有选择 `sum` 或技能榜时才强制计算四项评分。

### 修复

- 修复 `namer-pf` 大批量运行时内存持续上涨的问题。临时 profile 对局改用 uncached Runner 构造，避免把每一局的一次性模板写入全局 prepared 缓存。
- 修复 `namer-pf` 小数位只在整数后补零的问题；现在会保留真实分数小数，再按设置格式化。
- 修复技能榜相关按钮和输出文件提示的中文乱码。
- 修复通用输出文件控件中的“选择输出文件”“未选择输出文件”等中文显示。

### 验证

- `cargo check -p tswn_openbox --features "no_debug,mimalloc_alloc"`
- `cargo test -p tswn_core --features no_debug uncached_prepare_and_runner_construction_do_not_fill_prebuilt_cache`

## [0.3.0] - 2026-05-29

### 新增

- 新增 `setting\settings.toml` 预设读取：`targets` 用于靶子，`teammate` 用于 `pair` 队友选项。
- `settings.toml` 缺失时会从内嵌资源自动写入默认配置和默认预设文本。
- 新增自适应系统主题、浅色和深色模式切换。
- `pair` 队友预设支持 `head`、`name`、`file` 字段，运行时自动读取队友文件并使用对应 `head`。
- 新增“更多设置”弹窗，把低频选项收纳到高级区域。
- 新增停止按钮，可取消正在运行的任务。
- 新增“高亮超强名字”阈值，超过阈值的屏幕输出行会标红。
- `namer-pf` 支持 `pp`、`pd`、`qp`、`qd`、`sum` 分项勾选屏幕输出和文件输出。

### 调整

- `batch-rate` 在界面中改名为 `cqd/cqp`。
- 场数在普通界面改为精确度选项：`1%` / `10%` / `100%`，对应 `100` / `1000` / `10000` 场。
- 线程数只在更多设置中展示，默认使用系统线程自动配置。
- `--keep-rq` 在界面中改名为“不低估短号”；`cqd/cqp` 和 `pair` 默认开启，`namer-pf` 默认关闭。
- `--minions` 在界面中改名为“召唤物diy”。
- `head` 在界面中改名为“保留前几”。
- `++` 分组改名为 `DIYcqp（++分割名字）`，并移动到更多设置。
- `pair` 普通设置不再显示靶子选择，默认优先使用 `settings.toml` 中 `id = 2` 的靶子。
- `pair` 普通设置只显示从 `settings.toml` 导入的队友选项；手动输入和从文件读取移动到更多设置。
- 菜单栏、运行按钮、进度条和速度/剩余时间显示做了放大和整理。

### 输出

- `cqd/cqp` 仅保留“每组胜率”详情选项。
- `pair` 新增“每组 cqp”和“有效 cqp”两个互斥详情选项。
- 右侧区域只显示日志，输出文件需要提前选择。
- 输出格式对齐 CLI：默认格式、`JSONL (--log)`、`名字 (--pure)`。
- `日志阈值` 对应 `--min-screen`，`文件阈值` 对应 `--min-file`。
- 多行显示框不自动换行，内容过长时使用横向滚动。

### 修复

- 修复中文字体显示为方框的问题，改为使用内嵌 `SarasaMonoSC-Regular.ttf`。
- Windows GUI 构建不再显示控制台窗口。
- 修复重构后 `cqd/cqp` 和 `pair` 的文件阈值、输出格式失效问题。
- 移除界面中的“详细”和 `perf` 选项。

## [0.2.0] - 2026-05-26

### 新增

- 新增 `to-diy`、`namer-pf`、`bench batch-rate` 和 `bench pair` 的基础 GUI 面板。
- `to-diy` 支持可选输出文件。
- `namer-pf` 支持可选输出文件。
- `batch-rate` 和 `pair` 支持屏幕日志和文件输出。
- 输入文件模式支持只预览前 10 行，运行时读取完整文件。

### 重构

- 将 GUI 状态、输入源、控件和任务启动逻辑拆到 `src/app/`。
- 将解析、评分、格式化和执行逻辑拆到 `src/backend/`。

### Windows

- Windows GUI 构建启用 `windows_subsystem = "windows"`。
