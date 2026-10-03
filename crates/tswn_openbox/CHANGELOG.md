# 更新日志

## [Unreleased] - 0.5.1

### 新增

- 表格高度可自行调整：拖动表格与选中行明细之间的分隔条即可改变高度，拖动后按工具页保存；「排版设置」可输入具体像素值或切回“自动”（按可用高度的 55% 分配），非法值回退自动。
- 卡片与表格里的「复制全部」改为复制当前选中项：表格里是选中行、卡片里是当前卡片，每张展开卡片的右下角也提供同样的按钮；内容是该结果的标题与明细，不再等同于整页日志。「复制日志」仍复制文本流水。

### 界面调整

- 卡片与表格文字改用与纯文本日志相同的等宽字体、字号与行距，三种视图逐行对齐；行高不再固定 24px，而是按日志行高计算。
- 「跟随最新」勾选后立即跳到最新一条，不再等下一条结果；向上滚动暂停跟随，滑回最底部会重新自动勾选。
- 表格高分高亮时名字与分数一起标色，便于一眼定位命中的行。
- 表格与选中行之间新增带名字的可拖动分隔条，替代原来的单独标题行；表格下方的明细区紧贴分隔条并占满剩余高度，表格调小后随之变高。

### 修复

- 修复固定表格高度后明细区被挤出可视区的问题：`auto_shrink = false` 会把多余空间留在滚动区内部，
  横向滚动区此前没有高度上限，表格会吃掉整个结果区，导致分隔条与明细被推到可视区之外、中间留下大片空白；
  现在横向滚动区高度钉在表格区高度上，明细区紧贴分隔条并占满剩余高度。
- 配队：队友与选手一样按“逐个成员单独构建”冻结成 DIY 后再组成二人组，同公会队友不再单方面拿到组队加成；
  此前只有选手被冻结，带权队友（TOML 普通名字）预设会因此得到偏高的 cqp。已经带 `+diy` / `+ol` 的队友输入不受影响，
  日志与文件里的名字仍是原始输入行。

### 验证

- 新增回归覆盖：勾选“跟随最新”当帧跳到最新一行与到底自动恢复跟随、表格分隔条拖动改高、表格高度自动/固定/非法值回退与按页保存、
  固定表格高度后分隔条与明细仍紧贴表格且明细区随表格调小而变高（可视明细行数增加）、
  卡片与表格行高等于日志行高、差额高亮使用等宽字体、高分高亮同时标色名字与分数、展开卡片的「复制全部」只复制该卡片内容（含剪贴板命令断言）、
  复制文本只包含选中项的标题与明细。
- 配队冻结由后端回归测试覆盖：`cargo test -p tswn_openbox_backend`（52 通过）逐项断言选手与队友都等于各自单独构建。
- 本版本验证命令：`cargo test --workspace -- --test-threads=1`、`cargo test -p tswn_openbox`（40 通过）、
  `cargo test -p tswn_openbox_backend`（52 通过）、`cargo +nightly fmt --check`、
  `cargo clippy -p tswn_openbox -p tswn_openbox_backend --all-targets --features ui_capture --no-deps -- -D warnings`。

## [Unreleased] - 0.5.0

> 修复了 astra 写的 sb UI 的问题

### 新增

- 结果视图支持排版调整：卡片可选左、中、右对齐，表格可逐列设置宽度与对齐，也可拖动表头右边界调整列宽；表头与数据共享横向滚动，避免长表格错列。
- 界面偏好自动保存并在重启后恢复，包括主题、当前工具、各工具视图选择、跟随开关、卡片与表格排版，以及窗口和输入面板尺寸；排版按工具页分别保存，不写入输入内容、计算参数、日志或运行状态。
- 结果区提供复制入口：导出结果的导出行右侧可单独复制该名字的 DIY 内容，右下角“复制全部”复制本页全部结果；纯文本日志支持选中复制。
- 截图校验新增 `--capture-diy` 与 `--capture-align left|center|right`，可复现 DIY 纯文本、卡片和表格的排版检查；DIY 样例关闭自动跟随，从第一个输入组开始显示，便于核对标题与详情格式。

### 界面调整

