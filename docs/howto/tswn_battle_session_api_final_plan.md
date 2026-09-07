# TSWN BattleSession 公共 API 与 CLI Streaming 最终重构计划

> 状态：FINAL / 已锁定  
> 目标仓库：`shenjackyuanjie/tswn-core`  
> 基线：`tswn_core 0.5.3`、`tswn_py 0.5.2`、`tswn_wasm 0.5.3`、`tswn_capi 0.6.1`、C ABI 4  
> 适用范围：增量对局、完整 replay、Python/WASM/C 同步 API、CLI streaming 与历史 CLI 清理  
> 不包含：实时胜率模型、模型训练、WinProbState 设计

---

# 0. 最终结论

TSWN 的对局公共 API 统一为三层：

```text
BattleSession = 有状态、增量推进的正式 User API
BattleReplay  = BattleSession 的完整收集结果
Runner        = Runtime 级 Advanced API
```

所有语言绑定与 CLI 必须以：

```rust
tswn_core::cli_api::BattleSession
```

作为唯一对局执行语义源。

最终架构：

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
       ┌────────────┼───────┐        │
       │            │       │        │
       ▼            ▼       ▼        ▼
    Python         WASM      C      one-shot API
       │            │       │
       └────────────┼───────┘
                    ▼
                Web / CLI
```

禁止再由以下路径各自实现独立的推进循环、空回合处理、guard 或 winner 语义：

```text
battle_replay
WASM FightSession
Python replay wrapper
CLI fight
C binding
```

---

# 1. 已锁定的设计决策

## 1.1 命名

正式增量对局类型：

```text
BattleSession
```

正式完整回放：

```text
BattleReplay
battle_replay()
```

正式配置：

```text
BattleOptions
```

旧：

```text
BattleReplayOptions
```

仅保留 Rust deprecated alias。

---

## 1.2 模块布局

`tswn_core::cli_api` 内建立独立 battle 模块：

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
    公共 DTO、状态枚举、配置、结果结构

battle/session.rs
    BattleSession、推进状态机、snapshot、guard

battle/replay.rs
    battle_replay() one-shot collector

replay_view.rs
    继续保留底层 UI-ready rows/clips/parts 构造逻辑
```

`cli_api/mod.rs` 统一 re-export。

---

# 2. Core 公共类型

## 2.1 BattleOptions

最终定义：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleOptions {
    pub eval_rq: f64,
    pub include_icons: bool,
    pub max_rounds: usize,
}
```

默认值：

```rust
impl Default for BattleOptions {
    fn default() -> Self {
        Self {
            eval_rq: crate::namerena::eval_name::DEFAULT_EVAL_RQ,
            include_icons: false,
            max_rounds: crate::runtime::BINDING_COMPLETION_MAX_ROUNDS,
        }
    }
}
```

默认：

```text
eval_rq       = DEFAULT_EVAL_RQ
include_icons = false
max_rounds    = 20_000
```

兼容 alias：

```rust
#[deprecated(note = "use BattleOptions")]
pub type BattleReplayOptions = BattleOptions;
```

---

## 2.2 max_rounds 的严格语义

`max_rounds` 限制 Runtime `main_round()` 的实际调用次数。

每次执行：

```rust
runner.main_round()
```

必须：

```rust
rounds_advanced += 1;
```

无论该 round 有无可见 update 都计数。禁止继续使用：

```rust
frames.len() < max_rounds
```

作为 round guard。

---

## 2.3 BattleStatus

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleStatus {
    Running,
    Finished,
    Truncated,
}
```

JSON / Python / JS 值：

```text
"running"
"finished"
"truncated"
```

语义：

```text
running   = session 仍允许继续推进
finished  = 已产生真正 winner
truncated = session 已终止，但没有 winner
```

---

## 2.4 BattleStopReason

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleStopReason {
    Winner,
    MaxRounds,
    NoProgress,
}
```

JSON：

```text
"winner"
"max_rounds"
"no_progress"
```

User API 不暴露 `idle_guard` 这一实现术语。

---

# 3. BattleResult

`result()` 仅在 session terminal 后存在，因此结果结构中不增加 `done`。

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
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
result.status != running
```

Finished：

```text
finished = true
truncated = false
stop_reason = winner
```

Truncated：

```text
finished = false
truncated = true
stop_reason ∈ {max_rounds, no_progress}
winner_ids = []
winner_team_indices = []
```

---

# 4. BattleReplayFrame

现有 UI-ready frame 字段继续保留：

```text
finished
winner_ids
updates
rows
states
total_delay
```

新增：

```rust
pub frame_index: usize;
pub round_index: usize;
```

