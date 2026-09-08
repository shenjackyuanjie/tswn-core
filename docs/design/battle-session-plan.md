# TSWN BattleSession + Web Streaming 最终重构计划

> 状态：FINAL / 已锁定  
> 目标仓库：`shenjackyuanjie/tswn-core`  
> 本文覆盖：BattleSession 公共 API、Python/WASM/C 同步、CLI 清理与 JSONL streaming、网页真实 streaming 迁移、性能基线、未来扩展准备  
> 本文不覆盖：任何胜率模型、ModelState、训练数据、模型推理、模型 UI  
> 本文取代此前 BattleSession 计划文档，作为本轮实现唯一规格。

---

## 0. 本轮最终目标

本轮完成后，TSWN 的对局执行只存在一套正式语义：

```text
BattleSession = 有状态、增量推进的正式 User API
BattleReplay  = BattleSession 的完整收集结果
Runner        = Runtime 级 Advanced API
```

完整架构：

```text
                         tswn_core
                            │
                     cli_api::battle
                            │
                    ┌───────┴────────┐
                    │                │
                    ▼                ▼
              BattleSession     battle_replay()
                    │                │
             next_frame()       collect session
                    │                │
       ┌────────────┼────────────┐   │
       │            │            │   │
       ▼            ▼            ▼   ▼
    Python         WASM           C  one-shot
                    │
                    ▼
           BattleStreamSource
                    │
                    ▼
        BattleStreamController
                    │
          ┌─────────┴─────────┐
          │                   │
          ▼                   ▼
       历史                播放
                              │
                              ▼
                           Renderer
```

CLI 同样消费 `BattleSession`：

```text
tswn-cli fight
tswn-cli fight --jsonl
```

网页不再先运行完整 `battle_replay()`。

网页按：

```text
initial
frame
frame
frame
...
result
```

实时拉取和播放。

---

## 1. 实施纪律：写一块，测一块，提交一块

这是本计划的硬性要求。

不得：

```text
同时改 core + Python + WASM + C + web
最后一次性提交
```

必须：

```text
完成一个独立职责
    ↓
运行该职责对应测试
    ↓
确认当前 commit 可独立编译/运行
    ↓
立即提交
    ↓
再进入下一块
```

---

### 1.1 每个 commit 必须满足

每一个 commit 都必须：

- 只有一个清晰职责；
- 不混入无关重构；
- 对应 crate / JS 测试通过；
- 不留下临时注释、TODO 代替实现；
- 不提交已知 broken intermediate state；
- 不依赖“下一个 commit 才修好当前 commit”；
- 不混入全仓无关格式化；
- commit message 明确写出模块和行为。

---

### 1.2 Commit message 前缀

统一：

```text
core:
py:
wasm:
capi:
cli:
web:
docs:
test:
```

例如：

```text
core：添加规范 BattleSession 状态机
web：添加仅追加的流式回放计划
cli：以 fight jsonl 替换旧版原始输出
```

---

### 1.3 不 squash 成单一大提交

实现分支最终保留分块 commit 历史。

允许在单个职责内部修订 commit，但最终历史必须能清楚看出：

```text
Core API
Core Session
Bindings
CLI
Web source
Web controller
Web playback
Web perf
Docs
```

的推进过程。

---

## 2. Core 模块结构

建立：

```text
crates/tswn_core/src/cli_api/
├── mod.rs
├── battle/
│   ├── mod.rs
│   ├── dto.rs
│   ├── session.rs
│   └── replay.rs
├── bench.rs
└── parse.rs
```

职责：

```text
battle/dto.rs
    BattleOptions
    BattleStatus
    BattleStopReason
    BattleResult
    BattlePlayerState
    BattleUpdate
    BattleReplayFrame
    BattleReplay

battle/session.rs
    BattleSession
    main_round 推进
    no_progress guard
    snapshot
    frame 构造入口
    session 状态机

battle/replay.rs
    battle_replay()
    仅负责 collect BattleSession

replay_view.rs
    rows / clips / parts / HP / death effect
    继续作为共享 renderer semantic 层
```

---

## 3. BattleOptions

最终：

```rust
pub struct BattleOptions {
    pub eval_rq: f64,
    pub include_icons: bool,
    pub max_rounds: usize,
}
```

默认：

```text
eval_rq       = DEFAULT_EVAL_RQ
include_icons = false
max_rounds    = BINDING_COMPLETION_MAX_ROUNDS
              = 20_000
```

Rust 保留：

```rust
#[deprecated(note = "use BattleOptions")]
pub type BattleReplayOptions = BattleOptions;
```

---

## 4. max_rounds

`max_rounds` 的唯一语义：

> Runtime `main_round()` 的总调用次数。

每次：

```rust
runner.main_round();
```

都执行：

```rust
rounds_advanced += 1;
```

空 round 同样计数。

彻底删除：

```rust
frames.len() < options.max_rounds
```

这种以可见 frame 数代表 runtime round 数的行为。

---

## 5. BattleStatus

```rust
pub enum BattleStatus {
    Running,
    Finished,
    Truncated,
}
```

序列化：

```text
running
finished
truncated
```

---

## 6. BattleStopReason

```rust
pub enum BattleStopReason {
    Winner,
    MaxRounds,
    NoProgress,
}
```

序列化：

```text
winner
max_rounds
no_progress
```

不向 User API 暴露 `idle_guard` 术语。

---

## 7. BattleResult

```rust
pub struct BattleResult {
    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,

    pub finished: bool,
    pub truncated: bool,

    pub rounds_advanced: usize,
    pub frames_emitted: usize,

    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,

    pub final_states: Vec<BattlePlayerState>,
}
```

