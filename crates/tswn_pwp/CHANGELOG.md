# 更新日志

## [Unreleased] - 0.0.2

### 新增

- 新增 `build-manifest` 命令，将有标签 train 校准报告组装成 `baseline-64` 完整编码契约；
  校验全部必需数值槽与词表，填写契约摘要后发布，不以默认常数补缺字段。
- 离线导出升级为版本 2 训练包，附带完整 encoder manifest，分离 features、精确旁路、labels
  与逐行审计关联；默认过滤空标签，`--keep-unlabeled` 以 -1/零 mask 保留诊断样本。
- Python 读取器验证批布局、标签与源行关联数量，并提供逐批只读 NumPy 内存映射入口。

### 规模与完整性

- `encode` 改为投影扫描标签计数、按批流式编码，不再把选定切分的全部状态攒进内存；
  分片边界不打断批次，尾批仍使用实际样本数。
- 源数据持有共享锁并验证已提交分片摘要，训练包记录源 manifest 与整库内容身份。
- 输出要求不存在的新目录；使用同级临时目录写入、同步与回读校验，成功后改名提交，
  失败保留私有临时现场，不覆盖已有输出或旧临时包，不自动续跑。
- 为张量、标签、审计、encoder manifest 与批清单建立 SHA-256 引用链，读取端能发现同长度损坏。
- 写入前按实际选中行数估算完整张量和标签字节数并检查可用空间；不含 JSON 与文件系统开销，
  不改变未压缩张量容器或容量档位。

### 依赖更新

- `arrow-array`、`arrow-schema`、`parquet` 升级到 `60.0`，`sha2` 升级到 `0.11`；
  `serde_arrow` 的 Arrow 特性同步改为 `arrow-60`。
- `encode` 与 `calibrate` 的摘要文本改用 `random::hex` 编码，不再依赖 `sha2` 摘要类型的
  `LowerHex` 实现；小写十六进制输出与旧版一致，Parquet 与 manifest 契约不受影响。

### 验证

- 本次使用正常 ZSTD 配置复跑 PWP 测试；通过任务内 TEMP/TMP 避开本机 MSVC 临时目录问题，
  没有关闭压缩或修改系统配置。

- 本机 MSVC 工具链编译 `zstd-sys` 持续报 `D8050`（既有环境故障），故在临时关闭 `zstd`
  特性的配置下执行 `cargo check -p tswn_pwp --all-targets` 并通过；压缩相关的 5 个用例
  因该配置缺少 zstd 而以 `Disabled feature at compile time: zstd` 失败，需在 MSVC 正常的
  环境复跑。

## [0.0.1] - 2026-09-30

### 版本定位

- 首次封板版本调整为 `0.0.1`；原 `0.1.0` 仅为未发布的开发版本，不保留为正式发布记录。

### 依赖更新

- 更新 `clap` 与 `serde_arrow` 至现有兼容范围内的最新版本。

### 新增

- 新增 `encode` 子命令：读取 Parquet 状态与 `EncoderManifest`，按批输出各 tensor 的小端 `.bin`、
  `batch-manifest.json` 和总 manifest；新增 `scripts/read_encoded_batches.py` 只读校验器，检查
  manifest 摘要、shape、dtype、字节长度和尾批实际大小。
- `encode` 支持 `--split` 与 `--batch-size`，导出前校验 manifest 契约摘要与 state schema；
  每批记录源行身份摘要，尾批按实际样本数分配，不使用满批 padding 代替实际 B。
