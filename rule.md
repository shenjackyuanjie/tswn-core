# namerena工作区指南

现在用户正在进行一个dart项目的重写

## 源码说明

dart 原始项目位于 namer-src
请注意，这里的源码并不完整，并且是老版本的，不要试图运行它
所以如果用户指出"某个地方的实现不正确"不要急着说 dart 的实现就是这样的
要问用户是在哪里看到的实现，是dart还是js产物

dart 项目 有一个 dart compile js 产物
位于 fast-namerena
请注意，这个项目里面的js文件都非常长(2w行左右)，因此谨慎阅读
如果你确实需要读js的话，你会需要读的是 fast-namerena/branch/latest/md5.js
再次提醒，这个文件非常长，2w2行左右

重写结果是 rust 项目
位于 tswn-core
使用了 rust edition 2024 (如果你的知识库告诉你 rust 最新是 2021, 那就说明你的知识库太老了)
项目中遵循尽可能减少依赖、性能优先、扩展性优先的设计原则

## 重写规则

在重写过程中，实现方案优先参考 dart 版本，但是如果 dart 版本缺少了你所需要部分的代码
可以向用户提问，js产物中这里的逻辑实现是怎么样的/在哪里，尽可能减少盲目读取 js 源码的情况

**现在应当优先参考js实现**

目前系统上装了 bunjs，可以直接运行 js 产物中的 js 文件来观察它的行为

## 测试说明

这些测试都是为了 tswn-core 的实现细节是否与 js 产物一致
修改过程中很难通过一次修改就让某个测试从在中间失败变成完全通过
但是只要测试失败的 idx 增加，那就说明修改是正确的

### 测试回归追踪工具

项目提供了一个辅助测试回归的工具，可以追踪每次修改后测试失败点的变化，并支持存档点功能。该工具主要用于比较每次运行时测试失败的分叉点 idx，从而判断修改是否朝着与 JS/Dart 产物一致的方向前进。

**工具位置：** `tswn-core/track_test.py`

**基本用法：**

```bash
# 进入项目目录
cd tswn-core

# 运行测试并比较（默认追踪多个 large 测试分组与少量特殊测试）
python track_test.py

# 安静模式（-q），只输出关键结论，适合AI使用
python track_test.py -q

# 只显示当前失败状态，不运行测试
python track_test.py -s

# 重置历史记录
python track_test.py -r
```

注意：脚本的默认过滤器已更新，默认会运行并追踪以下测试关键词组合（可作为 cargo test 的 filter 参数）：

- `large`（所有 large 测试）
- `large_full`（相当于 fight_large / 完整 large 测试）
- `small_seed`（小样例的 seed 测试）
- `fight_multi`（多线程 fight 测试）

这些关键字可以组合或替换为具体测试名传入 `--filter`，例如只运行 `large`：`python track_test.py -f large`。

**默认过滤器（DEFAULT_FILTER）当前值：**

`large large_full small_seed fight_multi`

**参数说明：**

| 参数           | 说明                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------- |
| `-f, --filter` | 测试过滤表达式（默认见上方 DEFAULT_FILTER）。可传入任意关键词或具体测试名以控制运行的测试集。 |
| `-s, --show`   | 只显示当前失败状态，不运行测试                                                                |
| `-q, --quiet`  | 安静模式，只输出关键信息                                                                      |
| `-r, --reset`  | 重置历史记录                                                                                  |

解析与比对行为说明（重要）：

