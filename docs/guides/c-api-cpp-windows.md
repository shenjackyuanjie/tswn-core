# Windows 下使用 `tswn_capi` 编译 C++ 示例

本文说明在 Windows 环境下链接 `tswn_capi` 的推荐做法，以及 `PreparedRunner` 的 seed 与运行目录约定。

适用场景：

- 使用 `clang++` / `g++` 编译 C++ 示例
- 链接 `tswn_capi.dll.lib`（动态链接）
- 链接 `tswn_capi.lib`（静态链接）
- 使用 `PreparedRunner`
- 需要确认 `raw` / `prepared` 在 seed 与 no-seed 情况下的行为一致性

---

## 1. 目录与前提

从仓库根目录生成 C API 包，然后进入包根目录：

```powershell
uv run scripts/build_capi.py --release
cd crates/tswn_capi/dist/capi
```

Windows MSVC 构建的关键布局如下；从整合发布包使用时，同样进入其中的 `capi/` 子目录：

```text
capi/
  include/tswn_capi.h
  lib/tswn_capi.dll
  lib/tswn_capi.dll.lib
  lib/tswn_capi.lib
  examples/*.c
```

后文命令均从这个包根目录运行。`openbox_capi.cpp`、`openbox_capi_prepare.cpp` 和 `input.txt` 是**自备调用程序及输入的示例文件名**，不随当前包提供；使用这些命令前先将自己的文件放到包根目录。仓库现成用例见 [C 示例说明](../../crates/tswn_capi/examples/README.md)、[prepared_runner.c](../../crates/tswn_capi/examples/prepared_runner.c) 和 [runner_fight.c](../../crates/tswn_capi/examples/runner_fight.c)。

请在 Visual Studio Developer PowerShell 中运行 Windows MSVC 链接命令，确认 LLVM 使用 MSVC 目标，且程序、Rust 库与 Windows SDK 的架构一致。当前 C ABI 版本为 4；头文件和库应来自同一构建。`PreparedRunner` 兼容接口仍保留，正式战斗会话用法可参考 [battle_session.c](../../crates/tswn_capi/examples/battle_session.c)。

---

## 2. C++ 源文件推荐写法

### 2.1 头文件 include

推荐直接写：

```cpp
#include "tswn_capi.h"
```

然后在编译命令里传：

```text
-Iinclude
```

不推荐再写这种相对路径 include：

```cpp
#include "capi/include/tswn_capi.h"
```

因为这会强依赖当前工作目录与 bundle 的相对布局，容易导致找不到头文件。

---

### 2.2 不要在源码里写 `#pragma comment(lib, ...)`

不推荐：

```cpp
#pragma comment(lib, "capi/lib/tswn_capi.lib")
```

原因：

- 它会和命令行里显式传入的库路径重复
- 当前工作目录变化后，相对路径很容易失效
- 最终会出现类似：
  - linker 能看到你命令里传的库
  - 但又额外尝试打开一个错误的相对路径库

推荐做法：

- 源码里不写 `#pragma comment(lib, ...)`
- 统一在编译命令里显式传库路径

---

## 3. `PreparedRunner` 的 seed 格式

`PreparedRunner` 只预解析队伍；每局的 seed 由创建 Runner 时的参数提供，原始文本中的 seed 不会保存在 prepared 句柄里。

### 3.1 错误写法

错误地把 seed 当作裸值传入：

```text
33554431@!
```

裸值不是完整 seed 行，无法保证与 raw 文本中 `seed:...` 的输入语义一致；对照两条路径时应使用完整格式。

### 3.2 正确写法

`tswn_runner_new_from_prepared()` 的 seed 参数应传入**完整 seed 行**：

```text
seed:33554431@!
```

也就是说：

- 不传 seed：传 `NULL`
- 传 seed：传完整的 `seed:...` 字符串

### 3.3 为什么必须这样

`raw` 路径本质上是在原始输入文本里追加一整行：

```text
seed:33554431@!
```

为了保持与 `raw` 路径一致，`prepared` 路径也必须传同样格式的字符串。

---

## 4. `raw` 无 seed 与 `prepared` 无 seed 是否一致

相同队伍和 `eval_rq` 下，raw 不含 seed 行、prepared 创建 Runner 时传 `NULL`，使用的是同一条无 seed 初始化路径。
[C API 实现](../../crates/tswn_capi/src/runner_api.rs) 将 raw 拆成队伍和 seed，再调用核心 Runner；
[核心会话构造](../../crates/tswn_core/src/runtime/session.rs) 的 raw 路径也先准备队伍，再以 seed 创建对局。

有 seed 时，将 raw 文本中的完整 `seed:...` 行传给 `tswn_runner_new_from_prepared()`，即可保持相同的输入语义。

---

## 5. Windows 下的链接形式

当前 Windows MSVC 打包结果中通常包含（文件均位于 `lib/`）：

```text
tswn_capi.dll
tswn_capi.dll.lib
tswn_capi.lib
tswn_capi.dll.exp
```

`tswn_capi.pdb` 是启用调试信息时的可选产物，默认 release 包不保证包含它；打包脚本只收集当前构建中实际存在的文件。

含义：

- `tswn_capi.dll`
  - 动态库本体
- `tswn_capi.dll.lib`
  - import lib，供动态链接使用
- `tswn_capi.lib`
  - staticlib，供静态链接使用

---

## 6. `clang++` 动态链接命令

这是 Windows 下最稳妥、最省心的方案。

以下命令继续以 C API 包根目录为工作目录：

```text
crates/tswn_capi/dist/capi
```

### 6.1 编译 `raw` 版

