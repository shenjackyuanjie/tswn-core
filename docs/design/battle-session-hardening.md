# TSWN BattleSession API Freeze 前 Hardening 修改要求

> 状态：FINAL / 可直接执行\
> 目标仓库：`shenjackyuanjie/tswn-core`\
> 基线：BattleSession + Web Streaming 已完成后的当前 `main`\
> 目的：在进入下一阶段前，修复已发现的兼容语义问题，补强 C ABI 演进能力、类型契约和失败态说明，并建立 Python DTO 转换性能基线。\
> 本文不重新设计 BattleSession，不修改网页 streaming 架构，不包含任何胜率模型、ModelState、训练或推理工作。

---

## 0. 验收结论与本轮定位

当前 BattleSession / Web Streaming 主体实现通过结构性验收：

- `BattleSession` 已成为 canonical 增量对局状态机；
- `battle_replay()` 已改为收集同一个 `BattleSession`；
- Python / WASM / C / CLI JSONL 已实现跨绑定 parity；
- CLI 已完成 `raw` / `--out-raw` 清理和 `runtime diff` 迁移；
- 网页已改为真实 streaming，初始状态先渲染，frame 按需拉取；
- 两帧 buffer、暂停、seek、历史续播、turbo、资源释放、generation isolation 已实现；
- Edge 实测 `next_frame + WASM DTO` p95 显著低于 16.7 ms。

因此本轮**不是第二次架构重构**。

本轮只允许做 API freeze 前的 hardening。

最终优先级：

```text
P0-1  修复 WASM legacy FightSession owner_id 映射
P0-2  修复 C BattleOptions struct_size 的长期演进契约

P1-1  增加 BattleSession failure/poisoned 明确查询能力
P1-2  收紧 WASM TypeScript literal 类型

P2-1  建立 Python DTO 转换性能基线
       不在没有数据的情况下直接重写 Python DTO 转换
```

---

## 1. 实施纪律

继续执行上一轮已经采用的原则：

> 写一块，测一块，提交一块。

不得把本要求中的五项修改混成一个提交。

每个 commit 必须：

- 单一职责；
- 当前 commit 自身可编译；
- targeted tests 通过；
- 不依赖后一个 commit 才恢复正确性；
- 不夹带无关格式化；
- `git diff` 只包含当前任务；
- 修改完成立刻提交。

最终建议保留 6 个独立 commit，见本文末尾实施顺序。

---

## 2. P0-1：修复 WASM FightSession `owner_id` 兼容映射

### 2.1 当前问题

canonical `BattlePlayerState` 同时存在：

```text
owner_id
source_id
```

当前 core 构造语义：

```text
owner_id  = RuntimePlayerSnapshot.owner_id
source_id = RuntimePlayerSnapshot.root_owner_id
```

二者不是同义字段。

当前 WASM compatibility wrapper：

```rust
fn state_from_core(state: &BattlePlayerState) -> PlayerState {
    PlayerState {
        ...
        owner_id: state.source_id,
        ...
    }
}
```

这是错误映射。

legacy `PlayerState.owner_id` 应映射 canonical 的直接：

```text
BattlePlayerState.owner_id
```

而不是：

```text
BattlePlayerState.source_id
```

在直接 owner 与 root owner 不同的实体链上，当前实现会改变 legacy API 的 ownership 语义。

### 2.2 必须修改

文件：

```text
crates/tswn_wasm/src/fight.rs
```

修改：

```rust
owner_id: state.source_id,
```

为：

```rust
owner_id: state.owner_id,
```

### 2.3 禁止修改

不得为了这个问题：

- 改 canonical `BattlePlayerState.owner_id` 语义；
- 改 `source_id` 语义；
- 删除 legacy `PlayerState.owner_id`；
- 把 legacy API 改成 root owner；
- 修改网页 canonical state。

这是 compatibility wrapper 的单点 bug。

### 2.4 必须增加的单元测试