最终结构：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
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

示例：

```text
main round 0 -> empty
main round 1 -> empty
main round 2 -> visible

frame_index = 0
round_index = 2
```

不变量：

```text
frame_index = 0, 1, 2, ...
round_index 单调递增
round_index >= frame_index
```

---

# 5. BattlePlayerState

第一版 streaming 保持当前公共 `BattlePlayerState` shape，不在本轮拆分 `EntityMeta / EntityState / EntityDiff`。

现有字段继续作为 canonical state DTO，包括：

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

所有绑定统一使用同一字段语义。

---

# 6. icon 处理

`BattleOptions::default()`：

```text
include_icons = false
```

`BattleSession` 内部维护：

```rust
HashMap<String, String>
```

形式的 `icon_key -> PNG Base64` cache。启用 icons 后，同一个 `icon_key` 在一个 session 生命周期中最多实际渲染一次。

网页 streaming 固定使用：

```text
include_icons = false
```

第一次看到新 `icon_key` 时调用已有 `name_to_png_base64(icon_key)` 并缓存 CSS sprite。

---

# 7. BattleSession

## 7.1 Core 结构

```rust
pub struct BattleSession {
    runner: RuntimeRunner,
    options: BattleOptions,

    initial_states: Vec<BattlePlayerState>,
    current_states: Vec<BattlePlayerState>,

    icon_cache: HashMap<String, String>,

    rounds_advanced: usize,
    frames_emitted: usize,
    idle_rounds: usize,

    status: BattleStatus,
    stop_reason: Option<BattleStopReason>,
}
```

外部不依赖私有字段布局。

---

## 7.2 构造

```rust
impl BattleSession {
    pub fn new(
        raw: &str,
        options: BattleOptions,
    ) -> CliApiResult<Self>;
}
```

构造步骤：

```text
1. 校验 raw
2. 校验 options
3. split namerena groups / seed
4. 构造 RuntimeRunner
5. 获取 initial_states
6. rounds_advanced = 0
7. frames_emitted = 0
8. idle_rounds = 0
9. 检查 Runtime 是否在初始状态已存在 winner
```

若初始即 winner：

```text
status = finished
stop_reason = winner
next_frame() -> None
result() -> Some(BattleResult)
```

不人为制造 round_index 不存在的 frame。

---

# 8. BattleSession 公共方法

最终 Rust API：

```rust
impl BattleSession {
    pub fn new(
        raw: &str,
        options: BattleOptions,
    ) -> CliApiResult<Self>;

    pub fn initial_states(&self) -> &[BattlePlayerState];
    pub fn current_states(&self) -> &[BattlePlayerState];

    pub fn next_frame(
        &mut self,
    ) -> CliApiResult<Option<BattleReplayFrame>>;

    pub fn status(&self) -> BattleStatus;
    pub fn stop_reason(&self) -> Option<BattleStopReason>;

    pub fn is_done(&self) -> bool;
    pub fn is_finished(&self) -> bool;
    pub fn is_truncated(&self) -> bool;

    pub fn rounds_advanced(&self) -> usize;
    pub fn frames_emitted(&self) -> usize;

    pub fn result(&self) -> Option<BattleResult>;
}
```

---

# 9. next_frame 的唯一语义

`next_frame()` 推进 Runtime，直到产生下一个用户可见 frame，或 session 进入 terminal 状态。

它内部允许消耗多个没有可见 update 的 main round。调用方不需要自行：

```text
识别空 updates
实现 idle guard
判断 max_rounds
拼 replay frame
```

---

# 10. next_frame 精确状态机

```rust
pub fn next_frame(
    &mut self,
) -> CliApiResult<Option<BattleReplayFrame>> {
    if self.is_done() {
        return Ok(None);
    }

    loop {
        if self.rounds_advanced >= self.options.max_rounds {
            self.finish_truncated(BattleStopReason::MaxRounds);
            return Ok(None);
        }

        let previous_states = self.current_states.clone();
        let updates = self.runner.main_round();
        self.rounds_advanced += 1;

        let states = self.snapshot_states();
        let winner = self.runner.have_winner();

        if updates.updates.is_empty() {
            self.current_states = states.clone();

            if winner {
                let frame = self.build_visible_frame(
                    &updates,
                    &previous_states,
                    &states,
                );
                self.frames_emitted += 1;
                self.finish_winner();
                return Ok(Some(frame));
            }

            self.idle_rounds += 1;

            if self.rounds_advanced >= self.options.max_rounds {
                self.finish_truncated(BattleStopReason::MaxRounds);
                return Ok(None);
            }

            if self.idle_rounds >= self.current_no_progress_limit() {
                self.finish_truncated(BattleStopReason::NoProgress);
                return Ok(None);
            }

            continue;
        }

        self.idle_rounds = 0;

        let frame = self.build_visible_frame(
            &updates,
            &previous_states,
            &states,
        );

        self.current_states = states;
        self.frames_emitted += 1;

        if winner {
            self.finish_winner();
        } else if self.rounds_advanced >= self.options.max_rounds {
            self.finish_truncated(BattleStopReason::MaxRounds);
        }

        return Ok(Some(frame));
    }
}
```

