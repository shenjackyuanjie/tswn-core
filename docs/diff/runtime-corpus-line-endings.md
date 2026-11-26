# Runtime 语料换行与构建产物复用

## 换行在解析前已规范化

在提交 `812ab564`，golden harness 把实际输入传给 [RuntimeRunner](../../crates/tswn_core/src/runtime/runner.rs#L367-L377)。[`PreparedBattleInit::split_namerena_raw`](../../crates/tswn_core/src/runtime/prepared_init/init.rs#L3-L20) 先把 CRLF 和独立的 CR 转成 LF，再截去行尾空白、提取 seed、按空行分组。因此行尾回车不会进入名字、阵营或 seed，也不会改变队伍划分。输入哈希另行规范化换行，两者无依赖。

在该提交的独立工作树和独立构建目录里，用相同内容的 LF 与 CRLF 分别跑完 39 条 runtime stress 语料。全部逐轮事件、胜者和 RC4 完整状态（含 S-box）一致。代表用例 [`3v3v3-711d7e09df7b7da3.txt`](../../crates/tswn_test/cases/runtime_stress/3v3v3-711d7e09df7b7da3.txt) 的 LF/CRLF 输入分别是 314/325 字节，均解析为三队各三人、无显式 seed，均得到 60 次 `run_round`、60 个可见帧、7039 分、胜者队伍 1、最终 RC4 `(203, 151)`。两种输入也各自重新编译为 `include_str!` 资源，均通过原冻结断言。工作区换行变化不需要重算 golden 或删除旧的逐轮摘要。

## 56 轮来自另一份可执行文件

先前把 56 轮、RC4 `(74, 97)` 误判为 LF 的结果：强制更新输入文件 mtime 前，实验复用了带魅惑到期同步改动的可执行文件。强制更新 mtime 并重跑后，原始版本与仅改自然解冻均为 60 轮；仅改到期同步与同时改两项均为 56 轮。机制分叉见[魅惑到期与自然解冻取证](charm-expiry-and-thaw.md#单局实跑先保留-rust-的到期清理时序)。

遇到疑似换行差异，优先在同一进程把两份原始文本分别交给 runner，直接比较解析输入与逐轮输出。必须换编译期资源时，应固定代码版本和构建目录，核实输入字节已嵌入，并确认预期的测试目标确实重新编译，避免把旧产物的结果当成新输入的结果。
