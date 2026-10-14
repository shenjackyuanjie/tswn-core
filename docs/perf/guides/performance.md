# 性能优化与验证准则

[返回性能索引](../README.md)

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

第二条命令运行 release/no_debug 主 Runtime、release CLI 与完整冻结 corpus。
旧 API 与架构约定留在 [迁移指南](../../guides/runtime-0.5-migration.md)，不以源码扫描或固定测试数量冻结实现。C、Python、WASM 和 OpenBox 等包装层修改还应运行：

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
  --targets crates\tswn_openbox_backend\assets\targets\target1.txt `
  --limit all --target-limit all --count 1000 --threads 0
```

当前 fixed30 与 score 使用公开主 Runtime API 的 `perf_runtime` example：

```powershell
cargo build -p tswn_core --release --features no_debug --example perf_runtime
target\release\examples\perf_runtime.exe -n 13000 --threads 1 --timed
target\release\examples\perf_runtime.exe --mode score -i docs\perf\score\mario.txt -n 13000 --timed
```

输出字段、计时范围与自动线程口径见 [性能追踪](../benchmark-history.md#2-30-case-固定-benchmark)。
0.5.0 的完整发版数据、环境和 A/B 判定见 [历史发版基准](../reports/runtime-0.5.0-749fcd1-release-benchmark.md)。
该版本的 fixed30 与 score 使用临时外部 harness，现行 example 并非那个历史工具，不直接混用结果。

正式结果必须记录被测 commit、rustc/Cargo 版本、feature、输入哈希、线程口径和原始轮次；
机器状态变化时，以同一会话中交替顺序的旧版/新版 A/B 为准。

`--perf` 会打开逐场 init/fight 计时（四次 QPC/场）。不需要 init/fight 拆分时不要加
`--perf`，批量路径默认走不计时的快路径；库调用方要读 timing 请用 `*_timed` 入口。

### 噪声带

同一个二进制在**不同会话之间**可以差到 3%（实测：`5.266/5.303/5.398` vs
`5.103/5.141/5.226`）。因此：

- 必须**同会话交替顺序**跑新旧二进制，至少 5 轮取中位数；
- 小于 **1.5%** 的差异不得作为优化成立的依据，也不要写进长期表格；
- 「先跑三次旧的，再跑三次新的」这种方式量到的 2%~3% 基本都是机器漂移。

## PGO 构建

PGO 不改运行时代码；历史非训练输入曾测得约 25% 的收益，当前提交仍需按同机 A/B 重新验收。CLI 可使用独立流程：

```powershell
python scripts/pgo_build.py --kind cli                   # 插桩 -> 训练 -> merge -> profile-use
python scripts/pgo_build.py --kind cli --train-runs 8000 # 加大训练量
python scripts/pgo_build.py --kind cli --skip-train      # 仅在已有匹配 profile 时复用
```

Openbox 的资源已迁到 `crates/tswn_openbox_backend/assets/`，但脚本的训练靶子仍使用旧
`crates/tswn_openbox/assets/targets/`。这一路径适配前，`--kind openbox` 及同时给 CLI/Openbox
启用 PGO 的 `build_all.py --pgo` 不作为当前完整发布流程；聚合包使用普通 `--release`。
CLI 与 Openbox 的 profile 分别位于 `target/pgo/cli/`、`target/pgo/openbox/`，不要混用。

约束：

- `llvm-profdata` 的 LLVM 大版本必须等于 `rustc -vV` 的 LLVM 大版本，脚本会强制校验；
  没有时用 `rustup component add llvm-tools-preview`。
- 训练强制单线程：LLVM 的 IR 插桩计数器不是原子的，多线程训练会丢计数。
- 训练需运行本机二进制，不能与 `build_all.py --target` 的交叉编译流程同时使用。
- CLI 训练输入默认覆盖 `docs/perf/fixed_cases_30` 全部 30 个 case 加 score 路径；
  只用单一样本训练会削弱泛化性。
- PGO 结果留档必须同时记录 profdata 的生成参数，不同 profile 的结果不可直接比较。

## 采样

Windows 的管理员态主 CPU profiler 是 **AMD uProf CLI**（本机为 Zen 3）。它会把热点函数、
调用栈和源码归因写入 CSV，适合让自动化 agent 直接读取；`samply` 保留为手动查看 Firefox
火焰图 / 时间线的备选工具。完整流程见 [AMD uProf 采样指南](amd-uprof.md)。

先用 Time-Based Profile（TBP）定位热函数。采样工作负载应至少运行约 15 秒，且不要追加
CLI 的 `--perf`（它会改变热路径的计时开销）：

```powershell
$uProf = (Get-Command AMDuProfCLI.exe -ErrorAction Stop).Source
cargo build -p tswn_core --release --features no_debug --bin tswn-cli --config "profile.release.debug=1"
& $uProf collect --config tbp -g `
  -o target\amduprof\before `
  target\release\tswn-cli.exe bench win-rate `
  -f docs\perf\fixed_cases_30\01_1v1-6a2ace7473581042.txt -n 6000000 -s
& $uProf report -i target\amduprof\before --detail -g
```

上述构建为采样显式启用 `debug=1`，Windows 上保留同目录 PDB；根 release profile 默认未启用该设置。正式吞吐 A/B 使用普通 release 构建，分别记录配置。

uProf 5.3 的 `report` 不接受旧版本文档中的 `-o`；它会自动在采样目录创建
`report.csv`。先看其中的 `HOTTEST FUNCTIONS` / `HOTTEST CALLSTACKS`，优化后再以新目录
复跑。吞吐是否真正变好仍以本节上方规定的同会话交替 A/B benchmark 为准，不能以 profiler
带来的 CPU_TIME 直接断言收益。

AMD uProf 的 PMC / IBS 采样需要在已提权的终端（或已提权的 agent 会话）中执行。若没有
管理员权限，改用 PGO instrumentation 拿函数级执行计数：

```powershell
python scripts/pgo_build.py
llvm-profdata show --topn=45 target\pgo\cli\merged.profdata
```

结构体体积用 `cargo +nightly rustc -p tswn_core --release --lib -- -Zprint-type-sizes`。

有管理员权限时，也可用 `samply` 直接采样公开 CLI，用于手动查看时间线：

```powershell
cargo build -p tswn_core --release --features no_debug --bin tswn-cli --config "profile.release.debug=1"
samply record --save-only --unstable-presymbolicate `
  --windows-symbol-server https://msdl.microsoft.com/download/symbols `
  -o target\samply_tswn_cli.json.gz -- `
  target\release\tswn-cli.exe bench win-rate -f input.txt -n 13000 -s
```
