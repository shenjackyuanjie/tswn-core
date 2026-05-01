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
| `fight(raw_input, options?)` | 一次性跑完整局，返回 `FightReplay` |
| `fight_summary(raw_input, options?)` | 轻量摘要，返回 `FightSummary`（赢家、玩家列表、最终状态） |
| `win_rate_sync(raw_input, total_rounds, options?)` | 同步跑完胜率统计，返回 `WinRateResult` |

### FightSession

适合逐回合播动画：

```js
const session = new wasm.FightSession(rawInput, {
    evalRq: 0.5,          // 可选，名称评分参数
    includeIcons: true,   // 可选，是否生成 PNG Base64 头像
    captureReplay: true,  // 可选，是否捕获逐帧回放（默认 true）
});
session.players();       // PlayerMeta[] — 玩家元数据列表
session.state();         // PlayerState[] — 当前全量状态快照
session.step();          // RoundFrame — 推进一步
                         //   { finished, winner_ids, updates, states, total_delay }
session.isFinished();    // bool — 是否已产生胜者
session.winnerIds();     // number[] — 获胜方 ID 列表
session.runToEnd(limit?); // FightReplay — 跳过动画直接结算，可限制最大帧数
```

### WinRateSession

增量式胜率统计，不阻塞主线程：

```js
const session = new wasm.WinRateSession(rawInput, 5000, {
    evalRq: 0.8,   // 可选，名称评分参数（默认 WIN_RATE_EVAL_RQ）
    thread: 1,     // 可选，wasm 目标下恒为 1
});
session.evalRq();          // number — 当前 eval_rq 值
while (!session.isFinished()) {
    session.step(100);     // batch_size 可选，默认 100；返回 WinRateProgress
    console.log(session.progress());  // { done, roundsDone, totalRounds, wins, percent }
}
session.result();  // WinRateResult — 含 timing（initNanos, fightNanos）
```

### 数据模型

| 类型 | 说明 |
|------|------|
| `PlayerMeta` | 玩家元数据：`id`, `teamIndex`, `idName`, `displayName`, `iconPngBase64?` |
| `PlayerState` | 玩家状态：`hp`, `maxHp`, `mp`, `attack`, `defense`, …, `ownerId?`, `alive` |
| `RoundFrame` | 回合帧：`finished`, `winnerIds`, `updates[]`, `states[]`, `totalDelay` |
| `UpdateView` | 单条更新消息：`casterId`, `targetId`, `messageRendered`, `messageTemplate`, `tone`, … |
| `MessageTone` | 消息色调：`"normal"` / `"damage"` / `"recover"` / `"knockout"` |
| `FightReplay` | 完整回放：`players`, `frames[]`, `winnerIds`, `finalStates` |
| `FightSummary` | 轻量摘要：`finished`, `players`, `winnerIds`, `finalStates` |
| `WinRateProgress` | 增量进度：`done`, `roundsDone`, `totalRounds`, `wins`, `percent` |
| `WinRateResult` | 最终结果：`done`, `roundsDone`, `wins`, `percent`, `timing?` |
| `WinRateTiming` | 耗时统计：`initNanos`, `fightNanos`（wasm32 下均为 0） |

### 错误

所有可能失败的函数返回 `WasmResult<T>`（即 `Result<T, JsValue>`），错误对象结构为 `{ code: string, message: string }`。

错误码：
- `INVALID_INPUT` — 输入为空或格式错误
- `INVALID_OPTIONS` — options 解析失败
- `RUNNER_INIT_FAILED` — Runner 初始化失败
- `WIN_RATE_INVALID_GROUPS` — 胜率统计要求至少两个非空分组
- `INTERNAL_ERROR` — 内部异常

## 示例

`examples/` 目录包含两套静态页面：

| 文件 | 说明 |
|------|------|
| `demo.html` / `demo.js` / `demo.css` | 快速功能验证（战斗 + 胜率） |
| `show.html` / `show.js` / `show.css` | 完整对局动画展示 |
| `show-wasm.js` | WASM 模块加载与初始化 |
| `show-utils.js` | DOM 渲染工具函数 |
| `show-render.js` | 玩家状态 / 头像渲染 |
| `show-replay.js` | 逐帧回放逻辑 |

```bash
# 构建 wasm 分发目录后启动静态服务器
cd crates/tswn_wasm/dist/wasm
python -m http.server 8000
# 打开 http://127.0.0.1:8000/examples/demo.html
# 或   http://127.0.0.1:8000/examples/show.html
```

详细运行说明见 `examples/README.md`。

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
