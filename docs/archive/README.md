# 项目历史与兼容复盘

[返回文档中心](../README.md)

本目录保留具有独立阅读价值的项目背景、原始实现研究和排查结论。历史材料按注明的基线阅读；现行 Rust 架构见 [core 原理指南](../core-guide/README.md)，调用契约见 [公共 API](../reference/public-api.md)。

## 项目背景

- [项目起源](project-origins.md)：从逆向 Dart 网页实现到 Rust 重写的背景随笔。
- [旧项目 README](legacy-readme.md)：原 `README.old.md`，保留作者的早期介绍与开发随笔。

## 原始实现与兼容经验

- [原始 Dart 架构](dart-architecture.md)：Dart 模块与调用时序，供原始行为研究使用。
- [why_ns 机制与排查复盘](why-ns-review.md)：合并原四篇分析，说明 JS 构造序号、门控与已定位的兼容差异。

## 号库回归方案

- [SBY Test 号库差分回归方案](sby-test-plan.md)：保留完整目标、六种对局模式、分档采样、失败定位及历史运行记录。`tests/sqp5900.txt`、`tests/sqp6000.txt` 是本地外部号库，按 `.gitignore` 约定另行准备；仓库中的固定 corpus 保存部分已发现缺陷的回归样例。号库方案用于继续发现固定样例之外的行为差异，执行前按当前 CLI 和 JS 工具核对原文命令。

## 查阅完整历史

已完成的 Runtime/custom 迁移、旧 Storage 方案和旧工作区约定，其完整过程可从 [整理前的 Git 快照](https://github.com/shenjackyuanjie/tswn-core/tree/821e3d47439983099e256bc2a07898a8b999bef8/docs/archive) 查阅。升级替代关系继续维护在 [Runtime 迁移指南](../guides/runtime-0.5-migration.md)，开发约定维护在 [AGENTS.md](../../AGENTS.md)。

版本和实验的原始证据分别保存在 [版本记录](../releases/README.md)、[性能资料](../perf/README.md) 和 [差分报告](../diff/README.md)。
