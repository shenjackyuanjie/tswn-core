# tswn-pwp 数据集生成器

本 crate 是 **tswn-pwp（player winchance predictor）** 的数据工具，负责把 Rust Runtime 的机制状态采样为可校验的 Parquet 数据集，并调用 `tswn_core::encoder` 离线编码张量；模型训练和线上推理不在本 crate 内。

当前模块边界：

- `generate` / `validate` / `storage`：数据集生成、提交与完整性校验
- `sampling` / `random` / `input`：确定性抽样、阵容输入与切分
- `stats` / `bench` / `calibrate`：数据分布、生成规模统计与逐字段标量校准
- `manifest`：从 train 校准报告构造并验证完整 EncoderManifest
- `encode`：编排流式张量导出；`encode/source.rs` 管理源锁、摘要与选样计数，`encode/output.rs` 管理预算、私有暂存与安全提交
- [Python 只读 loader](../../scripts/read_encoded_batches.py) 与 [HP-only 基线](../../scripts/winprob_hp_baseline.py) 已在脚本侧提供；模型训练、推理及在线编码绑定仍未实现

相关文档入口：

- [仓库文档中心](../../docs/README.md)
- [设计索引](../../docs/design/README.md)
- [BattleModelState 审计](../../docs/design/battle-model-state-audit.md)
- [FeatureEncoder 规格](../../docs/design/feature-encoder-spec.md)
- [规模基线](../../docs/perf/reports/winprob-dataset-scale-baseline.md)

生成器直接运行 Rust 战斗引擎，按初始状态和可见帧末采样，输出嵌套 Parquet。
它与 `BattleSession` 共用推进和终止策略，不构造回放 DTO、图标或玩家展示快照。

## 使用

从仓库根目录运行：

```powershell
cargo run -p tswn_pwp --bin tswn-pwp -- generate --input crates/tswn_pwp/examples/matchup.txt --games-per-matchup 20 --seed demo --out target/winprob-demo
cargo run -p tswn_pwp --bin tswn-pwp -- validate --out target/winprob-demo
```

`--input` 也接受目录，按路径排序递归读取 `.txt` 文件。没有空行时每行一队（FFA），含空行时按空行分队，至少两支非空队伍；输入中的 seed 行由生成器派生的 seed 替换。完全相同的有序阵容只生成一次；顺序变体保留，但进入同一个切分集合。

从名字池抽取阵容：

```powershell
cargo run -p tswn_pwp --bin tswn-pwp -- generate --names crates/tswn_pwp/examples/names.txt --team-sizes 2,2,2 --matchups 10 --games-per-matchup 20 --seed demo --out target/winprob-names
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
cargo build -p tswn_pwp --bin tswn-pwp --release
target/release/tswn-pwp.exe stats --out target/winprob-demo --json-out target/stats.json
```

`bench` 在本进程内运行 `generate`/`validate` 并采样自身 RSS、线程数与 CPU 时间，再读产物
Parquet 元数据，输出吞吐、存储、行组与列块压缩比、嵌套字段占比：

```powershell
target/release/tswn-pwp.exe bench --out target/winprob-demo `
  --names crates/tswn_pwp/examples/names.txt --team-sizes 2,2,2 `
  --matchups 10 --games-per-matchup 20 --seed demo
```

`stats` 只读已提交的数据分片；`bench` 会在 `--out` 指定目录内运行 `generate`/`validate`，**会写入数据**。
首次生成可以使用新目录或已有空目录；已有 `manifest.json` 时需要 `--resume`，且输入、配置和可执行文件摘要必须匹配，否则应换目录。非空但没有 manifest 的目录不能通过 `--resume` 接续。
`stats` 还会按冻结的 `baseline-64` 容量档位报告 `e/t/r/h/l/s/q/v/x` 九维的上限、峰值与超限样本占比
（上界计费，每槽按 3 条 X 记录）；超限样本会被 encoder 判 `CapacityExceeded`。容量权威随 encoder 落在
`tswn_core::encoder::capacity`，本 crate 的 [capacity.rs](src/capacity.rs) 只是再导出；推导与依据见
[FeatureEncoder 规格](../../docs/design/feature-encoder-spec.md) 第 4 节。
10k/100k 规模的实际结果见
[winprob 数据集生成规模基线](../../docs/perf/reports/winprob-dataset-scale-baseline.md)。

## 标量校准

`calibrate` 只读已提交分片，默认只用 `train` 切分且标签非空的行，采集逐字段标量分布并拟合
`encoder-manifest` 的归一化常数（`s_f = max(1, Q50(|x|))`、`c_f = max(1, Q99(|x|))`）：

```powershell
target/release/tswn-pwp.exe calibrate --out target/winprob-100k --json-out target/encoder-calibration.json
```

输出 JSON 记录每个字段的样本数、min/max/p50/p99 与 `s_f`/`c_f`。覆盖范围是
[FeatureEncoder 规格](../../docs/design/feature-encoder-spec.md) 第 5 节"逐字段标量"：全局机制计数、
实体 runtime 与模板标量、lane 等级与 boost、状态 priority 与各 payload 数值、
`core.entity.minion_counter` 计数、`clone_build` 的属性/修正/名称因子叶子。分类、引用、bit、
精确注册序和其余槽值不参与统计。缺字段时不写默认值，由 encoder 返回 `MissingCalibration`；
`--keep-unlabeled` 可保留空标签行，`--print-fields` 打印逐字段摘要；`--split` 可选择其他集合做分布检查。
用于训练的归一化常数仍应只从 `train` 拟合，Python 训练侧只加载结果，不重新拟合。

