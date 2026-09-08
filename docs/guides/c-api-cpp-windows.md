# Windows 下使用 `tswn_capi` 编译 C++ 示例

本文记录在 Windows 环境下使用 `tswn_capi` 编译和运行 C++ 示例时的推荐做法，以及这次实际踩到的几个坑。

适用场景：

- 使用 `clang++` / `g++` 编译 C++ 示例
- 链接 `tswn_capi.dll.lib`（动态链接）
- 链接 `tswn_capi.lib`（静态链接）
- 使用 `PreparedRunner`
- 需要确认 `raw` / `prepared` 在 seed 与 no-seed 情况下的行为一致性

---

## 1. 目录与前提

假设当前仓库根目录为：

```text
tswn-core
```

假设当前工作目录为：

```text
tswn-core/dist/all
```

此时关键文件路径如下：

```text
dist/all/
  openbox_capi.cpp
  openbox_capi_prepare.cpp
  input.txt
  tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/
    capi/
      include/tswn_capi.h
      lib/tswn_capi.dll
      lib/tswn_capi.dll.lib
      lib/tswn_capi.lib
```

---

## 2. C++ 源文件推荐写法

### 2.1 头文件 include

推荐直接写：

```cpp
#include "tswn_capi.h"
```

然后在编译命令里传：

```text
-Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
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

这是本次最关键的坑。

### 3.1 错误写法

错误地把 seed 当作裸值传入：

```text
33554431@!
```

这会导致：

- `raw` 路径和 `prepared` 路径结果不一致
- 在实际测试中，`10000` 场的胜率会出现明显偏差

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

### 3.4 实测结论

实测发现：

- `raw` + `seed:...`
- `prepared` + 裸 seed

结果不一致。

修正为：

- `prepared` + 完整 `seed:...`

之后两边结果对齐。

---

## 4. `raw` 无 seed 与 `prepared` 无 seed 是否一致

这个问题已经通过 Rust 测试验证。

结论：

- `raw` 不给 seed
- `prepared` 不给 seed（即 `NULL` / 空 seed）

结果一致。

验证方式包括比较：

- winner
- battle score
- 回合数
- replay trace

因此可以认为：

- no-seed 情况下，`raw` 与 `prepared` 是一致的
- 有 seed 时也可以一致，但前提是 `prepared` 必须传完整 `seed:...` 行

---

## 5. Windows 下的链接形式

当前 Windows 打包结果中通常包含：

```text
tswn_capi.dll
tswn_capi.dll.lib
tswn_capi.lib
tswn_capi.dll.exp
tswn_capi.pdb
```

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

假设当前在：

```text
dist/all
```

### 6.1 编译 `raw` 版

```powershell
clang++ openbox_capi.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

### 6.2 编译 `prepared` 版

```powershell
clang++ openbox_capi_prepare.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

### 6.3 运行前需要 DLL

动态链接时，运行前要确保 `tswn_capi.dll` 能被找到。

最简单做法是把 DLL 复制到当前目录：

```powershell
copy tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle\capi\lib\tswn_capi.dll .
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

### 7.1 编译 `raw` 版

```powershell
clang++ openbox_capi.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.lib "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib" -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

### 7.2 编译 `prepared` 版

```powershell
clang++ openbox_capi_prepare.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.lib "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib" -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
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
g++ openbox_capi.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

```powershell
g++ openbox_capi_prepare.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
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
gendef tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle\capi\lib\tswn_capi.dll
```

### 9.2 从 `.def` 生成 `.dll.a`

```powershell
dlltool -d tswn_capi.def -l libtswn_capi.dll.a -D tswn_capi.dll
```

### 9.3 然后再用 `g++`

```powershell
g++ openbox_capi.cpp libtswn_capi.dll.a -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

```powershell
g++ openbox_capi_prepare.cpp libtswn_capi.dll.a -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
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

示例程序通常从当前目录读取：

```text
input.txt
```

例如：

```text
喘际瞬爆@昀澤
蕾蒂·怀特洛可-65HEZHB264LFPFQ@Squall
```

运行后再从标准输入输入测试次数，例如：

```text
10000
```

---

## 12. 已确认的结论

### 12.1 `PreparedRunner` seed 规则

- `NULL`：不传 seed
- `seed:33554431@!`：正确
- `33554431@!`：错误，不应这样传

### 12.2 无 seed 一致性

已通过 Rust 测试确认：

- `raw` 不给 seed
- `prepared` 不给 seed

二者一致。

### 12.3 有 seed 一致性

已通过实际对战验证：

- `prepared` 若传完整 `seed:...`
- 则与 `raw` 路径结果一致

---

## 13. 常见报错与排查

### 13.1 找不到头文件

若报：

```text
fatal error: 'tswn_capi.h' file not found
```

检查：

- 是否用了：
  - `#include "tswn_capi.h"`
- 是否传了：
  - `-Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include`

---

### 13.2 动态运行时找不到 DLL

若报找不到 `tswn_capi.dll`：

- 把 `tswn_capi.dll` 复制到 exe 同目录
- 或把 DLL 所在目录加入 `PATH`

---

### 13.3 静态链接缺少 `NtWriteFile` / `RtlNtStatusToDosError`

补：

```text
"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib"
```

---

### 13.4 `g++` 不认 `.dll.lib`

这是库格式兼容问题，不是命令写错。  
解决方式：

- 换用 `clang++`
- 或先把 DLL 转成 `.dll.a`

---

## 14. 最短可用命令总结

### `clang++` 动态链接

```powershell
clang++ openbox_capi.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

```powershell
clang++ openbox_capi_prepare.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.dll.lib -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

运行前：

```powershell
copy tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle\capi\lib\tswn_capi.dll .
```

### `clang++` 静态链接

```powershell
clang++ openbox_capi.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.lib "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib" -o openbox_capi.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

```powershell
clang++ openbox_capi_prepare.cpp tswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/lib/tswn_capi.lib "C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\ntdll.lib" -o openbox_capi_prepare.exe -O3 -ffast-math -funroll-loops -Itswn_core_0_2_11_capi_0_1_1_py_0_1_9_bundle/capi/include
```

---

## 15. 备注

如果后续还会长期维护这两个 C++ 示例，建议继续保持以下约定：

- 头文件统一 `#include "tswn_capi.h"`
- 不在源码里写 `#pragma comment(lib, ...)`
- `PreparedRunner` seed 永远传完整 `seed:...` 行
- Windows 下优先提供 `clang++` 的动态链接命令作为默认文档示例
