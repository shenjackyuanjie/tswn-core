# tswn_wasm 设计

## 目标

`tswn_wasm` 的目标不是把现有 C API 直接搬进浏览器，而是给网页前端提供一层 **JS 友好的 wasm 接口**，让 `tswn_core` 可以直接支撑一个类似 `fast-namerena/index.html` 的静态页面。

这个 crate 需要解决三件事：

1. 把 `Runner` / `PreparedRunner` / `RunUpdate` 封装成浏览器能直接消费的对象和 JSON 风格数据。
2. 屏蔽指针、手动内存释放、C ABI 字符串等不适合前端的边界。
3. 在 wasm 环境下提供可用的胜率运行模型，避免依赖 `std::thread`。

## 非目标

- 不在 `tswn_wasm` 内处理 DOM、iframe、按钮、语言包、广告位、分享面板。
- 不在 `tswn_wasm` 内复刻旧版页面样式。
- 不继续扩展 `tswn_capi` 作为网页主接口。

网页 UI、样式、交互流程应由单独的前端目录负责；`tswn_wasm` 只负责“算”和“吐数据”。

## 当前基础

目前 `tswn_core` 已经具备做网页接口的大部分能力：

- `Runner` 可以逐回合推进，适合驱动战斗动画。
- `RunUpdate` 已经包含 `score`、`delay0`、`delay1` 等 web 端需要的字段。
- `PreparedRunner` 适合胜率和批量模拟。
- 玩家显示名可以直接从 `Player::display_name()` 取出。
- 头像渲染逻辑已经存在，可以继续复用。

本次已额外处理一项 wasm 兼容性问题：

- `win_rate` 在 `target_family = "wasm"` 下强制使用单线程，避免浏览器默认环境下触发 `std::thread` 路径。

这意味着 `tswn_wasm` 可以直接建立在 `tswn_core` 之上，不需要先扩展 `tswn_capi`。

## 建议目录结构

建议新增一个 workspace crate：

```text
crates/
  tswn_wasm/
    Cargo.toml
    src/
      lib.rs
      error.rs
      model.rs
      render.rs
      fight.rs
      win_rate.rs
```

职责建议如下：

- `lib.rs`: wasm 导出入口，注册 panic hook，导出 class/function。
- `model.rs`: 导出给 JS 的数据结构。
- `render.rs`: 把 `RunUpdate` 渲染成前端可直接显示的消息。
- `fight.rs`: 单局对战接口与 `FightSession`。
- `win_rate.rs`: 增量胜率接口与 `WinRateSession`。
- `error.rs`: 把 Rust 错误转换成 JS 可读错误对象。

## Cargo 设计

建议 `Cargo.toml` 如下：

```toml
[package]
name = "tswn_wasm"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
tswn_core = { path = "../tswn_core", features = ["no_debug"] }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
js-sys = "0.3"
console_error_panic_hook = "0.1"
```

说明：

- `wasm-bindgen` 负责导出 wasm 接口。
- `serde` + `serde-wasm-bindgen` 用来返回 plain object，而不是手写 JS glue。
- `console_error_panic_hook` 只用于开发态调试，可以后续改成 feature。

## 导出原则

### 1. 不导出裸指针

前端不应该看到 `*mut T`、`free()`、ABI 字段结构。`tswn_wasm` 应当直接导出：

- `String`
- `Vec<T>`
- `JsValue`
- `#[wasm_bindgen]` class

### 2. 不耦合 DOM

`tswn_wasm` 不直接操作 `document`，也不创建页面元素。这样它可以同时被：

- 纯静态页面
- Vite/React 前端
- Web Worker

复用。

### 3. 优先返回前端即用数据

前端最讨厌的是“只差一步自己拼”。所以导出时应优先返回：

- `display_name`
- `team_index`
- `icon_png_base64`
- `message_rendered`
- `alive`

而不是只返回内部 ID 再让前端自己猜。

## 推荐对外 API

建议分成“简单函数”和“可持续会话对象”两层。

### 简单函数

适合页面初始化或一次性调用。

```ts
export function version(): string;
export function name_to_png_base64(name: string): string;
export function fight(rawInput: string, options?: FightOptions): FightReplay;
export function fight_summary(rawInput: string, options?: FightOptions): FightSummary;
```

说明：

- `fight` 直接跑完整局并返回完整回放，适合“点击开始后直接播放”。
- `fight_summary` 只返回赢家、玩家列表、结算状态，不返回完整帧，适合轻量场景。

### FightSession

适合像旧版页面那样逐回合播动画。

```ts
export class FightSession {
  constructor(rawInput: string, options?: FightOptions);
  players(): PlayerMeta[];
  state(): PlayerState[];
  isFinished(): boolean;
  winnerIds(): number[];
  step(): RoundFrame;
  runToEnd(limit?: number): FightReplay;
}
```

行为约定：

