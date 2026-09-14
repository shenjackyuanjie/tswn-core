# 战斗状态数据生成器

生成器直接运行 Rust 战斗引擎，按初始状态和可见帧末采样，输出嵌套 Parquet。
它与 `BattleSession` 共用推进和终止策略，不构造回放 DTO、图标或玩家展示快照。

## 使用

从仓库根目录运行：

```powershell
cargo run -p tswn_winprob_dataset --bin tswn-winprob-dataset -- generate --input crates/tswn_winprob_dataset/examples/matchup.txt --games-per-matchup 20 --seed demo --out target/winprob-demo
cargo run -p tswn_winprob_dataset --bin tswn-winprob-dataset -- validate --out target/winprob-demo
```

`--input` 也接受目录，按路径排序递归读取 `.txt` 文件。没有空行时每行一队（FFA），含空行时按空行分队，至少两支非空队伍；输入中的 seed 行由生成器派生的 seed 替换。完全相同的有序阵容只生成一次；顺序变体保留，但进入同一个切分集合。

从名字池抽取阵容：

```powershell
cargo run -p tswn_winprob_dataset --bin tswn-winprob-dataset -- generate --names crates/tswn_winprob_dataset/examples/names.txt --team-sizes 2,2,2 --matchups 10 --games-per-matchup 20 --seed demo --out target/winprob-names
```

名字池每行一个角色，可以包含引擎支持的武器、Boss 和 DIY 输入。按引擎规则处理行末空白；空行和 seed 行不参与抽样。相同条目去重，单局不重复抽取；不同阵容抽取任务可能得到相同阵容，这些对局仍共享切分组。

常用选项：

| 选项 | 默认值与含义 |
| --- | --- |
| `--samples-per-game` | 8，包含初始状态的最大样本数 |
| `--battles-per-shard` | 1000，以对局编号范围固定分片 |
| `--threads` | 0，按可用 CPU 数和待生成分片数确定 |
| `--max-rounds` | 20000，沿用会话终止上限 |
| `--eval-rq` | 沿用核心默认值，实际值写入 manifest |
| `--resume` | 校验输入、配置和可执行文件摘要后跳过已完成分片 |

并行度取 `--threads`（默认可用逻辑 CPU 数）与待生成分片数的较小值，因此小规模数据集要靠减小 `--battles-per-shard` 才能吃满 CPU；分片数成为瓶颈时生成器会打印提示。分片边界只由对局编号范围决定，不随机器或线程数变化。

续跑时保持原命令参数，只增加 `--resume`；可以调整 `--threads`。重新编译导致可执行文件摘要变化时，需要使用新的输出目录。校验已有数据不要求使用原可执行文件。

## 抽样和标签

每局使用相同阵容、seed 跑两遍。第一遍记录可见帧边界及结果；按最终推进轮数划分 `K-1` 个等宽区间，每个非空区间随机选一帧，再从未选帧中均匀补足空区间名额。第二遍只导出选中的状态，并核对全部帧边界和最终结果。

初始状态单独保留；已产生胜者的终局不进入样本。可用帧不足时样本数可以小于 K。`max_rounds` 或 `no_progress` 截断保留样本，胜者列为空。初始化即结束的对局只有对局记录。单局样本数上限设得很大时，会增加每个 worker 缓存选中状态的内存。

胜者编号使用**输入队伍索引**。`state.entities[].runtime.team` 是运行时队伍编号，不能直接作为标签索引；魅惑等临时阵营信息还存在状态载荷中。

## 输出与恢复

```text
manifest.json
summary.json
shard-000000/
  battles.parquet
  samples.parquet
  complete.json
```

- `manifest.json` 保存配置、原输入、派生阵容、阵容哈希和可执行文件摘要。
- `battles.parquet` 保存 seed、终止原因、轮数、唯一胜者和样本计数。
- `samples.parquet` 的 `state` 是模型输入；标签是 `winner_team_index`。其余列是审计信息。
- `complete.json` 保存对局范围、行数和两个 Parquet 文件的 SHA-256。
- 分片先写 `.shard-NNNNNN.tmp`，关闭、回读验证后改名提交。未提交分片在续跑时重做；已提交但损坏的分片报错，保留现场。
- 输出目录使用进程文件锁，避免并发任务写入同一目录。操作系统在进程退出后释放锁。

Runtime 错误、panic、导出失败和两遍不一致都会停止生成。已完成分片保留，`failure.json` 记录原因，临时分片的 `active-battle.json` 包含复现阵容、seed 和参数。部分低层自定义配置可以通过导入但在技能执行时报错，例如没有召唤模板的 bed2；这类失败不会变成未决样本。

`validate` 回读所有行，校验文件摘要、对局顺序、标签、状态引用、抽样计数和切分。训练/验证/测试按规范化阵容哈希分为 80/10/10，同阵容的所有 seed 与队伍顺序变体不跨集合；小数据集不保证恰好达到此比例。

## 分布统计与基准

`stats` 只读取已提交分片，汇总对局轮数/帧数、胜者与终止原因、样本进度桶、实体与状态条目数量，
以及技能 ID、载荷 kind、模板 kind、Boss 与阵营分组频次：

```powershell
target/release/tswn-winprob-dataset.exe stats --out target/winprob-demo --json-out target/stats.json
```

`bench` 在本进程内运行 `generate`/`validate` 并采样自身 RSS、线程数与 CPU 时间，再读产物
Parquet 元数据，输出吞吐、存储、行组与列块压缩比、嵌套字段占比：

```powershell
target/release/tswn-winprob-dataset.exe bench --out target/winprob-demo `
  --names crates/tswn_winprob_dataset/examples/names.txt --team-sizes 2,2,2 `
  --matchups 10 --games-per-matchup 20 --seed demo
```

`stats` 只读已提交的数据分片；`bench` 会在 `--out` 指定目录内运行 `generate`/`validate`，**会写入数据**。
首次生成可以使用新目录或已有空目录；已有 `manifest.json` 时需要 `--resume`，且输入、配置和可执行文件摘要必须匹配，否则应换目录。非空但没有 manifest 的目录不能通过 `--resume` 接续。
10k/100k 规模的实际结果见
[winprob 数据集生成规模基线](../../docs/perf/reports/winprob-dataset-scale-baseline.md)。

## Python 读取

安装 `pyarrow` 后：

```powershell
python scripts/read_winprob_dataset.py target/winprob-demo
```

该示例默认排除空标签，只读取 `state` 和 `winner_team_index` 两列。不要将 seed、原名字、阵容哈希、切分标记或最终 `progress` 混入模型输入。状态仍是机制数据，张量化、ID 引用处理、浮点 bit 转换和归一化属于后续训练工作。

Rust 固定数组和元组在当前 Parquet 中映射成字段名为 `0`、`1` 等的 struct；Python 读取后按这些编号还原顺序。可变长序列使用 list，无载荷枚举使用普通字符串。

完整设计见 [战斗分析设计](../../docs/design/battle-analyze.md)，字段来源与排除理由见 [Runtime 状态审计](../../docs/design/battle-model-state-audit.md)。