## 离线张量编码

`calibrate` 输出的是校准报告，**不是** `encode --manifest` 要求的完整 EncoderManifest。
使用 `build-manifest` 将有标签 `train` 报告组装为 `baseline-64` 完整契约，填写契约摘要并通过
核心 `FeatureEncoder::new` 门禁后才写入文件。缺少任何必需字段时明确报 `MissingCalibration`，
应补足训练数据中的机制观测并重新校准，不使用人工默认常数。组装、词表和编码仍只在核心实现。

从已生成的数据集开始，先校验，再校准、发布契约和编码：

```powershell
target/release/tswn-pwp.exe validate --out target/winprob-demo
target/release/tswn-pwp.exe calibrate --out target/winprob-demo --json-out target/encoder-calibration.json
target/release/tswn-pwp.exe build-manifest --calibration target/encoder-calibration.json --out target/encoder-manifest.json
target/release/tswn-pwp.exe encode --dataset target/winprob-demo --manifest target/encoder-manifest.json --out target/winprob-encoded --split train --batch-size 8
python scripts/read_encoded_batches.py target/winprob-encoded
```

- 输出为版本 2 训练包；根目录保存完整 `encoder-manifest.json`，`manifest.json` 记录切分、标签策略、样本数、批次数、空间预算、契约摘要和源数据摘要。旧版基础导出包需要重新编码。
- 每个 `batch-NNNNNN/` 含各张量的 `.bin` 文件及 `batch-manifest.json`；张量为小端、C 连续布局，shape 首维为实际批次大小，尾批可以小于 `--batch-size`。
- 批清单分为 `features`、`exact`、`labels`、`audit` 四个通道；`extra_bits` 与 `order_key` 属精确旁路，不默认作为数值模型输入。
- 标签是小端 i32 `[B]` 的 `winner_team_index` 和 u8 `[B]` 的 `label_mask`。默认过滤空标签；诊断导出可用 `--keep-unlabeled`，此时空标签为 -1/mask=0，不能给予监督权重。
- `rows.json` 数组与批内槽位一一对应，记录源分片/原始行号、battle_id、阵容、帧边界及 progress。源行号在过滤前计数；标签无需另读 Parquet 拼接，审计列不进入 `features`。
- 编码先只扫描 split/标签列计数，再流式读取状态；待编码状态最多一批，可跨源分片连续填充。除当前批与编码缓冲外，还保留固定的 Parquet 读取批（128 行）及分片/批清单摘要，不再收集整库状态。
- 每个特征、旁路、标签、审计文件与 encoder manifest 副本都记录字节数和 SHA-256；写入同步后回读验证。根清单记录每批清单的摘要，Python 消费端检查整条文件引用链，能发现同长度内容损坏。

### 空间预算与安全提交

导出前核对源分片完成记录与两张 Parquet 的摘要，并持有数据集共享锁，阻止同一数据集并发生成或续跑；
该过程只读源文件。源身份由原始 manifest 文件摘要和按序分片完成记录组成，写入 `dataset_sha256`，
不同数据内容、encoder 契约、切分或标签策略应导出到新的目录，不自动复用旧缓存。
共享锁使用生成器的 `.generator.lock`；拷贝已完成的数据集时也保留此文件。

`--out` 必须是**不存在的新目录**，已有目录（包括空目录）或旧编码包都不覆盖。
程序在同级 `.输出目录名.encode.tmp/` 中写入，全部成功后改名提交。失败时目标目录不出现，
临时包保留供诊断；再次运行不覆盖旧临时包，可换输出目录，或人工检查现场后清理。编码目前不支持 `--resume`。

预算按完整固定容量的 69 个张量和两个标签文件计算，在写入前打印并检查可用空间。
`baseline-64` 约 4.2 MiB/样本；该预算**不包含 JSON、文件系统分配及其他进程后续写入**，不能视为绝对磁盘保证。
张量仍为未压缩小端 `.bin`，流式读取只降低内存，不降低整库磁盘占用；首次验证请先用小数据集。

安装 NumPy 后，训练侧可使用 `scripts/read_encoded_batches.py` 的 `iter_encoded_batches(root)`：
逐批返回只读内存映射的 `features` 和 `labels`，无需 PyArrow；需要关系计算旁路时显式传入
`include_exact=True`。该读取器只加载已编码数组，不重写 bit、引用或归一化逻辑。
示例名字池不保证观测到所有罕见机制，因此可能不足以发布完整契约；生成器能保存的 Boss 专属载荷
也不等于当前 encoder 已支持这些载荷，详见契约的 `support` 声明。

## Python 读取

安装 `pyarrow` 后：

```powershell
python scripts/read_winprob_dataset.py target/winprob-demo
```

该示例默认排除空标签，只读取 `state` 和 `winner_team_index` 两列。不要将 seed、原名字、阵容哈希、切分标记或最终 `progress` 混入模型输入。张量化、ID 引用处理、浮点 bit 转换和归一化由 `tswn_core::encoder` 实现；Python 训练侧应加载同一完整契约和编码产物。

Rust 固定数组和元组在当前 Parquet 中映射成字段名为 `0`、`1` 等的 struct；Python 读取后按这些编号还原顺序。可变长序列使用 list，无载荷枚举使用普通字符串。

完整设计见 [战斗分析设计](../../docs/design/battle-analyze.md)，字段来源与排除理由见 [Runtime 状态审计](../../docs/design/battle-model-state-audit.md)。