- `players()` 返回稳定元数据，不随回合变化。
- `state()` 返回当前全量状态快照。
- `step()` 推进一步，返回本轮事件与推进后的玩家状态。
- `runToEnd()` 用于跳过动画直接结算。

### WinRateSession

浏览器主线程下，大量胜率计算不能做成一次性同步阻塞调用，否则 UI 会卡住。

因此推荐导出增量式对象，而不是只提供一个同步 `win_rate()`：

```ts
export class WinRateSession {
  constructor(rawInput: string, totalRounds: number, options?: WinRateOptions);
  isFinished(): boolean;
  progress(): WinRateProgress;
  step(batchSize?: number): WinRateProgress;
  result(): WinRateResult;
}
```

建议语义：

- `step(batchSize)` 每次只推进一小批，例如 `100` 或 `500` 局。
- 前端可以在 `requestAnimationFrame`、`setTimeout` 或 `Web Worker` 中反复调用。
- wasm 下内部始终单线程；批量推进由外层调度。

如果后续确定胜率一定跑在 Worker 中，可以额外补一个：

```ts
export function win_rate_sync(rawInput: string, totalRounds: number, options?: WinRateOptions): WinRateResult;
```

但不建议把它作为页面主线程默认接口。

## 数据结构建议

### FightOptions

```ts
type FightOptions = {
  evalRq?: number;
  includeIcons?: boolean;
  captureReplay?: boolean;
};
```

说明：

- `evalRq` 对齐现有 core 行为。
- `includeIcons` 控制是否在 `players()` 时一并返回 base64 PNG。
- `captureReplay` 用于控制 `fight()` 是否缓存完整回放。

### WinRateOptions

```ts
type WinRateOptions = {
  evalRq?: number;
  thread?: number;
};
```

说明：

- 保留 `thread` 字段是为了接口兼容。
- 在 wasm 下，`thread` 只接受但不真正并行，内部会被钳制为 `1`。

### PlayerMeta

```ts
type PlayerMeta = {
  id: number;
  teamIndex: number;
  idName: string;
  displayName: string;
  iconPngBase64?: string;
};
```

### PlayerState

```ts
type PlayerState = {
  id: number;
  hp: number;
  maxHp: number;
  mp: number;
  movePoint: number;
  attack: number;
  defense: number;
  speed: number;
  agility: number;
  magic: number;
  resistance: number;
  wisdom: number;
  point: number;
  allSum: number;
  nameFactor: number;
  atBoost: number;
  attract: number;
  frozen: boolean;
  alive: boolean;
};
```

### UpdateView

```ts
type UpdateView = {
  score: number;
  delay0: number;
  delay1: number;
  casterId: number;
  targetId: number;
  targetIds: number[];
  updateType: "win" | "none" | "next_line";
  messageTemplate: string;
  messageRendered: string;
  param?: number;
};
```

### RoundFrame

```ts
type RoundFrame = {
  finished: boolean;
  winnerIds: number[];
  updates: UpdateView[];
  states: PlayerState[];
};
```

### FightReplay

```ts
type FightReplay = {
  players: PlayerMeta[];
  frames: RoundFrame[];
  winnerIds: number[];
  finalStates: PlayerState[];
};
```

### WinRateProgress / WinRateResult

```ts
type WinRateProgress = {
  done: boolean;
  roundsDone: number;
  totalRounds: number;
  wins: number;
  percent: number;
};

type WinRateResult = WinRateProgress & {
  timing?: {
    initNanos: number;
    fightNanos: number;
  };
};
```

## 消息渲染策略

`RunUpdate::msg()` 当前使用的是数字 ID 替换，而网页需要可读名字。因此 `tswn_wasm` 不应直接把 `update.msg()` 暴露给前端，而应在 wasm 包装层里自己做一遍渲染。

建议新增一个 `render.rs`，提供如下逻辑：

```rust
fn render_update_message(update: &RunUpdate, names: &HashMap<PlrId, String>) -> String
```

规则：

- `[0]` 替换成 `caster.display_name`
- `[1]` 替换成 `target.display_name`
- `[2]` 如果是 `param` 则渲染数字
- `[2]` 如果是 `targets` 列表则渲染成显示名拼接字符串

这样可以做到：

- 不改 `tswn_core` 现有消息结构
- wasm 前端拿到的就是可直接显示的日志

后续如果这个逻辑被多个导出层复用，再考虑把它上移到 `tswn_core`。

## FightSession 实现建议

### 内部状态

```rust
#[wasm_bindgen]
pub struct FightSession {
    runner: Runner,
    player_order: Vec<PlrId>,
    player_names: HashMap<PlrId, String>,
    player_team: HashMap<PlrId, usize>,
    include_icons: bool,
}
```

### 初始化

1. 使用 `Runner::new_from_namerena_raw()` 构造 runner。
2. 通过 `runner.all_plrs()` 固定玩家顺序。
3. 从 `storage.get_player()` 收集：
   - `display_name`
   - `id_name`
   - `team_index`
4. 如果 `include_icons = true`，同时计算 PNG base64。