不变量：

```text
result 永远只存在于 terminal session

status = finished
    finished = true
    truncated = false
    stop_reason = winner

status = truncated
    finished = false
    truncated = true
    stop_reason = max_rounds | no_progress
    winner_ids = []
    winner_team_indices = []
```

---

## 8. BattleReplayFrame

最终：

```rust
pub struct BattleReplayFrame {
    pub frame_index: usize,
    pub round_index: usize,

    pub finished: bool,
    pub winner_ids: Vec<usize>,

    pub updates: Vec<BattleUpdate>,
    pub rows: Vec<BattleReplayRow>,
    pub states: Vec<BattlePlayerState>,

    pub total_delay: i32,
}
```

索引全部 0-based。

```text
frame_index
    仅统计用户可见 frame

round_index
    统计真实 Runtime main round
```

示例：

```text
round 0 -> empty
round 1 -> empty
round 2 -> visible

frame_index = 0
round_index = 2
```

---

## 9. BattlePlayerState

本轮不做 state diff，不拆 meta/state。

保持完整 canonical DTO：

```text
id
team_index
input_team_index
owner_id
source_id
id_name
id_key_name
icon_key
display_name
display_index
base_name
player_type
minion_kind
icon_png_base64
hp
max_hp
magic_point
move_point
attack
defense
speed
agility
magic
resistance
wisdom
point
all_sum
name_factor
at_boost
attract
frozen
alive
active
status_labels
```

未来任何消费者都基于同一个 canonical state 流。

---

## 10. BattleSession

最终公开 Rust API：

```rust
pub struct BattleSession { ... }

impl BattleSession {
    pub fn new(
        raw: &str,
        options: BattleOptions,
    ) -> CliApiResult<Self>;

    pub fn initial_states(
        &self,
    ) -> &[BattlePlayerState];

    pub fn current_states(
        &self,
    ) -> &[BattlePlayerState];

    pub fn next_frame(
        &mut self,
    ) -> CliApiResult<Option<BattleReplayFrame>>;

    pub fn status(
        &self,
    ) -> BattleStatus;

    pub fn stop_reason(
        &self,
    ) -> Option<BattleStopReason>;

    pub fn is_done(
        &self,
    ) -> bool;

    pub fn is_finished(
        &self,
    ) -> bool;

    pub fn is_truncated(
        &self,
    ) -> bool;

    pub fn rounds_advanced(
        &self,
    ) -> usize;

    pub fn frames_emitted(
        &self,
    ) -> usize;

    pub fn result(
        &self,
    ) -> Option<BattleResult>;
}
```

---

## 11. next_frame

唯一语义：

> 推进 Runtime，直到产生下一个用户可见 frame，或者 session terminal。

调用方不处理：

```text
空 update
max_rounds
no_progress
winner frame
replay view 构造
```

---

## 12. terminal 优先级

固定：

```text
1. Runtime error
2. winner
3. max_rounds
4. no_progress
```

如果最后一个允许 round 同时产生 winner：

```text
winner 优先
```

---

## 13. no_progress

统一：

```rust
const NO_PROGRESS_ROUNDS_PER_ENTITY: usize = 16;
```

limit：

```text
max(1, current entity count) * 16
```

动态实体数量变化后重新计算。

触发：

```text
status = truncated
stop_reason = no_progress
```

所有外层不允许再实现自己的 guard。

---

## 14. winner on empty update

如果某次 `main_round()`：

```text
updates = []
winner = true
```

仍生成 terminal frame：

```text
updates = []
states = final states
finished = true
winner_ids = [...]
rows 含 winner row
```

保证 UI 不漏结束事件。

---

## 15. terminal 幂等性

terminal 后无限次：

```rust
session.next_frame()
```

都：

```rust
Ok(None)
```

不报错。

---

## 16. result

运行中：

```text
result() = None
```

terminal：

```text
result() = Some(BattleResult)
```

---

## 17. battle_replay

必须完全改为：

```text
BattleSession collect
```

禁止再直接构造 Runner loop。

硬 contract：

```text
collect(BattleSession)
==
battle_replay.frames
```

---

## 18. BattleReplay

保留：

```text
finished
truncated
initial_states
frames
final_states
winner_ids
winner_team_indices
state_granularity
```

新增：

```text
status
stop_reason
rounds_advanced
frames_emitted
```

`state_granularity` 固定：

```text
round
```

Python 旧 `tick` 类型残留删除/隔离。

---

## 19. Error model

Core 是稳定错误码唯一来源。

```rust
pub enum CliApiErrorCode {
    InvalidInput,
    InvalidArgument,
    UnsupportedOption,
    RunnerInitFailed,
    RuntimeFailed,
    InternalError,
}
```

字符串：

```text
INVALID_INPUT
INVALID_ARGUMENT
UNSUPPORTED_OPTION
RUNNER_INIT_FAILED
RUNTIME_FAILED
INTERNAL_ERROR
```

`CliApiError`：

```rust
pub enum CliApiError {
    InvalidInput(String),
    InvalidArgument(String),
    UnsupportedOption(String),
    RunnerInit(String),
    Runtime(String),
    Internal(String),
}
```

绑定层只转译，不再自行发明 code。

---

## 20. Python

新增：

```text
_types_battle.pyi
```

DTO 使用 TypedDict。

`BattleSession` 使用 PyClass。

支持：

```python
session = tswn_py.BattleSession(raw)

for frame in session:
    render(frame)

result = session.result()
```

`battle_replay()` 返回类型明确为：

```python
BattleReplay
```

不再是：

```python
dict[str, object]
```

---

## 21. WASM

正式导出：

