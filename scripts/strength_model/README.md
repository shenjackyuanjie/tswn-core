# 1v1 静态强度模型（CPU）

训练目标是由真实 `tswn_core` Runtime 得到的、固定对手分布下的候选平均胜率（0～1）；此目录不重写战斗模拟器。

特征 CSV 每行一个 `candidate_id`，仅可含开战前可得的数值。初始路径可使用 PBB 导出的八围、十族评分、16 个技能 ID 和频次（`pbb_feature_schema.json`）；后续可再并入 `tswn_core` 的本体/召唤物蓝图特征。schema 的 `feature_columns` 是严格白名单；禁止 ID、名字、seed、结果或运行态字段。

标签 CSV 应含 `candidate_id,target_win_rate` 及上述全部特征列。候选只来自 PBB 已命中的结果，不把未过 PBB 初筛的随机字符串混入训练集。标签由逐候选、逐对手、双输入位置、无重复 seed 的真实战绩按预先固定的对手层权重聚合，截断局单列记录而不静默算负例。

```bash
cd /rdxfs/user/lulab/xiongyk/projects/name_arena/PBB-linux/PBB/source
bash tools/build_feature_export.sh
build/pbb_feature_export < ../nightly/results/out/pbb_hits.txt > data/pbb_features.csv

cd /rdxfs/user/lulab/xiongyk/projects/name_arena/tswn-core
python -m pip install -r scripts/strength_model/requirements.txt
taskset -c 0-55 python scripts/strength_model/train_strength.py --data data/training.csv --schema scripts/strength_model/pbb_feature_schema.json --output-dir models/strength_v1 --threads 56
python scripts/strength_model/score_strength.py --features data/new_candidates.csv --model-dir models/strength_v1 --output results/candidates_scored.csv
```

`0-55` 为本机 56 个物理核心；不使用超线程同胞 `56-111`。训练不得与夜间 PBB 同时运行。训练器按 `candidate_id` 稳定哈希作 80/10/10 候选隔离切分；若 Rust 导出器已完成机制家族切分，可写 `split` 并加 `--use-input-split`。

输出包括 CatBoost 模型、固定特征顺序/线程数/随机种子的元数据，以及独立 test 预测。模型误差不是筛选成果；必须再用未见候选、独立对手池、全新 seed 对 Top 候选与随机、净化/幻术规则基线作同预算复测。

## 每日增量路径

每个新增 1T 扫描先以 `pbb_feature_export` 导出当天命中快照，再运行
`analyze_pbb_hits.py` 生成无标签的 PBB 分布报告和候选优先队列。它不是胜率训练。

当抽样候选经 `tswn-core` 产生真实 `target_win_rate` 后，首轮用
`train_strength.py` 训练一个基座模型；之后每日使用 `incremental_train.py`。该脚本只
读取当天新标签分区和一个固定小型历史 replay 集，以旧模型为 `init_model` 微调；它会将
当天 CSV 归档为不可覆盖的 `daily_YYYY-MM-DD.csv`，并在注册表中拒绝重复 candidate_id，
因此无需整体重训历史数据。

```bash
python scripts/strength_model/analyze_pbb_hits.py \
  --input /rdxfs/user/lulab/xiongyk/projects/name_arena/out/analysis/fAIgFUL_base62_9_all_hits.features.csv \
  --output-dir /rdxfs/user/lulab/xiongyk/projects/name_arena/out/analysis

python scripts/strength_model/incremental_train.py \
  --new-data data/labeled_YYYY-MM-DD.csv --day YYYY-MM-DD \
  --schema scripts/strength_model/pbb_feature_schema.json \
  --base-model models/strength_v1/strength_model.cbm \
  --replay-data data/replay_fixed.csv \
  --dataset-dir data/strength_daily --output-dir models/strength_YYYY-MM-DD \
  --threads 40
```