不要只依赖真实战斗 fixture，因为当前 fixture 未必天然构造：

```text
owner_id != source_id
```

直接在 `fight.rs` test module 中构造一个 `BattlePlayerState`。

必须满足：

```rust
state.owner_id = Some(7);
state.source_id = Some(3);
```

调用：

```rust
let legacy = state_from_core(&state);
```

断言：

```rust
assert_eq!(legacy.owner_id, Some(7));
```

额外断言 canonical input 未被改变。

测试名：

```text
legacy_player_state_preserves_direct_owner_id
```

### 2.5 验收标准

- 新测试失败于旧代码；
- 修改后新测试通过；
- 原 `FightSession` compatibility tests 全部继续通过；
- canonical cross-binding parity 不发生任何变化。

---

## 3. P0-2：修复 C BattleOptions `struct_size` 演进契约

### 3.1 当前问题

当前 public struct：

```c
typedef struct tswn_battle_options_t {
    uint32_t struct_size;
    double eval_rq;
    size_t max_rounds;
    uint8_t include_icons;
} tswn_battle_options_t;
```

当前读取逻辑本质为：

```rust
if struct_size < size_of::<tswn_battle_options_t>() {
    reject;
}

let options = options.read();
```

这对当前 V1 caller 正确。

但当前文档又宣称：

> 更大的未来尾部会被忽略 / options 可以继续追加。

这只解决了：

```text
新 caller -> 旧 library
```

的一部分情况。

它没有正确解决：

```text
旧 caller -> 新 library
```

因为未来一旦 `tswn_battle_options_t` 增长，新 library 的：

```rust
size_of::<tswn_battle_options_t>()
```

也会增长，旧 caller 仍传旧 `struct_size`，会直接被拒绝。

此外，如果未来新字段被编译器放入旧 V1 struct 的 tail padding，单纯比较 `struct_size` 甚至可能错误判断某个新字段“存在”。

因此当前实现不能称为真正 append-compatible 的 versioned options struct。

---

## 4. C BattleOptions 最终演进规则

### 4.1 V1 prefix 永久冻结

在 Rust CAPI 内定义一个**私有、永久冻结**的 V1 prefix：

```rust
#[repr(C)]
#[derive(Clone, Copy)]
struct BattleOptionsV1 {
    struct_size: u32,
    eval_rq: f64,
    max_rounds: usize,
    include_icons: u8,
}
```

它只用于读取历史 V1 caller。

必须定义：

```rust
const BATTLE_OPTIONS_V1_SIZE: usize =
    size_of::<BattleOptionsV1>();
```

该 private prefix 今后不得增加字段。

### 4.2 public struct 当前保持 V1 layout

当前：

```rust
pub struct tswn_battle_options_t
```

与 C header 中的：

```c
tswn_battle_options_t
```

本轮不得改字段顺序、字段类型或当前 ABI layout。

本轮不新增 options 字段。

原因：

> 本次只修演进机制，不顺便制造 V2。

### 4.3 `read_options()` 不得读取未来完整 struct

禁止未来继续：

```rust
options.read::<tswn_battle_options_t>()
```

作为 versioned struct 的读取方案。

当前必须重构为：

```text
1. 只读 struct_size
2. 检查 struct_size >= V1_SIZE
3. 只读取冻结的 BattleOptionsV1 prefix
4. 基于默认 BattleOptions 构造 core options
5. 如果未来存在扩展字段，再按“字段存在性”单独读取
```

当前 V1 可实现为：

```rust
unsafe fn read_options(
    options: *const tswn_battle_options_t,
) -> FfiResult<BattleOptions> {
    if options.is_null() {
        return Ok(BattleOptions::default());
    }

    let struct_size =
        unsafe { options.cast::<u32>().read_unaligned() }
            as usize;

    if struct_size < BATTLE_OPTIONS_V1_SIZE {
        return Err(...);
    }

    let v1 = unsafe {
        options
            .cast::<BattleOptionsV1>()
            .read_unaligned()
    };

    ...
}
```

