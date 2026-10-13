# tswn_wasm 示例

本目录提供 `tswn_wasm` 的浏览器示例页面。

## Streaming 结构

- `show-stream.js`：observer、完整 canonical history、最多 2 帧的预取和 source 生命周期。
- `show-display.js`：昵称派生、按 icon_key 缓存图标；不修改原始 DTO。
- `show-metrics.js`：TTIS / TTFE、拉帧和渲染计时；`?perf=1` 才输出统计。
- `show.js` / `show-replay.js`：可增长的播放计划、暂停/单步/历史检查点/重播。
- `show-theme-compat.js`：原生主题声明与 Dark Reader 注入提醒，不参与战斗和播放状态。
- `show.css`：样式入口，按原始层叠顺序导入 `show-base.css`、`show-players.css`、`show-replay.css`、`show-panels.css`。

前端回归测试位于源码仓库的 `crates/tswn_wasm/tests/`，不随示例分发。在仓库根目录执行 `bun test ./crates/tswn_wasm/tests` 或 `node --test crates/tswn_wasm/tests/show-*.test.mjs`；页面行为测试和浏览器基线命令见 [性能基线](../../../docs/perf/reports/web-streaming-baseline.md)。

## 示例一览

### `demo.html` — 快速功能验证

- 提供输入区、战斗控制区和胜率控制区。
- 使用 `fight_summary()` 获取摘要，通过 `FightSession` 逐帧推进对局，通过 `WinRateSession` 分批统计胜率。
- 适合快速测试 wasm 是否正确构建、核心战斗逻辑是否正常。

### `index.html` — 完整对局动画展示

- 全功能对战回放播放器，支持逐帧动画、分段推进。
- 包含多文件模块：
  - `show-wasm.js` — WASM 加载、`BattleSession` source 和显式释放
  - `show-utils.js` — DOM 渲染工具函数（头像、状态标签、`replayDisplayName()` 等）
  - `show-render.js` — 玩家状态 / 头像 / 状态标签渲染，seed 行展示
  - `show-replay.js` — 回放介绍、播放速度控制、逐段推进逻辑
  - `show-routing.js` — URL 输入与分享链接参数处理
- 支持 normal / fast / turbo 三种播放速度。
- 右下角盾形按钮（快捷键 `B`）切换盾条显示，默认关闭，使用 `localStorage` 保存选择。开启后左侧显示独立盾条，右侧消息在护盾变化时显示名字上方的盾条，按与血条相同的每 4 点 1px 比例展示新增/消耗区段；切换立即作用于已播放历史，保留播放进度。
- 盾条为 4px 高的青色条，配合明暗主题描边与高光；正文预留上方空间，与 HP 条保持间距，不改变盾值的宽度比例。
- 有护盾时，角色名字下方的状态标签区还会与其他正面状态一起显示绿色的 `护盾 +剩余值`。标签不受盾条开关影响，护盾消耗时更新，破盾或死亡时移除，并保留其他状态标签。
- 盾值来自 core 输出的 `shield` 快照，包含实际吸收护盾，不包含铁壁的单次伤害阈值；旧数据缺省和死亡态按零处理。现有护盾快照为回合级，同回合内多次护盾变化展示净变化。
- 支持从原始输入中提取 `seed:` 行并显示在玩家列表顶部。
- 支持通过 URL 参数直接传入对局输入并自动播放：`index.html?input=<url-safe-base64>`。参数值按 UTF-8 解码，Base64 使用 URL-safe 字符集（`+`→`-`、`/`→`_`，可省略末尾 `=`）。`replay` 和 `data` 可作为输入参数兼容别名；参数为空、Base64 非法或 UTF-8 解码失败时会停留在输入面板并显示错误。
- 页面按需拉取公共 `BattleSession` 的 canonical replay view；历史 `engine` / `runtime` 参数会从分享链接中清理，不再提供 `FightSession` fallback。
- 支持在右下角控制栏复制当前对局的分享链接，链接会使用同一套 `input` 参数格式。
- 召唤单位（clone / summon / shadow / zombie）会按类型显示对应的中文名；分身名字里的编号使用底层 `display_index`，左侧仍单独保留 `#playerId`。
- 只消费 `BattleReplayFrame.rows[].clips[]` 结构化 replay view，由 WASM 提供延迟、文本片段、血条变化、死亡特效和侧栏快照信息；战斗正文不再从 `message_template` / `message_rendered` / `hp_delta` 反推展示语义。
- normal 播放模式下，对战结束后等待 `1500ms` 再显示底部结算表；fast / turbo / 单步跳转保持即时显示。左侧玩家 HP 条变化使用较慢动画，方便观察血量变化。
- 页面与角色详情面板跟随明暗主题切换。深色模式使用更清晰的蓝灰名字框、可读的死亡名字和状态标签；像素图标保留原图配色，通过浅色衬底区分原图的深色描边。
- `show-wasm.js` 通过 `createBattleStreamSource()` 创建 `BattleSession`；页面先展示初始状态，再由 `BattleStreamController` 按需获取 core 提供的 `rows/clips/parts`。

