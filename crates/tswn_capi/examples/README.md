# C-API Examples

本目录提供 `tswn_capi` 的最小 C 示例。

- `version_and_error.c`: 版本、常量与错误读取
- `runner_fight.c`: 从 raw 输入创建 `Runner` 并跑完整场对局
- `prepared_runner.c`: 复用 `PreparedRunner` 构造多场对局
- `updates_round.c`: 读取 `main_round()` 返回的更新帧
- `win_rate.c`: 调用 `tswn_win_rate`
- `group_win_rate.c`: 调用 `tswn_group_win_rate`
- `icon.c`: 导出 RGBA / PNG / Base64 图标
- `player_snapshot.c`: 读取玩家快照

这些示例默认通过相对路径包含 `../include/tswn_capi.h`。

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

典型命令：

`clang -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`

说明：

- `-Iinclude` 指向头文件目录
- 直接显式传入 `lib/tswn_capi.dll.lib` 参与链接
- 这种写法适合当前产物命名为 `tswn_capi.dll.lib` 的 Windows 包

### gcc（Windows / MinGW 风格）

典型命令：

`gcc -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`

### clang / gcc / cc（Linux/macOS）

若目录中是 `libtswn_capi.so` / `libtswn_capi.dylib`，可使用：

`cc -Iinclude examples/version_and_error.c -Llib -ltswn_capi -o examples/version_and_error`

## 运行说明

- 示例源码默认通过相对路径 `../include/tswn_capi.h` 引用头文件
- Windows 下如使用 MSVC 链接，通常需要 `.lib` 文件
- 运行时请确保动态库可被找到；最简单的方式通常是把生成的 `.exe` 与 `tswn_capi.dll` 放在同一目录
- 若运行时提示找不到 `tswn_capi.dll`，可先把 `lib/tswn_capi.dll` 复制到生成的 `.exe` 同目录，再重新执行
- 若你想编译别的示例，只需把上述命令里的 `examples/version_and_error.c` 替换成对应源文件即可