使用 `read_unaligned()` 是为了让 prefix 读取不依赖调用方指针额外满足 Rust 对完整 public struct 的读取假设。

---

## 5. 未来新增 C options 字段的硬规则

本文必须同步写入 CAPI source comment / README。

未来新增例如：

```text
some_future_option
```

时必须满足：

### 规则 A：禁止复用 V1 tail padding

新字段的：

```text
offset_of(new_field)
```

必须：

```text
>= BATTLE_OPTIONS_V1_SIZE
```

不得利用 V1 tail padding 塞新字段。

若 compiler layout 会把新字段放进 V1 tail padding：

```text
必须增加显式 padding / reserved 区域
```

把新字段推到 V1_SIZE 之后。

### 规则 B：每个版本永久保留 prefix size

每一个新版本定义永久冻结 prefix size：

```text
BATTLE_OPTIONS_V1_SIZE
BATTLE_OPTIONS_V2_SIZE
...
```

旧 size 常量永久保留。

### 规则 C：字段存在性按 field end 判断

每个新字段必须按：

```text
struct_size >= field_end_offset
```

判断是否存在。

不存在：

```text
使用 BattleOptions::default() 中对应默认值
```

### 规则 D：旧 caller 必须可调用新 library

不得因为 library 自己的 current struct 变大而拒绝旧 caller。

即未来必须支持：

```text
old header / old caller
    -> new library
```

只要 caller 至少提供 V1 prefix。

---

## 6. C header 文档必须修改

当前类似：

```text
小于当前版本的 struct_size 会被拒绝
```

必须改成：

```text
小于 V1 最小结构尺寸会被拒绝。
较旧调用方的已知 prefix 在新版 library 中继续有效；
新版 library 对调用方未提供的尾部字段使用默认值。
较大 struct_size 的未知尾部由旧 library 忽略。
```

不能再写：

```text
小于“当前版本完整 struct”就拒绝
```

---

## 7. C options 必须增加的测试

至少增加以下测试。

### 7.1 null options

```text
options = NULL
```

等价：

```text
BattleOptions::default()
```

### 7.2 exact V1

正常 `tswn_battle_options_default()`：

```text
struct_size == V1_SIZE
```

创建成功。

### 7.3 too small

```text
struct_size = V1_SIZE - 1
```

返回：

```text
TSWN_ERR_INVALID_ARGUMENT
```

稳定错误 code：

```text
INVALID_ARGUMENT
```

### 7.4 larger future caller

创建一段：

```text
V1_SIZE + 64
```

的 buffer。

前缀填合法 V1 options，尾部填非零垃圾数据。

```text
struct_size = V1_SIZE + 64
```

必须创建成功，并且行为仅由 V1 已知字段决定。

### 7.5 simulated old caller contract

增加 private test struct：

```rust
#[repr(C)]
struct SimulatedOldBattleOptionsV1 { ... }
```

使用其 pointer 调当前 `read_options()`。

断言成功。

这个测试的目的不是当前 layout，而是确保以后 public struct 扩展时，该 test 仍然必须通过。

### 7.6 ABI layout assertions

对 V1 prefix 增加明确测试：

```text
field offsets
V1 size
public V1 prefix 与 private prefix offset 一致
```

不要只检查最终 `size_of`。

---

## 8. C ABI 版本

完成上述 hardening 后：

```text
tswn_capi_abi_version() 仍保持 4
```

原因：

- 不删除 symbol；
- 不修改现有函数签名；
- 不修改现有 V1 field 顺序和语义；
- 只修 library 对 `struct_size` 的接受规则。

如果实现过程中不得不改变 `tswn_battle_options_t` 的现有二进制 layout，则立即停止该方案并重新评估 ABI bump。

本要求默认：

> 不允许改变 V1 layout。

---

## 9. P1-1：明确 BattleSession failed / poisoned 状态

