# tswn_ladder

内部天梯更新器：从 PostgreSQL 读取玩家，生成对局并运行 core 战斗，更新 ELO、handicap、排名与历史记录。不进入默认聚合分发包。

## 构建（线上口径）

```bash
cargo build --release -p tswn_ladder --features "no_debug,mimalloc_alloc"
```

线上服务使用 `target/release/tswn_ladder`；不要用 debug 或 release-fast 产物部署。

## 运行

先配置环境变量，再从仓库根目录执行 `cargo run --release -p tswn_ladder --features "no_debug,mimalloc_alloc"`。启动会连接指定数据库并执行更新，不是只读查询工具；调试时使用独立测试数据库。

| 环境变量 | 用途 |
| --- | --- |
| `DATABASE_URL` | 必填，PostgreSQL 连接串；不要写入仓库 |
| `LADDER_CRON_SCHEDULE` | 默认每分钟；接受五字段 cron，解析时补秒字段 |
| `LADDER_GROUPS` | 逗号分隔分组，默认 `1a,1b,2a,2b,3a` |
| `LADDER_RUN_ONCE` | 开启后只执行一轮，否则按 cron 持续运行 |
| `LADDER_RANK_LOG_DIR` | rank 日志目录，默认当前工作目录 |
| `LADDER_STATE_DIR` | 运行状态目录（累计场次快照），默认与 `LADDER_RANK_LOG_DIR` 相同 |
| `LADDER_K_SHRINK_MATCHES` | 自适应 K 的收缩尺度，默认 `200` |
| `LADDER_K_FLOOR` | 自适应 K 的下限，默认 `12`（`BASE_K_FACTOR` 的 15%） |
| `LADDER_HANDICAP_PERSIST_QUANTUM` | handicap 回写刻度，默认 `1.0` |
| `LADDER_K_BOOST_ROUNDS` | 换号后保持满 K 的轮数，默认 `250`；设 `0` 关闭 |
| `LADDER_DRY_RUN` | 只算不写：完整跑一轮但不执行任何 UPDATE/INSERT/DELETE |

线上调参不需要重新编译：改 `/etc/tswn-ladder.env` 后 `systemctl restart tswn-ladder`。

### 干跑验证（不写库）

```bash
cd /myfile/tswn-core
set -a; . /etc/tswn-ladder.env; set +a
LADDER_RUN_ONCE=1 LADDER_DRY_RUN=1 ./target/release/tswn_ladder
```

输出里每个组会多一行 `[dry-run] 跳过写库：member X 行、handicap Y 行、删除 Z 行`，
可以直接和线上日志里的 `success=`/`对局=` 对照，用来确认改动后的写入量，全程不落盘。

## 内部结构

- `main.rs`：进程入口、共用数据类型与评分常量（含 `decay_step` / `adaptive_k` /
  `handicap_needs_persist` 三个纯函数，便于单测）。
- `config.rs`：环境配置、cron 解析与关闭信号。
- `updator.rs`：单轮/周期更新编排、内存监控、自适应 K 与场次快照。
- `matching.rs`：候选匹配、配额与预期分差。
- `battle.rs`：战斗执行、ELO/handicap 更新与错误记录。
- `storage.rs`：PostgreSQL 读取/回写、排名日志、场次快照与 handicap 维护。
- `sql/handicap_maintenance.sql`：handicap 表的一次性空间回收与索引整理（见下节）。

## 评分与匹配策略

### 自适应 K

一分钟一轮、每人每轮约 9 场，一天要打上万场。固定 `K = 80` 时，稳态估计误差比真实
实力差还大，名次每分钟都在互穿。现在改成

```text
K_i = BASE_K_FACTOR / (1 + 累计场次_i / LADDER_K_SHRINK_MATCHES)，不低于 LADDER_K_FLOOR
```

新玩家仍用满 K 快速就位（约 130 轮 ≈ 2 小时收缩到下限），老玩家的单场改分收敛到 `K=12`。
累计场次存在 `LADDER_STATE_DIR` 下的小文本快照里（默认 `ladder_matchcount.txt`），
每 `MATCHCOUNT_SAVE_EVERY` 轮和收到退出信号时原子写回；文件丢了只会让所有人按新玩家
K 重新跑两小时，不影响数据正确性。