---

# 11. terminal 优先级

同一 main round 同时满足多个条件时固定：

```text
1. Runtime error
2. winner
3. max_rounds
4. no_progress
```

Runtime error 直接返回 `Err`，不转成 stop reason。

若最后允许的 round 同时产生 winner，则 winner 优先于 max_rounds。

---

# 12. no_progress guard

统一常量：

```rust
const NO_PROGRESS_ROUNDS_PER_ENTITY: usize = 16;
```

limit：

```rust
fn current_no_progress_limit(&self) -> usize {
    self.runner
        .all_player_ids()
        .len()
        .max(1)
        .saturating_mul(NO_PROGRESS_ROUNDS_PER_ENTITY)
}
```

每次空 main round 后重新计算。触发条件：

```text
idle_rounds >= current_no_progress_limit()
```

所有 `battle_replay / Python / WASM / C / CLI fight` 不得再次实现自己的 no-progress guard。

---

# 13. winner 发生于空 update round

若 `runner.main_round()` 返回空 updates，但 Runtime 已产生 winner，仍生成 terminal `BattleReplayFrame`。

该 frame：

```text
updates = []
finished = true
winner_ids = [...]
states = final current states
```

`build_replay_view_frame` 负责加入 winner row，UI 不漏胜者展示。

---

# 14. terminal 后 next_frame

一旦 `status != running`，以后任意次数调用：

```rust
next_frame()
```

都返回：

```rust
Ok(None)
```

幂等，不报错。

---

# 15. result()

运行中：

```rust
session.result() == None
```

terminal 后：

```rust
session.result() == Some(BattleResult)
```

调用方区分：

```text
当前状态 -> current_states()
执行状态 -> status()
最终结果 -> result()
```

---

# 16. battle_replay

完整回放不再拥有自己的 Runner loop。

```rust
pub fn battle_replay(
    raw: &str,
    options: BattleOptions,
) -> CliApiResult<BattleReplay> {
    let mut session = BattleSession::new(raw, options)?;
    let initial_states = session.initial_states().to_vec();
    let mut frames = Vec::new();

    while let Some(frame) = session.next_frame()? {
        frames.push(frame);
    }

    let result = session
        .result()
        .expect("session must be terminal");

    Ok(BattleReplay::from_session(
        initial_states,
        frames,
        result,
    ))
}
```

硬性 contract：

```text
collect(BattleSession.next_frame()) == battle_replay(...).frames
```

---

# 17. BattleReplay 最终 shape

保留现有字段：

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

```rust
pub struct BattleReplay {
    pub finished: bool,
    pub truncated: bool,

    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,

    pub rounds_advanced: usize,
    pub frames_emitted: usize,

    pub initial_states: Vec<BattlePlayerState>,
    pub frames: Vec<BattleReplayFrame>,
    pub final_states: Vec<BattlePlayerState>,

    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,

    pub state_granularity: &'static str,
}
```

`state_granularity` 固定为：

```text
"round"
```

仅作为完整 replay 兼容字段保留，不复制到 Session / Result / Frame。

Python 旧 `Literal["tick"]` 必须修正，不再描述新的 Battle API。

---

# 18. Error Model

Core 成为稳定错误码唯一来源。

## 18.1 Error code enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

## 18.2 CliApiError

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

提供：

```rust
impl CliApiError {
    pub fn code(&self) -> CliApiErrorCode;
}

impl CliApiErrorCode {
    pub const fn as_str(self) -> &'static str;
}
```

所有语言绑定使用 `error.code().as_str()`，不得再次自行构造另一套 code mapping。

---

# 19. BattleOptions 校验

固定：

```text
raw 为空
    -> INVALID_INPUT

文本输入本身不合法
    -> INVALID_INPUT

max_rounds == 0
    -> INVALID_ARGUMENT

eval_rq 为 NaN / +inf / -inf
    -> INVALID_ARGUMENT

Runner 构造失败
    -> RUNNER_INIT_FAILED

Runtime 执行失败
    -> RUNTIME_FAILED

绑定/序列化内部不变量失败
    -> INTERNAL_ERROR
```

