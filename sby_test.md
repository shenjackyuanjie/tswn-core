# SBY Test 修复要求与执行计划

这份文档不是单纯的测试说明，而是一份可以直接交给 AI 执行的修复任务文档。

目标是让一个刚进入当前工作区的 AI，在读完本文后，能够立刻明白：

- 当前要修什么
- 以什么作为正确行为基准
- 应该先做什么、后做什么
- 修复过程中哪些事情必须遵守
- 如何判断本轮修改是前进、无效还是退步

如果本文与其他零散说明存在冲突，**就 SBY 大样本测试修复这件事而言，以本文为准**；而在行为基准层面，**以 `md5.js` 的实际逻辑为第一优先级**。

## 当前任务

你当前的任务是修复 `tswn-core`，使其在 `tswn_case_miner` 生成的大样本对比测试中，逐步逼近并最终对齐现有 JS 产物行为。

这里的“修复”不是泛指代码整理，而是非常具体的目标：

- 找出 Rust 实现与现有 JS 产物之间的行为差异
- 基于 failed case 定位分歧点
- 修改 Rust 实现，使输出继续向 JS 侧对齐
- 在理解 `md5.js` 复杂逻辑后，为后续检查补充必要注释

## 执行原则

在开始任何修改前，先接受下面这些前提：

1. 这项工作首先是“对齐现有 JS 行为”，其次才是“解释旧 Dart 代码应该是什么意思”。
2. 如果 Dart、`out_md5.ts`、Rust 当前实现之间存在冲突，优先以 `md5.js` 的实际逻辑与实际运行结果为准。
3. 不要追求一次修改就让全部 case 通过。只要 failed case 变少，或相同 case 的 `first_mismatch_idx` 延后，就说明方向大概率是对的。
4. 不要因为看到一段可疑逻辑就大面积重写。应从 failed case 出发，做小步验证、小步修复。
5. 如果你在分析过程中真正搞懂了 `md5.js` 某段复杂未反混淆逻辑，必须顺手把注释补回去，为下一轮排查降低成本。

## 推荐工作流

如果你是第一次接手这份任务，默认按下面顺序工作：

1. 先通读本文，明确目标、基准、验证方法和注释要求。
2. 运行固定的 SBY 测试，获得当前失败概况。
3. 查看 `summary.json`、`report.md` 和若干 failed case 的 `diff.txt`，挑一个具有代表性的失败方向。
4. 围绕这个 failed case 反推 Rust 与 JS 的分歧位置。
5. 优先参考 `md5.js`，必要时结合 `out_md5.ts` 的外部行为验证，不要优先相信 Dart 源码。
6. 对 Rust 做尽量小且可验证的修改。
7. 先跑 `cargo test -p tswn_core`，再跑固定 SBY 测试。
8. 比较本轮与上轮结果，确认是改进、无变化还是退步。
9. 如果本轮修复依赖了你对 `md5.js` 某复杂片段的新理解，立刻把逻辑注释补回 `md5.js`。
10. 重复以上流程，直到 `diff_failures = 0`。

## 对 AI 的明确要求

如果你是执行这份文档的 AI，请直接按以下要求工作，而不是停留在泛泛分析：

- 默认目标是直接推进修复，不是只输出猜测
- 默认要自己先看结果文件、定位 failed case、再做修改
- 默认应优先缩小问题范围，而不是一次性阅读大段 `md5.js`
- 默认在需要调查 `md5.js` 的具体行为时，优先使用 subagent 进行定向分析
- 默认在识别出可以并行进行的独立任务时，优先使用 subagent 并行执行以提高效率
- 默认每次修改后都要重新验证
- 默认把"failed case 减少"或"mismatch idx 延后"视为有效进展
- 默认把"新失败增多""idx 提前""大量 `mismatch_idx = 0`"视为危险信号
- 默认在确认 `md5.js` 某复杂片段含义后回写注释

