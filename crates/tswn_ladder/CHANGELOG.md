# 更新日志

## [0.2.0] - 2026-10-23

### 研究结论（不改行为）

- **补位 / 模拟退火保持关闭**。把它接进模拟器复核（106 人 / 700 轮，算法与线上同源）：
  目标 `Σ(本轮期望改分)²` 从 1367 降到 264（−81%），但每轮 |Δ名次| 0.64→0.64、
  相邻 Spearman 0.9985→0.9985、对真值准确度 0.973→0.975 全部不动，场次 +21%。
  结论：它优化的是二阶量（每人每轮 handicap 偏置约 3 分 vs 单场正常改分 K/2 = 6 分），
  对天梯质量没有可测影响。旧实现另有四个缺陷（无「比不补位更好才采纳」门槛、初始贪心
  接受负收益配对、`MIN_SQUARED_LOSS_IMPROVEMENT` 只当降温判据、候选筛选过窄）。
  详见 `main.rs` 中 `MAX_MATCHES` 的注释。

### 修复

- 一轮内的分数结算改成「先跑完所有对局，再统一结算」（`battle.rs::settle_round`）。
  旧实现每打完一场就改 `players[].score`，同一轮里后打的场次看到的是已经被前几场改过的
  分数：同一批对局换个顺序会算出不同的分数，并且给后打的场次掺进一个与对手无关的偏置。
  现在所有对局的期望胜率都按轮初分数计算，每人的 K 也在轮初采样一次（不再在轮内漂移）。
  概率取整从「每场一次」变成「每轮一次」，顺带少掉 ~1.5 分的取整噪声。
- 单测 `round_settlement_uses_round_start_scores` 锁定「结算与对局顺序无关」这一不变量。

### 变更（评分）

- 自适应 K：`K_i = BASE_K_FACTOR / (1 + 累计场次_i / 200)`，下限 12。一分钟一轮、每人
  每轮约 9 场的前提下，固定 K=80 的稳态估计误差超过真实实力差，是排名每分钟大幅互穿的
  主因。累计场次保存在 `LADDER_STATE_DIR` 下的小文本快照里（默认 `ladder_matchcount.txt`），
  每 10 轮和收到退出信号时原子写回；快照丢失只影响后续两小时的收敛速度，不影响数据。
- 统一衰减：赛季衰减从 `score * DROPRATE` 改成每人扣掉 `(1 - DROPRATE) * 组内平均分`。
  按比例扣会持续压缩分差（高分端扣得多），既放大了名次抖动，又把「分数尺度」和 K 绑在
  一起；改成扣同一个量后均值仍稳定在 ~11000，分差不再受影响。

### 变更（数据库写入）

- handicap 回写改成「只写与库里值差 ≥ 1.0 的行」。旧实现每轮把活跃玩家的全部两两组合
  （线上约 1.8 万行）无条件 upsert 一遍，是 `handicap` 表膨胀到 24 GB 的直接原因。
- 只读取本轮活跃玩家之间的 handicap 行；离开天梯的玩家留下的行不再参与读取和回写。
- 去掉每轮那条全组 `DELETE FROM handicap WHERE match_count = 0 AND ...`，改成按
  `(number, player1_id, player2_id)` 精确删除没有信号的行。
- 新增每 30 轮一次的维护：删除「对局双方已不活跃」的 handicap 行。
- 新增每 10 轮一次的自维护 `VACUUM (ANALYZE) handicap`：线上实测共享集群的 autovacuum
  会整小时不启动，不能指望它回收死元组；表只有几 MB，毫秒级代价，且不阻塞读写。
- 分数与名次合并成一条 `UPDATE member ... FROM UNNEST(...)`，并跳过没有变化的行
  （旧实现先写分数再写名次，同一批行写两次）。

### 新增

- `LADDER_DRY_RUN`：完整跑一轮但不执行任何 UPDATE/INSERT/DELETE，用于线上干跑验证写入量。
- 换号识别：成员名称变化时该玩家临时恢复满 K（`LADDER_K_BOOST_ROUNDS`，默认 250 轮）。
  站点换号会扣 10% 分数并清 handicap，旧版 K=80 时约 3 小时补回；降到 K=12 之后同样的
  缺口要 20 小时（配对模拟实测 50% 需 552 轮、90% 需 1181 轮）。恢复满 K 后 50%/90%
  回补分别缩到 47/62 轮，与旧版持平且不影响其他玩家的波动。