---

# 20. Python 公共 API

## 20.1 新类型文件

新增：

```text
crates/tswn_py/tswn_py/_types_battle.pyi
```

描述：

```text
BattleStatus
BattleStopReason
BattlePlayerState
BattleUpdate
BattleReplayTextPart
BattleReplayClip
BattleReplayRow
BattleReplayFrame
BattleResult
BattleReplay
BattleSession
```

旧 `_types_replay.pyi` 只服务 Advanced / compatibility replay API。

## 20.2 类型 alias

```python
BattleStatus = Literal[
    "running",
    "finished",
    "truncated",
]

BattleStopReason = Literal[
    "winner",
    "max_rounds",
    "no_progress",
]
```

DTO 使用 TypedDict。

## 20.3 BattleSession

```python
class BattleSession:
    def __init__(
        self,
        raw: str,
        *,
        eval_rq: float | None = None,
        include_icons: bool = False,
        max_rounds: int | None = None,
    ) -> None: ...

    def initial_states(self) -> list[BattlePlayerState]: ...
    def current_states(self) -> list[BattlePlayerState]: ...
    def next_frame(self) -> BattleReplayFrame | None: ...

    def status(self) -> BattleStatus: ...
    def stop_reason(self) -> BattleStopReason | None: ...

    def is_done(self) -> bool: ...
    def is_finished(self) -> bool: ...
    def is_truncated(self) -> bool: ...

    def rounds_advanced(self) -> int: ...
    def frames_emitted(self) -> int: ...

    def result(self) -> BattleResult | None: ...

    def __iter__(self) -> BattleSession: ...
    def __next__(self) -> BattleReplayFrame: ...
```

支持：

```python
for frame in session:
    render(frame)
```

`__next__()` 遇 `next_frame() is None` 时抛 `StopIteration`。

## 20.4 battle_replay

```python
def battle_replay(
    raw: str,
    eval_rq: float | None = None,
    include_icons: bool = False,
    max_rounds: int | None = None,
) -> BattleReplay: ...
```

不再标注为 `dict[str, object]`。

---

# 21. WASM 公共 API

JS：

```js
const session = new wasm.BattleSession(rawInput, {
  eval_rq: 4.0,
  include_icons: false,
  max_rounds: 20000,
});

renderInitial(session.initial_states());

for (;;) {
  const frame = session.next_frame();
  if (frame === null) break;
  await playFrame(frame);
}

const result = session.result();
```

方法：

```text
constructor(raw_input, options?)
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

`next_frame()` 返回 `BattleReplayFrame | null`；`result()` 返回 `BattleResult | null`。

DTO 不复制为第二套 WASM Rust struct：

```text
core DTO -> serde_wasm_bindgen -> plain JS object
```

WASM 只保留输入 options adapter 处理 JS optional fields。

---

# 22. WASM 旧 FightSession

保留：

```text
FightSession
fight()
fight_summary()
```

作为 Advanced / Compatibility API，但内部必须改成 core `BattleSession` wrapper。

禁止继续：

```text
FightSession 自己持有 Runner
FightSession 自己实现 main_round loop
FightSession 自己实现 idle guard
```

---

# 23. C API

## 23.1 Opaque session

```c
typedef struct tswn_battle_session_t tswn_battle_session_t;
```

## 23.2 Versioned options struct

```c
typedef struct tswn_battle_options_t {
    uint32_t struct_size;
    double eval_rq;
    size_t max_rounds;
    uint8_t include_icons;
} tswn_battle_options_t;
```

初始化：

```c
void tswn_battle_options_default(
    tswn_battle_options_t* options
);
```

未来扩展只能 append 到尾部。

## 23.3 Status / reason

```c
typedef enum tswn_battle_status_t {
    TSWN_BATTLE_RUNNING = 0,
    TSWN_BATTLE_FINISHED = 1,
    TSWN_BATTLE_TRUNCATED = 2
} tswn_battle_status_t;

typedef enum tswn_battle_stop_reason_t {
    TSWN_BATTLE_STOP_NONE = 0,
    TSWN_BATTLE_STOP_WINNER = 1,
    TSWN_BATTLE_STOP_MAX_ROUNDS = 2,
    TSWN_BATTLE_STOP_NO_PROGRESS = 3
} tswn_battle_stop_reason_t;
```

## 23.4 创建 / 释放

```c
tswn_status_t tswn_battle_session_new(
    const char* raw_text_utf8,
    const tswn_battle_options_t* options,
    tswn_battle_session_t** out_session
);

