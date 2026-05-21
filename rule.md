# tswn-core 项目性能优化准则

## 核心准则

> 所有性能优化的前提都是建立在不出现行为异常的前提下

## Release profile 选择

- `--release`：正式 benchmark 口径，使用 `lto = "fat"` 和 `codegen-units = 16`；benchmark 不启用 `mimalloc_alloc`，最终 release 构建再启用。
- `--profile release-fast`：日常快速验证口径，使用 `lto = "thin"` 和更多 codegen units，编译更快；性能结果只能作本地参考，不建议写入长期性能表。

## 行为验证方法

- 最低要求
  - `cargo test` 全量通过

- 基本要求
- ```bash
cargo run -p tswn_core --release --features no_debug --bin tswn_case_miner -- \
    --library 'D:\githubs\namer\tswn-core\tests\sqp6000.txt' \
    --md5-tool 'D:\githubs\namer\fast-namerena\branch\latest\out_md5.ts' \
    --out-dir '.\target\ts_diff_cases' \
    --modes '1v1,2v2,3v3v3,ffa' \
    --ffa-sizes '4,6,8' \
    --case-offset-per-mode 0 \
    --max-cases-per-mode 1000
  ```

- 高级要求
- ```bash
cargo run -p tswn_core --release --features no_debug --bin tswn_case_miner -- \
    --library 'D:\githubs\namer\tswn-core\tests\sqp6000.txt' \
    --md5-tool 'D:\githubs\namer\fast-namerena\branch\latest\out_md5.ts' \
    --out-dir '.\target\ts_diff_cases' \
    --modes '1v1,2v2,3v3v3,ffa' \
    --ffa-sizes '4,6,8' \
    --case-offset-per-mode 0 \
    --max-cases-per-mode 4000
  ```

- 完整要求
- ```bash
cargo run -p tswn_core --release --features no_debug --bin tswn_case_miner -- \
    --library 'D:\githubs\namer\tswn-core\tests\sqp6000.txt' \
    --md5-tool 'D:\githubs\namer\fast-namerena\branch\latest\out_md5.ts' \
    --out-dir '.\target\ts_diff_cases' \
    --modes '1v1,2v2,3v3v3,ffa' \
    --ffa-sizes '4,6,8' \
    --case-offset-per-mode 0 \
    --max-cases-per-mode 4000
  cargo run -p tswn_core --release --features no_debug --bin tswn_case_miner -- \
    --library 'D:\githubs\namer\tswn-core\tests\sqp5900.txt' \
    --md5-tool 'D:\githubs\namer\fast-namerena\branch\latest\out_md5.ts' \
    --out-dir '.\target\ts_diff_cases' \
    --modes '1v1,2v2,3v3v3,ffa' \
    --ffa-sizes '4,6,8' \
    --case-offset-per-mode 0 \
    --max-cases-per-mode 4000
  ```

## benchmark 方法

```powershell
cargo run -p tswn_core --release --features aux_bins,no_debug --bin track_perf_cases -- `
  --case-dir docs/perf/fixed_cases_30 `
  --out-dir docs/perf/fixed_cases_30_results `
  --bench-runs 13000 `
  --thread 1
```

也可以用 `release-fast` 做日常快速试跑（不要作为正式留档数据）：

```powershell
cargo run -p tswn_core --profile release-fast --features aux_bins,no_debug --bin track_perf_cases -- `
  --case-dir docs/perf/fixed_cases_30 `
  --out-dir target/perf_cases_fast `
  --bench-runs 13000 `
  --thread 1
```

在Windows上如果有 samply，可以参考以下命令进行采样
```powershell
samply record --save-only --unstable-presymbolicate `
  --windows-symbol-server https://msdl.microsoft.com/download/symbols `
  -o target\samply_track_perf_cases_symbols.json.gz `
  -- target\release\track_perf_cases.exe `
  --case-dir docs/perf/fixed_cases_30 `
  --out-dir target\samply_perf_cases_symbols `
  --bench-runs 13000 --thread 1 -q
```
