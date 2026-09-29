# Openbox 实时结果通路验证（2026-09-29）

基线为 `9473188c`，候选为本次工作区实现。机器为 Windows / Ryzen 7 5800X，
使用 workspace `release` 配置（fat LTO），8 个计算线程，开启全部明细。
每项预热一次，随后旧版、新版兼容入口、新版实时入口交替执行三轮，下面取耗时中位数。

## 吞吐对照

| 工具 / 场景 | 规模与场数 | 旧版（秒） | 新版兼容入口（秒） | 实时入口（秒） | 实时相对旧版 |
| --- | --- | ---: | ---: | ---: | ---: |
| pair 短任务 | 20 × 16 × 41，1 场 | 0.511967 | 0.517328 | 0.514917 | +0.58% |
| pair 常规 | 8 × 4 × 41，100 场 | 0.655950 | 0.644532 | 0.660157 | +0.64% |
| pair 长任务 | 2 × 2 × 2，10000 场 | 0.574348 | 0.571800 | 0.580028 | +0.99% |
| cqd/cqp 短任务 | 1000 × 41，1 场 | 0.859508 | 0.855100 | 0.854023 | -0.64% |
| cqd/cqp 常规 | 20 × 41，100 场 | 0.120636 | 0.114032 | 0.113070 | -6.27% |
| cqd/cqp 长任务 | 2 × 2，10000 场 | 0.100735 | 0.101332 | 0.101054 | +0.32% |

本次样本未出现超过 3% 的回退。短时运行存在系统噪声，负差异不作为加速承诺。
实时入口包括结构化结果生成、有界 `LiveFeed` 和每 100ms 的消费者，不包含 GPU 呈现。
`elapsed_s` 排除最后一次收件等待；`first_ms` 从任务开始计时，不等于结果产生后的显示延迟。
原始各轮耗时、首条结果时间、裁剪数和 RSS 见 [JSON](openbox-live-2026-09-29.json)。

pair 输入分别取 `docs/perf/cqp/sqp6000_first20.txt`、
`crates/tswn_openbox/assets/teammates/teammate_fz.txt`、
`crates/tswn_openbox/assets/targets/target2.txt` 的前 N 行，`head=3`、`detail=every`。
cqd/cqp 选手为 `probe0@red` 到 `probe999@red` 的前 N 行，靶子取同一 `target2.txt` 的前 N 行。

复测入口：

```powershell
openbox_pair_probe --players players.txt --teammates mates.txt --targets targets.txt --count 100 --threads 8 --head 3 --detail every
openbox_pair_probe --players players.txt --teammates mates.txt --targets targets.txt --count 100 --threads 8 --head 3 --detail every --live
openbox_mem_probe --players batch-players.txt --targets targets.txt --limit 1000 --target-limit 41 --count 1 --threads 8 --show-matchups --report-ms 100000 --live
```

上述 cqd/cqp 实时样本的结束 RSS 最大为 47708 KiB，未发生收件箱容量裁剪。
这是包含输入矩阵和计算器的进程 RSS；8MiB 是展示数据预算，不是整个进程内存上限。

## 正确性与界面

- pair 三组样本的新旧兼容入口 stdout SHA-256 一致。
- CLI 的普通 DIY、旧 DIY、召唤物、五项评分与技能榜共五组样例逐字节一致。
- 单元测试验证三种文件格式、并发重名、跨窗口、带权靶子与队友、镜像、跳过和稳定同分排序。
- 通过首条明细回调立即取消，验证 cqd/cqp、pair、to-diy 确实在整批结束前发布结果。
- 10000 条结构化结果的 egui 测试验证只布局可见行，并覆盖展示容量裁剪。
- 原生窗口执行真实 pair 任务，使用 egui 截图回传验证运行中切换三种视图及停止状态。
  截图：[卡片](../../images/openbox-live/cards.png)、[表格](../../images/openbox-live/table.png)、
  [文本](../../images/openbox-live/text.png)、[停止](../../images/openbox-live/stopped.png)。

可用默认关闭的 `ui_capture` feature 复现上述原生窗口检查：

```powershell
cargo run -p tswn_openbox --bin tswn_openbox --features ui_capture -- --capture-dir target/openbox-captures
```

## 检查命令

执行 `cargo test -p tswn_core`、`cargo test`、
`cargo test -p tswn_openbox --features ui_capture`、
`cargo clippy --workspace --all-targets --features tswn_openbox/ui_capture` 和
`cargo +nightly fmt --check`。Clippy 保留仓库已有警告，不修改无关代码。

本机默认临时目录不可写，验证进程将 `TEMP` / `TMP` 指向工作区 `target/test-tmp`；
开发测试使用 `CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`，避开最初的 MSVC 调试记录错误。
release 对照使用原 profile，没有降低优化或修改依赖 feature。