void tswn_battle_session_free(
    tswn_battle_session_t* session
);
```

`options == NULL` 使用 `BattleOptions::default()`。

## 23.5 状态查询

```c
tswn_status_t tswn_battle_session_status(
    const tswn_battle_session_t* session,
    tswn_battle_status_t* out_status
);

tswn_status_t tswn_battle_session_stop_reason(
    const tswn_battle_session_t* session,
    tswn_battle_stop_reason_t* out_reason
);

uint8_t tswn_battle_session_is_done(
    const tswn_battle_session_t* session
);

uint8_t tswn_battle_session_is_finished(
    const tswn_battle_session_t* session
);

uint8_t tswn_battle_session_is_truncated(
    const tswn_battle_session_t* session
);

size_t tswn_battle_session_rounds_advanced(
    const tswn_battle_session_t* session
);

size_t tswn_battle_session_frames_emitted(
    const tswn_battle_session_t* session
);
```

## 23.6 states

```c
tswn_status_t tswn_battle_session_initial_states_json(
    const tswn_battle_session_t* session,
    tswn_str_t* out_json
);

tswn_status_t tswn_battle_session_current_states_json(
    const tswn_battle_session_t* session,
    tswn_str_t* out_json
);
```

## 23.7 next frame

```c
tswn_status_t tswn_battle_session_next_frame_json(
    tswn_battle_session_t* session,
    uint8_t* out_has_frame,
    tswn_str_t* out_json
);
```

有 frame：

```text
status = TSWN_OK
out_has_frame = 1
out_json = frame JSON
```

terminal：

```text
status = TSWN_OK
out_has_frame = 0
out_json.ptr = NULL
out_json.len = 0
```

## 23.8 result

```c
tswn_status_t tswn_battle_session_result_json(
    const tswn_battle_session_t* session,
    uint8_t* out_has_result,
    tswn_str_t* out_json
);
```

running：`out_has_result = 0`；terminal：`out_has_result = 1` 且返回 `BattleResult` JSON。

---

# 24. C ABI

新增 opaque handle、enum、options struct 和新函数，不修改既有 public struct layout 与既有函数签名，因此：

```text
tswn_capi_abi_version() 保持 4
```

本轮不因 additive API bump ABI。

---

# 25. CLI 最终重构

CLI 允许破坏历史兼容，不保留误导性的旧入口。

当前历史问题：

```text
fight --out-raw
    输出 legacy raw 聚合对局日志

raw
    普通输入 -> 同一个 legacy raw 日志
    !test!   -> 偷偷切换 benchmark

diff
    顶层暴露 Runtime/parity 调试输出
```

最终全部重构。

---

# 26. 最终 CLI 命令树

User battle：

```bash
tswn-cli fight
tswn-cli fight --jsonl
```

Runtime Advanced / diagnostic：

```bash
tswn-cli runtime normalized-run
tswn-cli runtime diff
```

Benchmark：

```bash
tswn-cli bench auto
tswn-cli bench win-rate
tswn-cli bench group-win-rate
tswn-cli bench batch-rate
tswn-cli bench pair
```

删除：

```bash
tswn-cli raw
tswn-cli diff
tswn-cli fight --out-raw
```

---

# 27. raw 历史入口删除

全部删除：

```text
FightRawCommand
ParsedCommand::FightRaw
raw_bench.rs
RawRoute
!test! magic header
run_raw()
```

`!test!` 不再是 CLI 控制协议。

原功能映射：

```text
旧 raw + 普通对局 -> fight
旧 raw + !test! 单组 -> bench auto
旧 raw + !test! 双组 -> bench auto / bench win-rate
```

不保留 alias。

---

# 28. --out-raw 删除

删除：

```bash
fight --out-raw
```

删除内部：

```rust
out_raw: bool
```

删除 legacy raw fight formatter：

```text
collect_runtime_fight_raw_lines
fmt_runtime_update_raw
normalize_trace_line
is_action_line
emit_current_turn
```

本轮不提供“legacy raw battle text”的新名称。其主要价值是旧 namerena/parity 对齐，不属于新的 User API。

---

# 29. diff 迁移

删除顶层：

```bash
tswn-cli diff
```

迁移为：

```bash
tswn-cli runtime diff
```

`diff` 明确归类为 Runtime/parity diagnostic。保留现有 diff trace 语义和 formatter。

---

# 30. Fight CLI 内部模型

新增：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightOutput {
    Human,
    Jsonl,
}
```

ParsedCommand：

```rust
ParsedCommand::Fight {
    raw: String,
    output: FightOutput,
    max_rounds: usize,
}
```

Clap：