```text
BattleSession
```

方法：

```text
initial_states()
current_states()
next_frame()
status()
stop_reason()
is_done()
is_finished()
is_truncated()
rounds_advanced()
frames_emitted()
result()
```

DTO 通过：

```text
core DTO
-> serde_wasm_bindgen
-> plain JS object
```

不复制第二套 BattleFrame / BattleState Rust schema。

---

## 22. WASM 旧 FightSession

保留为 Advanced / Compatibility。

但内部改成：

```text
core BattleSession wrapper
```

禁止旧 FightSession 自己：

```text
持有 RuntimeRunner
实现 main_round loop
实现 guard
```

---

## 23. C API

新增 opaque：

```c
typedef struct tswn_battle_session_t
    tswn_battle_session_t;
```

版本化 options：

```c
typedef struct tswn_battle_options_t {
    uint32_t struct_size;
    double eval_rq;
    size_t max_rounds;
    uint8_t include_icons;
} tswn_battle_options_t;
```

默认初始化：

```c
void tswn_battle_options_default(
    tswn_battle_options_t* options
);
```

创建：

```c
tswn_status_t tswn_battle_session_new(
    const char* raw_text_utf8,
    const tswn_battle_options_t* options,
    tswn_battle_session_t** out_session
);
```

frame：

```c
tswn_status_t tswn_battle_session_next_frame_json(
    tswn_battle_session_t* session,
    uint8_t* out_has_frame,
    tswn_str_t* out_json
);
```

result：

```c
tswn_status_t tswn_battle_session_result_json(
    const tswn_battle_session_t* session,
    uint8_t* out_has_result,
    tswn_str_t* out_json
);
```

C ABI 保持 4。

---

## 24. CLI 清理

最终命令：

```text
tswn-cli fight
tswn-cli fight --jsonl

tswn-cli runtime diff
tswn-cli runtime normalized-run

tswn-cli bench ...
```

删除：

```text
tswn-cli raw
tswn-cli diff
fight --out-raw
!test! magic routing
```

删除对应：

```text
FightRawCommand
ParsedCommand::FightRaw
raw_bench.rs
RawRoute
collect_runtime_fight_raw_lines
legacy raw formatter
```

---

## 25. CLI fight --jsonl

输出只包含：

```text
initial
frame
result
```

每条一行 JSON。

每行后立即：

```rust
stdout.flush()
```

成功 stdout 不输出 error event。

错误：

```text
stderr
非零退出码
```

不打印 banner。

---

## 26. Web 本轮正式纳入交付范围

网页 streaming 不是“下一阶段”。

它属于本轮 Definition of Done。

目标：

```text
点击开始
    ↓
加载/复用 WASM
    ↓
创建 BattleSession
    ↓
立即取得 initial_states
    ↓
立即渲染双方
    ↓
按需 next_frame()
    ↓
边计算边播放
```

页面禁止再在开始时调用：

```text
battle_replay()
```

生成完整 replay。

---

## 27. Web 文件职责重构

最终文件职责：

```text
show-wasm.js
    WASM 加载
    createBattleStreamSource()
    icon loader
    WASM session dispose
    不再构造完整 replay

show-stream.js
    BattleStreamController
    帧历史
    2-frame buffer
    source lifecycle
    stream observers
    stream metrics hooks

show.js
    页面状态
    playback cursor
    pause/resume
    step/seek
    DOM orchestration

show-render.js
    纯渲染
    不主动拉 frame

show-replay.js
    replay/playback 辅助
    result summary
    delay helper

show-utils.js
    通用纯函数

show-page-contract.test.mjs
    页面入口 contract

show-wasm.test.mjs
    WASM adapter contract

show-stream.test.mjs
    streaming controller contract
```

---

## 28. 删除 show-wasm.js 的历史 adapter

页面切换后，删除用于完整 normalized/eager replay 的旧 adapter 代码。

包括不再被使用的：

```text
buildMainNormalizedReplay
buildMainReplayFromNormalizedRun
buildMainFrame
runtimeRowsFromUpdates
runtimeClipFromUpdate
runtimePartsFromMessage
inferMainRoundStartStates
winnerIdsFromOutcome
```

以及其他只为“前端重新推断 replay view”存在的 helper。

网页不再：

```text
从 runtime frame 猜 rows/clips/HP
```

这些语义已经由 core `BattleReplayFrame.rows` 提供。

---

## 29. BattleStreamSource

`show-wasm.js` 新增正式内部接口：

```js
export async function createBattleStreamSource(
  rawInput,
  versionInfo,
  coreVersionInfo,
  modulePathInfo,
  options = {},
)
```

返回对象：

```js
{
  raw_input,
  seed_line,
  players,
  initial_states,

  async nextFrame(),
  result(),
  isDone(),
  dispose(),
}
```

---

### 29.1 nextFrame 返回 Promise

即使当前 WASM：

```text
BattleSession.next_frame()
```

是同步函数，

页面 adapter 仍统一暴露：

```js
await source.nextFrame()
```

接口。

当前实现内部同步调用 WASM，再包装为 Promise。

这样未来：

```text
Worker
其他异步 source
```

可以替换 source，而不用重写 playback/controller。

本轮不实现 Web Worker。

---

## 30. WASM session 生命周期

每一个 source 必须持有唯一 WASM `BattleSession`。

以下时机必须调用：

```js
session.free()
```

或对应 wasm-bindgen 释放方法：

```text
session terminal 且 result 已复制到 JS
用户启动新战斗
当前战斗被 abort
页面清理 source
发生不可恢复 streaming error
```

不得依赖浏览器 GC 猜测 Rust/WASM handle 生命周期。