- 玩家状态快照扩展为 `id 累计场次 名称哈希 K恢复截止轮`，仍兼容旧的 `id 累计场次` 格式
  （缺的字段按 0 处理，不会被误判成换号）。
- 环境变量 `LADDER_STATE_DIR`、`LADDER_K_SHRINK_MATCHES`、`LADDER_K_FLOOR`、
  `LADDER_HANDICAP_PERSIST_QUANTUM`，线上调参不必重新编译。
- `sql/handicap_maintenance.sql`：handicap 表空间回收（`VACUUM FULL`）、去掉三个无用索引、
  设置 `fillfactor = 70`。
- 每组日志追加 `handicap写入=<n> 删除=<n>`，便于观察写入量。

### 验证

- `cargo test -p tswn_ladder`：10 项通过（新增统一衰减的分差不变性、自适应 K 的收缩与
  下限、handicap 回写刻度、场次快照往返）。
- 线上干跑（2026-10-23，`LADDER_DRY_RUN=1`，五组）：对局 1812 场与旧版一致，
  handicap 回写 1810 行（旧版同口径约 18,700 行），history 3624 行不变。
- 新 DML 语句在 `pg_temp` 临时表上逐条验证：第二次相同的 member `UPDATE` 影响 0 行
  （跳过未变化行的判断生效），handicap upsert/精确删除/维护删除均按预期命中主键索引。

## [0.1.1] - 2026-10-13

### 构建

- 线上构建统一 `--release --features "no_debug,mimalloc_alloc"`；部署使用
  `target/release/tswn_ladder`，不要用 debug / release-fast 产物。

### 修复（部署特化）

- seed 生成的时间戳固定使用 `Asia/Shanghai`，不再受部署主机本地时区影响。
- PostgreSQL 连接关闭时自动重连：每轮/每组开始前检查连接，处理异常时若确为连接关闭也重连一次，
  避免长驻服务在数据库连接中断后持续空转失败。

### 内部整理

- 将环境配置、更新编排、匹配、战斗及数据库存储从单一入口文件拆分；共用类型和常量保留在入口，不改数据库语句、随机调用顺序与评分公式。
- 2026-10-13 的 `cargo test -p tswn_ladder` 三项回归通过；使用正常 mimalloc 配置，未连接或修改真实数据库。

### 依赖更新

- `cron` 升级到 `0.17`；`Schedule::from_str` 与 `upcoming` 用法无需调整。

### 验证

- `cargo check -p tswn_ladder --all-targets` 通过（本机以临时关闭 mimalloc 的配置绕过
  `libmimalloc-sys` 的 `D8050` 环境故障）。

## [0.1.0] - 2026-09-30

### 新增

- 新增 `tswn_ladder` 天梯更新器，用于按 cron 周期从 PostgreSQL 读取活跃玩家、生成对局、执行 `tswn_core` 战斗并回写分数、排名、handicap 和历史记录。
- 新增 `DATABASE_URL` 必填环境变量，用于连接 PostgreSQL。
- 新增 `LADDER_CRON_SCHEDULE`、`LADDER_GROUPS`、`LADDER_RUN_ONCE`、`LADDER_RANK_LOG_DIR` 环境变量，分别控制调度周期、分组列表、单轮运行模式和 rank 日志输出目录。
- 新增 handicap 参与的匹配生成与 ELO 更新逻辑，支持扰动、衰减、稀疏持久化和对局质量统计。
- 新增运行时内存监控，超过内部限制时主动退出，方便由外部 supervisor 拉起。
- 新增 rank 日志输出，按分组追加记录每轮排序后的玩家 ID 列表。

### 变更

- 战斗执行迁移至 core 的主 Runtime `Runner`，适配旧执行器与对象模型删除后的接口。

### 说明

- 该 crate 作为内部运维工具加入 workspace，本轮首次封板为 `0.1.0`，不进入默认聚合包。