> 部署提醒：`LADDER_STATE_DIR` **必须对服务用户可写**。仓库目录 `/myfile/tswn-core`
> 属主是 root、服务用户只有读权限，所以线上用 systemd drop-in 指到自己的状态目录：
>
> ```ini
> # /etc/systemd/system/tswn-ladder.service.d/state-dir.conf
> [Service]
> Environment="LADDER_STATE_DIR=/var/lib/tswn-ladder"
> ReadWritePaths=/var/lib/tswn-ladder
> ```
>
> 目录要 `install -d -o tswn-ladder -g tswn-ladder -m 700 /var/lib/tswn-ladder`。
> 写不进去时日志会提示一次（不会每 10 轮刷屏），天梯照常运行，只是重启后 K 从新玩家重新累计。

### 统一衰减

原来的赛季衰减是 `score * 0.9996`，按分数比例扣，扣得最多的是高分端，等于持续把分差
往中间挤。分差被压缩后，同样大小的改分噪声会造成更大的名次抖动；而且「分数尺度」和 K
绑在一起，降 K 就等于压缩全榜分数。现在改成每人扣掉同一个量：

```text
step = (1 - DROPRATE) * 组内平均分
score = score - step
```

均值同样稳定在 ~11000（衰减量正比于组均值，天然自校正），但分差完全不受影响。

### 换号（成员名称变化）

站点的换号逻辑在 `app/main/team_routes.py`：改 `member.name` 的同时，把该成员在当前组的
**分数打 9 折**（`score > 1000`；否则直接归 0）并**删掉它两个方向上的 handicap 行**。
天梯这边只能看到名称变化，于是把它当作「评分不确定性变大」的信号处理：

- 检测到某成员名称变化 → 该玩家**临时恢复满 K（80）**，持续 `LADDER_K_BOOST_ROUNDS` 轮；
- 期间他的分数按原来的速度回补（模拟：50% 缺口 ~47 分钟、90% ~62 分钟），
  过期后自动回到自适应 K 的低值。

为什么需要这一条：把 K 从 80 降到 12 之后，同样的 10% 扣分要靠 ELO 慢慢磨回来——
模拟显示 50% 要 552 轮（9 小时）、90% 要 1181 轮（20 小时），相当于把换号处罚悄悄加重了
6 倍。恢复满 K 之后回补速度回到原来的量级，而**正常玩家的波动不受影响**（因为对手的
改分用的是对手自己的 K）。

顺带解决了另一个问题：被扣掉的那 10% 分数是真·凭空消失（不是转给别人）。恢复满 K 的玩家
在「分数被低估」状态下会以 80 的步长赢分、而对手只按 12 的步长输分，两者差值就是系统
净生成的分——正好把换号扣掉的那部分补回总分池。长期均值另有锚（见下），所以不会失控。

### 一轮的耗时构成（线上实测，2026-10-24）

日志里有 `[perf:<组>] 读玩家=.. 读克制=.. 匹配=.. 对战=.. 回写=..` 一行，五组合计：

| 阶段 | 时间 | 占比 |
| --- | --- | --- |
| 对战（core Runner） | 274.8 ms | 69% |
| 回写（member / handicap / history / rank 日志） | 100 ms | 25% |
| 读克制（handicap SELECT） | 14.6 ms | 3.6% |
| 匹配（候选生成 + 贪心） | **6.0 ms** | **1.5%** |
| 读玩家（member SELECT） | 5.6 ms | 1.4% |

所以要省 CPU 的话，匹配不是目标：它只占 1.5%，而其中「绕到另一端的探测」真正
必然被资格门拒掉的只有 2.9%（因为 0.45~0.55 换算成分数是 ±4410，很松，
绕过去的探测有 84% 其实能过门），合计约 0.17 ms/轮 = 0.04%。
`matching.rs::build_candidate_list` 里已经把这部分必然无效的探测预筛掉了
（结果与随机数消耗都不变），再往下压只能动场次或回写。

### 匹配机制的现状与有意取舍

配对逻辑本身（`matching.rs`）沿用 Node 版实现，本次未改。几个要知道的点：

- 资格门是**绝对分数差**：`0.45~0.55` 换算过来是 ±4,410 分，第二轮 `0.40~0.60` 是 ±8,912 分。
  候选来自「名次 ±40」的环形邻域，外加约 20% 的全局随机探索。