- 输出解析：脚本会解析 `cargo test` 的输出，识别 `test ... ... FAILED/ok` 的行，并在出现 `mismatch at idx=...` 的行时尝试提取 idx。
  - 增强点：跟踪最近的 panic/thread header（如 "---- ... stdout ----" 或 "thread '...'"），这样 mismatch 行如果没有内联 thread 信息也能关联到对应的测试。
  - 若 `mismatch` 行包含 `thread '...'`，会用线程名作为测试名关联 idx。
  - 若 `mismatch` 行不包含 thread 信息，脚本会：
    - 尝试使用最近出现的 header/thread 信息关联到对应的测试；
    - 尝试根据输出中的文本（例如 `sampled case-N`、`fight_large`、`large_full`、`large_\d{2}` 或 `::large_18` 之类的线程名）恢复到对应的测试名；
    - 并额外直接检测行中是否包含一些在代码中注册为"直接匹配项"的测试名（当前实现包含 `small_seed` 和 `simple_fight`），如果命中则把该 idx 关联到对应测试名。
- 比较规则修正（已修复的问题）：
  - 脚本现在只在当前运行和上次运行都存在该测试记录的情况下，才判断状态变化（即 NEW_FAIL / NEW_PASS）。这避免了因为某次运行未包含该测试而造成的误报。
  - 仅当当前与上次都有有效的 idx（>= 0）时，才比较 idx 并报告 **IMPROVED**（idx 变大）或 **REGRESSED**（idx 变小）。如果任意一侧 idx 为 -1（未知/无效），则不会报告 idx 变化。
  - 如果一个测试通过（非 FAILED），它默认的 idx 为 -1，不会被误判为"新失败"。
- 存档点比较：脚本会同时把当前结果与最近的存档点对比（若存在），并输出相应结论。

**存档点子命令：**

```bash
# 将当前记录保存为存档点（可指定名称）
python track_test.py save [名称]

# 列出所有存档点
python track_test.py list

# 对比当前记录与指定存档点（不指定则与最近存档点对比）
python track_test.py diff [名称]

# 删除存档点
python track_test.py delete 名称
```

存档点保存在 `target/test_checkpoints/` 目录下，用于标记重要的进度节点（如某次重要修复前后），方便长期对比。

**输出解读：**

每次运行测试后，工具会输出两段对比：

1. **vs 上次运行**：与上一次运行结果对比
2. **vs 存档点**：与最近存档点对比（如果存在）

变化类型：

- **[改进]**：测试分叉点延后（idx 变大），说明修改正确（仅在 idx 均有效时报告）
- **[退步]**：测试分叉点提前（idx 变小），说明修改引入问题（仅在 idx 均有效时报告）
- **[新失败]**：之前通过的测试现在失败了（仅在两次都有记录且状态发生变化时报告）
- **[修复]**：之前失败的测试现在通过了（仅在两次都有记录且状态发生变化时报告）

**结论判断：**

- `结论: 修改有效 (有改进且无退步)` → 可以继续
- `结论: 修改有问题 (存在退步)` → 需要回退修改
- `结论: 无明显变化` → 修改未产生影响

**安静模式输出示例：**

```text
[track_test] 运行测试: large large_full small_seed fight_multi
测试失败，分析中...
--- vs 上次运行 ---
结论: 修改有效 (有改进且无退步)

--- vs 存档点 "before_fix" (2026-02-26 10:00:00) ---
结论: 修改有效 (有改进且无退步)
```

安静模式只输出关键信息，适合在自动化流程或AI调用时使用。

### `tswn_case_miner` 跟踪工具

现在另有一个专门给 `tswn_case_miner` 用的跟踪脚本：

**工具位置：** `tswn-core/track_case_miner.py`

**执行提醒（重要）：**

- 在 `tswn-core` 里不要直接跑裸 `cargo run`。
- 如果你要运行普通 CLI，对 Cargo 来说二进制名是 `tswn-cli`，不是 `tswn_cli`。
- `tswn_cli.rs` 只是源码文件名；`cargo run --bin ...` 里必须写 `tswn-cli`。
- 很多 AI/agent 会因此踩到 `error: no bin target named 'tswn_cli' in default-run packages`，然后 Cargo 会提示相近目标其实是 `tswn-cli`。
- 日常跑 case miner 时，优先使用 `python ./track_case_miner.py ...`。
- 如果确实需要直接启动 miner，必须显式写成 `cargo run --bin tswn_case_miner -- ...`，不要省略 `--bin tswn_case_miner`。
- 如果需要直接重放某个 failed case 的原始战斗输出，命令应写成 `cargo run --bin tswn-cli -- --out-raw --file <input.txt>`。

