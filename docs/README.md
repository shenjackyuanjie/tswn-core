# 文档中心

按用途选择入口。接口接入以 [公共 API](reference/public-api.md)、各 crate README 和源码类型为准；设计规格、机制分析与历史记录不等同于当前接口承诺。

## 从这里开始

- 构建与打包：[全量构建](guides/build-all.md)、[OpenHarmony 交叉编译](guides/openharmony.md)。
- 接入对局：[公共 API 与跨语言契约](reference/public-api.md)、[Windows C/C++ 接入](guides/c-api-cpp-windows.md)。
- 自定义角色：[DIY / OL 格式](reference/diy-overlay.md)、[往返验证](guides/diy-validation.md)。
- 从旧版本迁移：[Runtime 0.5 迁移指南](guides/runtime-0.5-migration.md)。
- 查看 BattleSession 工作规格：[重构计划](design/battle-session-plan.md)、[API 冻结前加固要求](design/battle-session-hardening.md)。
- 性能测量与历史结果：[性能索引](perf/README.md)。

## 目录导航

| 目录 | 内容与适用范围 |
| --- | --- |
| [guides/](guides/README.md) | 使用、构建、验证与迁移指南 |
| [reference/](reference/README.md) | 公共 API 契约与输入输出格式 |
| [design/](design/README.md) | 实施计划、加固要求与验收规格；状态以正文为准 |
| [mechanics/](mechanics/README.md) | 分身、技能衰减、name_factor / rq 等机制分析 |
| [perf/](perf/README.md) | 性能方法、调查报告、历史基线、固定输入与原始数据 |
| [diff/](diff/README.md) | JS / Rust 差分报告与样例 |
| [releases/](releases/README.md) | 按版本保存的更新记录，包括标为开发中的记录 |
| [archive/](archive/README.md) | 项目起源、已淘汰架构与历史分析；不作为当前实现说明 |

## 文档约定

- 除入口 `README.md` 和版本号文件外，手写文档使用小写英文、连字符分词，例如 `battle-session-hardening.md`；不使用序号前缀、含糊的 `original` / `short` 或重复的 `tswn_` 前缀。
- 文件名说明主题；标题使用清晰的中文或 API 名称。一个文档只有一个一级标题，正文逐级分节。
- 每篇文档从所属分类索引可达。新增或移动文档时，同时更新索引、相对链接、仓库内引用和读取该文档的脚本。
- 设计和历史文档保留当时结论、状态与证据；归档不表示重新验收。旧源码路径、旧命令与历史外部链接可能失效。
- `perf/` 和 `diff/` 的固定输入、原始 JSON、图片及工具生成报告保留原文件名与路径，不套用手写文档命名规则，不重生成基线。性能报告中的测量值、版本、提交与原始数据内容不因整理而改写。
- 示例命令默认从仓库根目录执行；Markdown 链接相对当前文档。

主 Runtime 独立性与 release corpus 的验证入口：

```powershell
python scripts/check_runtime_release.py --corpus
```