- **榜尾不在维护范围内**（2026-10-23 确认的有意设计）：胜率极低、分数远离集群的玩家
  （含故意的「送分」账号）会被资格门排除，可能出现整轮 0 场、分数不再变化。实测
  417 名活跃成员里 373 人稳定拿到每轮 9 场，7 人一场没打，且全部落在各组榜尾。
  **不要**为此加「宽窗兜底」或「强制补配」——那会让这些账号重新参与计分。
- 一轮内的结算顺序是固定的：**所有对局都按轮初分数算期望，全部打完后一次性结算**
  （见 `battle.rs::settle_round`）。早期实现每打完一场就改分，导致同一批对局换个顺序
  分数就不同，也会给后打的场次掺进与对手无关的偏置。
- **补位 / 模拟退火（`SUPPLEMENT_RESERVED_SLOTS`）保持关闭**（2026-10-23 复核）。
  它优化的是 `Σ(玩家本轮期望改分)²`，也就是 handicap 诱导的偏置能不能互相抵消。
  模拟复核（106 人 / 700 轮）：目标 loss 降 80%，但每轮 |Δ名次| 0.64→0.64、
  相邻 Spearman 0.9985→0.9985、对真值准确度 0.973→0.973 全部不变，而场次 +21%
  （对战与 history 写入等比增加）。原因是它优化的量是二阶的：每人每轮的 handicap
  偏置约 3 分，而单场正常改分就是 `K/2 = 6` 分，且各对偏置符号随机、跨轮不累积。
  完整数据与旧实现的实现缺陷见 `main.rs` 里 `MAX_MATCHES` 的注释。**除非 handicap
  量级发生数量级变化，不要重新启用。**

### handicap 只写变化

旧实现每轮把活跃玩家的全部两两组合（线上约 1.8 万行）无条件
`INSERT ... ON CONFLICT DO UPDATE` 一遍，一分钟一次。表本身只有几百到几千行有效数据，
24 GB 里绝大多数是这种反复重写留下的死元组，每轮还要额外跑一条全组 `DELETE` 兜底。

现在：

- 只回写「与库里值差 ≥ `LADDER_HANDICAP_PERSIST_QUANTUM`」的行（内存里保留精确值，
  匹配精度不受影响）；
- 只读取「本轮活跃玩家之间」的行，离开天梯的玩家留下的行不再参与回写；
- 没有信号的行按 `(number, player1_id, player2_id)` 精确删除，不再每轮全组扫一遍；
- 每 `HANDICAP_MAINTENANCE_EVERY` 轮清理一次「对局双方已不活跃」的行，避免表随历史
  玩家数量一直膨胀；
- 每 `HANDICAP_VACUUM_EVERY` 轮自己跑一次 `VACUUM (ANALYZE) handicap`。线上实测共享集群的
  autovacuum 会整小时不启动（`last_autovacuum` 停在 6 小时前，期间死元组涨到 4.5 万，
  索引和堆一起膨胀），不能指望它按时干活；表只有几 MB，自己收一次是毫秒级，
  而且 `VACUUM` 只取 SHARE UPDATE EXCLUSIVE，不挡天梯自己的读写；
- 分数和名次合并成一条 `UPDATE`（旧实现分成两条，同一批行写两次），并跳过没有变化的行。

## 数据库维护

代码改完之后，**先要跑一次空间回收**，否则旧的死元组还占着磁盘：

```bash
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f crates/tswn_ladder/sql/handicap_maintenance.sql
```

脚本内容与注意事项见该文件头部注释。核心是 `VACUUM FULL`（或等价的表重建）+
去掉三个没用的索引 + `fillfactor=70` 让后续更新走 HOT。线上实测（2026-10-23）：
`handicap` 24 GB / 2.85M 页里只有约 1.9 万行有效数据，回收后约 2–3 MB。

## 验证

```powershell
cargo test -p tswn_ladder
cargo +nightly fmt -p tswn_ladder --check
```

单元测试覆盖 cron、core 胜者映射、统一衰减的分差不变性、自适应 K 的收缩与下限、
handicap 回写刻度判断、场次快照往返。`sql/handicap_maintenance.sql` 之外的数据库行为
（完整更新、事务回写、信号处理）仍需在测试数据库或干跑模式中单独验收，不能把单元测试
当作数据库端到端验证。