#### 控制栏快捷键

右下角控制栏支持以下快捷键。字母键不区分大小写，仅在焦点不位于输入框、文本域或可编辑元素，且没有按下 Ctrl、Alt、Meta 修饰键时生效。

| 按键 | 操作 |
| --- | --- |
| `C` | 收起/展开控制栏 |
| `P` | 普通速度播放 |
| `F` | 快进播放 |
| `T` | 极速播放至结束 |
| `I` | 打开输入面板 |
| `R` | 重新播放当前对局 |
| `S` | 复制当前对局分享链接 |
| `D` | 切换明暗主题 |
| `B` | 显示/隐藏盾条（默认关闭，保存选择） |
| `Space` | 暂停/恢复播放或进入逐帧状态 |
| `←` / `→` | 后退/前进一个事件 |
| `↑` / `↓` | 后退/前进一帧 |

生成参数示例：

```js
const rawInput = "云剑狄卡敢\n白胡子\n\n史莱姆\n田一人";
const bytes = new TextEncoder().encode(rawInput);
const base64 = btoa(String.fromCharCode(...bytes));
const input = base64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const url = `index.html?input=${input}`;
```

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
- `http://127.0.0.1:8000/examples/`

## 说明

- 两套页面都优先从 `../pkg/tswn_wasm.js` 加载 wasm glue，推荐按上述分发目录启动。
- `demo.js` 还会尝试 `../dist/wasm/pkg/tswn_wasm.js`，支持从源码示例目录调试；`show-wasm.js` 的另一个候选是 `./pkg/tswn_wasm.js`，用于页面与 `pkg/` 同级的扁平部署。
- 由于浏览器的 ES module / wasm 加载要求，示例需要通过 HTTP 服务访问，不能直接双击本地文件运行。

## Dark Reader 与原生主题

show 页面已有原生明暗主题。页面使用 `color-scheme` 元数据同步当前主题，并按 [Dark Reader 官方约定](https://github.com/darkreader/darkreader/blob/main/CONTRIBUTING.md#disabling-dark-reader-on-your-site) 放置 `darkreader-lock`，请求插件不要再次转换本页；不删除插件样式、不改插件设置，也不强制改变用户保存的页面主题。

若插件仍在本页启用 Dynamic 深色转换，页面根据它写入的 `data-darkreader-mode` / `data-darkreader-scheme` 显示提醒，关闭转换后自动收起；这些标记的来源见 [官方 Dynamic 引擎](https://github.com/darkreader/darkreader/blob/main/src/inject/dynamic-theme/index.ts)。

**识别边界**：插件被 lock 成功跳过本站时不会写入启用标记，网页也无法知道它是否在其他站点或全局开启。当前提醒针对带有这些标记的 Dynamic 模式，不承诺识别所有旧版本、Filter/Static 模式或其他深色插件。遇到文字重影，应先关闭本站的插件转换，再核对原生主题。

## 外观验收截图

2026-10-13 使用隔离 Edge `155.0.4283.45`、debug WASM `0.5.7` 和真实 `seed:7@!` 护盾帧验收。正文同时覆盖生成盾值和破盾后的 HP 变化；盾条 4px，位于本行内部，与 HP 条间隔 2px。未改原始状态、护盾计量或回放规则。

- [深色盾条](../../../docs/images/wasm-show/shield-dark.png)
- [浅色盾条](../../../docs/images/wasm-show/shield-light.png)
- [移动布局与提醒](../../../docs/images/wasm-show/shield-mobile.png)：提醒由注入标记模拟触发。
- [真实 Dynamic 引擎提醒](../../../docs/images/wasm-show/darkreader-notice.png)：隔离页加载官方 `4.9.133` npm API；先验证 lock 生效，再仅在验收页移除 lock 触发转换，最后 disable 验证提醒消失。没有向用户的 Edge 安装或修改扩展。
