# tswn_wasm Examples

本目录提供 `tswn_wasm` 的最小浏览器示例。

- `demo.html`: 静态页面壳，提供输入区、战斗控制区和胜率控制区。
- `demo.js`: 通过 `wasm-bindgen` 生成的 JS glue 调用 `tswn_wasm` 导出接口。

## 运行方式

推荐先构建 wasm 分发目录：

```powershell
uv run scripts/build_wasm.py --release
```

默认会生成：

```text
crates/tswn_wasm/dist/wasm/
  pkg/
  raw/
  examples/
```

随后在输出目录下启动一个本地静态服务器，例如：

```powershell
cd crates/tswn_wasm/dist/wasm
python -m http.server 8000
```

然后在浏览器打开：

- `http://127.0.0.1:8000/examples/demo.html`

## 说明

- `demo.js` 会优先尝试从打包结果目录的 `../pkg/tswn_wasm.js` 加载 wasm glue。
- 若你直接在源码目录下调试，它也会回退尝试 `../dist/wasm/pkg/tswn_wasm.js`。
- 由于浏览器的 ES module / wasm 加载要求，这个 demo 需要通过 HTTP 服务访问，不能直接双击本地文件运行。