```rust
struct FightCommand {
    #[command(flatten)]
    input: InputArgs,

    #[arg(long)]
    jsonl: bool,

    #[arg(
        long = "max-rounds",
        default_value_t = BINDING_COMPLETION_MAX_ROUNDS,
    )]
    max_rounds: usize,
}
```

`--jsonl` -> `FightOutput::Jsonl`，否则 `Human`。

---

# 31. fight --jsonl

`fight --jsonl` 是正式机器可读实时流。

它必须：

```text
使用 BattleSession
逐 frame 推进
每条事件一行 JSON
每行后立即 flush stdout
```

stdout 只允许三类事件：

```text
initial
frame
result
```

## 31.1 initial

```json
{"type":"initial","initial_states":[]}
```

## 31.2 frame

```json
{"type":"frame","frame":{"frame_index":0,"round_index":0}}
```

直接嵌入 canonical `BattleReplayFrame`。

## 31.3 result

```json
{"type":"result","result":{"status":"finished"}}
```

直接嵌入 canonical `BattleResult`。

---

# 32. JSONL flush

实现必须在每个 `initial / frame / result` 后调用 `flush()`，确保 pipe / subprocess 在战斗进行中就能收到 frame。

```rust
use std::io::{self, Write};

let stdout = io::stdout();
let mut stdout = stdout.lock();

writeln!(stdout, "{}", serialized)?;
stdout.flush()?;
```

---

# 33. JSONL 错误行为

成功 stdout stream 不输出：

```json
{"type":"error"}
```

错误统一：

```text
stderr
非零 exit code
```

因此 stdout 始终可直接按 JSONL 解析。

---

# 34. Banner

以下模式不打印 banner：

```text
fight --jsonl
runtime diff
runtime normalized-run
namer-pf
以及既有机器可读模式
```

普通 `tswn-cli fight` 保留用户 banner。

---

# 35. CLI human fight

普通 `tswn-cli fight` 也必须基于 `BattleSession`，不再自己持有 Runtime loop。

流程：

```text
BattleSession::new
-> 打印 initial states
-> while next_frame
-> 按 frame.updates / rows 输出 human text
-> BattleResult
-> 打印结果
```

如果 CLI 需要 `total_score / score_by_caster`，在消费 `frame.updates[].score` 时本地累计，不塞进 BattleSession 公共语义。

---

# 36. Web streaming API 基础

完成 BattleSession 后，网页改成：

```text
start
-> new BattleSession
-> initial_states()
-> 立即渲染
-> pull next_frame
-> 播放 frame clips
-> pull next_frame
-> ...
```

网页维护 `received_frames[]`，供 pause / step back / step forward / seek / replay 使用。

尚未计算的未来 frame 不存在。

---

# 37. Web 小缓冲

网页保持 1~2 个 frame 的小缓冲，不重新 eager-run 整场。

目标：Runtime 推进成本被当前 frame 的动画时间隐藏，同时保持真实 streaming。

---

# 38. Web icon

网页创建：

```js
new BattleSession(rawInput, {
  include_icons: false,
});
```

第一次看到新 `icon_key` 时调用 `name_to_png_base64(icon_key)`，进入 CSS sprite cache。召唤物按首次出现懒加载。

---

# 39. API 分层

## User API

```text
BattleSession
battle_replay
win_rate_summary
team_win_rate_summary
group_win_rate_summary
score
namer_pf
batch_rate
pair_rate
to_diy
to_diy_batch
icon_info
parse_group_lines
```

## Advanced API

```text
Runner
PreparedRunner
WinRateSession
RC4
default_custom_runtime_normalized_run
runtime diff
```

## Compatibility API

```text
WASM FightSession
fight
fight_summary
Python Runner.build_replay
旧 replay DTO
```

Compatibility API 继续存在，但内部必须复用 canonical BattleSession。

CLI 的 `raw / --out-raw / 顶层 diff` 不属于该兼容范围，直接删除或迁移。

---

# 40. Cross-binding parity contract

固定 fixture 下，相同 raw + seed + BattleOptions，Rust / Python / WASM / C 必须得到逻辑等价的：

```text
initial_states
frame_index
round_index
frames
states
rows
clips
parts
final_states
status
stop_reason
winner
rounds_advanced
frames_emitted
```

JSON 先反序列化再比较结构，不要求 key 原始字节顺序一致。

---

# 41. Core 测试矩阵

必须覆盖：

```text
1v1
2v2
FFA 4
FFA 6
FFA 8
3v3v3
clone
summon
shadow
zombie
revive
protect
charm
显式 seed
动态实体生成
winner on normal frame
winner on empty-update round
max_rounds truncate
no_progress truncate
```

