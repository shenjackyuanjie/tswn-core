# 性能分支简报（2026-10-29）

## 本次同步

`perf/ffa-profile-20261027` 从 `befd56a7` 合入 main `701c632e`，不改 main、不重写历史。新增上游内容包括 Boss 主动技能、魔法攻击与柯南推理秒杀、青雉/斑鸠防御吸收、马里奥满血复活和史莱姆分裂；既有武器移植原样保留。

自动合并无文本冲突。继续保留已验证的非 PGO 优化：目标抽样的标量排除范围、RC4 小范围精确取余、空伤害 hook 快路、World 派生索引，以及超过四个动态 profile 时的四路 KSA 分块。没有趁同步调整战斗语义或新增 unsafe。

## 历史结论（本次未重新计时）

以下是 **2026-10-28、旧 main `7f6a7b17` 上的结果**。本次新增战斗逻辑改变了工作负载，因此不能直接声称最新 main 仍有相同提速。

| 范围 | 历史结果与限制 |
|---|---|
| FFA 核心优化 | 扩展集 FFA41 吞吐中位 +7.87%；独立留出 FFA20 +5.40%。均为7轮同机配对；Boss FFA分层仅 +3.99%，不是所有场景达到5% |
| namer-pf | 最终只保留大于4个动态 profile 的 KSA 分块。两组8名 qd中位 +5.09%/+7.69%；qp未确认普遍收益，单名 qd/unicode曾 −4.68%，不能宣称四项全面提速 |
| RC4 unchecked | 调查未证明新增 unsafe 的实际 E2E价值，未保留生产 unsafe候选；不把微基准收益加到上述结果 |

测量使用 Rust 1.99.1、AMD EPYC 9V74、release fat LTO、`no_debug`、默认 mimalloc；无PGO、无额外RUSTFLAGS。重复轮复用同一seed区间，MAD不是置信区间；短案例存在明显双向波动。独立trace与分配计数核验通过，但FFA请求分配字节略增，不能称为内存优化。

历史细节如需追溯，可查看提交 [9aa6dbc3](https://github.com/shenjackyuanjie/tswn-core/commit/9aa6dbc39ff3664007c7feaea11795b0f8d2b1ca)（FFA报告）及 [befd56a7](https://github.com/shenjackyuanjie/tswn-core/commit/befd56a7193464471eb5d0c4b4521687e0372531)（namer-pf/RC4报告）。本次从分支最新状态删除这些冗长报告与生成的CSV/JSON/采样表，只保留本简报；Git历史仍在，不做force-push。保留可复用的benchmark、生成器、差分测试与诊断工具，其他上游文档和历史数据不动。

## 本次验证

- 核心库 **687项通过**，完整CLI **91项通过**；其中Boss 24、史莱姆分裂4、武器17项均通过。
- release验收：默认runtime 504、`no_debug` runtime 500、CLI runtime 6、完整corpus 124项通过。
- workspace：**1007通过、5失败、7 ignored**，不是全绿。5项DS4 golden失败已在未修改的main `701c632e`独立复现；每项首个断言的左右字节数组经CRLF归一化后与合并分支一致，未改golden掩盖失败。
- nightly格式检查和14项Python性能工具测试通过；独立代码复核未发现自动合并遗漏。

上述调用存在测试重叠，不相加当作唯一用例总数。本次没有重跑性能采样或大规模benchmark，也不把历史百分比冒充最新移植逻辑的实测结果。

## 复现入口

```bash
cargo +nightly fmt --check
cargo test -p tswn_core
python scripts/check_runtime_release.py --corpus
cargo test --workspace --no-fail-fast

# 固定Runtime矩阵；比较时两侧必须同编译器、features和输入。
cargo build --locked --release -p tswn_core --features no_debug --example perf_runtime
cargo run --locked --release -p tswn_core --features no_debug --example perf_runtime -- \
  --input docs/perf/fixed_cases_30 --runs 13000 --threads 1 --mode win-rate
python scripts/compare_perf_runtime.py --help
python scripts/benchmark_namer_pf_confirm.py --help
```

进一步复测时将生成数据写入本地 `target/` 或仓库外，不再默认提交逐轮结果。新旧main比较必须重新建立匹配基线；不能拿本简报的历史百分比作为新移植逻辑的性能保证。