- 每个工具页各自持有日志与结果，切换页面不再看到上一页的输出；运行中也可以切换页面，正在运行的任务页用蓝色标出，停止按钮会说明它属于哪一页。
- 卡片与表格的详情改为按纯文本结果逐行渲染：导出行、「原始信息」区块、属性行与按行动顺序排列的技能行完全一致，技能不再两行四列并排；组队加成仍只标出带 `(+N)` / `(-N)` 的项。
- 纯文本日志按窗口宽度自动折行，长导出行不再需要横向滚动；渲染仍按可见段虚拟化，只绘制窗口内的行。
- 纯文本改用 `9473188ca61068a605ecd64075aebf196052c264` 的结果格式：DIY 导出行与「原始信息」详情块、评分 `名字 指标:分数`、胜率与配队 `分数 名字` 及缩进明细；
  不再插入逐项「预览」「完成」前缀。不同输入组之间以一行 `=========` 分隔，第一组之前不加，组内空行保留。
- DIY 表格不再显示没有数据的「分数」列；结果较少时详情紧随表格，不再出现半屏空白。
- 纯文本、卡片与表格共用同一套面板版面：去掉纯文本日志独有的外框，统一中央面板边距，三种视图的内容起始位置保持一致。

### 修复

- 修复纯文本被结构化结果改写后无法复制旧格式的问题：旧版文本由后端随结果一并给出，界面不再自行拼接预览文案。
- 修复结果已生成时仍显示“运行结果会显示在这里”的提示。

### 验证

- 新增回归覆盖：真实 DIY 结果在三种视图下的复制文本与旧 API 一致（忽略新增的组分隔行）、分隔行插入位置与空白块处理、合并行只高亮差额项、同一技能行多个差额、
  横向滚动时表头与数据列保持同偏移、拖拽列宽边界、万条结果仅绘制可见行、运行中切页后任务仍写入发起页、设置按页保存与恢复（含缺失字段与非法宽度）、
  长行折行切分与裁剪后段索引。
- 本版本验证命令：`cargo test --workspace -- --test-threads=1`、`cargo test -p tswn_openbox`（28 通过）、
  `cargo test -p tswn_openbox_backend`（49 通过）、`cargo +nightly fmt --check`、
  `cargo clippy -p tswn_openbox -p tswn_openbox_backend --all-targets --features ui_capture --no-deps -- -D warnings`。

### 说明

- 纯文本以外的视图仍使用结构化结果；后端与旧版文本的对应关系另见 `../tswn_openbox_backend/CHANGELOG.md`。

## [0.4.6] - 2026-09-30

### 新增

- 聚合打包脚本同时收录 Openbox GUI 与 `openbox-cli` 可执行文件，并在包内清单和说明中分别列出。
- 新增 DS4 页面：工作目录、队伍名、配置读写、各类筛选阈值、后台执行、阶段进度、本轮统计及结果目录入口；直接调用 `tswn_ds4` Rust API。
- 共用计算与预设模块提取为 `tswn_openbox_backend`，GUI、DS4 与 CLI 复用同一实现，原有公开接口保持兼容。
- 新增按工作目录加载靶子预设的 Rust API；默认开启的 `gui` feature 管理 GUI 与 DS4 页面依赖，
  `openbox-cli` 可通过 `--no-default-features` 无头构建；DS4 直接依赖 `tswn_openbox_backend`，不依赖 GUI crate。
- 新增默认关闭的 `ui_capture` feature，按需用应用自身渲染回传生成四种状态的校验截图。
- 日志区支持纯文本流水、可折叠卡片和表格三种视图，支持运行中切换及跟随最新。
- 四个工具支持增量结果：按导出行、评分指标、靶子胜率和队友 cqp 发布，不再等待整批计算返回。
- 并发结果使用输入序号关联；预览、当前 Top、未达阈值和停止后的不完整结果有明确标记。

### 性能与兼容

- cqd/cqp 拆分为独立模块，按最多 4096 个对局的窗口执行，避免一次创建完整请求矩阵；缓存镜像身份，保持镜像优先、原输入顺序汇总。pair 复用每组队伍文本和镜像身份，准备请求时也能响应取消。
- 文件预览最多读取 16 KiB、显示十行，不再扫描完整大文件或逐帧复制预览；运行时仍验证完整输入。四个工具共用线程启动和异常处理，线程创建失败会正常显示错误。
- 实时入口跳过旧格式日志及批量明细的重复构建；文件创建与排序拆分为独立模块，排序分数每行只解析一次，排序回写使用缓冲，避免再拼接整份文件。
- 结果行索引仅在结构变化时重建；pair 明细缓存排序结果，Top 模式仅排序入选部分。技能榜展开视图改为按可见行绘制，不再逐帧拼接、布局全部文本。
- GUI 采用有界收件箱、进度合并、约 100ms 批量消费和可见行渲染，慢界面不阻塞计算线程。
- 保留 CLI、文件输出、稳定加权及同分排序行为；新增回归测试覆盖提前发布、并发重名与文件兼容性。

