# 比赛排名模型：训练与检索设计

目标是减少真实对局和榜单试验的组合爆炸；模型不取代比赛站点的最终排名。

## 三个模型

|模型|合并训练来源|一个样本|标签|用途|
|---|---|---|---|---|
|`single`|1A + 1B|一个名字|所属比赛快照的归一化名次 / 两两胜负|筛单名候选|
|`duo`|2A + 2B|两个名字的组合|组合名次 / 两两胜负|筛二人组合|
|`trio`|三人组|三个名字的组合|组合名次 / 两两胜负|筛三人组合|

1A/1B、2A/2B 分别合并训练。每行仍保存 `format`、规则版本和榜单快照 ID，仅用于漂移诊断、验证和必要的赛制条件化，不拆分为独立模型。

名次在每个比赛快照内归一化：`rank_score = 1 - (rank - 1)/(group_size - 1)`。优先保留同快照内两两胜负，按 pairwise/listwise 排名训练；用 NDCG@K、Top-K 命中率和 Spearman 评估。不同规模榜单的原始名次不得直接混合。

## 特征

- 单人：PBB 八围、评分、技能、频次及后续 `tswn-core` 导出的开战前机制特征。
- 双人/三人：保留成员特征，并加入逐列 sum/mean/min/max/range、技能重叠/互补、总属性和及成员差值。
- 阵容输入顺序若影响比赛，保留位置；若不影响，按稳定 candidate_id 排序，并在训练和推断使用同一规范化规则。

## 避免组合爆炸

1. 单人模型从 PBB 池保留分层 Top-N，并保留高不确定性和多样性候选。
2. 双人用 beam search：每个单人种子仅扩展有限互补伙伴，由 `duo` 模型重排。
3. 三人只从高分二人 beam 扩展有限第三成员，再由 `trio` 模型重排。
4. 只把靠前、模型分歧大和随机对照的一小批组合送去实战/比赛；结果回流为下一轮标签。

## 可替换训练集与增量训练

```text
rank_data/
  partitions/{single,duo,trio}/<snapshot_id>.csv
  manifests/{single,duo,trio}.json
  replay/{single,duo,trio}.csv
```

manifest 保存 `active_partitions`、来源组配额、规则版本和替换历史。新增结果只加入对应模型的增量批次，并用小型固定 replay 池保持旧分布。

当某个名字组更换时，停用旧分区、启用替代分区并更新 replay。增量模型不能真正忘记旧样本，因此此时只从**当前有界 active + replay 数据**重建受影响的单个模型；无需整体重训历史数据。

## 每行最小字段

```text
sample_id,model_kind,format,snapshot_id,rules_version,
member_1_id,member_2_id?,member_3_id?,rank,group_size,rank_score,
opponent_id?,pair_outcome?,battle_count,unresolved_count,
feature_schema_version,created_at
```

候选、组合及成员置换必须按组隔离切分；未决局不能写成负例。