### 9.1 当前语义

当前 core 在 Runtime validation/error 后会：

```text
保存 failure
后续 next_frame() 返回相同错误
result() = None
stop_reason() = None
```

这是合理的 sticky failure。

但是：

```text
status() = running
is_done() = false
```

仍会成立。

这是因为：

```text
BattleStatus
```

目前只建模正常 battle 生命周期：

```text
running
finished
truncated
```

Runtime failure 没有 result，也不应伪装成 truncated。

本轮不新增：

```text
BattleStatus::Failed
```

避免改变已经冻结并跨四端验证的 DTO enum。

---

## 10. 新增 `is_failed()` 查询

为避免用户看到：

```text
status = running
```

却不知道 session 已经不可继续，本轮新增：

```rust
pub fn is_failed(&self) -> bool
```

语义：

```text
failure.is_some()
```

### 10.1 Rust

`BattleSession`：

```rust
pub fn is_failed(&self) -> bool {
    self.failure.is_some()
}
```

### 10.2 Python

增加：

```python
def is_failed(self) -> bool: ...
```

### 10.3 WASM

增加：

```js
session.is_failed()
```

返回 boolean。

### 10.4 C

增加：

```c
uint8_t tswn_battle_session_is_failed(
    const tswn_battle_session_t* session
);
```

如果现有 C session bool query 全部是直接返回 `uint8_t`，保持同一风格。

---

## 11. failure 契约

公共文档必须明确：

```text
next_frame() 返回 Runtime error 后：

is_failed() = true
result() = null / None
stop_reason() = null / None

后续 next_frame()
    返回同一个 sticky error

调用方应停止推进并释放 session。
```

`is_done()` 语义保持：

> 是否已经产生正常 terminal BattleResult。

因此失败后：

```text
is_done() = false
is_failed() = true
```

这是刻意区分：

```text
normal terminal
vs
execution failure
```

不修改 `BattleStatus` enum。

---

## 12. failure 测试

Core 已有 sticky Runtime error test，扩展为：

```text
第一次 next_frame -> RUNTIME_FAILED
is_failed == true
is_done == false
result == None
stop_reason == None

第二次 next_frame -> 同 code + 同 message
```

并分别在：

```text
Python
WASM
C
```

增加 smoke test，至少确认 `is_failed()` 对真实失败 session 可见。

Cross-binding canonical success payload comparison无需增加 `is_failed` 字段，因为它不是 DTO 字段。

---

## 13. P1-2：收紧 WASM TypeScript literal 类型

### 13.1 当前问题

`battle_types.d.ts` 已经正确将：

```ts
BattleStatus
BattleStopReason
```

做成 literal union。

但多个稳定枚举语义仍被写成宽泛：

```ts
string
```

包括：

```text
BattlePlayerState.minion_kind
BattleUpdate.update_type
BattleUpdate.tone
BattleReplayClip.tone
BattleReplayTextPart.kind
```

这会导致 TypeScript 用户无法：

- exhaustively switch；
- 在拼写错误时获得编译器提示；
- 明确知道公共枚举合法值。

---

## 14. 必须新增的 TS alias

```ts
export type BattleMinionKind =
    | "clone"
    | "summon"
    | "shadow"
    | "zombie";

export type BattleUpdateType =
    | "win"
    | "none"
    | "next_line";

export type BattleTone =
    | "normal"
    | "damage"
    | "recover"
    | "knockout"
    | "status_exit";

export type BattleReplayTextPartKind =
    | "text"
    | "highlight"
    | "player"
    | "data";
```

---

## 15. 必须替换的 TS 字段

从：

```ts
minion_kind: string | null;
update_type: string;
tone: string;
kind: string;
```

替换为：

```ts
minion_kind: BattleMinionKind | null;
update_type: BattleUpdateType;
tone: BattleTone;
kind: BattleReplayTextPartKind;
```

`BattleReplayClip.tone` 同样改成：