---

# 42. Session vs one-shot contract test

核心测试：

```rust
let replay = battle_replay(raw, options)?;
let mut session = BattleSession::new(raw, options)?;
let mut frames = Vec::new();

while let Some(frame) = session.next_frame()? {
    frames.push(frame);
}

assert_eq!(frames, replay.frames);
```

并比较最终 `BattleResult` 与 replay 顶层结果字段。

---

# 43. max_rounds 测试

构造包含空 round 的 fixture，验证：

```text
rounds_advanced <= max_rounds
空 round 计入 rounds_advanced
```

不再使用 `frames.len()` 表示 guard 计数。

---

# 44. terminal precedence 测试

覆盖：

```text
winner on max round -> winner
max_round + no_progress 同时达到 -> max_rounds
Runtime error -> Err
```

---

# 45. terminal idempotency

```rust
while session.next_frame()?.is_some() {}
assert!(session.is_done());
assert_eq!(session.next_frame()?, None);
assert_eq!(session.next_frame()?, None);
```

---

# 46. Python tests

扩展 `scripts/verify_py_cli_api.py`：

```python
session = tswn_py.BattleSession(raw, max_rounds=20000)
frames = list(session)
replay = tswn_py.battle_replay(raw, max_rounds=20000)
assert frames == replay["frames"]
assert session.result() is not None
```

检查 TypedDict 与 Literal 类型。

---

# 47. WASM tests

Node 测试：

```text
constructor
initial_states
current_states
next_frame
status
stop_reason
result
terminal null
parity with battle_replay
```

---

# 48. C tests

新增 `examples/battle_session.c`，覆盖：

```text
options default
new/free
initial_states_json
current_states_json
next_frame_json
terminal
result_json
status
stop_reason
null pointer
error code
repeated next_frame
```

---

# 49. CLI tests

必须覆盖：

```text
fight 默认 human
fight --jsonl
runtime diff
runtime normalized-run
```

并断言旧：

```text
raw
fight --out-raw
顶层 diff
```

不再存在于 clap command tree。

---

# 50. CLI JSONL contract tests

固定 seed fixture，读取 stdout：

```text
line 0   -> type = initial
line 1..N -> type = frame
last line -> type = result
```

断言：

```text
每行独立合法 JSON
没有 banner
没有普通文本
成功时 stderr 为空
```

并比较：

```text
JSONL frames == battle_replay.frames
```

---

# 51. 文档更新

必须更新：

```text
docs/public_api.md
crates/tswn_core/README.md
crates/tswn_py/README.md
crates/tswn_wasm/README.md
crates/tswn_capi/README.md
crates/tswn_py/*.pyi
crates/tswn_capi/include/tswn_capi.h
各 crate CHANGELOG
CLI --help examples
WASM examples README
```

---

# 52. 实施顺序

## Commit 1 — Core DTO / error model

```text
cli_api/battle 模块
BattleOptions
BattleStatus
BattleStopReason
BattleResult
frame_index
round_index
CliApiErrorCode
新 CliApiError variants
```

## Commit 2 — Core BattleSession

```text
BattleSession
canonical max_rounds
canonical no_progress guard
icon cache
next_frame
result
```

## Commit 3 — battle_replay collector

删除 `battle_replay` 自己的 Runner loop，改成 `BattleSession collect`，完成 one-shot parity tests。

## Commit 4 — Python

```text
BattleSession pyclass
_types_battle.pyi
TypedDict DTO
iterator
battle_replay precise return type
error code mapping
```

## Commit 5 — WASM

```text
BattleSession wasm_bindgen
plain JS DTO
Battle options input adapter
old FightSession -> BattleSession wrapper
```

## Commit 6 — C API

```text
tswn_battle_options_t
BattleSession opaque handle
status/reason
state JSON
frame JSON
result JSON
examples/tests
```

ABI 保持 4。

## Commit 7 — CLI command tree cleanup

删除：

```text
raw command
FightRawCommand
FightRaw ParsedCommand
raw_bench.rs
!test! magic
--out-raw
legacy raw fight formatter
```

迁移：

```text
top-level diff -> runtime diff
```

新增：

```text
FightOutput
fight --max-rounds
fight --jsonl
```

## Commit 8 — CLI BattleSession migration

`fight` 和 `fight --jsonl` 全部改为 consume canonical BattleSession，增加 JSONL contract tests。

## Commit 9 — Docs / cross-binding parity

完成 public API 文档、四 binding 示例、cross-binding fixtures、README、CHANGELOG。此 commit 后 BattleSession API 冻结。

## Commit 10 — Web streaming migration

