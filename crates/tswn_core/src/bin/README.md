# tswn_core bin 目录说明

这个目录存放 tswn_core crate 的所有二进制入口，可以分成两类：

- 面向日常使用的统一 CLI：tswn-cli。
- 面向回归追踪、对拍和性能分析的辅助工具。

如果只是想跑对战、benchmark、图标导出或 DIY 文本转换，优先使用 tswn-cli；其余 bin 主要服务于仓库内部验证脚本和问题定位。

## 运行方式

单个 bin 的常见启动方式：

```bash
cargo run -p tswn_core --bin tswn-cli -- --help
cargo run -p tswn_core --bin track -- --help
cargo run -p tswn_core --features aux_bins --bin track_test -- --help
```

如果要一次性构建所有辅助工具，建议显式带上 aux_bins：

```bash
cargo build -p tswn_core --bins --features aux_bins
```

说明：

- tswn_cli/ 是 tswn-cli 的实现模块目录，不是独立 bin。
- track_test、track_case_miner、track_perf_cases 依赖 aux_bins feature。
- tswn-cli、track、track_diy_roundtrip、tswn_case_miner 默认可构建。

## Bin 一览

| bin | 主要用途 | 典型场景 | 额外 feature |
| --- | --- | --- | --- |
| tswn-cli | 统一的用户侧命令行入口 | 对战、benchmark、图标导出、DIY/OL 导出 | 无 |
| track | 对常用追踪工具的薄封装 | 用更短命令启动 test/miner/diy 追踪 | 无 |
| track_test | 记录并比较 cargo test 失败集 | 回归追踪、失败集 checkpoint 管理 | aux_bins |
| track_case_miner | 带历史记录和 checkpoint 的 case miner 包装器 | 持续跟踪 TS/Rust diff 失败变化 | aux_bins |
| track_diy_roundtrip | 校验 DIY/OL 导出后的回读和战斗一致性 | 导出链路回归验证 | 无 |
| track_perf_cases | 选择复杂度阶梯 case 并输出 benchmark 报告 | 构造/维护性能基准集 | aux_bins |
| tswn_case_miner | 直接生成 TS vs Rust trace diff case 与 summary | 对拍、收集失败样例、缓存 TS trace | 无 |

## 各 bin 说明

### tswn-cli

统一对外入口，覆盖大多数日常命令：

- fight / raw / diff：普通对战、raw 输出、runner diff 输出。
- bench auto / win-rate / group-win-rate / batch-rate(cqp) / pair：评分与胜率相关 benchmark。
- namer-pf：与 ica-plugin /namer-pf 对齐的四项评分；可用 `--mode pp pd` 只跑指定项。
- icon show / b64 / save：图标预览与导出。
- to-diy：导出 DIY/OL overlay 文本。

示例：

```bash
cargo run -p tswn_core --bin tswn-cli -- fight -r "mario\nluigi\n\npeach\nbowser"
cargo run -p tswn_core --bin tswn-cli -- bench cqp -l targets.txt -p players.txt -n 10000
cargo run -p tswn_core --bin tswn-cli -- to-diy -f names.txt -o diy.txt
```

### track

这是一个薄包装器，用于把短命令转发到更具体的回归工具：

- track test -> track_test
- track miner -> track_case_miner
- track diy -> track_diy_roundtrip

它适合本地迭代时快速调用常用追踪命令，但不会覆盖所有辅助 bin；例如 track_perf_cases 仍需直接调用。

### track_test

运行带过滤条件的 cargo test，记录当前失败集，并和上次结果比较。除直接运行外，还支持 save/list/diff/delete 维护 checkpoint。

默认关注的测试过滤表达式是：large large_full small_seed fight_multi。

主要产物：

- target/test_regression.json：当前失败记录。
- target/test_regression.log：最近一次汇总日志。
- target/test_checkpoints/：手动保存的 checkpoint。

### track_case_miner

这是 tswn_case_miner 的追踪包装器。它负责：

- 调用 tswn_case_miner 生成或复用 case 结果。
- 读取输出目录中的 summary.json。
- 与上次记录比较，给出 improved/regressed/fixed/new failed 的结论。
- 维护 checkpoint。

默认输出目录是 target/ts_diff_cases，默认共享缓存目录是 tests/tswn_case_miner_cache。

### track_diy_roundtrip

这个工具验证“名字 -> DIY/OL overlay -> 引擎状态/战斗流程”这一链路是否保持一致。可以直接从号库生成 case，也可以复用已有 case 目录。

适合修改以下逻辑后做回归检查：

- to_diy 导出格式。
- DIY 解析/存储恢复逻辑。
- 战斗初始化或回合更新逻辑。

默认输出目录：target/diy_roundtrip。

### track_perf_cases

这个工具先从号库中生成大量候选 case，再通过采样选出一组从简单到复杂的性能阶梯 case，最后跑正式 benchmark 并输出报告。

它有两个常见用法：

- 从号库自动选 case，生成新的性能基准集。
- 对已有固定 case 目录重新跑 benchmark，保证不同版本的横向可比性。

默认输出目录：target/perf_cases。

### tswn_case_miner

直接生成 TS 与 Rust 的 trace 对比 case，是定位行为不一致的底层工具。它会：

- 按给定模式生成唯一输入 case。
- 复用或生成 TS trace 缓存。
- 执行 Rust trace 并逐行比较输出。
- 把失败 case、空输出 case、summary 和报告写入输出目录。

默认输出目录：target/ts_diff_cases。默认会自动尝试推导 fast-namerena/branch/latest/out_md5.ts 作为 TS 基准工具。

## 建议选择

- 日常对战、胜率和导出操作：用 tswn-cli。
- 想快速看测试是否退步：用 track test。
- 想持续跟踪 TS/Rust 行为差异：用 track miner 或直接用 tswn_case_miner。
- 想验证 DIY/OL 导出链路：用 track diy 或直接用 track_diy_roundtrip。
- 想维护性能基准样例：用 track_perf_cases。