### 界面调整

- DS4 页面使用彩色流程与分区，所有配置项提供上游语义 tooltip；新增六类可固定帮助，覆盖增量缓存、技能容差、三人组合、模型诊断与三轮实战规则，截图驱动支持 `--capture-ds4`。
- 四个工具补充输入规则、评分和导出提示；阈值、运行、停止、复制与清空提供悬停说明，新增导出帮助和可固定的实时结果说明。
- 统一深浅主题语义配色：工具标题分别强调，预览、完成、停止、高亮和错误同时以颜色与文字区分；卡片边框、状态列、进度条与操作按钮同步配色。
- 截图校验支持 `--capture-light`，可检查浅色主题的可读性。
- 缩小卡片与表格的行距、按钮内边距、明细缩进和输出区边距，并收紧表格列宽；表格行较少时详情紧随其后，消除中间的大块留白。
- 按内容设置默认视图：`to-diy`、`pair` 使用卡片，`namer-pf`、`cqd/cqp` 使用表格；各页独立记住本次会话的选择。
- 视图选项仍按“纯文本、卡片、表格”排列，结果排序保持原有行为。

### 修复与清理

- 精简静态文案、内部缓存地址、简单替换及重复解析等低价值测试；保留并发、文件兼容、窗口一致性与大输入边界回归，阈值异常合并进已有参数校验。
- 按业务拆分原 `backend/tasks.rs` 为导出、评分、批量胜率和配队模块，矩阵执行与任务入口分离；GUI 的任务生命周期、结果模型和结果绘制各自独立，测试随所属模块迁移。
- 导出详情与技能榜共用技能中文名称表，公共评分类型、输出及高亮规则集中维护；公开后端入口保持不变。
- CLI 的非负阈值拒绝 `NaN` 和无穷值，与 GUI 校验保持一致；读取无 BOM 文件时不再复制完整内容。
- 评分名称复用公共定义，移除多余导入与无用的计时转换，Openbox 自身的编译和 Clippy 警告已清理。

### 说明

- DS4 在后台线程同步执行，通过阶段回调更新进度；目前没有全流程取消接口，同一工作目录
  不应并发运行。ABCP5 程序、模型与运行库由用户提供，不随 Openbox 分发。
- 独立后端拆分后的变更另见 `../tswn_openbox_backend/CHANGELOG.md`。

## [0.4.5] - 2026-09-29

### 依赖更新

- 显式依赖 `egui` 并直接使用其 API；`eframe`、`egui` 及配套组件更新至 `0.36.2`。
- 更新 `clap`、`toml` 等依赖至现有兼容范围内的最新版本。

### 新增

- 新增 `openbox-cli` 无头命令行（`cargo run -p tswn_openbox --bin openbox-cli`）：
  与 GUI 共用同一套后端（`src/backend/`）与预设（`src/presets/`），输出逐字节一致，
  用于把面板工作流脚本化。四个子命令对应四个面板：`to-diy`、`namer-pf`、
  `cqd`（别名 `cqp`）、`pair`。数据行走 stdout、进度与状态走 stderr；
  预设读 `./setting/settings.toml`（缺失时自动释放内嵌默认资源），
  技能榜阈值默认读 `./setting/score_now.toml`，`--skill-board FILE` 可显式指定。
  `--metric NAME[:MIN_SCREEN[:FILE[:MIN_FILE]]]` 语法与 `tswn-cli namer-pf`
  一致；`--show-matchups`（默认开）与 `--detail every`（默认）跟随 GUI 勾选状态；
  高亮颜色降级为普通行，未实现 GUI 的停止按钮。
- backend `NamerPfSkillBoardOptions` 增加 `config: Option<PathBuf>`：
  CLI 可显式指定技能榜阈值文件；GUI 传 `None`，行为不变。

### 重构

- `app/target_presets.rs` 上移为 lib 级 `src/presets.rs`（`pub mod presets`），
  预设解析与默认资源释放从此与 GUI 解耦，CLI 直接复用；新增
  `load_target_preset_text` / `load_teammate_preset_text` 按预设直接读取。

### 变更

