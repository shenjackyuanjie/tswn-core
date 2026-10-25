# 天梯评分/匹配离线模拟器

用来在没有数据库的情况下量化「改动一次评分或匹配参数，排名波动、数据库写入量、
以及天梯排名与真实环境强弱的吻合度会怎么变」。纯内存，一分钟能跑上百轮
（每轮约 1800 场对局），不需要战斗运行、不连任何数据库。

## 为什么要有它

天梯一分钟跑一轮、每轮约 1800 场，改一次参数如果直接在线上试，要几个小时才能看出效果，
而且会往 24 GB 的表上再写几百万行。这个模拟器忠实复刻 `crates/tswn_ladder` 的现行实现
（匹配候选与配额、ELO/handicap 更新、持久化策略），只把「真实战斗」换成合成胜负模型：

```text
P(i 胜 j) = 1 / (1 + 10^(-(S_i - S_j + 对位优势) / 50600))
```

## 标定

`scores.csv` 是线上 `member` 表里 417 名活跃玩家当前分数的快照（2026-10-23），
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

## 脚本

| 文件 | 用途 |
| --- | --- |
| `ladder_sim.py` | 模拟器本体 + 命令行（`python ladder_sim.py --rounds 600`） |
| `sweep.py` | 参数扫描（K / 衰减 / 持久化 / 配额） |
| `p2_eval.py` | 补位/退火复核：为什么保持关闭（见 `main.rs` 里 `MAX_MATCHES` 注释） |
| `p1_sim.py` | 带「阵容克制」的群体模型 + 补偿强度方案 |
| `p1_axes.py` | 三条旋钮扫描：补偿强度 / 配对半径 / 克制是否进算分 |
| `p1_scale.py` | 组人数 × 配对半径对区分度的影响 |
| `p1_fairness.py` | 补偿强度对「真实对局公平度」的影响 |
| `p1_final.py` | 收敛后的定稿对比（1500 轮） |

`ladder_sim.py` 里的 `Params` 就是要调的旋钮，与 Rust 侧一一对应：

| Params 字段 | 对应 Rust 常量 / 环境变量 |
| --- | --- |
| `k_base` / `k_shrink_matches` / `k_min_ratio` | `BASE_K_FACTOR` / `LADDER_K_SHRINK_MATCHES` / `LADDER_K_FLOOR` |
| `decay_mode` / `droprate` | `DROPRATE` + `updator.rs` 里的统一衰减 |
| `persist_mode` / `persist_min_delta` | `LADDER_HANDICAP_PERSIST_QUANTUM` |
| `max_active` / `max_passive` / `pre_sa_max` | `MAX_ACTIVE_MATCHES` / `MAX_PASSIVE_MATCHES` / `PRE_SA_MAX_MATCHES` |
| `k_handicap` | `K_HANDICAP`（补偿比例 = `K_HANDICAP / 29.8`） |
| `handicap_weight` | 资格门里 handicap 的权重（0 = 不用克制配对） |
| `search_radius` | 候选搜索半径（`matching.rs::build_candidate_list` 里写死的 25/40） |
| `update_with_handicap` | 算分时是否扣掉克制项（对应 `battle.rs` 里用 `exp_h` 还是 `exp_base`） |
| `supplement_slots` / `supp_*` | 补位/退火（关闭，见 `MAX_MATCHES` 注释） |

## 已知局限

- 真实战斗里的**对位克制**用「类型循环罗盘」近似（4 类、固定流行度），不是真实的两两胜率矩阵；
  它能表达"换阵容有用"，但幅度是人为设定的（`ADV`）。
- 玩家实力被当作固定值（`drift_sd` 可以打开慢速漂移，但对 lag 1 的名次抖动没有可见影响，
  因为单轮抖动由改分噪声主导）。
- 进榜/退榜造成的整体名次平移没有建模（模拟里人数固定）；线上实测把这项排除后
  （在共同玩家集合内重新编号）与不排除只差 0.01 个名次。
- 自适应 K 的时间常数约 800 轮，跑不到 1000 轮以上时分数尺度还没收敛，
  绝对数值（尤其 `分数sd`）不能直接和线上比，只能同表内相对比较。
