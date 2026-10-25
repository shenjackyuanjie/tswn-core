# 文档中心

按用途选择入口：[core 原理指南](core-guide/README.md)解释概念和数据流，[公共 API](reference/public-api.md)维护调用契约，各 crate README 维护安装和运行方式。设计文档补充关键取舍与字段规格，历史记录按写作时的版本和环境阅读。

## 从这里开始

- 熟悉游戏、开始了解项目：[项目与 core 原理指南](core-guide/README.md)，按角色、战斗、hook、状态、接口和工具链循序阅读。
- 查阅 Rust 类型和方法：[Cargo doc 阅读方法](core-guide/README.md#在本机看-api-文档)。
- 构建与打包：[全量构建](guides/build-all.md)、[OpenHarmony 交叉编译](guides/openharmony.md)。
- 参与开发：[模块边界与验证](guides/development.md)、[性能准则](perf/guides/performance.md)。
- 接入对局：[公共 API 与跨语言契约](reference/public-api.md)、[Windows C/C++ 接入](guides/c-api-cpp-windows.md)。
- 自定义角色：[DIY / OL 格式](reference/diy-overlay.md)、[往返验证](guides/diy-validation.md)。
- 从旧版本迁移：[Runtime 0.5 迁移指南](guides/runtime-0.5-migration.md)。
- 了解 BattleSession 的分层与设计取舍：[会话与网页增量播放](design/battle-session.md)。
- 性能测量与历史结果：[性能索引](perf/README.md)。
- 从外部号库继续发现差异：[SBY Test 回归方案](archive/sby-test-plan.md)，与仓库固定 corpus 配合使用。
- 构建脚本与验收命令：[脚本索引](../scripts/README.md)。
- 项目起源、原始 Dart 与兼容排查复盘：[历史资料](archive/README.md)。
- 胜率预测模块：[tswn-pwp 设计索引](design/README.md)。

## 目录导航

| 目录 | 内容与适用范围 |
| --- | --- |
| [core-guide/](core-guide/README.md) | 面向 namerena 玩家与项目新读者的图文原理指南 |
| [guides/](guides/README.md) | 使用、构建、验证与迁移指南 |
| [reference/](reference/README.md) | 公共 API 契约与输入输出格式 |
| [design/](design/README.md) | 现行架构取舍、状态字段审计与特征编码规格 |
| [mechanics/](mechanics/README.md) | 分身、技能衰减、name_factor / rq 等机制分析 |
| [perf/](perf/README.md) | 性能方法、调查报告、历史基线、固定输入与原始数据 |
| [diff/](diff/README.md) | JS / Rust 差分报告与样例 |
| [releases/](releases/README.md) | 按版本保存的更新记录，包括标为开发中的记录 |
| [archive/](archive/README.md) | 项目起源、个人随笔、原始 Dart 分析与兼容复盘 |

## 文档约定

- 除入口 `README.md` 和版本号文件外，手写文档使用小写英文、连字符分词，以主题命名，例如 `battle-session.md`。
- 文件名说明主题；标题使用清晰的中文或 API 名称。一个文档只有一个一级标题，正文逐级分节。
- 每篇文档从所属分类索引可达。新增或移动文档时，同时更新索引、相对链接、仓库内引用和读取该文档的脚本。
- 同一主题按用途维护权威来源：接口字段在 `reference/`，操作命令在所属指南或 crate README，设计理由在 `design/`；原理指南通过链接衔接详细资料。
- 已完成的实施清单和被替代的分析精简到当前结论，有独立价值的证据留在报告中，完整过程可从 Git 历史查阅。发布记录与历史测量保留版本、环境和原始结果。
- `perf/` 和 `diff/` 的固定输入、原始 JSON、图片及工具生成报告保留原文件名与路径，不套用手写文档命名规则，不重生成基线。性能报告中的测量值、版本、提交与原始数据内容不因整理而改写。
- 示例命令默认从仓库根目录执行；Markdown 链接相对当前文档。

## 维护审阅记录

- [全仓库更新日志](../CHANGELOG.md)：跨组件变更汇总，各 crate 的历史条目继续保留。
- [2026-10 综合维护报告](maintenance-2026-10.md)：源码、依赖与文档审查的统一覆盖和验收。
- [2026-10 文档审查覆盖与验证](reviews/2026-10-maintenance-doc-audit.md)：148 份受跟踪 Markdown、33 份脚本源文件的逐项清单、主要过时点与实际验证边界。

主 Runtime 的 release 行为与完整 corpus 回归入口：

```powershell
python scripts/check_runtime_release.py --corpus
```