- 更新内嵌的 `pair` 默认资源：默认靶子切换到新版带权 TOML（单人组与双人组），
  新增五组带权队友 TOML 预设，并同步调整技能榜阈值数据；历史 TXT 预设仍保留供兼容使用。
- `to-diy` 的“单名详情”日志块改为按面板口径输出：`=== 原始信息 ===` 之后依次是
  成员名（`名字@队伍`）、`HP 攻 防 速 敏 魔 抗 智 八围 嘲讽` 与按行动顺序排列的
  `技能名 熟练度`。八围按 `七围之和 + HP / 3` 四舍五入到一位小数，嘲讽按
  `防*2 + 抗*2 - 攻*2 - 魔*2 - 速*2 - 敏 - 智` 取绝对值（公式本身为负值）；
  属性前七围与导出行同为 +36 的 DIY/OL 编码，HP 原样。
  详情按整行**整队**构建并逐个成员输出：组队行（如 `1@team+2@team`）会为该队每个成员
  各追加一块，不再跳过组队行；每项属性和每条技能再用该成员单独构建（去掉队友、
  保留自己的 overlay / 武器等后缀）的结果做差，变化时标成 `HP 245(+2)`、`护符 98(+14)`，
  用来一眼看出组队带来的变化。
  每一行的「导出行 + 详情块」之间、成员块之间都空一行，导出行不会和上一行的详情块
  贴在一起；空行只属于日志。`-f` 批量文件模式与 `--no-details` 都不输出详情，
  导出到 `+ol` / `+diy` 的行和输出文件内容不受影响。

### 性能

- GUI 日志改为按行维护 4 MiB 上限，超限时逐行淘汰旧内容，避免每次追加都搬移整段日志；
  高亮和技能榜标记随日志行一起保留或淘汰。
- 主日志仅绘制滚动区域内的可见行；技能榜内容仅在展开时生成，减少大量日志下的重绘开销。

### 验证

- 新增 `to-diy` 详情块回归测试：`1@team+2@team` 面板样例逐行逐值校验（组队行两个块、
  `1@team` / `HP 245(+2)` / 八围 568.7 / 嘲讽 281）、`冥河 …+光 …` 组队技能差额样例
  （`护符 98(+14)` 且无变化的技能不带括号）、多行输入的行间空行、
  overlay 固定输入逐字节输出、`-f` 批量与输出文件不含详情、
  勾选只追加日志行且导出行逐字节不变。
- 新增八围与嘲讽的算值测试，覆盖四舍五入与整数进位边界。
- 本版本验证命令与结果：`cargo test -p tswn_openbox`（`37 + 22 + 5` 通过）、
  `cargo +nightly fmt --check`（无输出）、`cargo clippy -p tswn_openbox --bins --lib`（无新增告警）。

## [0.4.4] - 2026-09-15

### 修复

- 修复默认 `settings.toml` 已引用五个新版 `teammates/*.toml`，但 release 内嵌资源释放清单仍只有旧 TXT 的问题；
  首次启动与已有配置两条路径都会补齐缺失的默认资源，且不会覆盖用户已经修改或自行创建的文件。
- 修复 `pair` 仍按“选手 → 队友 → 靶子”串行驱动大量独立 matchup，导致 Windows 等多核环境下
  CPU 利用率受单个 matchup 内部线程创建/回收与任务尾部空洞限制的问题。

### 性能与调度

- `pair` 改为按当前选手构造有界的队友×靶子矩阵并复用 `tswn_core` 共享 CQP/CQD 调度器；
  结果仍按原靶子顺序归约，队友权重、镜像 50%、重复名跳过、Top-K 与取消语义保持不变。
- OpenBox 不再为无需读取逐场 timing 的批量胜率路径调用 `_timed` 接口，减少 Windows QPC 等计时开销。
- 本机（Ryzen 7 5800X / 16 逻辑处理器）同机交替 A/B 实测：自动线程下 OpenBox pair 全网格
  `count=100` 由 24.13 s 降到 4.74 s（5.09×），CLI `bench pair` 全网格由 20.89 s 降到 4.61 s（4.53×），
  少 matchup × 长轮次由 0.111 s 降到 0.029 s；`fixed30` 与共享 CQP/CQD 矩阵未测到回退。

### 验证

- 新增默认资源补齐且不覆盖用户文件、pair 新旧汇总一致性、权重/镜像/重复名、矩阵窗口边界、
  取消以及共享调度器并发/回调边界回归测试。