`examples/show-wasm.js / show.js` 从 `battle_replay eager` 迁移到 `BattleSession streaming`，保留 pause / seek / step / replay history，采用 1~2 frame buffer。

---

# 53. Definition of Done

- [ ] `BattleSession` 成为正式 User API
- [ ] `battle_replay()` 只通过 BattleSession 执行
- [ ] `max_rounds` 真正按 Runtime main round 计数
- [ ] 空 round 不向用户输出空 frame
- [ ] winner 空-update round 不丢 terminal frame
- [ ] `no_progress` 规则只在 core 存在一份
- [ ] `frame_index / round_index` 正式进入 canonical frame
- [ ] terminal 后 `next_frame()` 永远返回 None/null
- [ ] `result()` 仅 terminal 后存在
- [ ] Python / WASM / C 同步暴露 BattleSession
- [ ] Python DTO 使用 TypedDict
- [ ] Python 支持 `for frame in session`
- [ ] WASM 返回 plain JS object
- [ ] C 使用 versioned options struct
- [ ] C frame/result 使用 JSON
- [ ] C ABI 仍为 4
- [ ] stable error code 由 core 统一
- [ ] WASM FightSession 不再拥有独立 Runtime loop
- [ ] CLI 删除 `raw`
- [ ] CLI 删除 `fight --out-raw`
- [ ] CLI 顶层 `diff` 移入 `runtime diff`
- [ ] CLI 删除 `!test!` magic routing
- [ ] CLI 增加 `fight --jsonl`
- [ ] JSONL 每个 frame 立即 flush
- [ ] JSONL stdout 不混 banner/error 文本
- [ ] CLI human fight 也走 BattleSession
- [ ] Core session 与 one-shot parity 全通过
- [ ] Python/WASM/C cross-binding parity 全通过
- [ ] 网页最终使用 streaming BattleSession
- [ ] 现有 replay UI rows/clips/parts 语义不发生分叉

---

# 54. 最终用户示例

## Rust

```rust
use tswn_core::cli_api::{
    BattleOptions,
    BattleSession,
};

let mut session = BattleSession::new(
    raw,
    BattleOptions::default(),
)?;

while let Some(frame) = session.next_frame()? {
    render(frame);
}

let result = session
    .result()
    .expect("battle is terminal");
```

## Python

```python
import tswn_py

session = tswn_py.BattleSession(raw)

for frame in session:
    render(frame)

result = session.result()

if result["finished"]:
    print("winner:", result["winner_ids"])
else:
    print("stopped:", result["stop_reason"])
```

## JavaScript / WASM

```js
const session = new wasm.BattleSession(rawInput, {
  include_icons: false,
});

renderInitial(session.initial_states());

for (;;) {
  const frame = session.next_frame();
  if (frame === null) break;
  await playFrame(frame);
}

renderResult(session.result());
```

## C

```c
tswn_battle_options_t options;
tswn_battle_options_default(&options);

tswn_battle_session_t* session = NULL;

if (tswn_battle_session_new(
        raw,
        &options,
        &session
    ) != TSWN_OK) {
    return 1;
}

for (;;) {
    uint8_t has_frame = 0;
    tswn_str_t json = {0};

    if (tswn_battle_session_next_frame_json(
            session,
            &has_frame,
            &json
        ) != TSWN_OK) {
        break;
    }

    if (!has_frame) break;

    consume_frame_json(json.ptr, json.len);
    tswn_str_free(json);
}

tswn_battle_session_free(session);
```

## CLI human

```bash
tswn-cli fight -r "left@red\n\nright@blue"
```

## CLI streaming JSONL

```bash
tswn-cli fight --jsonl -r "left@red\n\nright@blue"
```

输出：

```json
{"type":"initial","initial_states":[...]}
{"type":"frame","frame":{"frame_index":0,"round_index":0,...}}
{"type":"frame","frame":{"frame_index":1,"round_index":1,...}}
{"type":"result","result":{"status":"finished","stop_reason":"winner",...}}
```

---

# 55. 最终架构意义

完成后，TSWN 不再存在：

```text
“完整 replay 是一种执行方式”
“WASM step 又是另一种执行方式”
“CLI fight 又自己跑”
```

而统一为：

```text
                 BattleSession
                       │
        ┌──────────────┼──────────────┐
        │              │              │
        ▼              ▼              ▼
    Web live       CLI JSONL      Python/C
        │
        ▼
  实时胜率模型
  （后续独立阶段）
```

后续实时胜率、网页观战、GUI、数据采集、Web Worker streaming 都只需要消费同一个 BattleSession 状态流，不再修改基础 Runtime API。
