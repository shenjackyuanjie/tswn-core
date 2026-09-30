# tswn_ds4

`tswn_ds4` 提供 Rust library 与一个统一 CLI，跟进 `Data_Structure4.0` 的 2026-09-30 流程。工作区默认读取 `config.json`，按 `team_name` 筛选输入，依次执行去重、SP2/SP1 单人评分、二人配对、ABCP5 预测、八类三人评分和结果归档。

## Rust API

其他 workspace crate 可通过路径依赖 `tswn_ds4`，直接调用配置与执行接口：

```rust,no_run
use std::path::Path;
use tswn_ds4::{Config, Ds4Result};

fn process(root: &Path) -> Ds4Result<()> {
    let config = Config::load_from_root(root)?;
    let report = tswn_ds4::run(root, &config)?;
    println!("新增输入：{}", report.stage1.dedup.remaining);
    Ok(())
}
```

`Config::from_json` 可校验界面编辑后的 JSON；`run` 返回 `FullRunReport`，出错时返回 `Ds4Error`，不会退出宿主进程。`run_with_progress` 通过 `RunStage` 回调报告阶段，`screen_openbox_pairs` 可独立运行实战筛选。调用是同步的，GUI 应放到后台任务中执行；同一个工作目录不能并发运行。当前执行入口尚未提供全流程取消。

实战筛选直接调用共用的 `tswn_openbox_backend` library，不需要 `openbox-cli.exe`。Openbox GUI 的 DS4 页面直接调用本 crate；GUI 与 DS4 共用后端，避免循环依赖。原有 `tswn_openbox::backend` 和 `tswn_openbox::presets` 接口继续保留。

## 运行

从仓库根目录执行：

```powershell
cargo run -p tswn_ds4 -- run --root D:\path\to\workspace
```

工作目录至少包含 `config.json` 和 `input/`。可从 [`config.example.json`](config.example.json) 复制配置，并填入 `team_name`。启用 `abcp` 或 `get_3` 时，还需在工作目录的 `abcp5/` 放置上游 ABCP5 程序、`model4.onnx`、`scale.txt` 及其运行库；也可用 `TSWN_DS4_ABCP_DIR` 指向该目录。Rust crate 不分发这些外部文件。`score_now.txt` 可放在工作目录覆盖内置的 SP1 技能基线。

最后一个 `@` 后以 `team_name` 开头的输入会被接收，队伍名后的评分或其他尾缀会被截断，然后参与去重。

仍可读取旧的 `config.toml` 和 `config.txt`，用于原有的局部命令和旧流程测试。配置自动查找顺序为 `config.json`、`config.toml`、`config.txt`。

局部命令包括 `score-bc/fz/wc/fs/pj`、`pair-fc/wc/rh`、`merge`、`dedup`、`sort`、`show-config` 和 `openbox-cqp`，具体参数见 `--help`；`--version` 显示版本。

## Openbox 实战筛选

`openbox_cqp` 默认是 `0`；设为 `1` 后，主流程对 ABCP5 输出执行 100、1000、10000 局三轮筛选，阈值分别是 `three.pair_abcp_sieve / 100 + 1`、`+2`、`+2`（整数除法）。前两轮只输出组合，第三轮追加带分数结果到 `file/real_two.txt`。成功归档后清空 ABCP5 的 `input.txt`、`result.txt` 和 `result_without_score.txt`；失败保留数据。

筛选复用工作目录 `setting/settings.toml` 中 id 为 `2` 的预设，包括权重与 DIY 选项；缺失配置时由 Openbox 释放内置预设。也可独立调用：

```powershell
tswn_ds4 openbox-cqp --root D:\path\to\workspace
```

## 输出

- `tmp/` 保存本轮输入、单人结果及 ABCP5 分类后的二人结果；每次 `run` 前重建。
- `out/` 保存二人配对、可选 SP1 结果和被忽略的输入。
- `3ren/` 保存本轮八类三人结果。
- `file/` 保存历史单人和二人分类结果；`new/` 保存可选的本轮归档。
- `abcp5/result.txt` 与 `abcp5/result_without_score.txt` 保存最终预测结果。
- `file/two_old.txt` 保存已预测的二人组合，`file/FC_old.txt`、`WC_old.txt`、`RH_old.txt` 保存三人流程各来源的预测缓存；未达到筛选阈值的组合也会记入缓存。
- `tmp/two_new.txt` 仅包含本轮未预测过的组合；最终 ABCP5 结果合并历史并按分数降序排列，重复运行不会清空历史结果。

ABCP5 失败时依次降低为请求线程数的一半、四分之一和 1 线程重试，保留输入和对应的 `*.failure.txt` 诊断。调整模型或筛选阈值后如需重新评估历史组合，应使用新工作目录重新处理。

合并、team 筛选、SP1/SP2 评分和 ABCP5 中间文件按行或分块写出。二人及三人评分按有界候选块写出；排序仍需在内存中保存待排序记录，去重仍需保存名字索引。

## 验证

```powershell
cargo test -p tswn_ds4
powershell -ExecutionPolicy Bypass -File .\crates\tswn_ds4\scripts\compare_with_cpp.ps1 -FailOnDiff
```

对拍脚本默认使用相邻的 `..\Data_Structure4.0` C++ 包和 `ds4_preview` 样例，在 `target/compare-ds4/` 创建独立工作目录，连续运行两轮并分别报告每个文本输出的记录差异。可通过 `-CppRoot`、`-Fixture` 和 `-Rounds` 指定其他包、样例与轮数。