只有在缺少必要上下文、并且无法通过现有文件或运行结果补足时，才考虑向用户提问。

## 测试目标

以 TS 侧 `out_md5.ts` 的输出为基准，检查 Rust 侧在相同输入下的行为和输出是否一致。

在这组 SBY 测试相关问题上，**实现与行为判断应优先参考 `fast-namerena/branch/latest/md5.js`**。  
如果 `out_md5.ts`、Dart 源码、Rust 当前实现之间存在理解冲突，原则上先以 `md5.js` 中已经存在的实际逻辑为准，再去判断 Rust 侧应该如何对齐。

当前目标不是只看“有没有变好一点”，而是最终把这组测试推进到：

- `TS failures = 0`
- `Rust failures = 0`
- `diff failures = 0`

在本阶段排查中，`ts_empty_outputs` 先作为已知噪音暂不纳入修复判定，优先持续压低真实行为分歧（`diff_failures`）。

也就是最后达到 **全 pass**。

## 测试输入

- 号库文件：默认使用共享主仓库固定路径 `tests/sqp6000.txt`
- TS 基准工具：默认自动推导 `../fast-namerena/branch/latest/out_md5.ts`
- JS 逻辑优先参考：`../fast-namerena/branch/latest/md5.js`

case 生成方式由 `tswn_case_miner` 负责，覆盖：

- `1v1`
- `2v2`
- `3v3v3`
- `ffa_4`
- `ffa_6`
- `ffa_8`

每种模式生成 `2000` 个 case，总计 `12000` 个 case。

## 标准测试方法

在 `tswn-core` 目录下运行：

注意：这里优先运行 `python ./track_case_miner.py ...`，不要直接跑裸 `cargo run`。
如果需要直接运行普通 CLI 重放 failed case，Cargo 里的二进制名是 `tswn-cli`，不是 `tswn_cli`。
`tswn_cli.rs` 只是源码文件名；`cargo run --bin ...` 时必须写 `tswn-cli`。
否则 agent 很容易踩到 `error: no bin target named 'tswn_cli' in default-run packages`，而 Cargo 实际提示的相近目标是 `tswn-cli`。
如果你确实要直接启动 miner，命令必须写成 `cargo run --bin tswn_case_miner -- ...`，不能省略 `--bin tswn_case_miner`。

例如要直接查看某个 failed case 的原始输出，应写成：

```bash
cargo run -q --release --bin tswn-cli -- --out-raw --file "target/ts_diff_cases/failed/<case-id>/input.txt"
```

```bash
python ./track_case_miner.py -q \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 0 \
  --max-cases-per-mode 2000 \
  --keep-going
```

默认行为：

- 当前 worktree 下的脚本路径与输出路径统一按 `./` 相对路径使用
- 号库默认取共享主仓库中的 `tests/sqp6000.txt`
- `out_md5.ts` 默认自动推导到共享 `fast-namerena/branch/latest/out_md5.ts`
- bun 与 TS trace 缓存默认指向主 worktree 的 `target/tswn_case_miner_cache/`

如果默认推导不适用，再显式传入固定路径，例如：

```bash
python ./track_case_miner.py -q \
  --library D:/shared/tswn-core/tests/sqp6000.txt \
  --md5-tool D:/shared/fast-namerena/branch/latest/out_md5.ts \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 0 \
  --max-cases-per-mode 2000 \
  --keep-going
```

如果在 Windows PowerShell 下运行，可以写成：

```powershell
python .\track_case_miner.py -q `
  --modes 1v1,2v2,3v3v3,ffa `
  --ffa-sizes 4,6,8 `
  --case-offset-per-mode 0 `
  --max-cases-per-mode 2000 `
  --keep-going
