# OpenBox pair 并行基准（`openbox_pair_probe`）

[返回性能索引](../README.md)

OpenBox 的 `pair` 平时只能在 GUI 里触发，路径是 `backend::run_pair`：它把
「选手 × 队友 × 靶子」矩阵按窗口切分后交给 `tswn_core` 的共享 CQP/CQD 调度器。
`crates/tswn_openbox/src/bin/openbox_pair_probe.rs` 是这个后端的 headless 入口，
用来在没有窗口、没有 GUI 事件通道的条件下测量整批墙钟并核对输出。

需要区分的两条 pair 路径：

| 入口 | 覆盖的调度层 | 何时用 |
| --- | --- | --- |
| `openbox_pair_probe`（本页） | OpenBox GUI 后端的窗口化矩阵 + 共享 CQP/CQD 调度器 | 验证 OpenBox 内部的 pair 改动 |
| `tswn-cli bench pair` | CLI 外层并行（`bench_sched` + `bench/batch.rs`） | 验证 CLI 侧的 pair 改动 |

两者共用 `tswn_core::runtime::runtime_cqp_matchups`，但外层切分方式不同，不能互相替代。

## 1. 构建

```powershell
cargo build --release -p tswn_openbox --bin openbox_pair_probe
```

产出 `target\release\openbox_pair_probe.exe`。正式留档请用 `release`（workspace 的
`release` profile 带 `lto = "fat"`、`codegen-units = 1`），`tswn_openbox` 对 `tswn_core`
固定启用 `no_debug`，与 GUI 发布构建一致。

## 2. 参数

```text
用法: openbox_pair_probe [选项]

  --players <FILE>             选手列表，每行一个组合（必填）
  --teammates <FILE>           队友列表，每行一个组合（必填）
  --targets <FILE>             靶子列表（必填）
  --count <N>                  每个 matchup 的场数，默认 100
  --threads <N|auto|0>         线程数，auto/0 为自动线程，默认 auto
  --head <N>                   每名选手保留最高的 N 个队友组合，默认 5
  --detail <none|top|every>    屏幕日志的 cqp 明细模式，默认 top
  --teammate-factored          队友文件按带权 TOML（[[targets]]）解析
  --target-factored            靶子文件按带权 TOML（[[targets]]）解析
  -h, --help                   打印本说明
```

- `--count` 对应 GUI 的精度档：`100` = 1%、`1000` = 10%、`10000` = 100%。
- `--threads auto` 与 `openbox_mem_probe --threads 0` 同义；自动线程按逻辑核数取 1.5 倍
  （`--count <= 100`）或 2 倍（其它档），本机 16 逻辑核即 24 / 32 个 worker。
  注意 CLI 的 `-t 0` 是非法参数，两边别混用。
- `keep_rq` 固定 `true`，与 `openbox_mem_probe` 的批量口径一致，用于和历史数据对齐。
- `--detail every` 会打印每个队友组合的 cqp，输出量随队友数线性增长；只做计时时用
  `--detail none`，需要哈希对账时用默认的 `top`。

示例（全网格：20 选手 × 16 队友 × 41 靶子 = 13120 个 matchup）：

```powershell
.\target\release\openbox_pair_probe.exe `
  --players .\docs\perf\cqp\sqp6000_first20.txt `
  --teammates .\crates\tswn_openbox\assets\teammates\teammate_fz.txt `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt `
  --count 100 --threads auto --head 5
```

带权组合（队友权重会参与 `--head` 排序，靶子权重参与每个队友组合的加权平均）：

```powershell
.\target\release\openbox_pair_probe.exe `
  --players .\docs\perf\cqp\sqp6000_first20.txt `
  --teammates .\crates\tswn_openbox\assets\teammates\teammate_fz.toml --teammate-factored `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt `
  --count 100 --threads auto --head 5
```

## 3. 输出约定

- **stdout**：该路径产生的日志行，按原顺序逐行打印。屏幕日志不含耗时字段，因此同一份输入
  在新旧提交上的 stdout 应当逐字节一致，可以直接取哈希做正确性门禁。
- **stderr**：一行摘要。

```text
probe elapsed_s=0.009746 lines=3 progress_ticks=2 progress_last=2 progress_total=2 matrix_estimate=2x1x1=2 done=完成。
```

- `elapsed_s`：整批 `run_pair` 的墙钟，不含进程启动、输入解析与结果打印；
- `progress_last/progress_total`：最后一次进度事件，用于确认矩阵确实跑完（应等于 matchup 数）；
- `matrix_estimate`：按输入「组」数估算的规模（带权 TOML 按 `[[targets]]` 块计数，普通文本按
  非空行计数）。真实 matchup 数还要减去镜像与重名跳过项，所以只当量级参考。

## 4. 同机交替 A/B 流程

1. 两个隔离 worktree 各自 checkout 一个提交并构建 release，双方使用同一份输入；
2. 每个配置先各预热 1 轮，再跑 3 轮以上；奇数轮 旧→新、偶数轮 新→旧，抵消频率与温度漂移；
3. 取墙钟中位数，并同时比较 stdout 哈希。