```ts
BattleTone
```

---

## 16. 暂不收紧的字符串字段

本轮继续保留：

```text
player_type: string
color: string
message_template: string
message_rendered: string
status_labels: string[]
```

原因：

- `player_type` 当前稳定全集未在 canonical DTO 中作为 enum 固化；
- `color` 是动态颜色字符串；
- message/status 本身就是文本协议。

不要为了“类型更漂亮”顺手扩大 schema 重构。

---

## 17. TS 类型 contract tests

增加一个 Node/TypeScript declaration verification。

至少验证 `.d.ts` 含：

```text
BattleMinionKind
BattleUpdateType
BattleTone
BattleReplayTextPartKind
```

并检查字段引用 alias，不再是裸 `string`。

如果仓库当前没有 `tsc` test 环境，可用现有 Node contract test 读取声明文件进行结构断言。

不要求为了这一项引入完整 TypeScript toolchain。

---

## 18. P2-1：Python DTO 转换性能基线

### 18.1 当前实现

Python binding 当前每次 DTO 转换大致为：

```text
Rust DTO
-> serde_json::to_string()
-> Python json.loads()
-> Python dict/list
```

功能正确，跨绑定 payload 也已经验证一致。

当前没有证据证明它已经成为瓶颈。

因此：

> 本轮不直接重写为手工 PyDict/PyList，也不引入新 serde->PyObject 依赖。

先测。

---

## 19. 新增 Python BattleSession 性能 benchmark

新增脚本：

```text
scripts/benchmark_py_battle_session.py
```

固定使用已有 frozen fixture：

```text
1v1
2v2
ffa_8
3v3v3
```

每类至少：

```text
100 次 session
```

记录：

```text
session_create_ms p50 / p95
next_frame_wall_ms p50 / p95 / max
total_session_ms p50 / p95
frames_per_session
```

可额外记录：

```text
battle_replay one-shot total_ms
```

但主指标必须是 Python `BattleSession.next_frame()`。

---

## 20. Python DTO 优化 gate

基线写入：

```text
docs/perf/python_battle_session_baseline.md
```

本轮只在满足以下任一条件时，才允许继续做 DTO conversion 重写：

```text
A. next_frame Python p95 >= 1.0 ms
或
B. Python next_frame p95 >= 同环境 WASM next_frame p95 的 4 倍
或
C. 实际下一阶段采集任务 profiling 显示 DTO conversion >= 总 CPU 时间 25%
```

如果均不满足：

```text
保留当前 serde_json -> json.loads
```

并在文档写明：

> 当前不是优先瓶颈，暂不优化。

---

## 21. 如果触发 Python 优化 gate

这不属于本 hardening 的默认实现。

如果 benchmark 命中 gate，必须另起一个独立任务/commit，不得混入本轮前四项。

优化方案要求：

```text
保持 Python 返回 shape 完全不变
保持 TypedDict 不变
保持 cross-binding parity
```

禁止为性能改成用户可见的自定义 Python DTO class。

优先方向：

```text
直接构造 PyDict / PyList
或
使用与当前 PyO3 版本兼容的成熟 serde->PyObject 转换层
```

优化前后必须有同 fixture A/B benchmark。

---

## 22. 本轮明确不修改的内容

以下已经通过验收，本 hardening 禁止重新设计。

### 22.1 Core battle semantics

不改：

```text
BattleStatus = running / finished / truncated
BattleStopReason = winner / max_rounds / no_progress
winner > max_rounds > no_progress
max_rounds 统计 main_round
NO_PROGRESS = entity_count * 16
```

### 22.2 Battle DTO

不改：

```text
BattlePlayerState 字段集合
BattleReplayFrame 字段集合
BattleResult 字段集合
BattleReplay JSON shape
```

除新增 method：

```text
is_failed()
```

外不加新 DTO 字段。

### 22.3 CLI JSONL

保留：

```json
{"type":"initial","data":...}
{"type":"frame","data":...}
{"type":"result","data":...}
```

