# 6k 回归验证报告

## 本次变更

### 1. 运行时最小修复
- 文件：`crates/tswn_core/src/player/impl_runtime.rs`
- 变更：在特殊情况下抑制 `combat minion` 的 `"被击倒了"` 死亡日志，仅保留后续 `"消失了"`。
- 目的：对齐 TS 在战斗结束边界上的日志行为，修复此前 `1v1-13b2b47302f8846c` 一类差异。

### 2. tracker 根目录修正
- 文件：`track_case_miner.py:15`
- 变更前：`PROJECT_ROOT = Path("d:/githubs/namer/tswn-core")`
- 变更后：`PROJECT_ROOT = Path(__file__).resolve().parent`
- 目的：让 tracker 在当前 worktree 下运行时，将产物写入当前仓库的 `target/`，而不是写到固定外部目录。

## 验证命令

```bash
python track_case_miner.py -q \
  --library D:/githubs/namer/tswn-core/tests/sqp6000.txt \
  --md5-tool D:/githubs/namer/fast-namerena/branch/latest/out_md5.ts \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --max-cases-per-mode 1000 \
  --keep-going
```

## 产物位置

当前 worktree 下已正常生成：

- `target/case_miner_regression.json`
- `target/case_miner_regression.log`
- `target/ts_diff_cases/summary.json`
- `target/ts_diff_cases/report.md`
- `target/ts_diff_cases/failed/...`
- `target/ts_diff_cases/ts_empty/...`

## 6k 结果摘要

来源：`target/ts_diff_cases/summary.json`

- `total_generated`: 6000
- `unique_inputs`: 6000
- `executed`: 6000
- `ts_failures`: 0
- `rust_failures`: 0
- `ts_empty_outputs`: 586
- `diff_failures`: 8
- `deduped_diff_failures`: 8

按模式分布：

- `1v1`: 1
- `2v2`: 1
- `3v3v3`: 2
- `ffa_4`: 1
- `ffa_8`: 3

## 本次修复确认

此前关注的失败 case：

- `1v1-13b2b47302f8846c`

在本次 6k 结果中**已不再出现**，说明这次最小修复确实消除了该类差异。

不过 6k 全量仍有 8 个剩余 diff case，说明当前修复只解决了其中一类问题，没有把 6k 全清掉。

## 当前剩余 failed cases

来源：`target/case_miner_regression.json`

1. `1v1-a44fb8b4f39ee581`，`idx=4`
2. `2v2-e1ce34d819d7a0ae`，`idx=16`
3. `3v3v3-788b3368abe0fd48`，`idx=180`
4. `3v3v3-1b05f56bd05e2b3d`，`idx=214`
5. `ffa_4-e9bb2ac4d68113bd`，`idx=16`
6. `ffa_8-b89625453be5cc87`，`idx=274`
7. `ffa_8-6e834a086a927560`，`idx=142`
8. `ffa_8-c6aa4534e5baf72a`，`idx=102`

## 新的 1v1 剩余差异样例

来源：`target/ts_diff_cases/failed/1v1-a44fb8b4f39ee581/diff.txt`

首个分叉点在 `mismatch_idx=4`：

```diff
- 4 你是个可敬的对手 #3Kw9ivis7@Shabby_fish发动背刺
+ 4 你是个可敬的对手 #3Kw9ivis7@Shabby_fish发动背刺, THE_SKY #ifC401VhP@Shabby_fish防御

- 6 THE_SKY #ifC401VhP@Shabby_fish的铁壁被打消了, THE_SKY #ifC401VhP@Shabby_fish防御, THE_SKY #ifC401VhP@Shabby_fish受到86点伤害
+ 6 THE_SKY #ifC401VhP@Shabby_fish的铁壁被打消了, THE_SKY #ifC401VhP@Shabby_fish受到3点伤害
```

这说明当前新的 1v1 剩余问题，已经不是之前的 minion 死亡日志问题，而是**背刺 / 防御 / 铁壁结算顺序或伤害值**相关差异。

## 结论

1. `track_case_miner.py` 已改为基于当前 worktree 运行，验证产物会正确写入当前仓库 `target/`。
2. 运行时最小修复成功消除了原先关注的 `1v1-13b2b47302f8846c` 差异。
3. 当前 6k 全量结果仍有 8 个 diff case，说明后续还需要继续逐类定位和修复。
4. 下一步优先级建议从新的 `1v1-a44fb8b4f39ee581` 开始，因为分叉点最早（`idx=4`），复现和定位成本最低。