### step 行为

1. 调用 `runner.main_round()`。
2. 把 `RunUpdates` 转成 `Vec<UpdateView>`。
3. 读取所有玩家当前状态，转成 `Vec<PlayerState>`。
4. 检查是否已有 winner。
5. 返回 `RoundFrame`。

这里推荐 **每一帧返回全量状态快照**，而不是只返回变更 diff。原因很简单：

- 玩家数量通常不大。
- 前端实现会更简单。
- 更容易调试和做回放。

如果以后需要压缩体积，再加“只返回变更玩家”的优化模式。

## WinRateSession 实现建议

### 为什么不直接导出同步 `win_rate`

即使 `tswn_core` 已经在 wasm 下保证单线程可用，主线程里一次性跑几千到几万局也仍然会阻塞页面。浏览器真正需要的是：

- 可中断
- 可显示进度
- 可分批推进

因此 `WinRateSession` 应当是一个“增量推进器”。

### 内部状态

```rust
#[wasm_bindgen]
pub struct WinRateSession {
    prepared: PreparedRunner,
    total_rounds: usize,
    next_round: usize,
    wins: usize,
    eval_rq: f64,
    use_profile_seed: bool,
}
```

### step(batchSize) 行为

1. 计算当前 batch 的结束边界。
2. 循环构造 `Runner::new_from_prepared_with_seed()`。
3. `run_to_completion()`。
4. 统计第一队是否胜利。
5. 更新 `next_round` 和 `wins`。
6. 返回 `WinRateProgress`。

这个实现和 `tswn_core::win_rate` 保持同样的种子调度逻辑，但把“并行批处理”改成“外部调度的顺序批处理”。

## 错误处理建议

Rust 侧不要把错误直接变成裸字符串。建议统一输出：

```ts
type TswnError = {
  code: string;
  message: string;
};
```

Rust 侧可以通过 `JsValue` 返回一个对象，常见错误码：

- `INVALID_INPUT`
- `RUNNER_INIT_FAILED`
- `WIN_RATE_INVALID_GROUPS`
- `INTERNAL_PANIC`

这样前端更容易区分“输入错误”和“引擎错误”。

## 页面接入方式

`tswn_wasm` 的职责是替代旧的 `md5-api.ts` 计算层，而不是替代整个页面。

前端建议分两层：

1. `tswn-web-adapter.ts`
2. 页面 UI

### tswn-web-adapter.ts

负责：

- 加载 wasm 包
- 包一层页面需要的高层函数
- 把 `FightSession` / `WinRateSession` 接成页面能直接调用的接口

建议形状：

```ts
export async function initTswn(): Promise<void>;
export async function fight(rawInput: string): Promise<FightReplay>;
export function createFightSession(rawInput: string): FightSession;
export function createWinRateSession(rawInput: string, totalRounds: number): WinRateSession;
```

### 页面层

页面层自己负责：

- 输入框与开始按钮
- 战斗面板布局
- 状态条动画
- 日志滚动
- 分享和截图
- 语言包文本

这和现在 `fast-namerena/index.html` 的职责边界是一致的，只是计算核心从 `md5.js` 换成了 wasm。

## 建议的最小里程碑

### M1: 最小可跑通

- 新建 `crates/tswn_wasm`
- 导出 `version()`
- 导出 `name_to_png_base64()`
- 导出 `fight_summary()`

目标：先确认 wasm 包可被网页加载、可正确初始化一局战斗。

### M2: 战斗动画可用

- 实现 `FightSession`
- 实现 `players()` / `state()` / `step()`
- 实现 `messageRendered`

目标：前端可以像旧版一样逐回合播战斗。

### M3: 胜率可用

- 实现 `WinRateSession`
- 实现 `step(batchSize)` 进度推进
- 页面增加进度条或中途取消按钮

目标：浏览器里可跑胜率，不依赖多线程。

### M4: 页面接入

- 建一个新的静态前端目录
- 接上输入、战斗面板、结算面板
- 补部署脚本

目标：产出一个可以替代旧版入口页面的 demo。

## 可选后续优化

- 支持 Web Worker 版胜率执行器。
- 支持只返回增量状态，减少回放体积。
- 把消息渲染逻辑上移到 `tswn_core`，避免多导出层重复实现。
- 给 `FightReplay` 增加压缩导出格式，便于分享和存档。

## 结论

`tswn_wasm` 最合适的定位是：

- 下层直接依赖 `tswn_core`
- 上层服务纯静态网页
- 中间用 `wasm-bindgen + serde-wasm-bindgen` 提供 JS 友好接口

其中最关键的设计不是“怎么导出一个 wasm 文件”，而是：

1. 对战要能逐帧推进。
2. 胜率要能增量计算，不阻塞页面。
3. 消息和玩家信息要在 wasm 层就变成前端即用格式。

按这个边界实现，后续页面无论是复刻旧版 `index.html`，还是重新做一个新 UI，都可以直接复用同一套引擎接口。