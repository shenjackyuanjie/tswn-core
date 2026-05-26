# tswn_openbox

`tswn_openbox` 是一个简洁的原生交互面板，用来把常用的 `tswn-cli` 工作流做成点击即用的界面。

当前支持：

- `to-diy`
- `namer-pf`
- `bench batch-rate`
- `bench pair`

## 运行

在 workspace 根目录运行：

```bash
cargo run -p tswn_openbox
```

推荐的 release 构建命令：

```bash
cargo build -p tswn_openbox --release --features "no_debug,mimalloc_alloc"
```

构建完成后，可直接双击：

```text
target\release\tswn_openbox.exe
```

Windows 下可执行文件默认使用 `windows_subsystem = "windows"`，双击启动时不会额外挂出控制台窗口。

## 输入

面板支持直接粘贴文本，也支持勾选“从文件中读取”。从文件读取时，界面只预览前 10 行，运行时会读取完整文件。

已选择输入文件时，可以在界面里直接点“刷新预览”重新读取文件内容，适合边改边跑。

`batch-rate` 列表格式与 CLI 保持一致：每行一组，组内名字用 `+` 分隔。`pair` 的选手列表和队友列表为每行一个名字。

## 输出

`to-diy`、`namer-pf`、`batch-rate`、`pair` 现在都支持可选写入文件；不勾选“写入文件”时只在右侧日志区显示结果。

文件输出格式对应 CLI 选项：

- `分数 名字`：默认输出格式。
- `JSONL (--log)`：对应 CLI 的 `--log`。
- `名字 (--pure)`：对应 CLI 的 `--pure`。

`文件阈值` 对应 `--min-file`，控制哪些结果写入文件。`日志阈值` 对应 `--min-screen`，控制右侧日志显示。

`batch-rate` / `pair` 新增 `perf` 开关，用于把 total/init/fight 的耗时拆分输出到日志，便于对照 CLI 的 `--perf`。

`to-diy` 在未写入文件时仍支持单名详情输出；勾选“写入文件”后会只写导出结果，和 CLI 的 `-o/--out-file` 语义一致。

`batch-rate` 的日志阈值和文件阈值会校验为 `0~100`；`pair` 的阈值会校验为非负数，避免误填后静默回退。

## 字体

界面字体内嵌使用：

```text
crates\tswn_openbox\src\SarasaMonoSC-Regular.ttf
```

## 实现说明

源码现在按职责拆分为：

- `src/app.rs` + `src/app/`：GUI 状态、控件与任务启动。
- `src/backend.rs` + `src/backend/`：解析、执行、输出格式化与文件落盘。

这样后续继续补齐 `tswn-cli` 的能力时，不需要再在单个超长文件里来回改 UI 和业务逻辑。