不得改回其他包装。

### 22.4 Web streaming

不改：

```text
BattleStreamSource
BattleStreamController
2-frame buffer
append-only canonical history
pause / seek / history resume
turbo 24 visible chunks yield
BattleDisplay projection
WASM explicit free
generation isolation
```

### 22.5 Web Worker

当前性能基线不支持引入 Worker。

不得在本 hardening 中添加：

```text
Web Worker
SharedArrayBuffer
异步 Runtime 线程
```

### 22.6 Model

本轮仍然禁止：

```text
ModelState
胜率字段
训练数据
模型推理
模型 UI
Monte Carlo
```

---

## 23. 文档同步要求

必须更新：

```text
docs/reference/public-api.md
crates/tswn_capi/README.md
crates/tswn_capi/include/tswn_capi.h
crates/tswn_wasm/README.md
crates/tswn_py/README.md
各相关 CHANGELOG
```

关键文档必须写明：

#### C options

```text
V1 prefix 永久兼容
旧 caller -> 新 library 必须继续可用
未知尾部由旧 library 忽略
未提供的新尾字段使用默认值
未来字段不得复用 V1 tail padding
```

#### Session failure

```text
Runtime error 后 session is_failed = true
没有 BattleResult
不是 truncated
调用方停止推进并释放
```

#### TS DTO

列出 stable literal enum。

#### Python perf

链接：

```text
docs/perf/python_battle_session_baseline.md
```

---

## 24. 必须保留的现有验证

Hardening 完成后必须重新跑：

```text
scripts/verify_battle_cross_binding.py
scripts/verify_py_cli_api.py
scripts/verify_cli_battle.py
scripts/verify_wasm_battle.test.mjs

crates/tswn_wasm/examples/show-*.test.mjs
scripts/verify_web_playback.mjs
```

不能只跑新增 test。

---

## 25. 完整测试要求

Rust：

```bash
cargo +nightly fmt --check

cargo test -p tswn_core
cargo test -p tswn_py
cargo test -p tswn_wasm
cargo test -p tswn_capi
cargo test -p tswn_test
```

Clippy：

```bash
cargo clippy \
  -p tswn_core \
  -p tswn_py \
  -p tswn_wasm \
  -p tswn_capi \
  --all-targets
```

WASM/JS：

```bash
node --test crates/tswn_wasm/examples/show-*.test.mjs
node --experimental-vm-modules scripts/verify_web_playback.mjs
node scripts/verify_wasm_battle.test.mjs
```

Python：

```bash
python scripts/verify_py_cli_api.py
python scripts/benchmark_py_battle_session.py
```

Cross binding：

```bash
python scripts/verify_battle_cross_binding.py
```

如果具体构建前置步骤由现有 README/script 负责，沿用现有流程。

---

## 26. 分块提交顺序

必须按以下顺序提交。

### Commit 1 — P0 legacy owner fix

```text
wasm: preserve direct owner in legacy FightSession state
```

内容：

```text
state_from_core owner_id 修复
专门 fabricated owner/source divergence test
```

### Commit 2 — P0 C V1 options reader

```text
capi: make BattleOptions V1 prefix forward-compatible
```

内容：

```text
private BattleOptionsV1 prefix
BATTLE_OPTIONS_V1_SIZE
read_options prefix-only 读取
future-tail acceptance
old-caller simulation tests
layout assertions
```

### Commit 3 — C docs

```text
docs: define stable C BattleOptions prefix evolution rules
```

内容：

```text
C header comment
CAPI README
public_api C section
CHANGELOG
```

如果文档和 Commit 2 很小，也可以与 Commit 2 同一 commit，但优先分开。

### Commit 4 — failure query

```text
core: expose sticky BattleSession failure state
```

内容：

```text
core is_failed()
Python is_failed()
WASM is_failed()
C is_failed()
tests
public API docs
```