---

## 31. Web canonical data 原则

页面保存两层数据：

```text
规范流数据
display derived data
```

canonical：

```text
initial_states
received_frames[]
result
```

直接来自 WASM，不被昵称/UI 修改。

display：

```text
nickname
icon_class_id
DOM chunks
playback checkpoints
```

只在 render / plan 层派生。

这条是硬规则。

禁止再：

```text
修改 replay canonical frame 内的 player part
```

来持久化昵称。

---

## 32. StreamingBattle 数据结构

`show.js` 使用：

```js
currentBattle = {
  raw_input,
  seed_line,

  players,
  initial_states,

  frames: [],
  result: null,

  source_done: false,
}
```

`frames`：

```text
只追加，不修改 canonical 内容
```

---

## 33. BattleStreamController

新建：

```text
show-stream.js
```

核心：

```js
export class BattleStreamController {
  constructor(source, options)

  initialStates()
  frames()
  result()
  isSourceDone()

  async pullOne()
  async ensureBuffered()
  async ensureFrame(index)

  subscribe(listener)

  dispose()
}
```

---

## 34. Stream observer

Controller 支持内部订阅：

```js
const unsubscribe =
  controller.subscribe((event) => {
    ...
  });
```

事件：

```text
initial
frame
result
error
disposed
```

这不是 tswn 公共 API。

它是网页内部扩展点。

当前用途：

```text
metrics
debug
UI 状态同步
```

未来其他功能可订阅 canonical stream，而无需改 playback 核心。

本轮不加入任何模型 observer。

---

## 35. Buffer

固定：

```text
STREAM_BUFFER_FRAMES = 2
```

普通播放时：

```text
正在播放 frame N
最多预取到 N+2
```

不允许自动把整场拉完。

---

## 36. 初始启动策略

`startBattle()`：

```text
1. validate input
2. ensureApi
3. createBattleStreamSource
4. 得到 initial_states
5. 立即关闭输入面板
6. 立即渲染 initial states
7. 初始化 controller
8. pull 第一帧
9. 开始 playback
10. 后台仅补到 2-frame buffer
```

开始按钮 loading 只持续到：

```text
session 已创建
initial state 已可渲染
```

不等待整场对局完成。

---

## 37. 首帧体验

定义两个正式 UX 指标：

```text
TTIS
Time To Initial State

TTFE
Time To First Event
```

TTIS：

```text
用户点击开始
-> initial states 完成 DOM 渲染
```

TTFE：

```text
用户点击开始
-> 第一条可见 battle clip 插入 DOM
```

网页性能基线必须记录这两个值。

---

## 38. 回放计划改为仅追加

当前：

```text
prepareReplayPlan(replay)
```

依赖整场 `replay.frames`。

重构为：

```js
createReplayPlan(initialStates)

appendFrameToReplayPlan(
  plan,
  frame,
  previousStates,
  playersById,
)

markReplayPlanComplete(
  plan,
  result,
)
```

`currentPlan` 允许：

```text
随着 frame 到达持续增长
```

不需要提前知道总帧数。

---

## 39. Frame plan

每个收到的 frame 立刻生成：

```js
{
  frameIndex,
  frame,
  previousStates,
  start,
  end,
  chunks,
}
```

追加到：

```text
plan.frames
plan.flatChunks
```

已有 chunk renderer 尽量复用。

---

## 40. playbackCursor

`playbackCursor` 仍然：

```text
指向下一个要播放的 chunk
```

但：

```text
currentPlan.totalChunks
```

现在只代表：

> 当前已接收 frame 的已知 chunk 总数。

不代表整场最终总数。

---

## 41. forward 行为

如果：

```text
playbackCursor == currentPlan.totalChunks
source_done == false
```

则：

```text
不是播放结束
```

而是：

```text
需要 pull 下一 frame
```

自动播放：

```text
拉取 -> 追加计划 -> 继续
```

单步向前：

```text
pull exactly one frame if needed
-> 前进到下一个 visible chunk
```

按 frame 前进：

```text
pull exactly one frame if needed
-> 前进到该 frame end
```

---

## 42. backward / seek

后退只在：

```text
已接收历史
```

中移动。

不回滚 WASM Runtime。

不重新执行旧 frame。

使用已有：

```text
playback checkpoints
```

策略。

Checkpoint interval 保持：

```text
20 frames
```

历史 frame 全部保存在 JS，因此任意已发生位置可回看。

---

## 43. 从历史位置恢复播放

用户回退到旧位置后点击继续：

```text
先重放已接收历史
```

直到：

```text
cursor == 已接收尾部
```

再继续：

```text
pull 新 frame
```

禁止因为用户 seek backward 而创建第二个 Runtime session。

---

## 44. Pause

按暂停：

```text
停止 autoplay loop
```

允许：

```text
当前已发起的一次 pull 完成
```

但 pause 后 controller 不继续补 buffer。

buffer 最大仍为 2。

---

## 45. Normal / Fast

Normal：

```text
使用 core clip delay
```

Fast：

```text
继续使用现有压缩 delay 规则
```

frame source pull 不改变 delay。

计算和播放彻底分离。

---

## 46. 极速模式

极速模式：

```text
clip delay = 0
```

不断：

```text
pull frame
追加计划
render
```

但保持：

```text
TURBO_YIELD_VISIBLE_CHUNKS = 24
```

每处理 24 个可见 chunk：

```js
await sleep(0)
```

让出主线程。

Turbo 不允许一次同步拉完整场后再 render。

---

## 47. Result 展示

只有同时满足：

```text
source_done == true
playbackCursor >= currentPlan.totalChunks
```

