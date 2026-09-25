# 胜率数据集 HP-only 基线（100k 局）

> 采样时间：2026-09-25 ｜ 机器：Windows 10 / Ryzen 7 5800X ｜ 生成器：`tswn-pwp`（release）
> 数据集：2v2v2、10 万局、80 万样本

这是 [battle-analyze.md](../../design/battle-analyze.md) 要求的「第一个 baseline」：**不训练任何参数**，
只用血量比例给各输入队伍打分，用来确认数据集与标签可用，并给后续模型定出必须超过的下界。
评分维度、张量契约与容量档位见 [FeatureEncoder 规格](../../design/feature-encoder-spec.md)。

## 数据集与复现命令

```powershell
# 生成（Windows 上 sccache 偶发超时，故显式关闭 wrapper）
cargo run -q --release --config 'build.rustc-wrapper=""' -p tswn_pwp --bin tswn-pwp -- `
  generate --names tests/sqp5900.txt --team-sizes 2,2,2 `
  --matchups 500 --games-per-matchup 200 --battles-per-shard 1000 --seed bench --out target/winprob-100k

# 容量溢出与标量校准（都只读数据集）
target/release/tswn-pwp.exe stats --out target/winprob-100k --json-out target/100k-stats.json
target/release/tswn-pwp.exe calibrate --out target/winprob-100k --json-out target/encoder-calibration.json

# HP-only 基线
python scripts/winprob_hp_baseline.py --dataset target/winprob-100k --json-out target/hp-baseline.json
```

名字池为 `tests/sqp5900.txt`（3684 行，本地未跟踪数据）。10 万局全部以 `Winner` 终止，
截断 0；切分为 train 80800 / validation 9800 / test 9400 局，对应样本 646400 / 78400 / 75200，
空标签样本 0。

## 容量溢出（profile `baseline-64`，上界计费）

| 维度 | 上限 | 峰值 | 超限样本 | 占比 |
| --- | --- | --- | --- | --- |
| `e` | 64 | 30 | 0 | 0% |
| `t` | 32 | 3 | 0 | 0% |
| `r` | 32 | 3 | 0 | 0% |
| `h` | 512 | 120 | 0 | 0% |
| `l` | 4096 | 882 | 0 | 0% |
| `s` | 64 | 10 | 0 | 0% |
| `q` | 512 | 96 | 0 | 0% |
| `v` | 32768 | 2800 | 0 | 0% |
| `x` | 65536 | 1737 | 0 | 0% |

九维全部零超限，`e`／`h`／`l`／`q`／`V` 峰值与规格第 4 节记录的 100k 池实测一致。

## 标量校准

`tswn-pwp calibrate --split train` 选中 **646400 / 800000** 行（其余是 validation/test 与空标签），
输出 **104 个字段**的 `count`／`min`／`max`／`p50`／`p99`／`s_f`／`c_f`，并记录来源身份
（`input_sha256`、`executable_sha256`、选中行 `selected_rows_digest`、`calibrator`），
可作为 `encoder-manifest.json` 的数值来源。

## HP-only 基线结果

打分口径：每支输入队伍 `sum(存活成员 hp) / sum(该队全部成员 max_hp)`（分母含已阵亡成员），
再对有效队伍做 softmax。三分类随机基线的 Log Loss 为 `ln 3 ≈ 1.0986`。

| split | 样本 | Log Loss | Brier | Top-1 |
| --- | --- | --- | --- | --- |
| train | 646400 | 1.0509 | 0.6343 | 0.4644 |
| validation | 78400 | 1.0495 | 0.6333 | 0.4651 |
| test | 75200 | 1.0554 | 0.6373 | 0.4586 |

按 `progress` 分桶（test）：

| 进度桶 | 样本 | Log Loss | Brier | Top-1 |
| --- | --- | --- | --- | --- |
| 0-20% | 22554 | 1.1003 | 0.6678 | 0.3399 |
| 20-40% | 13119 | 1.0868 | 0.6593 | 0.3658 |
| 40-60% | 13150 | 1.0556 | 0.6384 | 0.4498 |
| 60-80% | 13164 | 1.0209 | 0.6138 | 0.5362 |
| 80-100% | 13213 | 0.9818 | 0.5858 | 0.6851 |

## 结论

1. 血量比例基线明显优于随机，但**幅度有限**：test Log Loss 1.0554 vs 随机 1.0986，
   Top-1 0.4586 vs 随机 0.3333。后续模型需要在这个下界之上有实质提升。
2. 分桶行为符合预期：越接近终局越确定（0-20% 桶 Top-1 0.340 → 80-100% 桶 0.685，
   Log Loss 1.1003 → 0.9818）。前三桶接近随机，说明早期局面确实缺少可辨识信号，
   这也与抽样按最终轮数分层有关。
3. **只报 accuracy 会低估难度**：整体 Top-1 只有 0.46，但远端的 80-100% 桶已到 0.685；
   评估必须按进度分桶与概率指标（Log Loss／Brier／ECE）一起看。
4. 一个已经修正的口径错误：第一版把分母写成「存活成员的 max_hp 之和」，导致死掉一半人时
   比例几乎不降，基线退化成随机（test Log Loss 1.0804、80-100% 桶 Top-1 0.556）；
   分母含阵亡成员后才有上表结果。

## 已核对的差异（原「待核对」）

`tswn-pwp stats` 的 `x` 峰值（**上界**计费）为 1737，而容量脚本按**真实记录数**曾报 2271——
方向应是「上界 ≥ 实测」，两者矛盾。核对结论：**容量脚本的 clone 计数有 bug**。

- Parquet 读回的 struct **子字段不继承父级 null**：`clone_build.score_skill_boost_plan` 全为 `None` 时，
  它的 `initially_boosted_mask`／`slot_boosts` 仍被判为 valid（实测父级 valid=0、孙字段 valid=全部行），
  于是脚本对**每个模板**都记 5 条计划叶子，X 被高估约 `5 × h`。
- 修正后同一数据集的 `X_required` 峰值为 **1671**（原 2271），`x_clone` 全 0；
  Rust 上界 1737 ≥ 1671，方向正确，差值来自「每槽一条实体 ref（上界计费）」与
  「按实际 ref 槽计数（实测）」。
- 顺带确认：`clone_build` 在**所有**模板上都存在（它是名字派生属性的构造参数，`attrs` 全非零），
  真正的可空项只有 `score_skill_boost_plan`；因此不存在 clone_build presence 丢失的问题。
- 规格第 4 节的 8 人池两张表已按修正脚本重测（统一 `--matchups 200 --games-per-matchup 25`，每模式 5000 局）：
  X 峰值由 3335 降为 **2465**（`3v1v1v1v1v1`），`e`／`h`／`l`／`q`／`V` 峰值与旧表基本一致；
  深测表（2 万局以上的大数据集）本轮未重测，X 列仍是修正前数值（按 `5×h` 估算约低 900）。

「零超限」结论在两种口径下都成立（都远低于 65536），不受影响。
