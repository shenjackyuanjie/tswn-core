# 更新日志

## [0.2.0] - 2026-09-30

> 跟进 2026-09-30 的 Data_Structure4.0，提供 Rust API 与统一 CLI

- Openbox 新增 DS4 页面，直接调用 Rust API；提供彩色分区、配置 tooltip 与可固定流程帮助；新增 `run_with_progress` 阶段回调，共用后端提取为 `tswn_openbox_backend`。
- 提取公开 Rust API，统一 CLI 复用 library，为 tswn 体系中的宿主集成提供配置、执行与报告接口。
- 公开 `Config::from_json`、`run`、`run_with_progress`、`RunStage` 与 `FullRunReport`；错误通过
  `Ds4Error` 返回而不退出宿主。阶段回调在执行线程同步调用，目前不支持全流程取消，同一工作目录不能并发运行。
- 队伍名按前缀接收并截断尾缀；新增二人和三人流程 ABCP5 历史缓存，重复运行保留历史预测结果。
- 新增默认关闭的 Openbox 三轮实战筛选及独立 `openbox-cqp` 命令，直接调用
  `tswn_openbox_backend` 和工作目录内的带权预设，无需 Openbox GUI 或外部 CLI 程序。
- 实战筛选按 100／1000／10000 局推进，阈值依次为 `three.pair_abcp_sieve / 100 + 1`、
  `+2`、`+2`（整数除法）；末轮追加到 `file/real_two.txt`，成功归档后清空 ABCP5 中间输入／结果，失败保留现场。
- ABCP5 调用支持降线程重试与失败诊断；新增增量、失败保留、Rust API 与 Openbox library 回归测试。
- ABCP5 目录支持 `TSWN_DS4_ABCP_DIR` 覆盖；外部程序、模型和运行库不随 crate 分发，
  线程重试依次降为请求值的一半、四分之一和 1，并保留 `*.failure.txt` 诊断。

## [0.1.0] - 2026-09-30

> 适配前的首个正式版，代码对应 `7a51f125`，跟进 20260824 版本 ds4preview

- crate 从 `tswn_ds3` 更名为 `tswn_ds4`，支持 DS4 `config.json` 与 `team_name` 输入筛选。
- 新增 QP、QD、PP、PD、CQD 五类 SP1 评分与技能筛选。
- 二人配对遍历左右候选全集；新增 ABCP5 分类、八类三人评分及历史结果增量归档。
- 合并、筛选、评分与 ABCP5 中间输出改为流式或分块写出，二人和三人结果按有界块输出。
- 增加 DS4 C++ 对拍样例及脚本；保留旧配置格式供原有局部流程使用。
