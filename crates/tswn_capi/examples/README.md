# C-API Examples

本目录提供 `tswn_capi` 的最小 C 示例。

- `version_and_error.c`: 版本、常量与错误读取
- `error_handling.c`: 状态码、last-error 与失败后恢复调用流程
- `runner_fight.c`: 从 raw 输入创建 `Runner` 并跑完整场对局
- `prepared_runner.c`: 复用 `PreparedRunner` 构造多场对局
- `prepared_win_rate.c`: 基于 `PreparedRunner` 批量统计胜率
- `updates_round.c`: 读取 `main_round()` 返回的更新帧
- `win_rate.c`: 调用 `tswn_win_rate`
- `group_win_rate.c`: 调用 `tswn_group_win_rate`
- `icon.c`: 导出 RGBA / PNG / Base64 图标
- `player_snapshot.c`: 读取玩家快照

这些示例默认通过相对路径包含 `../include/tswn_capi.h`。

### 错误处理范例

如果你想看更完整的错误处理流程，而不只是最小的“触发一次错误后读消息”，可以直接看：

- `examples/error_handling.c`

这个示例演示了：

- 如何检查 `tswn_status_t`
- 如何读取 `tswn_last_error_message()`
- 如何用 `tswn_clear_error()` 手动清空错误
- 如何在一次失败之后继续调用其他接口
- 成功调用后，旧的 last-error 会如何被清掉

## 重要说明

### PreparedRunner 的 seed 参数格式

`tswn_runner_new_from_prepared(prepared, seed_utf8, &runner)` 的 `seed_utf8` 建议传入与 raw 文本完全一致的完整 seed 行，例如：

- `NULL`：表示不传 seed
- `seed:33554431@!`
- `seed:33554432@!`

不建议只传裸 seed 值，例如：

- `33554431@!`

这是因为 raw 路径通常是在原始文本中追加一整行 `seed:...`；为了与 raw 路径保持一致，prepared 路径也应传同样格式的字符串。

`examples/prepared_runner.c` 与 `examples/prepared_win_rate.c` 中演示的都是这种完整 seed 行写法。

### `PreparedRunner` 胜率 helper

当前除了 `tswn_win_rate(...)` / `tswn_win_rate_with_eval_rq(...)` 之外，还提供了基于 `PreparedRunner` 的高层胜率接口：

- `tswn_prepared_win_rate(prepared, n, thread, &out_result)`
- `tswn_prepared_win_rate_with_eval_rq(prepared, n, thread, eval_rq, &out_result)`

适用场景：

- 同一组输入需要反复跑很多局
- 希望先 prepare 一次，再多次复用模板
- 避免每次都从 raw 文本重新构造整个对局模板

`examples/prepared_win_rate.c` 演示了这种用法。

### 胜率接口的 `thread` 参数

所有高层胜率接口都使用统一的 `thread` 约定：

- `0`：自动线程数
- `1`：单线程
- `n`：指定多线程数量

示例：

- `tswn_win_rate(raw, 1000, 0, &result)`：自动多线程
- `tswn_win_rate(raw, 1000, 1, &result)`：强制单线程
- `tswn_prepared_win_rate(prepared, 1000, 4, &result)`：固定 4 线程

### Windows staticlib 说明

当前 Windows 打包结果中除了 `tswn_capi.dll` / `tswn_capi.dll.lib` 之外，也会包含 `tswn_capi.lib`（Rust `staticlib` 产物）。

如果你希望把 `tswn_capi` 静态链接进自己的可执行文件，而不是在运行时依赖 `tswn_capi.dll`，可以链接这个 `tswn_capi.lib`。

需要注意：

- Windows 下链接 `tswn_capi.lib` 时，可能还需要额外补充系统库
- 目前已验证的一种情况是需要补 `ntdll.lib`
- 若使用的是 `clang++`，最稳妥的方式通常是显式传入 `ntdll.lib` 的完整路径

## 编译示例

下面假设你当前位于 `crates/tswn_capi/` 根目录，并以 `examples/version_and_error.c` 为例。

### MSVC (`cl`)

如果当前 shell 中没有 `cl`，可先执行 `start-vs-pwsh.ps1` 进入 Visual Studio Developer PowerShell 环境。

典型命令：

`cl /nologo /Iinclude examples\version_and_error.c /link /OUT:examples\version_and_error.exe lib\tswn_capi.dll.lib`

说明：

- 头文件搜索路径使用 `/Iinclude`
- 链接时使用 `lib\tswn_capi.dll.lib`
- 该写法已在当前仓库打包结果上实测可编译通过

### clang（Windows）

典型命令（动态链接）：

`clang -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`

典型命令（静态链接 `tswn_capi.lib`，仍可能需要补系统库）：

`clang -Iinclude examples/version_and_error.c lib/tswn_capi.lib "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib" -o examples/version_and_error.exe`

说明：

- `-Iinclude` 指向头文件目录
- 直接显式传入 `lib/tswn_capi.dll.lib` 可走 DLL/import-lib 方式
- 若改为显式传入 `lib/tswn_capi.lib`，则会尝试静态链接 `tswn_capi`
- 使用 `tswn_capi.lib` 时，可能还需要补充 `ntdll.lib` 等系统库
- 这种写法适合当前产物同时包含 `tswn_capi.dll.lib` 与 `tswn_capi.lib` 的 Windows 包

### gcc（Windows / MinGW 风格）

典型命令：

`gcc -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`

### clang / gcc / cc（Linux/macOS）

若目录中是 `libtswn_capi.so` / `libtswn_capi.dylib`，可使用：

`cc -Iinclude examples/version_and_error.c -Llib -ltswn_capi -o examples/version_and_error`

## 运行说明

- 示例源码默认通过相对路径 `../include/tswn_capi.h` 引用头文件
- Windows 下如使用 MSVC 链接，通常需要 `.lib` 文件
- 若你使用的是 `tswn_capi.dll.lib`，则运行时请确保动态库可被找到；最简单的方式通常是把生成的 `.exe` 与 `tswn_capi.dll` 放在同一目录
- 若运行时提示找不到 `tswn_capi.dll`，可先把 `lib/tswn_capi.dll` 复制到生成的 `.exe` 同目录，再重新执行
- 若你使用的是 `tswn_capi.lib`，则 `tswn_capi` 本体会在链接时进入最终可执行文件，但仍可能需要额外补充 Windows 系统库
- 若你想编译别的示例，只需把上述命令里的 `examples/version_and_error.c` 替换成对应源文件即可
