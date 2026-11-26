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

遵循 Rust 默认命名：模块、函数和文件用 `snake_case`，类型与 trait 用 `PascalCase`，常量用 `SCREAMING_SNAKE_CASE`。避免无关重排或大规模格式差异。格式检查与格式化**必须**使用 nightly：`cargo +nightly fmt`（检查用 `cargo +nightly fmt --check`）。`rustfmt.toml` 启用了 `fn_single_line`、`unstable_features` 等 nightly 专属选项；stable 的 `cargo fmt` 会产生伪差异且可能按错误配置改写代码。提交前确保该检查无输出。

## 测试要求

使用 Rust 内置测试框架（`#[test]`）。修复 `tswn_core` 的对齐缺陷（与 legacy / JS 行为的偏差）时，先添加可复现测试再修复；其他缺陷直接修复。涉及 runtime 或解析语义时，覆盖边界条件与已有 case。提交前运行受影响 crate 的测试；影响核心逻辑时运行 `cargo test -p tswn_core`，跨模块变更再运行 `cargo test`。除非有意更新语义，不要重生成 `crates/tswn_test/cases/` 中的 golden/corpus 基线。

测试应验证实际输入输出、状态变化、边界条件或资源管理。目录摆放、源码写法、架构选择、已移除入口及固定配置的维护约束留在对应文档，不单独增加“防止以后重新这么做”或冻结实现的测试。已发生缺陷的行为回归、公共接口兼容性与数据格式验证不属于这类维护约束。

## 提交与 PR

近期历史采用 Conventional Commit 风格，如 `feat(openbox): 添加版本信息弹窗`、`docs(repo): 添加 Agent 工作约定`。使用 `feat`、`fix`、`docs`、`chore` 等类型和简短 scope；提交主题和正文均使用中文。PR 应说明动机、关键实现与验证命令；关联 issue；若更改 Openbox 或 WASM 可见界面，附截图或录屏。避免混入无关格式化、构建产物或本地配置。

## 远程仓库与推送

仓库同时配置两个 remote，两边内容必须保持一致：

- `github`：`git@github.com:shenjackyuanjie/tswn-core.git`
- `gitea`：`ssh://git@git.shenjack.top:5101/shenjack/tswn-core.git`

**推送任何一个分支或 main 时，两个 remote 都要推**（只推一边会让两个远端分叉）：

```powershell
git push github <branch>
git push gitea <branch>
```

main 与 tag 同理（例如 `git push github main; git push gitea main`）。只推一边会让两个远端分叉，
之后 fetch 会出现“本地看起来落后或领先”的假象，也会让 clone 来源不同的人看到不同历史。
推送后可用 `git ls-remote github <ref>` 与 `git ls-remote gitea <ref>` 核对两边指向同一个 commit。

## Clever 独立发布约定

本工作目录的 Clever 引擎和前端以这里发布到 KODU16/tswn-core-clever main 的修改为准，仅在用户明确要求时同步上游主仓库，并保留 Clever 扩展。功能修改完成后验证并发布 Clever main，前端与 WASM 必须由同一提交构建。前端地址及同步机制见 docs/clever-search.md。本 fork 仅发布 origin，不推送 shenjackyuanjie 的 github/gitea remote；上文双 remote 规则适用于上游仓库。

### 当前 clone 可能只配了其中一个

不同来源的 clone 未必两个 remote 都在（从 gitea 镜像拉的就可能只有 gitea，手动加的也可能只加了 github）。**动手推送前先 `git remote -v` 看一遍**，缺哪个补哪个，不要因为缺一个就只推一边：

```powershell
# 只有 gitea，补 github
git remote add github git@github.com:shenjackyuanjie/tswn-core.git

# 只有 github，补 gitea
git remote add gitea ssh://git@git.shenjack.top:5101/shenjack/tswn-core.git
```n
如果唯一的 remote 叫 `origin`，先确认它指向哪个仓库，把它当作对应的那一个，再补另一个缺失的；不要为了统一名字去改 `origin`。补齐后照常两边都推，推完核对 `ls-remote`。

某个 remote 临时不可达（网络、SSH、权限）而另一边已经推成功时，两端就此分叉。GitHub 侧有禁止非快进推送的 ruleset，用 force push 硬推会被直接拒绝；正确做法是尽快把缺的一边补上，别用改写历史去掩盖。