才进入：

```text
playbackFinished = true
```

然后：

```text
normal
    等待 1500 ms
    展示 result

快进/极速/单步
    立即展示 result
```

保持当前结算体验。

---

## 48. 重播

当前完整 session 已结束后：

```text
Replay / Refresh
```

只重播：

```text
currentBattle.frames
```

不重新创建 BattleSession。

因此：

```text
同一次战斗 replay
```

完全稳定。

要重新计算新一局：

```text
用户重新点击开始/重新提交输入
```

创建新的 source/session。

---

## 49. 分享

分享 URL 继续只保存：

```text
raw input
```

不保存 runtime frame history。

打开分享链接：

```text
创建新 BattleSession
实时运行
```

显式 seed 输入继续保证可复现。

---

## 50. 昵称处理

昵称映射仍保存在 localStorage。

但不再 mutate canonical frame。

渲染时：

```text
规范玩家 id/name
    ↓
nickname lookup
    ↓
display token
```

用户修改昵称：

```text
1. 更新 nickname map
2. 清空 display checkpoints
3. 重建当前 cursor 之前的 display plan
4. canonical frames 不变
```

---

## 51. 图标处理

stream session 使用：

```text
include_icons = false
```

新增网页 icon cache：

```js
Map<icon_key, icon_class_id>
Map<icon_key, base64>
```

第一次遇到：

```text
新 root player
新 summon/minion
```

时：

```text
name_to_png_base64(icon_key)
```

只调用一次。

随后注入 CSS sprite。

---

## 52. 动态实体

每个 frame `states` 中发现未知 `id`：

```text
根据 state 生成 display player metadata
```

并：

```text
注册 playersById
确保 icon_key 已缓存
更新左侧布局
```

继续支持：

```text
clone
summon
shadow
zombie
```

---

## 53. Renderer contract

`show-render.js` 继续只消费：

```text
frame.rows[].clips[]
```

正文不得：

```text
从 message_template 猜展示语义
从 hp_delta 猜 HP 条
从 update tone 重新推断死亡
```

HP / death / sidebar：

```text
全部以 core replay view 为准
```

---

## 54. 结算统计

`show-replay.js` 的：

```text
score
kills
killed_by
winner summary
```

继续基于已接收 canonical frames 计算。

因为 result 展示时：

```text
source_done = true
```

此时全部 frame 已接收，统计语义与完整 replay 一致。

---

## 55. Streaming error

如果 `source.nextFrame()` 在中途报错：

```text
1. pause playback
2. mark controller failed
3. dispose WASM session
4. 保留已显示历史
5. header/status 显示错误
6. 输入面板可重新打开
```

不把错误伪装为：

```text
truncated result
```

Runtime API error 与正常 stop reason 分离。

---

## 56. Abort / 新战斗

用户在旧战斗尚未结束时启动新战斗：

```text
1. stop old playback token
2. controller.dispose()
3. old source.dispose()
4. 清空 buffer/history/checkpoints
5. 创建新 source
```

禁止旧异步 pull 完成后把 frame 写进新 battle。

为此每场 battle 使用独立：

```text
battleGenerationToken
```

旧 generation 的异步结果直接丢弃。

---

## 57. Web metrics

新建轻量 timing collector。

记录：

```text
wasm_load_ms
session_create_ms

ttis_ms
ttfe_ms

frame_pull_count
frame_pull_total_ms
frame_pull_max_ms
frame_pull_p50_ms
frame_pull_p95_ms

render_chunk_count
render_chunk_total_ms
render_chunk_p95_ms

frames_received
chunks_rendered

battle_source_total_ms
```

不记录用户原始输入内容。

---

## 58. Metrics 使用方式

默认不在正常 UI 显示。

支持：

```text
?perf=1
```

时：

```text
console.table(metrics)
```

并在战斗结束后打印完整指标。

不增加生产页面常驻性能面板。

---

## 59. Web 性能基线

建立：

```text
docs/perf/reports/web-streaming-baseline.md
```

固定四类 fixture：

```text
1v1
2v2
ffa_8
3v3v3
```

每类：

```text
20 次
```

记录：

```text
TTIS p50 / p95
TTFE p50 / p95
next_frame p50 / p95 / max
render chunk p50 / p95
总 frame 数
总 chunk 数
```

---

## 60. 性能验收条件

在项目用于基线测试的桌面浏览器环境中：

```text
next_frame WASM + DTO p95
必须 < 16.7 ms
```

如果超过：

```text
本轮不得直接忽略
必须 profile 并记录原因
```

此外结构性验收：

```text
startBattle 在开始播放前最多预取 2 个 frame
不得调用完整 battle_replay
不得构造整场 replay 后才显示 initial
```

---

## 61. 未来扩展准备边界

本轮会准备好：

```text
稳定的规范 BattlePlayerState
稳定 BattleReplayFrame
frame_index
round_index
BattleStreamController
subscribe(event)
仅追加的规范历史
stream metrics
async-compatible source interface
```

本轮明确不做：

```text
ModelState
特征向量
胜率 label
模型训练
模型权重
模型推理
模型概率字段
模型 UI
Monte Carlo
```

以后任何新功能只需要：

```text
订阅规范 initial/frame/result
```

即可扩展，不需要再次修改播放/Runtime 基础架构。

---

## 62. Web 测试

### 62.1 show-wasm.test.mjs

必须覆盖：

```text
createBattleStreamSource
使用 BattleSession
不调用 battle_replay
initial_states 正确
nextFrame terminal -> null
result 正确
dispose 调用 WASM free
中途 error 正确传播
```

---

### 62.2 show-stream.test.mjs

使用 fake source。

