# tswn_wasm 前端回归测试

本目录存放浏览器示例的前端回归测试，与 `examples/` 中的页面和运行模块分开，不随示例分发。

在仓库根目录执行：

```powershell
# 使用 Bun
bun test ./crates/tswn_wasm/tests

# 或使用 Node
node --test crates/tswn_wasm/tests/show-*.test.mjs
```

测试使用 Node 与 Bun 均支持的 `node:test`，无需安装 npm 依赖或预先构建 WASM。覆盖流式会话适配、source 生命周期、回放计划、昵称与图标、性能计数、URL 路由、护盾渲染、主题对比度及 Dark Reader 提醒的显示/清理。

真实页面行为测试与浏览器性能基线见 [Web streaming 基线](../../../docs/perf/reports/web-streaming-baseline.md)。Rust 测试仍使用 `cargo test -p tswn_wasm`。
