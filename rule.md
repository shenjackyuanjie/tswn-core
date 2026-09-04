# tswn-core 项目性能优化准则

## 核心准则

所有性能优化都必须建立在行为不回退的前提下。`tswn_core 0.5.0` 已删除旧执行器，
正确性判断统一使用主 Runtime 的冻结 corpus，不再通过生产 CLI 或辅助 binary 切换执行器对账。

## Release profile 选择

- `--release`：正式 benchmark 和发布口径，使用 fat LTO、`codegen-units = 1` 与默认 mimalloc。
- `--profile release-fast`：日常快速验证口径，使用 thin LTO 和更多 codegen units；结果不能写入正式性能表。
- `no_debug`：正式性能与发布构建必须启用，避免诊断逻辑进入热路径。

## 行为验证

日常修改至少运行相关测试；准备发布时必须运行完整门禁：

```powershell
cargo test --workspace
python scripts/check_runtime_release.py --corpus
```

第二条命令同时检查旧源码路径与禁用 API 未回流，并执行 87 个 JS exact trace 和
37 个冻结压力 golden。C、Python、WASM 和 OpenBox 等包装层修改还应运行：

```powershell
cargo test --workspace --all-targets --no-run
python scripts/verify_py_cli_api.py --release
```

0.5.0 之前使用的 `tswn_case_miner`、`track_perf_cases`、`track_score_perf` 和
legacy/runtime parity 工具已经随旧执行器删除，不得继续把历史文档中的命令作为当前门禁。

## Benchmark 方法

当前仓库可直接复测的 core 单线程胜率口径：

```powershell
cargo run -p tswn_core --release --features no_debug --bin tswn-cli -- `
  bench win-rate -f input.txt -n 13000 -s --perf
```

OpenBox CQP/CQD 可使用 `openbox_mem_probe`：

```powershell
cargo build -p tswn_openbox --release --bin openbox_mem_probe
target\release\openbox_mem_probe.exe `
  --players docs\perf\cqp\sqp6000_first20.txt `
  --targets crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 1000 --threads 0
```

0.5.0 的 fixed30、score、win-rate 与 OpenBox 完整发版数据、环境和 A/B 判定见
`docs/perf/runtime_0.5.0_749fcd1_release_benchmark.md`。fixed30 与 score 当时使用的临时
外部 harness 不属于发布源码，因此不能用已删除的旧 binary 复跑。

正式结果必须记录被测 commit、rustc/Cargo 版本、feature、输入哈希、线程口径和原始轮次；
机器状态变化时，以同一会话中交替顺序的旧版/新版 A/B 为准。

`--perf` 会打开逐场 init/fight 计时（四次 QPC/场）。不需要 init/fight 拆分时不要加
`--perf`，批量路径默认走不计时的快路径；库调用方要读 timing 请用 `*_timed` 入口。

### 噪声带

同一个二进制在**不同会话之间**可以差到 3%（实测：`5.266/5.303/5.398` vs
`5.103/5.141/5.226`）。因此：

- 必须**同会话交替顺序**跑新旧二进制，至少 4~5 轮取中位数；
- 小于 **1.5%** 的差异不得作为优化成立的依据，也不要写进长期表格；
- "先跑三次旧的，再跑三次新的"这种方式量到的 2%~3% 基本都是机器漂移。

## PGO 构建

PGO 是当前收益最大的一项（非训练输入实测 -25% 左右），且不改运行时代码：

```powershell
python scripts/pgo_build.py                    # CLI：插桩 -> 训练 -> merge -> profile-use
python scripts/pgo_build.py --kind openbox     # Openbox：训练负载走 openbox_mem_probe 的 CQP
python scripts/pgo_build.py --train-runs 8000  # 加大训练量
python scripts/pgo_build.py --skip-train       # 复用已有 profdata 只重建
python scripts/build_all.py --release --pgo    # 发布包的 CLI 与 Openbox 都用 PGO 产物
```

约束：

- `llvm-profdata` 的 LLVM 大版本必须等于 `rustc -vV` 的 LLVM 大版本，脚本会强制校验；
  没有时用 `rustup component add llvm-tools-preview`。
- 训练强制单线程：LLVM 的 IR 插桩计数器不是原子的，多线程训练会丢计数。
- 训练输入默认覆盖 `docs/perf/fixed_cases_30` 全部 30 个 case 加 score 路径；
  只用单一样本训练会削弱泛化性。
- PGO 结果留档必须同时记录 profdata 的生成参数，不同 profile 的结果不可直接比较。

## 采样

Windows 的管理员态主 CPU profiler 是 **AMD uProf CLI**（本机为 Zen 3）。它会把热点函数、
调用栈和源码归因写入 CSV，适合让自动化 agent 直接读取；`samply` 保留为手动查看 Firefox
火焰图 / 时间线的备选工具。完整流程见 [`docs/perf/amduprof.md`](docs/perf/amduprof.md)。

先用 Time-Based Profile（TBP）定位热函数。采样工作负载应至少运行约 15 秒，且不要追加
CLI 的 `--perf`（它会改变热路径的计时开销）：

```powershell
$uProf = (Get-Command AMDuProfCLI.exe -ErrorAction Stop).Source
cargo build -p tswn_core --release --features no_debug --bin tswn-cli
& $uProf collect --config tbp -g `
  -o target\amduprof\before `
  target\release\tswn-cli.exe bench win-rate `
  -f docs\perf\fixed_cases_30\01_1v1-6a2ace7473581042.txt -n 3000000 -s
& $uProf report -i target\amduprof\before --detail
```

uProf 5.3 的 `report` 不接受旧版本文档中的 `-o`；它会自动在采样目录创建
`report.csv`。先看其中的 `HOTTEST FUNCTIONS` / `HOTTEST CALLSTACKS`，优化后再以新目录
复跑。吞吐是否真正变好仍以本节上方规定的同会话交替 A/B benchmark 为准，不能以 profiler
带来的 CPU_TIME 直接断言收益。

AMD uProf 的 PMC / IBS 采样需要在已提权的终端（或已提权的 agent 会话）中执行。若没有
管理员权限，改用 PGO instrumentation 拿函数级执行计数：

```powershell
python scripts/pgo_build.py
llvm-profdata show --topn=45 target\pgo\merged.profdata
```

结构体体积用 `cargo +nightly rustc -p tswn_core --release --lib -- -Zprint-type-sizes`。

有管理员权限时，也可用 `samply` 直接采样公开 CLI，用于手动查看时间线：

```powershell
cargo build -p tswn_core --release --features no_debug --bin tswn-cli
samply record --save-only --unstable-presymbolicate `
  --windows-symbol-server https://msdl.microsoft.com/download/symbols `
  -o target\samply_tswn_cli.json.gz -- `
  target\release\tswn-cli.exe bench win-rate -f input.txt -n 13000 -s --perf
```
