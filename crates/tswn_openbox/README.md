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

## 输入

面板支持直接粘贴文本，也支持勾选“从文件中读取”。从文件读取时，界面只预览前 10 行，运行时会读取完整文件。

`batch-rate` 列表格式与 CLI 保持一致：每行一组，组内名字用 `+` 分隔。`pair` 的选手列表和队友列表为每行一个名字。

## 输出

`batch-rate` 和 `pair` 运行前需要先选择输出文件。

文件输出格式对应 CLI 选项：

- `分数 名字`：默认输出格式。
- `JSONL (--log)`：对应 CLI 的 `--log`。
- `名字 (--pure)`：对应 CLI 的 `--pure`。

`文件阈值` 对应 `--min-file`，控制哪些结果写入文件。`日志阈值` 对应 `--min-screen`，控制右侧日志显示。

## 字体

界面字体内嵌使用：

```text
crates\tswn_openbox\src\SarasaMonoSC-Regular.ttf
```
