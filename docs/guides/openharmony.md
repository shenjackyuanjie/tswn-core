# OpenHarmony 交叉编译

[返回使用指南](README.md)

通过环境变量为本机构建提供 SDK 路径。先设置 `OHOS_NATIVE_SDK` 指向
OpenHarmony native SDK 目录，也就是包含 `llvm/bin`、`sysroot` 和
`build/cmake/ohos.toolchain.cmake` 的目录。

Windows PowerShell:

```powershell
$env:OHOS_NATIVE_SDK="<path-to-openharmony-native-sdk>"
. .\scripts\ohos-env.ps1
rustup target add aarch64-unknown-linux-ohos
cargo build --target aarch64-unknown-linux-ohos -p tswn_core --bin tswn-cli --release --features no_debug
```

Linux/macOS:

```bash
export OHOS_NATIVE_SDK=/path/to/openharmony/native
. ./scripts/ohos-env.sh
rustup target add aarch64-unknown-linux-ohos
cargo build --target aarch64-unknown-linux-ohos -p tswn_core --bin tswn-cli --release --features no_debug
```

聚合打包时也可以把 OHOS CLI 一起构建进去：

```powershell
$env:OHOS_NATIVE_SDK="<path-to-openharmony-native-sdk>"
uv run scripts/build_all.py --release --clean --include-ohos-cli
```

输出文件位于聚合包的 `cli/bin/` 下，命名形如：

```text
tswn-cli_alpha_<core版本，点替换为下划线>_aarch64_unknown_linux_ohos_unsigned.bin
```

该文件是未签名 ELF 二进制；需要正式部署到受签名策略限制的位置时，仍需按目标环境要求另行签名。

也可以构建 C-API：

```powershell
cargo build --target aarch64-unknown-linux-ohos -p tswn_capi --release
```

脚本会为 `aarch64-unknown-linux-ohos` 导出 Cargo target linker、`AR`、
`CC`、`CXX`、`CFLAGS`、`CXXFLAGS`、`CARGO_ENCODED_RUSTFLAGS`、
`CC_SHELL_ESCAPED_FLAGS` 和 CMake toolchain。`CARGO_ENCODED_RUSTFLAGS`
包含目标、sysroot 和 MUSL 链接参数，并追加已有的同名环境变量内容。
配置逻辑见 [PowerShell 脚本](../../scripts/ohos-env.ps1) 和 [POSIX 脚本](../../scripts/ohos-env.sh)。

如果要检查除 Python/WASM/GUI 绑定之外的 native workspace：

```powershell
cargo check --target aarch64-unknown-linux-ohos --workspace --exclude tswn_py --exclude tswn_wasm --exclude tswn_openbox
```

`tswn_py` 基于 PyO3，交叉编译时需要额外配置 `PYO3_CROSS_*` 或 abi3；
`tswn_wasm` 面向 wasm32；`tswn_openbox` 是桌面 GUI crate。它们不包含在基础
OpenHarmony CLI/C-API 构建路径里。

如果只需要临时调用 C/C++ 编译器，可以使用仓库根目录的包装器：

```powershell
.\ohos-clang.ps1 --version
.\ohos-clangxx.ps1 --version
```

```bash
sh ./ohos-clang.sh --version
sh ./ohos-clangxx.sh --version
```

POSIX 包装器需要直接执行时，先加执行权限：

```bash
chmod +x ./ohos-clang.sh ./ohos-clangxx.sh
```
