# tswn_wasm

`tswn_core` 的浏览器端 WASM 接口（wasm-bindgen）。

## 目标

为网页前端提供 JS 友好的 wasm 封装，不依赖裸指针 / 手动释放 / C ABI 字符串。
可直接支撑类似 `fast-namerena/index.html` 的静态页面。

## 导出

### 顶层函数

| 函数 | 说明 |
|------|------|
| `version()` | wasm 包装层版本 |
| `core_version()` | tswn_core 版本 |
| `name_to_png_base64(name)` | 名称 → PNG Base64 |
| `fight(rawInput, options?)` | 一次性跑完整局，返回 `FightReplay` |
| `fight_summary(rawInput, options?)` | 轻量摘要（赢家、玩家列表） |

### FightSession

适合逐回合播动画：

```js
const session = new wasm.FightSession(rawInput, { includeIcons: true });
session.players();    // PlayerMeta[]
session.state();      // PlayerState[] — 当前全量快照
session.step();       // RoundFrame — 推进一步
session.runToEnd();   // FightReplay — 跳过动画直接结算
session.isFinished();
session.winnerIds();
```

### WinRateSession

增量式胜率统计，不阻塞主线程：

```js
const session = new wasm.WinRateSession(rawInput, 5000);
while (!session.isFinished()) {
    session.step(100);  // 每次推进 100 局
    console.log(session.progress());  // { done, roundsDone, percent, ... }
}
session.result();  // WinRateResult
```

## 示例

```bash
# 启动静态服务器
cd crates/tswn_wasm/examples
python -m http.server 8000
# 打开 http://127.0.0.1:8000/demo.html
# 或   http://127.0.0.1:8000/show.html
```

## 构建

```powershell
# 构建 wasm + wasm-bindgen 打包
uv run scripts/build_wasm.py --release
```

前置依赖：`wasm-bindgen-cli`

```powershell
cargo install wasm-bindgen-cli
```

## 设计

详见 [docs/tswn_wasm_design.md](../../docs/tswn_wasm_design.md)。

## 版本

当前版本见 [CHANGELOG.md](./CHANGELOG.md)。
