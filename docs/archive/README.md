# 历史归档

[返回文档中心](../README.md)

本目录保留历史上下文，不作为当前 API、模块结构或待实施任务。文中的“当前”指写作时的基线；部分源码、命令和行号已不存在。当前接口见 [公共 API](../reference/public-api.md)，架构切换结果见 [Runtime 0.5 迁移](../guides/runtime-0.5-migration.md)。

## 项目与旧架构

- [项目起源](project-origins.md)：从逆向 Dart 网页实现到 Rust 重写的背景随笔。
- [原始 Dart 架构](dart-architecture.md)：面向重写者的源实现架构分析。
- [Storage 内部可变性分析](storage-refactor.md)：已删除的旧 Rust 对象模型。
- [Runtime 核心重构实施规格](runtime-refactor-plan.md)：已完成的主 Runtime 迁移过程。
- [Custom Runtime 迁移审计](custom-runtime-migration.md)：已完成的 custom 产品线行为迁移审计。

## why_ns 历史分析

以下文档存在不同阶段的判断，保留而不合并。涉及修复后结论时先阅读复盘；其余材料用于追溯推理过程，不代表今天的实现。

- [修复复盘](why-ns-fix-review.md)：2026-05-18 修复后的分析。
- [机制概览](why-ns-overview.md)：JS 构造序号、退款与重置机制。
- [早期判断](why-ns-early-analysis.md)：保留“尚未实现”阶段的结论。
- [Rust 实现分析](why-ns-rust-analysis.md)：2026-05-18 的旧路径分析。