覆盖：

```text
initial event
pullOne
2-frame buffer 上限
不会 eager pull 完整 source
result event
subscribe 顺序
unsubscribe
pause 后不继续 prefetch
ensureFrame 按需拉取
dispose
中途 error
generation isolation
```

---

### 62.3 仅追加计划测试

覆盖：

```text
追加第 0 帧
追加第 1 帧
chunk start/end 连续
previousStates 正确
totalChunks 单调增加
已有 chunk 不被修改
```

---

### 62.4 播放测试

覆盖：

```text
normal
fast
turbo
pause
resume
step event forward
step frame forward
back event
back frame
seek checkpoint
历史回放后继续 live source
```

---

### 62.5 昵称测试

验证：

```text
改昵称后显示改变
canonical frame 不变
旧历史可重新渲染
未来 frame 自动用新昵称
```

---

### 62.6 图标测试

验证：

```text
相同 icon_key 只生成一次
动态实体首次出现才生成 icon
dispose/restart 不污染错误 battle state
```

---

### 62.7 页面 contract

`show-page-contract.test.mjs` 必须断言：

```text
页面不引用 buildMainNormalizedReplay
页面不在 startBattle 调 battle_replay
页面使用 createBattleStreamSource
页面使用 BattleStreamController
```

---

## 63. Core 测试矩阵

覆盖：

```text
1v1
2v2
FFA4
FFA6
FFA8
3v3v3

clone
summon
shadow
zombie
revive
protect
charm

显式 seed
动态实体
winner normal frame
winner empty-update round
max_rounds
no_progress
terminal repeated next_frame
```

---

## 64. Cross-binding parity

相同：

```text
raw
seed
BattleOptions
```

Rust / Python / WASM / C 必须得到逻辑等价：

```text
initial_states
frames
frame_index
round_index
states
rows
clips
parts
status
stop_reason
winner
rounds_advanced
frames_emitted
final_states
```

---

## 65. CLI JSONL 与 Web 的关系

CLI JSONL：

```text
initial
frame
result
```

Web source observer：

```text
initial
frame
result
```

两者使用同一个概念序列。

不要求字节 JSON shape 完全相同包装层，但内部 payload：

```text
BattlePlayerState
BattleReplayFrame
BattleResult
```

完全一致。

---

## 66. 分块提交顺序

以下 commit 顺序是本轮固定实施顺序。

每个 commit 完成后必须立即提交，不得跨块堆积。

---

### 提交 1

```text
core：添加规范战斗 DTO 和错误码
```

内容：

```text
cli_api/battle 模块骨架
BattleOptions
BattleStatus
BattleStopReason
BattleResult
frame_index
round_index
CliApiErrorCode
CliApiError variants
```

测试：

```text
cargo test -p tswn_core cli_api
cargo clippy -p tswn_core
```

---

### 提交 2

```text
core：添加规范 BattleSession 状态机
```

内容：

```text
BattleSession
max_rounds
no_progress
terminal precedence
next_frame
result
icon cache
```

测试：

```text
BattleSession 状态机测试
cargo test -p tswn_core
```

---

### 提交 3

```text
core: make battle_replay collect BattleSession
```

内容：

```text
删除 battle_replay 独立 loop
one-shot collector
session/replay parity
```

测试：

```text
全部回放测试
固定 battle fixture parity
```

---

### 提交 4

```text
py：暴露 BattleSession 和带类型的战斗 DTO
```

内容：

```text
PyClass
iterator
_types_battle.pyi
battle_replay precise return type
error code
```

测试：

```text
cargo test -p tswn_py
verify_py_cli_api.py
```

---

### 提交 5

```text
wasm：暴露规范 BattleSession
```

内容：

```text
wasm_bindgen BattleSession
serde_wasm_bindgen DTO
options adapter
JS/TS shape
```

测试：

```text
cargo test -p tswn_wasm
wasm/node API 测试
```

---

### 提交 6

```text
wasm: route legacy FightSession through BattleSession
```

内容：

```text
FightSession wrapper
fight/fight_summary wrapper
删除旧独立 Runner loop
```

测试：

```text
旧版兼容性测试
规范一致性测试
```

---

### 提交 7

```text
capi：添加 BattleSession 流式 API
```

内容：

```text
versioned options
opaque handle
status/reason
states/frame/result JSON
error code
examples
```

测试：

```text
cargo test -p tswn_capi
C examples compile/run
ABI remains 4
```

---

### 提交 8

```text
cli: remove legacy raw fight routing
```

内容：

```text
删除 raw
删除 !test!
删除 --out-raw
删除 raw_bench.rs
删除 legacy raw formatter
```

测试：

```text
clap 测试
CLI compile
确认旧入口不存在
```

---

### 提交 9

```text
cli: move diff under runtime diagnostics
```

内容：

```text
diff -> runtime diff
保留 diagnostic formatter
更新 ParsedCommand
更新 help
```

测试：

```text
runtime diff fixtures
CLI 帮助测试
```

---

### 提交 10

```text
cli：添加由 BattleSession 支持的 fight jsonl
```

内容：

```text
human fight -> BattleSession
fight --jsonl
fight --max-rounds
initial/frame/result
stdout flush
```

测试：

```text
human fight
jsonl parse
no banner
frame parity
```

---

### 提交 11

```text
web：添加 BattleStreamSource wasm 适配器
```

内容：

```text
createBattleStreamSource
BattleSession handle
async nextFrame interface
result
dispose
initial metadata
```

此 commit 页面尚不切换。

测试：

```text
show-wasm.test.mjs
伪实现/适配器测试
```

---

### 提交 12

