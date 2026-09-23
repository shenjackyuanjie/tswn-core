# tswn_ds4

`tswn_ds4` 跟进 `Data_Structure4.0` 的 20260824 `ds4preview` 流程。工作区默认读取 `config.json`，按 `team_name` 筛选输入，依次执行去重、SP2/SP1 单人评分、二人配对、ABCP5 预测、八类三人评分和结果归档。

## 运行

从仓库根目录执行：

```powershell
cargo run -p tswn_ds4 -- run --root D:\path\to\workspace
```

工作目录至少包含 `config.json` 和 `input/`。配置字段示例见 [`tests/fixtures/ds4_preview/config.json`](tests/fixtures/ds4_preview/config.json)。启用 `abcp` 或 `get_3` 时，还需在工作目录的 `abcp5/` 放置 `abcp5.exe`、`model4.onnx`、`scale.txt`；也可用 `TSWN_DS4_ABCP_DIR` 指向这三个文件所在目录。Rust crate 不分发这些外部文件。`score_now.txt` 可放在工作目录覆盖内置的 SP1 技能基线。

仍可读取旧的 `config.toml` 和 `config.txt`，用于原有的局部命令和旧流程测试。配置自动查找顺序为 `config.json`、`config.toml`、`config.txt`。

局部命令包括 `score-bc/fz/wc/fs/pj`、`pair-fc/wc/rh`、`merge`、`dedup`、`sort` 和 `show-config`，具体参数见 `--help`。

## 输出

- `tmp/` 保存本轮输入、单人结果及 ABCP5 分类后的二人结果；每次 `run` 前重建。
- `out/` 保存二人配对、可选 SP1 结果和被忽略的输入。
- `3ren/` 保存本轮八类三人结果。
- `file/` 保存历史单人和二人分类结果；`new/` 保存可选的本轮归档。
- `abcp5/result.txt` 与 `abcp5/result_without_score.txt` 保存最终预测结果。

合并、team 筛选、SP1/SP2 评分和 ABCP5 中间文件按行或分块写出。二人及三人评分按有界候选块写出；排序仍需在内存中保存待排序记录，去重仍需保存名字索引。

## 验证

```powershell
cargo test -p tswn_ds4
powershell -ExecutionPolicy Bypass -File .\crates\tswn_ds4\scripts\compare_with_cpp.ps1 -FailOnDiff
```

对拍脚本默认使用相邻的 `..\Data_Structure4.0` C++ 包和 `ds4_preview` 样例，在 `target/compare-ds4/` 创建独立工作目录，并报告每个文本输出的记录差异。可通过 `-CppRoot` 和 `-Fixture` 指定其他包与样例。