这个工具不会解析终端输出，而是直接读取 `tswn_case_miner` 生成的 `target/ts_diff_cases/summary.json`，追踪：

- failed case 集合变化
- 相同 failed case 的 `first_mismatch_idx` 变化
- 存档点对比

**基本用法：**

```bash
# 运行 miner 并与上次结果比较
python ./track_case_miner.py

# 安静模式
python ./track_case_miner.py -q

# 只显示当前记录，不运行 miner
python ./track_case_miner.py -s

# 重置历史记录
python ./track_case_miner.py -r

# 需要时显式指定固定号库路径
python ./track_case_miner.py --library D:/shared/tswn-core/tests/sqp6000.txt
```

**常用参数：**

| 参数                     | 说明                                                                     |
| ------------------------ | ------------------------------------------------------------------------ |
| `--library`              | 号库文件路径；默认优先使用共享主仓库的 `tests/sqp6000.txt`               |
| `--md5-tool`             | `out_md5.ts` 路径；默认自动推导 `fast-namerena/branch/latest/out_md5.ts` |
| `--out-dir`              | miner 输出目录，默认 `target/ts_diff_cases`                              |
| `--shared-cache-dir`     | 共享 bun/TS 缓存目录，默认在主 worktree 的 `target` 下                   |
| `--modes`                | 对战模式，默认 `1v1,2v2,3v3v3,ffa`                                       |
| `--ffa-sizes`            | ffa 人数列表，默认 `4,6,8`                                               |
| `--case-offset-per-mode` | 每种模式按稳定顺序跳过前 N 个唯一 case                                   |
| `--max-cases-per-mode`   | 每种模式的 case 上限                                                     |
| `--keep-going`           | 单个 case 失败时继续                                                     |
| `-s, --show`             | 只显示当前失败状态，不运行 miner                                         |
| `-q, --quiet`            | 安静模式，只输出关键结论                                                 |
| `-r, --reset`            | 重置历史记录                                                             |

如果前面一段 case 已经修完，可以配合使用：

```bash
python ./track_case_miner.py -q --case-offset-per-mode 2000 --max-cases-per-mode 2000 --keep-going
```

这表示每种模式都跳过前 2000 个稳定生成的唯一 case，直接检查后面的 2000 个。

**存档点子命令：**

```bash
python track_case_miner.py save [名称]
python track_case_miner.py list
python track_case_miner.py diff [名称]
python track_case_miner.py delete 名称
```

默认记录位置：

- 当前记录：`target/case_miner_regression.json`
- 日志：`target/case_miner_regression.log`
- 存档点：`target/case_miner_checkpoints/`

worktree 模式下的默认规则：

- 当前 worktree 内的脚本、输出、测试相关路径统一按 `./` 解析
- 默认号库优先指向共享主仓库中的 `tests/sqp6000.txt`
- bun/TS 结果缓存默认落到主 worktree 的 `tests/tswn_case_miner_cache/`

### 统一入口

如果你不想记两个脚本，可以直接用：

```bash
python track.py test ...
python track.py miner ...
```

其中：

- `python track.py test ...` 转发给 `track_test.py`
- `python track.py miner ...` 转发给 `track_case_miner.py`

**重要提示：**

- 首次使用不需要额外动作，工具会自动创建基线
- 每次运行测试后，失败 idx 会自动保存到 `target/test_regression.json`
- 下次运行时会自动与上次的 idx 比较，同时也与最近存档点比较
- 日志记录在 `target/test_regression.log`
- 如果你需要追踪额外的测试名，可以通过 `--filter` 传入更多关键词或具体测试名

## 检查注意事项

- **检查过程中不需要检查 RC4 实现的相关问题** — RC4 随机数生成器的实现已经过多次验证，无需重复检查