```text
web：添加 BattleStreamController 和有界预取
```

内容：

```text
show-stream.js
历史
2-frame buffer
observer
generation
dispose
error
```

页面尚可继续旧路径。

测试：

```text
show-stream.test.mjs
```

---

### 提交 13

```text
web：使回放计划仅追加
```

内容：

```text
createReplayPlan
appendFrameToReplayPlan
mark complete
动态 totalChunks
```

先让旧完整 replay 也能通过 append API 一次性喂入，保持页面行为不变。

测试：

```text
现有播放测试
新增追加计划测试
```

---

### 提交 14

```text
web: switch battle startup to live BattleSession
```

内容：

```text
startBattle
initial immediate render
BattleStreamSource
BattleStreamController
live frame pull
```

从这一 commit 开始，页面不再调用完整 `battle_replay()`。

测试：

```text
show-page-contract
startup
first frame
terminal
```

---

### 提交 15

```text
web：在流式历史中保持暂停、单步和跳转
```

内容：

```text
pause/resume
step event
step frame
back
checkpoint
历史后恢复实时流
```

测试：

```text
播放导航测试
```

---

### 提交 16

```text
web：添加有界极速流式传输和源生命周期
```

内容：

```text
极速实时拉取
24 chunk yield
buffer <= 2
abort/new battle
WASM free
generation token
```

测试：

```text
turbo
dispose
abort
no stale frame
```

---

### 提交 17

```text
web：将规范战斗数据与显示装饰分离
```

内容：

```text
nickname render-time mapping
按策略保持规范帧不可变
lazy icon cache
dynamic entity icon registration
```

测试：

```text
nickname
规范数据
icon dedup
dynamic entity
```

---

### 提交 18

```text
web: remove eager replay adapters
```

内容：

```text
删除 show-wasm.js 历史 normalized/eager adapter
删除前端 replay semantic inference helper
清理死代码
```

测试：

```text
node 测试
page contract
无 buildMainNormalizedReplay 引用
无 battle_replay page call
```

---

### 提交 19

```text
web：添加流式性能埋点
```

内容：

```text
TTIS
TTFE
frame pull metrics
render metrics
?perf=1
```

测试：

```text
指标单元测试
不记录 raw input
```

---

### 提交 20

```text
docs：记录 web 流式基线和公开 API
```

内容：

```text
web-streaming-baseline.md
public-api.md
Python README
WASM README
C README
CLI help examples
CHANGELOG
```

测试：

```text
docs examples smoke
全仓最终 test
```

---

## 67. 每个 commit 的工作流

每块固定执行：

```text
1. 修改当前块
2. rustfmt / JS format 仅限触及文件
3. 定向测试
4. clippy / 类型/存根测试
5. git diff 检查只含当前职责
6. git status 确认无意外文件
7. commit
8. 再开始下一块
```

不得：

```text
“这块先不测，后面一起测”
```

---

## 68. 最终回归

Commit 20 前必须执行：

```text
cargo test -p tswn_core
cargo test -p tswn_py
cargo test -p tswn_wasm
cargo test -p tswn_capi

cargo clippy \
  -p tswn_core \
  -p tswn_py \
  -p tswn_wasm \
  -p tswn_capi

Python API 验证

WASM Node 测试

show-wasm.test.mjs
show-stream.test.mjs
show-page-contract.test.mjs
show-routing.test.mjs
其他现有 show tests

CLI 契约测试
```

---

## 69. 最终完成定义

全部满足才结束本轮。

### Core/API

- [x] BattleSession 是正式 User API
- [x] battle_replay 只 collect BattleSession
- [x] max_rounds 真实统计 main_round
- [x] no_progress 只在 core 一处实现
- [x] winner empty-update frame 不丢
- [x] frame_index / round_index 稳定
- [x] terminal next_frame 幂等
- [x] result 仅 terminal 后存在
- [x] error code 由 core 统一

### Bindings

- [x] Python BattleSession
- [x] Python iterator
- [x] TypedDict battle DTO
- [x] WASM BattleSession
- [x] WASM plain JS DTO
- [x] FightSession 内部改用规范 session
- [x] C BattleSession
- [x] C versioned options
- [x] C ABI 仍为 4
- [x] Cross-binding parity

### CLI

- [x] 删除 raw
- [x] 删除 !test! magic
- [x] 删除 --out-raw
- [x] diff 移到 runtime diff
- [x] human fight 使用 BattleSession
- [x] fight --jsonl
- [x] JSONL 每行立即 flush
- [x] JSONL stdout 无 banner/error

### Web

- [x] 页面不调用完整 battle_replay
- [x] 点击开始后立即渲染 initial
- [x] 页面逐 frame 拉 Runtime
- [x] buffer 最大 2
- [x] 规范历史仅追加
- [x] pause 正常
- [x] resume 正常
- [x] forward event 正常
- [x] forward frame 正常
- [x] backward 正常
- [x] checkpoint seek 正常
- [x] 历史回放后可继续 live
- [x] 极速模式不急切运行整场
- [x] 极速模式每 24 个可见块让出执行权
- [x] result 展示逻辑保持
- [x] 昵称不修改规范帧
- [x] icon_key 懒加载/去重
- [x] 动态实体正常
- [x] source/session 正确 free
- [x] abort 不泄漏旧 frame
- [x] streaming error 不伪装 truncated
- [x] eager replay adapter 全部删除
- [x] TTIS / TTFE 可测
- [x] next_frame p95 基线已记录
- [x] web-streaming-baseline.md 已提交

### 范围边界

- [x] 未实现任何模型
- [x] 未定义 ModelState
- [x] 未加入胜率字段
- [x] 未加入模型 UI
- [x] 但规范流 / observer / async source / metrics 已准备完成

