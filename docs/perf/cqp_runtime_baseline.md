# CQP/CQD legacy Runtime 基线与 Runtime 优化结果

> 日期：2026-07-14
> 状态：六个验收档位均达到“相对 legacy Runtime 至少快 30%”目标
> 历史口径：本文的 legacy 对账与 `track_case_miner.py` 命令已随
> `tswn_core 0.5.0` 旧执行器删除；现行发版门禁见根目录 `rule.md`。

## 1. 口径

- 机器：AMD Ryzen 7 5800X，8 核 16 逻辑处理器，Windows x86_64-pc-windows-msvc；
- Rust：`rustc 1.99.0-nightly (f10db292a 2026-07-07)`；
- 构建：workspace release profile、`no_debug`、`mutable-noalias=yes`；
- 自动线程：legacy Runtime 基线为 20 worker；Runtime 短档为 24 worker，中长档为 32 worker；
- 胜率口径：`keep_rq=true`，1%/10%/100% 分别执行 100/1000/10000 场；
- 计时：采用 `openbox_mem_probe` 在业务调用外层记录的整批墙钟，不使用 CLI 并行路径中各 matchup 耗时之和；RSS 查询发生在墙钟取值之后；
- 基线：提交 `c763e7b` 的 OpenBox cqd/cqp，即 legacy Runtime `PreparedRunner` 路径；
- 优化结果：本轮 OpenBox cqd/cqp 与 CLI `bench cqp` 共用的 Runtime 矩阵执行器。

输入固定如下：

- 单人：`tests/sqp6000.txt` 前 20 行，保存为 `docs/perf/cqp/sqp6000_first20.txt`；对 `target1.txt` 的 35 组靶子，共 700 个 matchup；
- 双人：`cqp_double_target.txt` 的 32 组输入；对 `target2.txt` 的 41 组靶子，共 1312 个 matchup；
- 将换行统一为 LF 后计算 SHA-256：单人输入 `87054C01EC6D456EC83C6A419F9147D373C21F60C2428BEE51627652426A24F6`，双人输入 `EF42B91EFD02EC33EB8029329C95775A571CE1AA285D54B0A3A4D8D218D8020D`；
- 同口径靶子 SHA-256：`target1.txt` 为 `227C69B2C8C5680B594D26525A1D4CC6383C2615B48AC410F679B06921B7210B`，`target2.txt` 为 `A285A267AE4438BC254FE0511F9FDD1C39A458532CF872984FFB1C035E81569D`。

## 2. 验收结果

| 输入 | 精度 | legacy Runtime 基线 | 快 30% 目标线 | Runtime | 实际提升 | legacy/runtime 取样 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 单人 20 × target1 | 1% | 0.345417 s | ≤ 0.241792 s | 0.173947 s | 49.64% | 7 / 5 |
| 单人 20 × target1 | 10% | 3.145753 s | ≤ 2.202027 s | 1.531991 s | 51.30% | 5 / 3 |
| 单人 20 × target1 | 100% | 25.381207 s | ≤ 17.766845 s | 15.031414 s | 40.78% | 3 / 3 |
| 双人 32 × target2 | 1% | 1.287154 s | ≤ 0.901008 s | 0.714121 s | 44.52% | 5 / 5 |
| 双人 32 × target2 | 10% | 11.965849 s | ≤ 8.376094 s | 6.773550 s | 43.39% | 3 / 3 |
| 双人 32 × target2 | 100% | 111.456257 s | ≤ 78.019380 s | 66.997727 s | 39.89% | 1 / 3 |

除双人 legacy Runtime 100% 基线因单轮已超过 111 秒而只保留一轮外，其余表格值均为同一进程配置多次独立启动后的中位数。最小提升为双人 100% 的 39.89%，仍高于目标 9.89 个百分点。

## 3. 生效的优化

1. 把调度粒度从“一个选手包办全部靶子”改成独立的 `player × target` matchup，由一组常驻 worker 动态领取，消除复杂名字造成的尾部空洞，也不再为每个 matchup 重建内部线程组。
2. 自动模式按任务长度分档：100 场及以下使用逻辑核的 1.5 倍，中长任务使用 2 倍；显式 `-t/--thread` 不变，WASM 固定单线程。
3. 固定 roster 的 Runtime runner 只恢复战斗会修改的热字段，并复用 seed、world view、输入分组等小向量容量；每轮更换 profile 身份的 score 路径仍完整复位，不能误用该快路径。
4. 原生构建默认启用仓库已有的 mimalloc。隔离 A/B 中，单人矩阵 1%/10%/100% 从 0.286295/2.299596/22.502590 秒降至 0.193885/1.589951/15.450302 秒；WASM 显式关闭该原生 allocator feature，并继续保留 `png_render`。
5. CLI 与 OpenBox 都调用 `runtime_cqp_matchups`，不再维护两套不同的 cqd/cqp 调度策略。

## 4. 正确性门禁

- CLI 自动矩阵与 `-s` 串行路径对单人 20 × target1、每 matchup 100 场的 20 条结果逐字段比较：`label`、平均胜率、汇总胜率、`wins`、`total`、有效/跳过 matchup 数全部一致，差异数为 0；
- OpenBox 新增集成回归，直接把 Runtime 矩阵的平均胜率、输入顺序和进度终点与 legacy Runtime 小样本结果对照；
- `cargo test -p tswn_core` 通过：核心库 573 通过、2 忽略，CLI 59、runtime trace 3、engine 集成 29 均通过；
- 按 `sby_test.md` 执行 1v1、2v2、3v3v3、FFA 4/6/8 各 2000 case，共 12000 case：`ts_failures=0`、`rust_failures=0`、`diff_failures=0`、失败 case 数为 0。

SBY 命令：

```powershell
python .\track_case_miner.py -q `
  --modes 1v1,2v2,3v3v3,ffa `
  --ffa-sizes 4,6,8 `
  --case-offset-per-mode 0 `
  --max-cases-per-mode 2000 `
  --keep-going
```

## 5. 复测命令

```powershell
cargo build -p tswn_openbox --release --bin openbox_mem_probe

# 单人；把 count 依次改为 100、1000、10000
.\target\release\openbox_mem_probe.exe `
  --players .\docs\perf\cqp\sqp6000_first20.txt `
  --targets .\crates\tswn_openbox\assets\targets\target1.txt `
  --limit all --target-limit all --count 100 --threads 0

# 双人；把 count 依次改为 100、1000、10000
.\target\release\openbox_mem_probe.exe `
  --players .\cqp_double_target.txt `
  --targets .\crates\tswn_openbox\assets\targets\target2.txt `
  --limit all --target-limit all --count 100 --threads 0
```