- 新增可复现的胜率数据集生成器，直接复用 `BattleModelSession` 导出初始状态和可见帧末状态，并将胜者输入队伍索引作为监督标签。
- 支持从对局文件或名字池生成阵容，按规范化阵容哈希稳定划分 train/validation/test，队伍顺序变体不会跨集合。
- 输出嵌套 Parquet 分片及 manifest、summary、complete/failure 记录；使用临时目录、文件锁、摘要回读和双遍状态核对保证提交完整性。
- 支持 `--resume` 安全续跑，并验证输入、配置、可执行文件摘要和已提交分片。
- 新增 `validate`、`stats` 和 `bench` 子命令，覆盖引用完整性、标签、抽样、分布、吞吐、RSS、压缩比和行组规模审计。
- 新增 `calibrate` 子命令：默认只读 `train` 切分且标签非空的行，采集逐字段标量分布并拟合 `s_f`/`c_f`，作为 `encoder-manifest` 的数值来源；支持 `--split`、`--keep-unlabeled`、`--json-out` 与 `--print-fields`，缺字段不写默认值，由 encoder 报 `MissingCalibration`。
- 校准报告记录 state schema／数据格式版本、输入与生成器可执行文件摘要、选中行序列摘要、
  采集器身份及切分／标签策略，供 `EncoderManifest::from_calibration` 保留校准来源证据。
- 扩充标量校准覆盖状态 payload、槽内计数、槽内蓝图模板与 `clone_build` 叶子；分类、引用、bit
  与精确注册序不参与拟合，非有限值留给编码阶段拒绝。
- 新增冻结的 `baseline-64` 容量档位（`E/T/R/H/L/S/Q/V/X = 64/32/32/512/4096/64/512/32768/65536`）；`stats` 按该档位报告九维上限、峰值与超限样本占比，上界计费（每槽 3 条 X 记录）。

### 重构

- 容量权威（`BASELINE_64`、`CapacityMeasure` 与九维计费式）随 encoder 迁到 `tswn_core::encoder::capacity`；本 crate 的 `capacity` 模块改为再导出，`stats` 等调用方不变。容量常量与计费规则只保留唯一实现，不在 core/pwp/WASM 各复制一份。

### 修复

- 容量计费改用饱和算术，极端计数不再因加乘溢出而回绕；校准中的 `u64` 计数直接转为浮点，
  避免先转 `i64` 导致大值变为负数，并补齐被漏采集的模板字段。

### 性能

- 依赖 `tswn_core` 时启用 `no_debug` 与 `mimalloc_alloc`，与 `tswn_wasm`、`tswn_capi` 等调用方保持一致；逐分片 `complete.json` 摘要与关闭该特性的构建完全一致。
- 同一批 1000 局 2v2v2 的 8 线程生成由 14.7 s 降到 2.0 s，5000 局由 88 s 降到 11.7 s。主要来源是 `tswn_core` 不再逐 tick 读环境变量（见该 crate 更新日志），以及本 crate 的下述改动。
- `active-battle.json` 改为只在失败或 panic 时落盘。此前每局写一次并 `sync_all`，1000 局就是 1000 次卷级刷盘；多 worker 并行时这些刷盘互相排队。
- 样本行攒到 256 行再转一次 `RecordBatch`，摊薄约 450 列嵌套结构的固定转换开销。
- `validate_dataset` 按分片并行；`generate` 覆盖全部分片时直接合并各 worker 已回读校验的分片摘要，不再让整库 Parquet 重解一遍。续跑仍有完整校验。
- 分片数少于可用逻辑 CPU 时打印提示，说明并行度受 `--battles-per-shard` 限制。
- 按目标行数切分 Parquet 行组，降低大分片的峰值内存并保留稳定分片边界。
- 记录 10k/100k 规模生成和校验基线，供后续编码器与训练管线比较。

### 说明

- 该 crate 以 `0.0.1` 封板，仍为实验性数据工具，不进入默认 `scripts/build_all.py` 聚合包。
- FeatureEncoder 的八族 69 个张量编码已在 `tswn_core` 落地；本 crate 当前提供离线编码包导出，
  尚不包含模型训练或线上推理。`encode` 先收集所选切分的全部状态，再按批编码，不能将
  `--batch-size` 视为读取阶段的内存上限；跨绑定 parity 与真正逐行流式读取仍在后续验收阶段。