```

如果前面一段 case 已经修掉，想直接看后面的稳定顺序区间，可以加 `--case-offset-per-mode`。

它的语义是：**每种模式**都先按固定顺序跳过前 `N` 个唯一 case，再继续取后面的 `--max-cases-per-mode` 个 case。

例如下面这条命令会跳过每种模式前 2000 个 case，直接检查第 2001 到 4000 段：

```bash
python ./track_case_miner.py -q \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 2000 \
  --max-cases-per-mode 2000 \
  --keep-going
```

这不是全局总 offset，而是对 `1v1`、`2v2`、`3v3v3`、`ffa_4/6/8` 分别独立生效。

## 结果查看方法

本轮结果主要看这几个文件：

- `target/ts_diff_cases/summary.json`
- `target/ts_diff_cases/report.md`
- `target/ts_diff_cases/failed/<case-id>/input.txt`
- `target/ts_diff_cases/failed/<case-id>/diff.txt`
- `target/case_miner_regression.json`

其中：

- `summary.json` 看总数和分模式失败数
- `failed/<case-id>/input.txt` 看原始输入
- `failed/<case-id>/diff.txt` 看 TS 和 Rust 的首个差异上下文
- `case_miner_regression.json` 看当前记录是否相对上次有改进或退步

## 已知大样本运行记录（2026-04-22）

### `tests/sqp6000.txt` + `--max-cases-per-mode 4000`

- 命令：

```bash
python ./track_case_miner.py -q \
  --library ./tests/sqp6000.txt \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 0 \
  --max-cases-per-mode 4000 \
  --keep-going
```

- 实际生成量没有达到理论 `24000`，而是 `15792`（6 个模式各 `2632`）
- 结果：
  - `diff_failures = 0`
  - `deduped_diff_failures = 0`
  - `ts_empty_outputs = 1560`

### `tests/sqp5900.txt` + `--max-cases-per-mode 4000`

- 命令：

```bash
python ./track_case_miner.py -q \
  --library ./tests/sqp5900.txt \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 0 \
  --max-cases-per-mode 4000 \
  --keep-going
