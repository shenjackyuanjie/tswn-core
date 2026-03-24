# tswn_ds3

`tswn_ds3` 是 `DS3_demo3` 的 Rust 重写版本，目标是结果级兼容原 `all.exe` 流程，不再依赖 `system()` 和 pthread 风格实现。

## 目标与范围

- 覆盖 `all.cpp` 的完整调度链路：`merge -> dedup -> single -> pair -> copy/sort`
- 兼容 `config.txt` 旧格式，并支持 `config.toml`
- 保持目录与文件语义兼容：`input/`, `tmp/`, `file/`, `new/`, `out/`
- 线程配置由 `threads` 控制，使用 `rayon` 并行计算

## CLI

从 `tswn-core` 根目录运行：

```powershell
cargo run -p tswn_ds3 -- run --root D:\path\to\workspace
```

子命令（用于局部调试/对照）：

- `score-bc`, `score-fz`, `score-wc`, `score-fs`, `score-pj`
- `pair-fc`, `pair-wc`, `pair-rh`
- `merge`, `dedup`, `sort`, `show-config`

## 兼容行为说明

已显式保留的旧行为：

- `tmp/` 每次 `run` 前清空重建
- `tmp/blank.txt` 固定写入 `1@1`
- `copy_to_new=1` 时 `tmp/new_dup.txt` 追加两次到 `new/new.txt`
- 输出换行风格使用 `\r\n`

已兼容的 C++ 非直觉行为（pairing）：

- `TYPE=1` 异集合配对：只使用右侧首行参与计算
- `TYPE=0` 同集合配对：保留额外 off-by-one 风格 pair
- pair gate 语义：不丢弃 pair，而是对应侧分数置 0 再求和

## 验证与测试

### Rust golden tests

```powershell
cargo test -p tswn_ds3
```

当前覆盖：

- `tests/golden_single.rs`
- `tests/golden_pairing.rs`
- `tests/golden_pipeline.rs`

并包含 `threads=1` 与 `threads=4` 输出一致性断言。

### 与 C++ 对照

脚本：

`crates/tswn_ds3/scripts/compare_with_cpp.ps1`

示例：

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\tswn_ds3\scripts\compare_with_cpp.ps1 -Fixture basic
```

如果使用 `target\ds3-cpp-ref2` 作为 C++ 可执行基线：

```powershell
powershell -ExecutionPolicy Bypass -File .\crates\tswn_ds3\scripts\compare_with_cpp.ps1 -Fixture basic -UseRef2CppBin
```

脚本会生成：

- C++ 工作目录
- Rust 工作目录
- `diff-report.txt`（`NO_DIFF` 或差异列表）

可选参数：

- `-FailOnDiff`：有差异时返回非零退出码（便于 CI/自动化）

> 注：目前仓库内的稳定对照基线来自 `target\ds3-cpp-ref2`，所以推荐加 `-UseRef2CppBin`。

## Fixture 目录

位于 `crates/tswn_ds3/tests/fixtures/`：

- `single/`：单体评分输出基线
- `pairing/`：FC/WC/RH 局部配对基线
- `basic/`：完整 pipeline 基线（含 dedup / copy_to_new / pair）
- `no_dedup_no_copy/`：禁 dedup + 禁 copy_to_new 的流程基线
