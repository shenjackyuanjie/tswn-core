# OpenBox pair 并行修复：复测口径与 worker 验证记录

> 日期：2026-09-15  
> 基线：`9a2bdc1a14f6b5bb10844434e843b94d0bbc7b05`  
> 修复提交：`9b07a04ec51a7869dc409f5c0770fb874dd01318`  
> 状态：源码与回归测试已提交；本会话 worker **未产生可引用的 Rust 性能数字**

## 1. 为什么没有填写“看起来像 benchmark”的数字

本次要求是在 ChatGPT worker 上运行，不使用 GitHub Actions。当前 worker 的可用计算环境为：

- Linux `6.18.44` x86_64，KVM；
- 5 个可见 vCPU，`Intel Xeon Platinum 8573C`；
- worker 镜像没有 `rustc`、`cargo` 或 `rustup`；
- worker 对公网的 DNS/HTTPS 出站不可用，无法从 `static.rust-lang.org` 或 crates.io 安装工具链/依赖。

因此这里不把 GitHub CI、历史报告或模型估算伪装成本次实测。仓库已有的 Rust benchmark 无法在该 worker
上编译执行；后续拿到具备 Rust 工具链与依赖缓存的非 CI worker 后，应按下述口径直接补数字。

## 2. 仓库既有正式口径

本轮复测沿用仓库已有两类口径，而不是另造只对本 PR 有利的微基准：

1. **core fixed30**：`docs/perf/fixed_cases_30` 30 个固定 case，release + `no_debug`，每 case 13000 场。
   单线程用于确认纯战斗热路径没有无关回退；自动线程用于观察共享 batch 调度。入口为
   `crates/tswn_core/examples/perf_runtime.rs`。
2. **OpenBox CQP/CQD 矩阵**：沿用 `docs/perf/reports/cqp-runtime-baseline.md` 的固定输入：
   单人 20×35（700 matchup）与双人 32×41（1312 matchup），`count=100/1000/10000`，
   外层记录整批 wall time。该口径直接覆盖本 PR 修改过的 `runtime_cqp_matchups`。

历史正式报告强调：跨时段绝对值容易受机器频率、温度与后台负载影响，因此性能结论应使用**同一会话
交替 A/B 中位数**，并同时核对 wins/total/errors/guard 或 matchup 完成数，不能只看 wall time。

## 3. 本 PR 需要额外覆盖的 pair 口径

CQP/CQD 只能验证共享矩阵层，不能替代 `pair` 端到端验证。本轮还应增加：

- 固定同一份 player list、target list、teammate list；
- 至少覆盖 1%、10%、100% 三档（100/1000/10000 场）；
- 分别运行 `--thread 1`、显式 2/4 和自动线程；CLI 自动线程通过**省略 `-t/--thread`** 启用，`-t 0` 是非法参数；
- 每个配置至少 5 轮，旧→新 / 新→旧交替，取中位数；
- 记录整条 `bench pair` 的 wall time、CPU time、最终输出 SHA-256；
- 带权队友与带权靶子至少各保留一组，验证排序与 Top-K 汇总没有因并行完成顺序改变；
- Windows 11 实机额外记录逻辑处理器数与任务管理器/采样器的平均 CPU 利用率。

建议使用 release 二进制后从 shell 外层计时，避免把编译时间计入结果。示例：

```powershell
# 两个 worktree 分别 checkout 基线和候选，再各自构建 release。
cargo build --release -p tswn_core --bin tswn-cli --features no_debug

# pair：把 -n 依次换成 100 / 1000 / 10000。
# 自动线程：不要传 -t；显式线程对照时再追加 -t 1 / -t 2 / -t 4。
Measure-Command {
  .\target\release\tswn-cli.exe bench pair `
    -l .\crates\tswn_openbox\assets\targets\target2.txt `
    -p .\cqp_double_target.txt `
    --teammate-list .\crates\tswn_openbox\assets\teammates\teammate_fz.txt `
    --head 5 -n 100 -o .\target\pair-result.txt
}
Get-FileHash .\target\pair-result.txt -Algorithm SHA256
```

对于新版带权 TOML，应使用 CLI 当前对应的 factored 选项和同一份内嵌输入，另外保存结果哈希。

## 4. fixed30 与共享矩阵复测命令

```powershell
# fixed30：单线程 / 自动线程各自独立多轮。
cargo run -p tswn_core --release --features no_debug --example perf_runtime -- `
  --input .\docs\perf\fixed_cases_30 --runs 13000 --threads 1

cargo run -p tswn_core --release --features no_debug --example perf_runtime -- `
  --input .\docs\perf\fixed_cases_30 --runs 13000 --threads 0

# OpenBox 共享矩阵：沿用仓库历史 CQP/CQD 700 / 1312 matchup 口径。
cargo build -p tswn_openbox --release --bin openbox_mem_probe
.\target\release\openbox_mem_probe.exe `
  --players .\docs\perf\cqp\sqp6000_first20.txt `
  --targets .\crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 100 --threads 0
```

双人组使用仓库既有 `cqp_double_target.txt` 与 `target2.txt`，并把 `count` 依次改为 100、1000、10000。

## 5. 验收建议

这次修改的目标不是让所有 workload 都“CPU 100%”，而是消除已确认的串行层级和任务粒度上限。建议按以下规则判定：

- **正确性硬门禁**：旧/新输出、wins/total/errors/guard 或 matchup 完成数一致；任何不一致先视为失败。
- **fixed30 单线程**：不应出现与调度无关的明显回退；其主要作用是排除战斗热路径意外变化。
- **CQP/CQD 与 pair 多线程**：关注同机交替 A/B 中位数；自动线程应在“少量长任务”和“大量短任务”两类场景
  都不再因原先的串行外层/重复建线程而明显闲置。
- **Windows 11**：CPU 利用率只作为诊断指标，最终以 wall time + 结果一致性为准；线程过量时 CPU 更高但 wall 变差，
  不应视为优化。

## 6. 后续补数模板

| workload | count | threads | baseline median | candidate median | change | correctness |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| fixed30 overall | 13000 | 1 | 待测 | 待测 | 待测 | 待测 |
| fixed30 overall | 13000 | 0 | 待测 | 待测 | 待测 | 待测 |
| CQP 20×35 | 100/1000/10000 | 0 | 待测 | 待测 | 待测 | 待测 |
| CQD 32×41 | 100/1000/10000 | 0 | 待测 | 待测 | 待测 | 待测 |
| pair | 100/1000/10000 | 1/2/4/auto | 待测 | 待测 | 待测 | 输出 SHA-256 待测 |

本文件刻意保留“待测”，而不是引用 GitHub Actions 或历史机器数字替代当前提交的实测。