```powershell
# 用 PowerShell 7 重定向，避免 Windows PowerShell 5.1 的 UTF-16 输出影响哈希
.\target\release\openbox_pair_probe.exe `
  --players .\docs\perf\cqp\sqp6000_first8.txt `
  --teammates .\crates\tswn_openbox\assets\teammates\teammate_fz4.txt `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt `
  --count 100 --threads auto --head 5 > pair-run.txt 2> pair-run.err

Get-FileHash .\pair-run.txt -Algorithm SHA256
Get-Content .\pair-run.err
```

## 5. 输入与派生输入

仓库内可直接使用的输入：

| 文件 | 内容 |
| --- | --- |
| `docs/perf/cqp/sqp6000_first20.txt` | 20 个选手组 |
| `crates/tswn_openbox/assets/teammates/teammate_fz.txt` | 16 个队友组 |
| `crates/tswn_openbox/assets/teammates/teammate_fz.toml` | 同一批队友的带权 TOML 版本 |
| `crates/tswn_openbox/assets/targets/target1.txt` / `target2.txt` | 35 / 41 个靶子组 |

全网格跑 100% 精度档耗时较长。要控制单轮时长，可按下面的方式派生确定性子集
（`docs/perf/reports/openbox-pair-parallelism-2026-09-15.md` 用的就是这几份）：

```powershell
# 前 8 个 / 前 2 个选手组
Get-Content .\docs\perf\cqp\sqp6000_first20.txt | Select-Object -First 8 | Set-Content .\docs\perf\cqp\sqp6000_first8.txt -Encoding utf8
Get-Content .\docs\perf\cqp\sqp6000_first20.txt | Select-Object -First 2 | Set-Content .\docs\perf\cqp\sqp6000_first2.txt -Encoding utf8

# 前 4 个队友组（文本；带权 TOML 见下方说明）
Get-Content .\crates\tswn_openbox\assets\teammates\teammate_fz.txt | Select-Object -First 4 |
  Set-Content .\crates\tswn_openbox\assets\teammates\teammate_fz4.txt -Encoding utf8

# 单个靶子（用于“少 matchup × 长轮次”场景）
Get-Content .\crates\tswn_openbox\assets\targets\target1.txt | Select-Object -First 1 |
  Set-Content .\crates\tswn_openbox\assets\targets\target1_first1.txt -Encoding utf8
```

带权队友场景还需要 `teammate_fz4.toml`（`teammate_fz.toml` 的前 4 个 `[[targets]]` 块，
SHA-256 `400B4ED3EC0A5769B127BEA59DFCA395FFAEC760CE84EB302131357F145E1EB8`）；
2026-09-15 报告用的是这一份，逐块截取即可。

派生文件的 LF 归一化 SHA-256（与 2026-09-15 报告一致）：

| 文件 | SHA-256（LF） |
| --- | --- |
| `sqp6000_first20.txt` | `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6` |
| `sqp6000_first8.txt` | `5C8E6C5D231CE6ADB0EB5C18A2596F8AA4A661B115A67600E1E2DAD8FB51A1A3` |
| `sqp6000_first2.txt` | `1EFFC14D5339F01C5ABD35E79E8AD4E98625677E0500EDBD3C893FEC29866718` |
| `teammate_fz.txt` | `0FA09C42E0D7317919FD8E8F9C377DA00C4F01BF8A0473DD66B2FA6513193BBD` |
| `teammate_fz.toml` | `9B1AAB90F5FF64200057118EA8CF8D2885B9668967199009FF442CCEA40F20BB` |
| `teammate_fz4.txt` | `E6F3FDBFC16E032FF6245DB66026CC21A076A0414A60C96D9FFDB67CB718327A` |
| `target1.txt` | `227C69B2C8C5680B594D26525A1D4CC6383C2615B48AC410F679B06921B7210B` |
| `target2.txt` | `A285A267AE4438BC254FE0511F9FDD1C39A458532CF872984FFB1C035E81569D` |
| `target1_first1.txt` | `069E95BDD57DC20184D41C8700BBF664EF8F01BF75BFB3C4365969289F2C6BD0` |

上表中 `sqp6000_first8.txt`、`sqp6000_first2.txt`、`teammate_fz4.txt`、`target1_first1.txt`
是派生文件，其余是仓库内的固定输入。`teammate_fz4.toml` 见上文说明。

## 6. 其他入口的仪表注意事项

- `openbox_mem_probe` 默认每 2 秒在进度回调里上报一次 RSS，Windows 上要 spawn powershell
  （约 0.3 s），这段停顿会计入被测量区间，并可能通过后端的完成事件队列反向阻塞 worker。
  拿它做 A/B 时请显式加 `--report-ms 3600000`；`openbox_pair_probe` 没有周期性上报。
- 本探针不读取逐场 timing，也不暴露 `_timed` 接口，因此结果里没有 init / fight 拆分。
- 墙钟包含浮点加权汇总与日志格式化，但两者都在窗口边界发生，规模小于 matchup 计算本身。

## 7. 已发布结果

- [OpenBox pair 并行修复：同机交替 A/B 复测（2026-09-15）](../reports/openbox-pair-parallelism-2026-09-15.md)；
  原始样本见 [openbox_pair_parallelism_9b07a04e_ab_samples.json](../openbox_pair_parallelism_9b07a04e_ab_samples.json)。