- 新增 `openbox_pair_probe` headless 基准入口，用于在没有 GUI 的条件下复测 `pair` 后端；
  参数与同机 A/B 流程见 `docs/perf/guides/openbox-pair-probe.md`。
- 本次并行修复的复测环境、逐档中位数与仪表伪影结论记录在
  `docs/perf/reports/openbox-pair-parallelism-2026-09-15.md`，原始样本见同目录
  `openbox_pair_parallelism_9b07a04e_ab_samples.json`。

## [0.4.3] - 2026-09-08

### 新增

- `namer-pf` 技能榜新增 `[lessskl]` 白板号阈值；待评名字或组合的全部技能熟练度均小于 30 时，会按该阈值额外筛选输出。
- `pair` 队友预设新增可选 `factor_enabled`；启用后读取与带权靶子相同格式的 TOML 队友文件，将每个队友组合的平均胜率乘以对应 `factor` 后再按 `head` 取高分求和。

### 调整

- 从 `tests/jnb.txt` 导入当前技能榜阈值，并用白板号行初始化 `[lessskl]` 的 `pp`、`qp`、`qd`。

### 修复

- 同步 core 的批量判胜修复：参战队伍被清空后重新复活时，依赖批量胜率结果的评分与排名路径不再沿用粘性存活组计数误判胜者。

### 文档

- 补充 `namer-pf` 技能榜低熟练度白板号筛选规则，以及 `[lessskl]` 阈值配置示例。
- 补充 `pair` 带权队友的配置示例、靶子权重与队友权重的计算顺序，以及手动队友模式下权重配置不生效的说明。

### 验证

- `cargo check -p tswn_openbox`
- `cargo test -p tswn_openbox`
- `cargo +nightly fmt --check -p tswn_openbox`
- `cargo +nightly fmt --check`

## [0.4.2] - 2026-09-01

### 新增

- `pair` 选择 `factor_enabled = true` 的靶子预设时，支持带权 TOML 靶子，并按 `sum(胜率 * factor) / sum(factor)` 计算每个队友组合的平均 cqp。
- `pair` 的选手和队友输入支持一行多个玩家，可分别在更多设置中启用 `++` 分割；默认选手使用单个 `+`，队友使用 `++`。
- `pair` 支持将多名选手、多名队友与多玩家靶子正确拼接为同一场对局，并保留原始输入行作为日志和文件标签。

## [0.4.1] - 2026-08-31

### 新增

- 顶部新增“关于”弹窗，显示 `tswn_openbox` 与 `tswn_core` 的当前版本，并链接到项目 GitHub 仓库。
- 窗口标题与顶部栏标题旁直接显示 `tswn_openbox` 与 `tswn_core` 的版本号（构建时读取，无需手动维护）。
- 默认靶子预设新增 50 组带权单人组，并以更新后的 50 组带权双人组替换旧数据；首次启动写出默认配置时会一并生成两份新版预设文件。

### 验证

- `cargo +nightly fmt --check -p tswn_openbox`（`rustfmt.toml` 使用 nightly 专属选项，仓库新增 `AGENTS.md` 记录该约定）
- `cargo check -p tswn_openbox`

## [0.4.0] - 2026-08-30

### 新增

- `cqd/cqp` 靶子预设新增可选配置 `factor_enabled`；启用后从 TOML 靶子文件读取每组 `factor` 与 `players`，并提供内嵌的 50 组带权二人靶子预设。

### 调整

- 升级 GUI 框架至 `eframe`/`egui` 0.36.1，并更新 lockfile 中所有兼容依赖。
- 带权 `cqd/cqp` 按 `sum(胜率 * factor) / sum(factor)` 计算平均胜率；完全相同的双方阵容按 50% 参与加权，部分重名仍正常计算。

## [0.3.13] - 2026-07-19

### 新增

- 为 `namer-pf`、`cqd/cqp` 和 `pair` 的常用设置、更多设置与输出选项增加圆形 `i` 上下文帮助图标；鼠标悬浮可快速查看说明，点击后可固定为独立说明窗口。
- 将原界面标注中的精确度/场数对应关系、评分参考范围、技能榜条件、分组格式、队友与 cqp 规则、阈值和高亮逻辑整理为控件就近帮助。

### 调整