```powershell
clang++ openbox_capi.cpp lib/tswn_capi.dll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Iinclude
```

### 6.2 编译 `prepared` 版

```powershell
clang++ openbox_capi_prepare.cpp lib/tswn_capi.dll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Iinclude
```

### 6.3 运行前需要 DLL

动态链接时，运行前要确保 `tswn_capi.dll` 能被找到。

最简单做法是把 DLL 复制到当前目录：

```powershell
copy lib\tswn_capi.dll .
```

然后运行：

```powershell
.\openbox_capi.exe
```

```powershell
.\openbox_capi_prepare.exe
```

---

## 7. `clang++` 静态链接命令

如果希望把 `tswn_capi` 本体直接静态链接进最终可执行文件，可以使用：

- `tswn_capi.lib`

但 Windows 下需要注意：

- 可能还要额外补系统库
- 当前已验证的一种情况是需要：
  - `ntdll.lib`

Developer PowerShell 已配置 Windows SDK 库搜索路径，因此命令可直接写 `ntdll.lib`；其他环境应传入本机实际 SDK 版本与目标架构下的完整路径。其余系统库依具体链接器和错误信息补充。

### 7.1 编译 `raw` 版

```powershell
clang++ openbox_capi.cpp lib/tswn_capi.lib ntdll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Iinclude
```

### 7.2 编译 `prepared` 版

```powershell
clang++ openbox_capi_prepare.cpp lib/tswn_capi.lib ntdll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Iinclude
```

### 7.3 静态链接时的现象

如果不补 `ntdll.lib`，可能会看到类似错误：

```text
unresolved external symbol __imp_NtWriteFile
unresolved external symbol __imp_RtlNtStatusToDosError
fatal error LNK1120
```

这时应显式传入 `ntdll.lib` 的完整路径。

---

## 8. `g++` / MinGW 动态链接

### 8.1 可尝试命令

```powershell
g++ openbox_capi.cpp lib/tswn_capi.dll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Iinclude
```

```powershell
g++ openbox_capi_prepare.cpp lib/tswn_capi.dll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Iinclude
```

### 8.2 但需要注意

`tswn_capi.dll.lib` 通常是 **MSVC 风格 import lib**。  
`g++` / MinGW 对这种库格式不一定兼容。

因此可能出现：

- file format not recognized
- undefined reference
- 无法使用 `.dll.lib`

如果出现这种情况，不是命令错，而是：

- `g++` 不兼容这个 `.dll.lib` 格式

---

## 9. `g++` 更稳的做法：先生成 `.dll.a`

如果想让 MinGW / `g++` 更稳妥地动态链接，最好先把 DLL 转成 MinGW 风格 import lib，例如：

```text
libtswn_capi.dll.a
```

常见流程是：

### 9.1 从 DLL 生成 `.def`

```powershell
gendef lib\tswn_capi.dll
```

### 9.2 从 `.def` 生成 `.dll.a`

```powershell
dlltool -d tswn_capi.def -l libtswn_capi.dll.a -D tswn_capi.dll
```

### 9.3 然后再用 `g++`

```powershell
g++ openbox_capi.cpp libtswn_capi.dll.a -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Iinclude
```

```powershell
g++ openbox_capi_prepare.cpp libtswn_capi.dll.a -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Iinclude
```

运行时仍然需要 `tswn_capi.dll`。

---

## 10. 推荐顺序

在 Windows 下，推荐优先级如下：

### 推荐 1：`clang++` 动态链接

优点：

- 命令最简单
- 不需要额外系统库
- 兼容当前 `tswn_capi.dll.lib` 打包产物

### 推荐 2：`clang++` 静态链接

优点：

- 最终 exe 不依赖 `tswn_capi.dll`

缺点：

- 需要补 `ntdll.lib`
- 系统库问题更容易暴露

### 推荐 3：`g++`

只有在你明确使用 MinGW / `g++` 时再考虑。  
若 `g++` 不能直接吃 `.dll.lib`，则需要自行转 `.dll.a`。

---

## 11. 运行时输入

若自备程序采用文件输入，可约定从当前目录读取：

```text
input.txt
```

例如：

```text
喘际瞬爆@昀澤
蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall
```

若自备程序还接受标准输入中的测试次数，可以输入：

```text
10000
```

这些输入约定由调用程序决定；仓库的 C 示例使用内置输入，不要求额外提供 `input.txt` 或次数。

---

## 12. 常见报错与排查

### 12.1 找不到头文件

若报：

```text
fatal error: 'tswn_capi.h' file not found
```

检查：

- 是否用了：
  - `#include "tswn_capi.h"`
- 是否传了：
  - `-Iinclude`

---

### 12.2 动态运行时找不到 DLL

若报找不到 `tswn_capi.dll`：

- 把 `tswn_capi.dll` 复制到 exe 同目录
- 或把 DLL 所在目录加入 `PATH`

---

### 12.3 静态链接缺少 `NtWriteFile` / `RtlNtStatusToDosError`

补：

```text
ntdll.lib
```

---

### 12.4 `g++` 不认 `.dll.lib`

这是库格式兼容问题，不是命令写错。  
解决方式：

- 换用 `clang++`
- 或先把 DLL 转成 `.dll.a`

## 13. 备注

长期维护自己的 C++ 调用程序时，建议保持以下约定：

- 头文件统一 `#include "tswn_capi.h"`
- 不在源码里写 `#pragma comment(lib, ...)`
- `PreparedRunner` seed 永远传完整 `seed:...` 行
- Windows 下优先提供 `clang++` 的动态链接命令作为默认文档示例
