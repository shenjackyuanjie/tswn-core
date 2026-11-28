# Repository Guidelines

## 项目结构与模块

这是一个 Rust workspace。核心战斗与 CLI 位于 `crates/tswn_core/`；跨语言与界面适配分别在 `tswn_capi`、`tswn_py`、`tswn_wasm`、`tswn_openbox`，排名工具在 `tswn_*ranker`。共享或集成测试主要在 `crates/tswn_test/`，输入样例在 `tests/`，设计与构建说明在 `docs/`，自动化脚本在 `scripts/`。修改功能时优先将测试放在所属 crate 的模块内；跨 crate 行为放入 `tswn_test`。

## 仓库 skill

仓库内 skill 放在 `.agents/skills/`，DSH 会在本项目中自动发现，也可用 `skill` 工具显式加载：

- [`fixed30-benchmark`](.agents/skills/fixed30-benchmark/SKILL.md)：fixed30 性能基准的采样、汇总出表与 `docs/perf` 留档流程，附 `run_rounds.ps1`、`summarize.mjs` 与历史基线 `history.json`。

## 构建、测试与开发

- `cargo build --workspace`：构建全部 Rust crate。
- `cargo test`：执行工作区测试；日常核心改动可用 `cargo test -p tswn_core`。
- `cargo run -p tswn_core --bin tswn-cli -- fight -f input.txt`：以文本输入运行战斗 CLI。
- `cargo clippy --workspace --all-targets`：检查常见 Rust 问题。
- `cargo +nightly fmt --check`：提交前验证格式。

## 代码风格与命名

新增或修改的代码注释（包括 Rust 文档注释）和项目文档原则上使用中文，清楚说明意图、约束与行为。API 名称、代码标识符、命令、路径、协议字段及必要的技术术语保留原文，不强行翻译。此约定不改变代码标识符和文档文件名的命名规则，也不要求为统一语言而批量改写无关的既有内容。

可见性默认写 `pub`，不要用 `pub(crate)` / `pub(super)` / `pub(in ...)` 收窄范围。Rust 的 `pub` 只表示「对父模块可见」，外层模块本身私有的话 `pub` 不会泄漏到 crate 之外，所以这些修饰在这里基本不提供额外保护；反倒会在模块移动、目录重排时（例如把 `runtime/plain_slime.rs` 移进 `runtime/combat/boss_slime.rs`，`pub(super)` 的可见范围随之收窄）不断失效并连带打断调用方。确实不想暴露的东西用不带 `pub` 的私有项表达，而不是可见性修饰。

遵循 Rust 默认命名：模块、函数和文件用 `snake_case`，类型与 trait 用 `PascalCase`，常量用 `SCREAMING_SNAKE_CASE`。避免无关重排或大规模格式差异。格式检查与格式化**必须**使用 nightly：`cargo +nightly fmt`（检查用 `cargo +nightly fmt --check`）。`rustfmt.toml` 启用了 `fn_single_line`、`unstable_features` 等 nightly 专属选项；stable 的 `cargo fmt` 会产生伪差异且可能按错误配置改写代码。提交前确保该检查无输出。

## 测试要求

使用 Rust 内置测试框架（`#[test]`）。修复 `tswn_core` 的对齐缺陷（与 legacy / JS 行为的偏差）时，先添加可复现测试再修复；其他缺陷直接修复。涉及 runtime 或解析语义时，覆盖边界条件与已有 case。提交前运行受影响 crate 的测试；影响核心逻辑时运行 `cargo test -p tswn_core`，跨模块变更再运行 `cargo test`。除非有意更新语义，不要重生成 `crates/tswn_test/cases/` 中的 golden/corpus 基线。

测试应验证实际输入输出、状态变化、边界条件或资源管理。目录摆放、源码写法、架构选择、已移除入口及固定配置的维护约束留在对应文档，不单独增加“防止以后重新这么做”或冻结实现的测试。已发生缺陷的行为回归、公共接口兼容性与数据格式验证不属于这类维护约束。

## 提交与 PR

近期历史采用 Conventional Commit 风格，如 `feat(openbox): 添加版本信息弹窗`、`docs(repo): 添加 Agent 工作约定`。使用 `feat`、`fix`、`docs`、`chore` 等类型和简短 scope；提交主题和正文均使用中文。PR 应说明动机、关键实现与验证命令；关联 issue；若更改 Openbox 或 WASM 可见界面，附截图或录屏。避免混入无关格式化、构建产物或本地配置。

### AI 协作者署名

提交里有 AI 模型参与（写代码、审查、跑验证、整理文档都算），**必须**用 `Co-authored-by` 标出来。不要把 AI 的产出记在人名下，也不要因为怕麻烦就省掉——署名是回溯“这段是谁写的、哪个模型想出来的”的唯一线索。