- 按当前实现校正帮助文案：空阈值表述为“不限制”，高亮条件明确为 `分数 >= 日志阈值 + 高亮增量`，经验分数明确标记为参考范围而非默认阈值。
- 帮助图标由 egui 直接绘制，不新增图标或字体依赖；帮助功能不改变评分、筛选、默认设置和输出格式。

### 验证

- `cargo fmt --check -p tswn_openbox`
- `cargo check -p tswn_openbox`
- `cargo test -p tswn_openbox --lib --bin tswn_openbox`

## [0.3.12] - 2026-07-14

### 调整

- `cqd/cqp` 从 legacy `PreparedRunner` 切换到与 CLI 共用的 Runtime matchup 矩阵执行器；单人和双人输入都按 `player × target` 动态派发，输出仍按原始选手顺序汇总，显式线程数、重复号跳过、详情和取消语义保持不变。
- 原生默认启用 `mimalloc_alloc`，并将自动线程分为短任务 1.5 倍逻辑核、中长任务 2 倍逻辑核；仍可在更多设置中显式指定线程数。
- `openbox_mem_probe` 的最终耗时改为输出 6 位小数，便于稳定记录 1% 短档基线。

### 测试

- 新增 OpenBox Runtime 矩阵与 legacy 小样本平均胜率、选手输出顺序和进度终点对照。
- `cargo test -p tswn_openbox`
- `cargo check -p tswn_openbox --all-targets`
- CQP/CQD 单人 1%/10%/100% 相对 Runtime v1 快 49.64%/51.30%/40.78%，双人快 44.52%/43.39%/39.89%。

## [0.3.11] - 2026-06-25

### 新增

- `cqd/cqp` 的靶子预设支持在 `setting/settings.toml` 的 `[[targets]]` 中配置可选字段 `diy = true/false`；为 `true` 时该靶子文件按 `++` 分割名字，为 `false` 或省略时保持原有 `+` 分组逻辑。
- `cqd/cqp` 的更多设置中，勾选“使用手动靶子”后新增“DIY靶子（++分割名字）”，用于手动靶子列表启用同样的 `++` 分组语义。

### 调整

- 后端 `namer-pf` / `cqd-cqp` 的低精度外层并行改为复用 `tswn_core::bench_sched::run_outer_parallel_ordered`，删除 `tasks.rs` 内两份本地 work-stealing 调度器及相关常量，行为与 CLI 共用同一套实现。
- `cqd/cqp` 低精度并行的屏幕日志输出顺序由「选手完成顺序」改为「选手原始输入顺序」（更确定，数据不变）；输出文件收尾排序行为不受影响。

### 验证

- `cargo build -p tswn_openbox`
- `cargo test -p tswn_openbox --lib`
- `cargo clippy -p tswn_openbox --bins --lib`

## [0.3.10] - 2026-06-23

### 调整

- `to-diy` 面板改为复用 `tswn_core::cli_api::to_diy`，同步支持新版 `+` 组队语义：每行可用 `+` 分隔同组成员，导出前会先计算组队加成，再把成员 DIY/OL 用 `+` 拼回同一行。
- `to-diy` 的“单名详情”仅在当前输入按新版 `+` 语义解析后仍为单个玩家时显示，避免把 `1@a+2@a` 误当成旧式武器输入展示详情。

### 修复

- 修复 release 包首次启动或已有 `setting\settings.toml` 但缺少 `setting\score_now.toml` 时，`namer-pf` 技能榜找不到默认阈值文件的问题；现在默认 `score_now.toml` 会作为内嵌资源随 exe 编译，并在缺失时自动写入。
- 修复 `namer-pf` 评分项表格中输出文件路径提示的对齐：`未选择输出文件` 或已选择的输出文件路径现在从提示行行首开始显示，不再对齐到“选择”按钮列。
- 同步调整 `namer-pf` 技能榜输出文件路径提示，使其与普通评分项保持一致。

### 验证

- `cargo fmt -p tswn_openbox`
- `cargo check -p tswn_openbox`
- `cargo test -p tswn_openbox writes_default_score_now_file_when_missing`
- `cargo test -p tswn_openbox to_diy --lib`

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
- `cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox_backend/assets/targets/target2.txt --limit 2000 --target-limit 8 --count 1 --threads 8 --report-ms 1000`
- `cargo run -p tswn_openbox --bin openbox_mem_probe -- --players tests/allCO3pure.txt --targets crates/tswn_openbox_backend/assets/targets/target2.txt --limit 10000 --target-limit all --count 1 --threads 8 --report-ms 2000`

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
