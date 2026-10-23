# 天梯评分/匹配离线模拟器

用来在没有数据库的情况下量化「改动一次评分或匹配参数，排名波动和数据库写入量会怎么变」。
纯内存，一分钟能跑上百轮（每轮约 1800 场对局），不需要战斗运行、不连任何数据库。

## 为什么要有它

天梯一分钟跑一轮、每轮约 1800 场，改一次参数如果直接在线上试，要几个小时才能看出效果，
而且会往 24 GB 的表上再写几百万行。这个模拟器忠实地复刻 `crates/tswn_ladder` 的现行实现
（匹配候选与配额、ELO/handicap 更新、持久化策略），只把「真实战斗」换成合成胜负模型：

```text
P(i 胜 j) = 1 / (1 + 10^(-(S_i - S_j) / 50600))      S 为隐真实实力
```

## 标定

`tools/scores.csv` 是线上 `member` 表里 417 名活跃玩家当前分数的快照（2026-10-23），
模拟时直接把它当作「隐真实实力」，于是分数分布、名次间隔和线上完全一致。
在这个标定下，模拟器对现行算法的复现程度：

| 指标 | 线上 rank 日志实测 | 模拟器 |
| --- | --- | --- |
| 1a 每轮 \|Δ名次\| | 1.37 | 1.53 |
| 1a 名次变化率 | 67.1% | 68.6% |
| 1a 相邻轮 Spearman | 0.9978 | 0.9972 |
| 1a 每轮对局数 | 468 | 474 |
| 全站每轮 handicap 回写行数 | ~18,380（由 `n_tup_upd` / 运行分钟数反推） | 18,717 |

（对局数、名次变化率和回写行数误差都在 1.5% 以内，波动指标偏保守 10% 左右。）

## 用法

```bash
python ladder_sim.py --rounds 600            # 跑现行算法，打印各组指标
python sweep.py --rounds 900                 # 跑一组候选方案对比
```

`ladder_sim.py` 里的 `Params` 就是要调的旋钮，与 Rust 侧一一对应：

| Params 字段 | 对应 Rust 常量 / 环境变量 |
| --- | --- |
| `k_base` / `k_shrink_matches` / `k_min_ratio` | `BASE_K_FACTOR` / `LADDER_K_SHRINK_MATCHES` / `LADDER_K_FLOOR` |
| `decay_mode` / `droprate` | `DROPRATE` + `updator.rs` 里的统一衰减 |
| `persist_mode` / `persist_min_delta` | `LADDER_HANDICAP_PERSIST_QUANTUM` |
| `max_active` / `max_passive` / `pre_sa_max` | `MAX_ACTIVE_MATCHES` / `MAX_PASSIVE_MATCHES` / `PRE_SA_MAX_MATCHES` |
| `count_prune_below` | `HANDICAP_MAINTENANCE_EVERY` 的清理阈值 |

## 已知局限

- 真实战斗里的**对位克制**（同一对组合的固有优势）没有建模，因此模拟器无法评估
  handicap 对「对局公平度」的价值，只能评估它对数据库的影响。要验证 handicap 的学习效果，
  需要给 `GroupSim` 加一个反对称的 per-pair 效应项。
- 玩家实力被当作固定值（`drift_sd` 可以打开慢速漂移，但对 lag 1 的名次抖动没有可见影响，
  因为单轮抖动由改分噪声主导）。
- 进榜/退榜造成的整体名次平移没有建模（模拟里人数固定）；线上实测把这项排除后
  （在共同玩家集合内重新编号）与不排除只差 0.01 个名次。