```text
Co-authored-by: <模型标识> <模型标识@厂商域名>
```

- 多个模型参与就写多行，放在提交正文末尾的 trailer 区（GitHub 靠这个统计贡献）。整批提交保持同一组署名，不要挑着加。
- `<模型标识>` 用小写连字符拼接「厂商-模型-变体」，不要出现空格和大写。带空格的 `GPT 6 Luna` 这类写法会让 trailer 解析和 mailmap 都对不上。
- 邮箱域名按厂商固定：OpenAI 用 `@openai.com`、Anthropic 用 `@anthropic.com`、DeepSeek 用 `@deepseek.com`、StepFun 用 `@stepfun.com`。不要用 `@example.invalid` 之类的占位域名，那等于没署名。
- 真人协作者不适用这条：`KODU16` 这种 `Co-authored-by` 是真人往来，照常保留。

当前生效的写法（新模型按同一规则推名字，并补进这张表）：

| 模型 | 署名 |
| --- | --- |
| GPT-6.2 Sol | `gpt-6.2-sol <gpt-6.2-sol@openai.com>` |
| Claude Opus | `claude-opus-4-7 <claude-opus-4-7@anthropic.com>` |
| DeepSeek Flash | `deepseek-flash <deepseek-flash@deepseek.com>` |
| Step 5 Preview | `step-5-preview <step-5-preview@stepfun.com>` |
| Gemini 4 Neon | `gemini-4-neon <gemini-4-neon@google.com>` |

署名要落到**具体模型**上。`Copilot`、`Codex`、`Claude Code` 这类产品名和代理名都不算——它们只是入口，换个后端就换了脑子，挂上去说不清到底是谁在干活。历史上 `Codex GPT-5.5` 和 `GPT-5.5` 分裂成两条就是这么来的。被它们调起来跑的那个具体模型才该署名；认不出底层模型时按厂商-模型写，别拿产品名顶替。

同一个模型固定用同一种写法。历史上 `step5preview` 与 `step-5-preview` 并存、`deepseek-v4-1-flash` 用 `example.invalid` 占位，除了让统计碎成一地没有别的作用。自己是什么模型就用什么标识署名，不要借用别人的模型名。

存量提交不回填，这条只管新提交。

## 远程仓库与推送

主线对应两个必须保持同步的 remote：

- `github`：`git@github.com:shenjackyuanjie/tswn-core.git`
- `gitea`：`ssh://git@git.shenjack.top:5101/shenjack/tswn-core.git`

另外还有一个只读上游，**不参与推送**：

- `kodu16`：`git@github.com:KODU16/tswn-core-clever.git`。别人的 fork，独立内容开发线（新 Boss、clever 搜索、wasm 胜率图表），与主线是大跨度分叉，只在需要比对时 `git fetch kodu16`。它的 `pushurl` 已钉为 `no_push`，误推会直接报错。

**`github` 与 `gitea` 的内容必须一致。推送任何一个分支、main 或 tag 时，两个都要推**（只推一边会让两个远端分叉）：

```powershell
git push github <branch>
git push gitea <branch>
```

main 与 tag 同理（例如 `git push github main; git push gitea main`）。只推一边会让两个远端分叉，
之后 fetch 会出现“本地看起来落后或领先”的假象，也会让 clone 来源不同的人看到不同历史。
推送后可用 `git ls-remote github <ref>` 与 `git ls-remote gitea <ref>` 核对两边指向同一个 commit。

### 当前 clone 可能只配了其中一个

不同来源的 clone 未必两个 remote 都在（从 gitea 镜像拉的就可能只有 gitea，手动加的也可能只加了 github）。**动手推送前先 `git remote -v` 看一遍**，缺哪个补哪个，不要因为缺一个就只推一边：

```powershell
# 只有 gitea，补 github
git remote add github git@github.com:shenjackyuanjie/tswn-core.git

# 只有 github，补 gitea
git remote add gitea ssh://git@git.shenjack.top:5101/shenjack/tswn-core.git
```

如果唯一的 remote 叫 `origin`，先确认它指向哪个仓库，把它当作对应的那一个，再补另一个缺失的；不要为了统一名字去改 `origin`。补齐后照常两边都推，推完核对 `ls-remote`。

某个 remote 临时不可达（网络、SSH、权限）而另一边已经推成功时，两端就此分叉。GitHub 侧有禁止非快进推送的 ruleset，用 force push 硬推会被直接拒绝；正确做法是尽快把缺的一边补上，别用改写历史去掩盖。