---

## 70. 本轮结束后的系统状态

最终：

```text
Runtime
   │
   ▼
BattleSession
   │
   ├── battle_replay()
   ├── Python
   ├── C
   ├── CLI JSONL
   └── WASM
         │
         ▼
BattleStreamSource
         │
         ▼
BattleStreamController
         │
   ┌─────┴──────┐
   ▼            ▼
历史          播放
                │
                ▼
             Renderer
```

到这里，网页已经是真正的实时增量对局，而不是“整场算完再伪装播放”。

同时后续任何新功能都可以只消费：

```text
initial
frame
result
```

稳定流，而不需要再次修改 Runtime、回放语义、播放控制或跨语言 API。


## 实施验收记录（2026-09-08）

保留分块提交：Core 1–3、Python 4、WASM 5–6、C 7、CLI 8–10、Web 11–19、文档 20。最终回归发现的页面边界、Python 测试导入和 WASM 错误码引用各自独立修复提交；未 squash。

| 规格范围 | 实现/证据 | 验证结果 |
| --- | --- | --- |
| 0–19：统一状态机、DTO、计数、终止、错误 | `crates/tswn_core/src/cli_api/battle/{dto,session,replay}.rs`；session 测试覆盖空 round 预算、空 updates 获胜、最后预算轮获胜、no_progress、重复 terminal、图标和 sticky runtime error | core 测试通过；collector 与 session 精确一致 |
| 20：Python | `src/battle.rs`、`_types_battle.pyi`、`verify_py_cli_api.py` | 实际扩展的迭代器、全部 DTO keys、结果、ownership 与错误码验证通过 |
| 21–22：WASM | `src/battle.rs`、`battle_types.d.ts`、兼容 FightSession | 原生测试、wasm32 build、真实 Node 包 7 项测试通过 |
| 23：C | `src/battle_api.rs`、头文件、`examples/battle_session.c` | ABI 4、版本化 options、NULL/短结构/未来尾部/输出所有权、C 示例编译运行通过 |
| 24–25：CLI | fight driver、runtime 子命令、`verify_cli_battle.py` | human / JSONL / stdin / 非零错误退出 / 已删除路由验证通过；writer 测试验证逐行 flush |
| 26–39：网页 source / controller / plan | `show-wasm.js`、`show-stream.js`、`show-replay.js` | 不调用 eager replay；首屏先于 pull；2 帧 buffer、observer 顺序、追加区间、terminal/dispose 测试通过 |
| 40–49：播放、导航、结算、重播、分享 | `show.js`、`verify_web_playback.mjs`、routing tests | normal/fast/turbo、pause/resume、单事件/单帧、回退、20 帧 checkpoint、历史续播、结算等待暂停恢复、重播不新建 source 均通过 |
| 50–54：显示层与 renderer | `show-display.js`、display tests、canonical clip tests | frozen DTO 不变、昵称重建/未来帧、icon 去重、动态实体、HP/死亡/恢复/多目标/延时验证通过；真实浏览器截图检查通过 |
| 55–56：错误与 generation | controller 和真实页面模块测试 | 中途错误保留历史不生成 result；异常后可回看；旧 pull/source 创建迟到结果隔离；显式 free |
| 57–60：metrics 与浏览器基线 | [web-streaming-baseline.md](../perf/reports/web-streaming-baseline.md)、`show-metrics.test.mjs` | 四类固定输入各 20 次；TTIS/TTFE、pull、render 分位数和计数齐全；全部 pull p95 < 16.7 ms |
| 61：扩展边界 | canonical initial/frame/result、observer、async source、timing collector | 本轮没有 ModelState、模型训练/推理/字段/UI 或 Worker |
| 62–63：测试矩阵 | 全部 show tests、页面模块 DOM tests、`tswn_test::battle_session` | frozen corpus 37 类 case；1v1/2v2/FFA4/6/8/3v3v3、四种动态实体及复活/守护/魅惑，明确断言实际覆盖 |
| 64–65：真实跨语言 payload | `verify_battle_cross_binding.py`、`dump_battle_wasm.mjs` | Rust CLI / Python / C DLL / WASM，四 fixture × 1/20,000 轮，全部 initial/frames/result 字段精确相等 |
| 66–68：分块与最终回归 | git 提交历史；最终测试命令 | 四主要 crate 与全 workspace cargo test、指定 clippy、Python / WASM / CLI / Web 检查通过；nightly fmt、git diff --check 通过 |
| 69–70：文档与最终架构 | public_api、各绑定 README/CHANGELOG、CLI 示例、性能表格/截图与复现脚本 | Rust/Python/WASM 文档示例实际运行，C 示例编译运行；相对链接检查通过 |

最终命令：

```text
cargo test -p tswn_core -p tswn_py -p tswn_wasm -p tswn_capi
cargo test
cargo clippy -p tswn_core -p tswn_py -p tswn_wasm -p tswn_capi
cargo +nightly fmt --check
python scripts/verify_py_cli_api.py
python scripts/verify_cli_battle.py
node --test scripts/verify_wasm_battle.test.mjs
python scripts/verify_battle_cross_binding.py
node --test crates/tswn_wasm/examples/show-*.test.mjs
node --experimental-vm-modules scripts/verify_web_playback.mjs
node scripts/benchmark_web_streaming.mjs
python scripts/verify_battle_docs.py
git diff --check
```

Clippy 通过但仍有仓库已有告警（如参数数量、可简化表达式）；没有将告警当作本轮 API 失败，也未做无关批量清理。golden/corpus 基线没有重生成。
