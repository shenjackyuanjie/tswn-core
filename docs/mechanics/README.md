# 战斗机制分析

[返回文档中心](../README.md)

- [分身机制](clone.md)：本体与分身、衰减下限、SlotBoost 和 DIY 拟合。
- [技能衰减](skill-decay.md)：五种主动技能的衰减方式。
- [name_factor 与 rq](name-factor-rq.md)：实际对打和胜率评估的差异。
- [判胜语义](winner.md)：胜者定义、`alive_group_count` 的边界、Rust 与 legacy `md5.js` 的对照证据。

这些文档按当前 Runtime 源码说明机制，历史样本与 legacy 对照会单独标注；当前调用方式见 [公共 API](../reference/public-api.md)。JS 构造序号的历史机制和定位经验集中在 [why_ns 复盘](../archive/why-ns-review.md)。
