# 更新日志

## [0.3.10] - 2026-06-23

### 修复

- 修复 `namer-pf` 评分项表格中输出文件路径提示的对齐：`未选择输出文件` 或已选择的输出文件路径现在从提示行行首开始显示，不再对齐到“选择”按钮列。
- 同步调整 `namer-pf` 技能榜输出文件路径提示，使其与普通评分项保持一致。

### 验证

- `cargo fmt -p tswn_openbox`
- `cargo check -p tswn_openbox`

## [0.3.9] - 2026-06-20

### 修复

- 修复 `cqd/cqp` 大批量双人组运行时内存持续上涨的问题。Openbox 后端的批量胜率路径改用 uncached prepared runner，避免把 `tests\allCO3pure.txt` 这类几乎全是唯一组合的 matchup 写入全局 prepared 缓存。
- 修复 `cqd/cqp` 低精度外层并行时可能因按输入顺序等待结果而堆积大量已完成结果的问题；现在完成一个选手组就输出一个选手组，输出文件仍会在收尾阶段按分数排序。
- 将 GUI 与后端 worker 的进度事件通道改为有界通道，避免 UI 消费慢于计算线程时无界积压事件。
- 为主日志增加内存上限并同步维护高亮/技能榜行号，长时间任务不会无限保留旧日志。

### 调整

- `cqd/cqp` 不再单独维护“每组胜率”折叠日志；勾选“每组胜率”后，主日志直接按 `分数 名字` 加缩进子项显示。
- `pair` 的“每组 cqp”和“有效 cqp”屏幕输出统一为同样的块状格式：总分行在前，子项行缩进显示。
- 新增 `openbox_mem_probe` 调试入口，用于复现和采样 Openbox 后端任务内存占用。

### 验证

- `cargo check -p tswn_openbox`
- `cargo check -p tswn_openbox --bin openbox_mem_probe`
- `cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox/assets/targets/target2.txt --limit 2000 --target-limit 8 --count 1 --threads 8 --report-ms 1000`
- `cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox/assets/targets/target2.txt --limit 10000 --target-limit all --count 1 --threads 8 --report-ms 2000`

### 实测

- 修复前：`allCO3pure.txt` 取 2000 组、`target2.txt` 取 8 个靶子、`count=1` 时，RSS 从约 `5.7 MB` 上涨到约 `372 MB`。
- 修复后：同参数峰值约 `10.8 MB`，结束约 `8.5 MB`。
- 放大验证：`allCO3pure.txt` 取 10000 组、`target2.txt` 全 41 个靶子、共 `410000` 个 matchup，RSS 运行中稳定在约 `15-16 MB`，结束约 `9.1 MB`。

## [0.3.8] - 2026-06-19

### 修复

- 修复 pair 逐条详情（`PairDetailMode::Every`）绕过 `min_screen` 阈值，导致应被抑制的玩家残留孤儿 `<cqp> <teammate>` 日志行。
- 修复新版日志视图丢失高亮超强名字的红色加粗样式：`HighlightLog` 事件正常填充但渲染不再读取 `highlight_lines`。

### 代码质量

- 重构 `emit_namer_pf_result` 参数：提取 `SkillBoardEmitCfg` 结构体打包 `skill_board` 相关参数。
- suppress `bench_batch_rate_for_group` 与 `run_batch_rate_outer_parallel` 的 `too_many_arguments`。

## [0.3.7] - 2026-06-18

### 调整

- 补充 target1.txt 的靶子数据

## [0.3.6] - 2026-06-18

### 调整

- 收紧 Openbox 全局控件间距、按钮内边距、窗口边距和左右主面板内边距，减少界面空白占用。
- 收紧工具配置分区、更多设置窗口、日志区域、折叠日志区和 `namer-pf` 表格的内部间距。
- 文本输入和文件预览区域按更紧凑的行高计算高度，让同屏能显示更多内容。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_openbox`
- `cargo test -p tswn_openbox`
- `cargo test`
- `git diff --check`

## [0.3.5] - 2026-06-18

### 调整

- `namer-pf` 在精度为 `1%` 或 `10%`（场数不超过 `1000`）且线程数大于 1 时，改为跨多个名字组并行计算；每个子任务内部固定单线程，避免低场次下把线程全部压在单个名字组上。
- `cqd/cqp` 在精度为 `1%` 或 `10%`（场数不超过 `1000`）且线程数大于 1 时，改为跨多个玩家并行计算，同时保留每个靶子的实时进度和每组胜率日志。
- 低精度并行路径仍按原输入顺序写入屏幕汇总和输出文件；输出文件排序仍由原有收尾逻辑处理。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_openbox`
- `cargo test -p tswn_openbox`
- `cargo test`
- `git diff --check`

## [0.3.4] - 2026-06-18

### 调整

- 右上角主题切换改为紧凑的单字按钮，去掉额外“主题”文字占位，减少顶栏占用宽度。
- 主题按钮增加明确的选中底色、描边和文字对比度，避免当前主题状态看不出来。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_openbox`
- `cargo test`

## [0.3.3] - 2026-06-18

### 调整

- 恢复 `namer-pf`、`cqd/cqp` 和 `pair` 更多设置中的线程选择；“系统线程 * 1.5”继续走自动线程数，关闭后可手动指定线程数。
- `cqd/cqp` 的每组胜率明细改为进入可展开/收回的“cqd 每组胜率”区域，主日志只保留汇总、警告和完成信息。
- 运行结果日志改为双向滚动的等宽文本视图，长 CQP 行可以横向滚动并选择。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_openbox`
- `cargo test`

## [0.3.2] - 2026-06-18

### 新增

- `cqd/cqp`、`namer-pf` 和 `pair` 支持不选择输出文件时只输出到日志，并在界面上明确提示当前没有文件产物。
- `cqd/cqp` 支持运行结束后按分数重新读取并排序输出文件。
- `to-diy` 原始信息补充技能字段，便于直接检查导出的技能配置。
- `namer-pf` 技能榜日志支持折叠，长技能榜不会持续挤占主日志区域。

### 调整

- 固定底部运行按钮区域并取消运行按钮分栏，长内容场景下关键操作不会被滚动内容遮住。
- 右侧日志区改为适合长行输出的横向滚动与文本选择行为，减少长 CQP 行拖选困难。
- 默认开启每组 CQP 实时日志，并提升运行期日志刷新频率，让长时间任务能持续看到进度。
- 内嵌 SarasaMonoSC 始终作为界面首选字体，系统 emoji 字体仅作为后续 fallback，避免 emoji 字体抢占中文和普通文本渲染。

### 修复

- 修复无输出文件运行时的日志输出和完成状态提示。
- 修复长日志输出一卡一卡、不流畅的问题。
- 修复输出文件选择、未选择提示和相关字体 fallback 的显示问题。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_openbox`
- `cargo test`

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
