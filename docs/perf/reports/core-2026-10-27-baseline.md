# 2026-10-27 当前提交与上次记录的同机复测

## 结论

本次没有证据证明最新提交带来确定的核心性能变化。最新 `65ab2ee752f563384088e6808e682fa17d267ef5` 与上次维护后产品 `cfee708dddbaf7770d8a70fbbd448b259f77f917` 的 `crates/tswn_core/src`、crate 配置和 `perf_runtime` 源码完全相同；core 差异仅 README 和额外的 maintenance example。锁文件差异仅其他 workspace crate 自身版本号。

五轮同机交替的所有输入 `total / wins / errors / guard_exhausted` 一致，errors 全零；guard 保留原有行为。总体中位数比看似快 4.61%，逐轮配对中位数却慢 0.85%，配对 MAD 达 5.05 个百分点，**不能取有利的数字宣布优化**。FFA 中位数慢 1.50%，亦不构成源码变更导致的回退证据。

## 口径

- 上次留档：[2026-10-24 维护报告](maintenance-2026-10-24.md)，Windows 10 / Ryzen 7 5800X / Rust 1.99.0；它的秒数仅作历史背景，不能与本次 Linux 云端直接计算版本速度差。
- 本次：Linux x86_64 云端，AMD EPYC 9V74，进程允许 CPU 0–8；Rust 1.99.1，LLVM 23.1.1。没有修改系统频率、安全配置或全局调度。
- 两版各自 checkout / Cargo.lock，`cargo build --locked -p tswn_core --release --features no_debug --example perf_runtime`；fat LTO、单 codegen unit、默认 mimalloc；同一工具链。
- 内置 `perf_runtime`：固定 30 个输入，每输入 13000 场，单线程、无 `--timed`；每版预热一次，正式五轮交替 A/B。全部原始调用保留，不删除慢轮次。
- E2E 范围为公开批量 API 的模板准备、初始化、战斗与汇总；不只计算 fight，不包含输入文件读取、JSON 序列化或 GUI。另保留完整进程墙钟。
- 本轮没有固定 CPU 亲和性，云端调度噪声不可忽略；后续优化实验采用固定亲和性并单独留档，不混合两种口径。

## 实测

耗时变化为 `median(当前) / median(上次) - 1`，负值表示更快。配对变化按每轮当前/上次算，再取中位数。

| 分组 | 上次同机中位数 s | 当前中位数 s | 中位数比变化 | 配对中位变化 | 配对 MAD |
| --- | ---: | ---: | ---: | ---: | ---: |
| fixed30 overall | 14.084520 | 13.434957 | -4.61% | +0.85% | 5.05% |
| 9 个 FFA | 5.443677 | 5.525160 | +1.50% | +3.47% | 1.53% |
| 1v1 / 2v2 | 3.723615 | 3.793518 | +1.88% | -0.98% | 2.85% |

FFA 为 4/6/8 人的全部 9 个固定输入，并非单个有利 case。原始输出保留于本次执行记录；公开报告仅包含审核后的性能数据，不上传完整环境转储或日志压缩包。总体与子集有重叠，不按多项独立实验投票。

## 逐轮汇总与复现

以下为五轮正式测量的公开批量 API 总耗时（秒）；预热不纳入。

| 轮次 | 上次 overall | 当前 overall | 上次 FFA | 当前 FFA |
| --- | ---: | ---: | ---: | ---: |
| 1 | 12.836038572 | 13.083756350 | 5.049392835 | 5.224734794 |
| 2 | 13.476331788 | 14.272109375 | 5.499375286 | 5.714559317 |
| 3 | 14.469799640 | 13.350384211 | 5.419854285 | 5.525160049 |
| 4 | 14.084519815 | 13.434957307 | 5.768625143 | 5.178950673 |
| 5 | 14.382645030 | 14.505104812 | 5.443677499 | 5.741752583 |

两个 checkout 分别执行上面的构建命令。将下方路径替换为对应二进制，在当前仓库根目录运行；同一套固定输入、同一工具链，顺序交替，正式测量期间不要并行编译或运行其他 benchmark。

```sh
PRIOR=/path/to/prior/perf_runtime
CURRENT=/path/to/current/perf_runtime
mkdir -p benchmark-results
# round 0 预热；round 1–5 正式测量。
for round in 0 1 2 3 4 5; do
  if [ $((round % 2)) -eq 0 ]; then order="current prior"; else order="prior current"; fi
  for variant in $order; do
    if [ "$variant" = prior ]; then binary="$PRIOR"; else binary="$CURRENT"; fi
    "$binary" -i docs/perf/fixed_cases_30 -n 13000 --threads 1 \
      > "benchmark-results/$round-$variant.jsonl"
  done
done
```

每次输出按 `elapsed_seconds` 求和；FFA 仅选文件名含 `_ffa_` 的 9 个输入。各版分别取五轮中位数；配对变化为同轮 `当前 / 上次 - 1`，MAD 为这些变化与其中位数的绝对差的中位数。逐输入核对 `total / wins / errors / guard_exhausted`，不只核对聚合胜率。

本记录聚焦用户要求的内置核心 benchmark；没有声称重跑上次报告的 GUI、Python、WASM、PWP 全矩阵。优化与扩展样本/profile 的后续结果独立记录。
