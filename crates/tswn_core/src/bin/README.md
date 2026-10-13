# tswn_core 二进制入口

当前 crate 提供三个二进制目标：

| bin | 用途 | feature |
| --- | --- | --- |
| `tswn-cli` | 对战、评分、胜率、图标与 DIY/OL 导出 | 默认可用 |
| `track` | 将 `track test` 转发到测试 checkpoint 工具 | 默认可用 |
| `track_test` | 记录、比较和维护测试失败集 | `aux_bins` |

## tswn-cli

```powershell
cargo run -p tswn_core --bin tswn-cli -- --help
cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt
cargo run -p tswn_core --bin tswn-cli -- fight --jsonl -f input.txt --max-rounds 20000
cargo run -p tswn_core --bin tswn-cli -- runtime diff -f input.txt
cargo run -p tswn_core --bin tswn-cli -- bench win-rate -f input.txt -n 10000
cargo run -p tswn_core --bin tswn-cli -- runtime normalized-run -f input.txt --max-rounds 20000
```

`fight` / `fight --jsonl` 使用 BattleSession，`runtime diff` 与 `bench` 使用主 Runtime。旧 raw、顶层 diff、--out-raw 和 !test! 自动路由已移除。CLI 不提供执行器选择器或 parity 子命令。

## track / track_test

```powershell
cargo build -p tswn_core --features aux_bins --bin track --bin track_test
./target/debug/track test --help
cargo run -p tswn_core --features aux_bins --bin track_test -- --help
```

`track test` 优先调用同目录的 `track_test`，因此先按上述命令构建两个二进制；当前 Cargo fallback 不会自动启用 `aux_bins`。也可直接用带 `aux_bins` 的 `cargo run` 调用 `track_test`。`track` 只提供 `test` 转发。主 Runtime 的正式 release 回归不经过该 checkpoint 工具，而统一运行：

```powershell
python scripts/check_runtime_release.py --corpus
```
