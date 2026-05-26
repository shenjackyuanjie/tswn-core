# tswn_core

名字竞技场的 Rust 核心引擎。

## 功能

- **战斗模拟**：解析 namerena 输入格式，完整模拟回合制对战
- **胜率统计**：支持单线程/多线程批量胜率计算，含 JS profile seed 调度
- **CLI 工具**：`tswn-cli` 提供 bench、win-rate、fight 等子命令
- **DIY 转换**：`tswn-cli to-diy -r NAME` 默认输出 `+ol`，加 `--old` 输出 `+diy`，也支持 `-f/--file` 和 `-o/--out-file`
- **图标渲染**：玩家名称 → 16×16 像素头像（PNG / Base64 / RGBA）
- **DS3 兼容**：通过 `tswn_ds3` crate 提供 DS3_demo3 流程兼容

## 快速开始

### CLI

```powershell
# 构建
cargo build -p tswn_core --bin tswn-cli --release

# 胜率基准测试
./target/release/tswn-cli bench win-rate --total 100000 --perf

# 单局对战（stdin 输入）
echo '<your raw input>' | ./target/release/tswn-cli fight

# DIY/OL 导出
./target/release/tswn-cli to-diy -r "mario@team+fire"
./target/release/tswn-cli to-diy -r "mario@team+fire" --old
./target/release/tswn-cli to-diy -f names.txt -o diy.txt

# 批量胜率输出筛选与导出格式
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt --min-screen 60
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o out.txt --min-file 65
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o out.jsonl --log
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt -o names.txt --pure
./target/release/tswn-cli bench batch-rate -l targets.txt -p players.txt --wr-precision 5

# 二人组队友筛选；player-list 和 teammate-list 都是每行一个名字
./target/release/tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 3
./target/release/tswn-cli bench pair -l targets.txt -p players.txt --teammate-list teammates.txt --head 5 -o pair.txt --min-file 250
```

`bench pair` 会为每个 player 与每个 teammate 组成二人组，分别计算 batch rate，并取最高的 `--head <N>` 个 batch rate 求和作为最终分数。player-list 中非 DIY/OL 名字会自动转为默认 `+ol` 格式。

### 作为库使用

```toml
[dependencies]
tswn_core = { path = "crates/tswn_core" }
```

```rust
use tswn_core::{Runner, PreparedRunner};

let runner = Runner::new_from_namerena_raw(raw_input, eval_rq).unwrap();
// 逐回合推进或直接 run_to_completion()
```

## 构建配置

| feature | 默认 | 说明 |
|---------|------|------|
| `png_render` | ✅ | PNG 图标渲染支持 |
| `no_debug` | ❌ | 关闭调试输出，用于 release 分发 |

## 测试

```powershell
cargo test -p tswn_core
```

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