```

- 这轮实际把 `24000` 跑满（6 个模式各 `4000`）
- 结果：
  - `diff_failures = 3`
  - `deduped_diff_failures = 3`
  - `ts_empty_outputs = 2064`
  - `per_mode_failures = {"3v3v3": 3}`

当前 3 个 failed case：

1. `3v3v3-6c9aabe2aa79d92f`
   - `first_mismatch_idx = 148`
   - input: `target/ts_diff_cases/failed/3v3v3-6c9aabe2aa79d92f/input.txt`
   - diff: `target/ts_diff_cases/failed/3v3v3-6c9aabe2aa79d92f/diff.txt`
   - meta: `target/ts_diff_cases/failed/3v3v3-6c9aabe2aa79d92f/meta.json`
2. `3v3v3-f2506ec63a25c5fa`
   - `first_mismatch_idx = 172`
   - input: `target/ts_diff_cases/failed/3v3v3-f2506ec63a25c5fa/input.txt`
   - diff: `target/ts_diff_cases/failed/3v3v3-f2506ec63a25c5fa/diff.txt`
   - meta: `target/ts_diff_cases/failed/3v3v3-f2506ec63a25c5fa/meta.json`
3. `3v3v3-e74a295d9212e438`
   - `first_mismatch_idx = 146`
   - input: `target/ts_diff_cases/failed/3v3v3-e74a295d9212e438/input.txt`
   - diff: `target/ts_diff_cases/failed/3v3v3-e74a295d9212e438/diff.txt`
   - meta: `target/ts_diff_cases/failed/3v3v3-e74a295d9212e438/meta.json`

## `md5.js` 相关检查原则

由于这组测试本质上是在逼近现有 JS 产物行为，因此分析失败 case 时，关于逻辑正确性的判断应遵循下面顺序：

1. 优先看 `md5.js` 的实际实现与运行结果
2. 再看 `out_md5.ts` 暴露出来的对外行为
3. 最后才参考 Dart 源码中的旧实现

注意事项：

- 不要因为 Dart 源码里“看起来像是某种实现”就直接断定 Rust 应该这样写
- 如果 Dart 版本缺了某段实现，或者描述不完整，应优先去 `md5.js` 中定位对应逻辑
- `md5.js` 文件很长、且包含未完全反混淆的部分，阅读时应尽量围绕具体 failed case、具体调用链、具体状态变量缩小范围，而不是整文件盲读
- 当前系统已安装 bun，可以直接运行 JS 产物来观察行为，必要时可用它辅助验证 `md5.js` 某段逻辑的输入输出

## 研究 `md5.js` 时的注释维护要求

如果在排查某个 failed case 的过程中，已经弄清楚了 `md5.js` 里某段**复杂、难读、未反混淆完全**的逻辑到底在做什么，那么不应只把这部分理解留在本次修复里。

必须同步做一件事：

- 直接修改 `md5.js` 的对应位置，补充简洁但有信息量的**逻辑注释**

这些注释的目标不是美化代码，而是为了下次继续检查时能快速复用本次结论。注释内容应尽量说明：

- 这段逻辑的职责是什么
- 关键输入、关键状态、关键分支分别影响什么
- 它为什么会影响当前这类 case 的结果
- 如果这段代码有明显的“表面写法”和“真实语义”不一致之处，应明确写出来

要求：

- 注释优先服务于“后续对照 Rust / JS 行为差异”的检查
- 注释应尽量贴在具体复杂逻辑附近，避免只在别处留下零散说明
- 不要大规模重写 `md5.js`，但对于已经确认功能的复杂片段，应留下足够帮助下次定位的说明
- 如果只是猜测，先不要写成确定性注释；只有在确认逻辑后再补充

## 尝试修复后的验证方法

每次尝试修复后，按下面顺序验证：

1. 先跑项目原有测试，避免修复过程中把已有行为打坏。

```bash
cargo test -p tswn_core
```

1. 如果改了 Rust 代码格式，再执行：

```bash
cargo +nightly fmt --package tswn_core
```

1. 再跑这组固定的 SBY 测试：

```bash
python ./track_case_miner.py -q \
  --modes 1v1,2v2,3v3v3,ffa \
  --ffa-sizes 4,6,8 \
  --case-offset-per-mode 0 \
  --max-cases-per-mode 2000 \
  --keep-going
```

1. 对比本轮与上轮：
   - `diff failures` 是否下降
   - 相同 failed case 的 `first_mismatch_idx` 是否延后
   - 是否出现新的 failed case
   - 是否出现退步

1. 如果本轮改动有效（`diff failures` 下降或可确认关键 case 修复），应在验证通过后执行 git 提交：
   - 提交前确保工作区只包含本轮相关改动
   - commit message 需要写明“修复了什么”与“为什么这样修”，并且必须使用中文
   - 保持一次 commit 对应一个清晰修复点，便于后续回溯

1. 如果这轮修复依赖了对 `md5.js` 某个复杂片段的新理解，再检查一次：
   - 对应位置是否已经补上逻辑注释
   - 注释是否足够帮助下次继续排查同类问题

## 修复判定标准

如果出现下面任意情况，说明修复是有效的：

- `diff failures` 总数下降
- 某些旧 failed case 消失
- 相同 case 的 `first_mismatch_idx` 变大
- 没有引入新的 Rust 执行失败

如果出现下面情况，需要重点检查：

- `cargo test -p tswn_core` 失败
- `TS failures` 或 `Rust failures` 变成非零
- `diff failures` 总数上升
- 出现大量新的 `mismatch_idx = 0`

## 最终修复目标

最终目标不是“比上次少几个 failed case”，而是这组测试 **全部通过**：

- `total_generated = 12000`
- `ts_failures = 0`
- `rust_failures = 0`
- `diff_failures = 0`

也就是 Rust 在这组固定大样本上与 TS 基准完全一致。