这是一个跨绑定 API feature，因此允许在一个 commit 中同步四端，前提是该 commit 自身全部测试通过。

### Commit 5 — TS literal contract

```text
wasm: tighten canonical battle TypeScript literals
```

内容：

```text
BattleMinionKind
BattleUpdateType
BattleTone
BattleReplayTextPartKind
字段替换
declaration contract test
```

### Commit 6 — Python baseline

```text
perf: record Python BattleSession DTO conversion baseline
```

内容：

```text
benchmark_py_battle_session.py
python_battle_session_baseline.md
结果与优化 gate 结论
```

如果未触发 gate：

```text
本轮到此结束
```

如果触发：

```text
另开后续优化任务
```

不得把 DTO conversion rewrite 偷塞进 Commit 6。

---

## 27. Definition of Done

全部满足后，本 hardening 才算完成。

### P0 correctness

- [ ] legacy `PlayerState.owner_id` 映射 canonical `owner_id`
- [ ] fabricated `owner_id != source_id` test 存在
- [ ] legacy FightSession 其他行为未变

### C ABI

- [ ] V1 prefix 类型永久冻结
- [ ] V1 size 常量不依赖 future public struct size
- [ ] `read_options` 只读 V1 prefix
- [ ] NULL options 正常
- [ ] exact V1 正常
- [ ] V1-1 拒绝
- [ ] larger future buffer 接受
- [ ] simulated old caller test 通过
- [ ] future field 不得利用 V1 tail padding 规则已文档化
- [ ] ABI version 仍为 4

### Failure state

- [ ] core `is_failed()`
- [ ] Python `is_failed()`
- [ ] WASM `is_failed()`
- [ ] C `tswn_battle_session_is_failed()`
- [ ] sticky error test 验证 code/message 稳定
- [ ] failure 不产生 result
- [ ] failure 不伪装 truncated
- [ ] public docs 明确 poisoned session 语义

### TypeScript

- [ ] `BattleMinionKind`
- [ ] `BattleUpdateType`
- [ ] `BattleTone`
- [ ] `BattleReplayTextPartKind`
- [ ] 对应字段不再是裸 string
- [ ] declaration contract test

### Python perf

- [ ] benchmark script 可复现
- [ ] 4 类 frozen fixture
- [ ] 每类至少 100 session
- [ ] p50/p95/max 记录
- [ ] baseline 文档提交
- [ ] 明确写出是否触发优化 gate

### Regression

- [ ] Core tests
- [ ] Python tests
- [ ] WASM tests
- [ ] CAPI tests
- [ ] tswn_test
- [ ] cross-binding parity
- [ ] CLI verifier
- [ ] web streaming tests
- [ ] browser playback verifier
- [ ] formatting/clippy

---

## 28. 完成本轮后的冻结状态

Hardening 完成后，以下部分视为可进入稳定使用阶段：

```text
BattleSession lifecycle
BattleOptions V1
BattlePlayerState
BattleReplayFrame
BattleResult
BattleReplay
CLI JSONL event wrapper
Python canonical dict shape
WASM canonical plain object shape
C BattleSession ABI 4
Web streaming initial/frame/result consumption
```

后续胜率估计或其他新功能应作为**消费者**接入：

```text
initial
frame
result
```

而不是再次修改：

```text
BattleSession 推进语义
网页 playback 基础设施
cross-binding DTO shape
```

除非发现新的 correctness bug。

---

## 29. 给实现者的最终要求

本任务的目标不是“继续优化已经能工作的代码”。

目标是：

> 在 API freeze 之前，把已知的 owner compatibility bug、C versioned-options 演进漏洞、failed session 可观察性和 TS 类型契约补齐，并用数据判断 Python DTO conversion 是否值得优化。

优先顺序不得调整：

```text
correctness
> ABI 演进
> API 可观察性
> 类型体验
> 性能优化
```

没有 benchmark 数据时，不允许为了“感觉更快”重写 Python binding。
