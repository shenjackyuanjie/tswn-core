# 更新日志

## [0.2.10] - 2026-06-19

### 修复

- 修复零伤害攻击的 `hp_delta` 使用 UI 展示分（`score`）而非实际伤害值（`param`），导致回放血量变化与 `[s_dmg0]` 消息不一致。

## [0.2.9] - 2026-06-18

### 新增

- 补齐一批与 `tswn-cli` / `tswn_py` 对齐的高层 WASM 导出：
  - 胜率摘要：`win_rate_summary()`、`team_win_rate_summary()`、`group_win_rate_summary()`
  - 评分与 namer-pf：`score()`、`namer_pf()`
  - 批量胜率与配队：`batch_rate()`、`pair_rate()`
  - DIY / 图标 / 列表解析：`to_diy()`、`to_diy_batch()`、`icon_info()`、`parse_group_lines()`
- 新增对应的强类型 `Cli*Result` / `CliIconInfo` 数据模型，供 JS / TS 调用方直接消费。
- show 示例新增事件/帧级单步控制、暂停态前后移动、结束结算表和角色详情小窗。
- 角色详情支持按输入 `id_name` 保存自定义昵称，并通过 `localStorage` 应用到后续回放。

### 变更

- 新增导出统一复用 `tswn_core::cli_api` 的共享高层逻辑，避免 WASM 包装层单独维护一份 CLI helper 实现。
- README 补充新的 CLI 对齐接口说明与返回值示例。
- show 示例整理为独立 Fight 展示页：移除胜率混杂入口，弱化帧分隔视觉，使用单独结算表替代调试 winner 行。
- 左侧角色面板按输入顺序和真实队伍分组；召唤物、分身和临时单位挂在 owner 下方，并在逐消息播放时同步侧栏元数据。
- owner 下的召唤物/分身保留运行期出现顺序，不再按 id 重排，后出现的单位会追加到该 owner 子列表底部。
- show 播放速度改为新战斗默认恢复正常速度；雷击、地裂等大范围技能使用更短延迟，避免正常速度下拖慢回放。
- 左侧 HP/MP 展示改为逐消息刷新，补齐蓝量恢复/消耗、瘟疫和体力减少等血条变化，并细化死亡血条消失动画。
- 状态变更着色改为依赖 WASM 提供的结构化 `status_change_tokens`，不再只靠前端正则猜测潜行识破、中止和打消等状态。

### 修复

- 修复解析失败后仍残留上一场结果或继续展示不可信回放的问题；严重输入错误会明确提示行号并阻止生成。
- 补强 `+ol:{...}` 和 `+diy[...] {...}` 输入对象校验，能发现对象尾部垃圾内容、缺字段值和属性数量错误。
- 修复重播、重置和窗口失焦后左侧角色高亮残留的问题。
- 修复手动跳帧或播完单帧后侧栏继续保留上一条消息 caster/target 高亮的问题；帧结束快照只同步最终状态。
- 修复逐消息侧栏刷新时头像、详情按钮和行标题复用旧单位元数据的问题。
- 修复上一帧已死亡的召唤物/临时单位在本帧重新存活时仍沿用死亡 UI 的问题，同时避免本帧才出现的单位在消息触达前提前显示。

### 验证

- `cargo fmt --check`
- `cargo check -p tswn_wasm`
- `cargo test`
- `bun build crates/tswn_wasm/examples/show.js --target=browser --outdir=target/codex-check/show --root=crates/tswn_wasm/examples`
- `bun --check crates/tswn_wasm/examples/show-render.js crates/tswn_wasm/examples/show-replay.js crates/tswn_wasm/examples/show-utils.js crates/tswn_wasm/examples/show-wasm.js crates/tswn_wasm/examples/show.js`

## [0.2.8] - 2026-05-24

### 新增

- 为 WASM 数据模型引入 `tsify`，自动生成 `FightReplay`、`PlayerState`、`RoundFrame`、`WinRateResult` 等 TypeScript 领域类型。
- `FightOptions` / `WinRateOptions` 现在在生成的 `.d.ts` 中暴露为可选字段接口，适合 JS/TS 调用方直接传入配置对象。

### 变更

- `fight(...)`、`fight_summary(...)`、`win_rate_sync(...)`、`group_win_rate(...)` 改为强类型 WASM 边界签名，生成的 TypeScript 声明不再退化为用户级 `any`。
- `FightSession.players()` / `state()` / `step()` / `run_to_end()` / `winner_ids()` 和 `WinRateSession.progress()` / `step()` / `result()` 改为强类型返回。
- 清理旧的 `JsValue` 手写序列化 / options 解析辅助路径，统一交给 `tsify` + `serde-wasm-bindgen` 的边界转换处理。

## [0.2.7] - 2026-05-21

### 变更

- show / demo 示例加载 WASM 模块时为 JS glue 与 `.wasm` 文件追加本次页面生命周期内统一的 cache bust 参数，避免浏览器复用旧构建产物。
- WASM 初始化改为显式传入匹配的 `tswn_wasm_bg.wasm` URL，确保备用加载路径下 JS glue 与 wasm 二进制来自同一目录。
- normal 自动播放从首个 chunk 开始时不再等待首帧延迟，进入回放后立即渲染第一步。

## [0.2.6] - 2026-05-19

### 新增

- 补齐跨语言通用顶层接口：
  - `default_eval_rq()`
  - `win_rate_eval_rq()`
  - `name_to_png_bytes(name)`
  - `name_to_icon_rgba(name)`
  - `group_win_rate(target, against, total_rounds, options?)`
- `group_win_rate(...)` 返回 `GroupWinRateResult[]`，每项包含 `opponent` 和完整 `WinRateResult`。

### 变更

- `win_rate_sync(...)` 现在复用统一的同步胜率执行路径，避免单局 / 批量接口各自维护一套逻辑。
- README 示例与字段说明统一改为真实的 snake_case 导出名。

## [0.2.5] - 2026-05-17

### 对齐

- 玩家头像机制对齐混淆版 `md5.js` 的 `Sgls.o6(fy)` 行为：按 `icon_key` 缓存头像，并按首次出现顺序分配 `icon_N` 类名。
- `PlayerMeta` / `PlayerState` 新增 `icon_key`，用于保留原版 renderer 的头像缓存 key。
- 运行期单位（分身、使魔、幻影、丧尸）在 `include_icons` 启用时会随状态携带自身头像数据，show 回放优先渲染自身头像，不再默认继承主人头像。
- show 示例的 CSS 注入改为使用回放归一化后的 `icon_styles` 表，避免同队多名玩家或运行期单位头像 class 与原版不一致。

### 验证

- `cargo check -p tswn_wasm`
- `cargo test -p tswn_wasm`
- `bun build crates\tswn_wasm\examples\show.js --target=browser --outdir target\codex-js-check\show`

## [0.2.4] - 2026-05-17

### 对齐

- `RoundFrame.total_delay` 改为按混淆版 `md5.js` 的可见 update 等待规则计算：每条可见消息等待 `max(update.delay0, 上一条可见 update.delay1)`，每帧初始上一条 delay 为 `1800`。
- 回放 chunk 构建不再使用 `delay1 || delay0`，改为携带混淆版 `md5.js` 的原始未缩放等待时间。
- normal 播放改为先等待再渲染当前 chunk，并按混淆版 `md5.js` 的 `sqrt(角色数 / 2)` 规则缩放等待时间。
- 结束帧补齐混淆版 `RunUpdateWin` 的 `3000ms` 等待。

### 文档

- 更新 show 示例中的中文注释，明确 delay 逻辑以混淆版 `md5.js` 为准。

## [0.2.3] - 2026-05-07

### 变更

- 状态标签增强：蓄力显示 step 数值（如 `蓄力 (2)`），潜行显示锁定目标（如 `潜行至 #5`）。

## [0.2.2] - 2026-05-07

### 新增

- 玩家联动高亮：鼠标悬停角色时高亮页面中所有同名元素（show.js）。
- 战斗回放展示种子信息（show-replay.js）。
- `MessageTone` 枚举添加 `#[serde(rename_all = "snake_case")]` 序列化支持（model.rs）。

### 优化

- 渲染引擎重构：HP/MP 条改为固定像素宽度（`ceil(值/4)`），更直观紧凑（show-render.js / show-utils.js / show.css）。
- 简化 body 背景、微调 HP/MP 条高度、移除冗余 font-weight 声明（show.css）。
- 消息分隔符从中文逗号改为英文逗号，对齐 JSON 输出风格（show-render.js）。

### 重构

- 结算表逻辑重写：添加存活/死亡判断、HP 条绝对定位、完善 JSDoc 类型标注（show-replay.js）。

### 修复

- 修复图标渲染：使用 `player.id_key_name()` 替代 `display_name` 生成图标，确保召唤物/分身正确关联主人（fight.rs）。

## [0.2.1] - 2026-05-07

### 变更

- 去掉 `tswn_core` 依赖的显式 `version` 字段，仅保留 `path` 引用。

### 同步

- JS 示例文件进一步对齐 snake_case 字段名（`iconClassId` → `icon_class_id` 等）。

## [0.2.0] - 2026-05-07

### ⚠️ Breaking Changes

- **所有 JSON 字段从 camelCase 切换为 snake_case**：移除所有 `#[serde(rename_all = "camelCase")]` 属性，字段名直接使用 Rust 侧原生命名。

#### 影响到的数据模型

| 结构体                   | 变更字段示例                                                                                                                                                                                                                                                                                                               |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PlayerMeta`             | `idName` → `id_name`、`displayName` → `display_name`、`iconPngBase64` → `icon_png_base64`、`teamIndex` → `team_index`                                                                                                                                                                                                      |
| `PlayerState`            | `maxHp` → `max_hp`、`mp` → `magic_point`、`movePoint` → `move_point`、`ownerId` → `owner_id`、`allSum` → `all_sum`、`nameFactor` → `name_factor`、`atBoost` → `at_boost`、`statusLabels` → `status_labels`、`teamIndex` → `team_index`、`idName` → `id_name`、`displayName` → `display_name`、`minionKind` → `minion_kind` |
| `RoundFrame`             | `totalDelay` → `total_delay`、`winnerIds` → `winner_ids`                                                                                                                                                                                                                                                                   |
| `FightReplay`            | `winnerIds` → `winner_ids`、`finalStates` → `final_states`                                                                                                                                                                                                                                                                 |
| `FightSummary`           | `winnerIds` → `winner_ids`、`finalStates` → `final_states`                                                                                                                                                                                                                                                                 |
| `UpdateView`             | `casterId` → `caster_id`、`targetId` → `target_id`、`targetIds` → `target_ids`、`updateType` → `update_type`、`messageTemplate` → `message_template`、`messageRendered` → `message_rendered`                                                                                                                               |
| `WinRateProgress`        | `roundsDone` → `rounds_done`、`totalRounds` → `total_rounds`                                                                                                                                                                                                                                                               |
| `WinRateTiming`          | `initNanos` → `init_nanos`、`fightNanos` → `fight_nanos`                                                                                                                                                                                                                                                                   |
| `WinRateResult`          | `roundsDone` → `rounds_done`、`totalRounds` → `total_rounds`                                                                                                                                                                                                                                                               |
| `FightOptions`（输入）   | `evalRq` → `eval_rq`、`includeIcons` → `include_icons`、`captureReplay` → `capture_replay`                                                                                                                                                                                                                                 |
| `WinRateOptions`（输入） | `evalRq` → `eval_rq`                                                                                                                                                                                                                                                                                                       |

#### JS 示例文件同步更新

- `demo.js`、`show.js`、`show-render.js`、`show-replay.js`、`show-utils.js`、`show-wasm.js`：全部字段访问已改为 snake_case。

### 变更

- 依赖 `tswn_core` 从 `0.2.20` 更新为 `0.3`。
- `PlayerState.mp` → `magic_point`（与 core 层同步）。

## [0.1.2] - 2026-05-06

### 新增

- **状态标签数值化**：所有状态标签带上具体数值。中毒显示层数、铁壁/疾走显示加成值、诅咒显示倍率、狂暴/魅惑/迟缓显示剩余回合。数值为 0 时省略后缀。
- **守护标签双向展示**：被保护者显示 `被 #id 守护`，守护者显示 `守护 #id 中`（通过 `SkillTrait::protect_to_id()` 查询）。同一角色可同时显示两个标签。
- **`SkillTrait::protect_to_id()` 支持**：配套 core 层新增方法，用于在 WASM 层查询 ProtectSkill 当前保护目标。
- **玩家 ID 显示**：左侧角色名字旁显示 ` #playerId` 灰色小字。
- **空格快捷键**：按空格切换暂停/恢复播放。

### 优化

- **列布局调整**：从 5 列缩减为 4 列，合并蓝量和体力为一列 `mp / movePoint%`，名字列加宽。
  - 新列序：角色 | HP | 蓝量/体力 | 状态
- **体力改为百分比**：体力列从原始 `movePoint` 值改为 `movePoint / 2048 * 100` 百分比显示。
- **列名简化**：蓝量/体力列标题不带括号后缀。

### 样式

- `.player-name-head` 宽度从 `118px` 增加到 `150px`，名字区更宽。
- 新增 `.player-mp-move-cell` 样式：合并单元格字体略小（`0.85em`），mp 值蓝色、体力值黑色。
- 新增 `.player-id` 样式：灰色 `0.95em`，间距 `4px`。

### 修复

- 修复 clippy 警告：`collapsible_if`（protect 标签检测嵌套 if → `&& let` 语法）。

## [0.1.1] - 2026-05-04

### 新增

- 头像 CSS Sprite 系统：新增 `iconClassName()`、`buildIconClassCss()`、`withTeamIconClassIds()`、`renderIconSprite()`，头像从 `<img src="data:...">` 改为 `<span class="icon_N">` + background-image 方式渲染，减少 DOM 节点数，便于统一管理头像样式。
- `withTeamIconClassIds()` 确保同队玩家使用该队首个玩家的头像编号，多对多场景下整队头像一致。
- 新增 `playbackCheckpoints` 检查点缓存系统，每 `SEEK_CHECKPOINT_FRAME_INTERVAL`（20）帧保存一次 DOM 快照，回退渲染时优先从最近检查点恢复，大幅加速回退操作。
- `PlayerState` 新增 `idName` / `displayName` 字段，WASM 层直接暴露玩家的真实 id_name 与 display_name，前端不再对召唤单位一律退化为"幻影 #id"。
- `PlayerState` 新增 `minionKind` 字段（`clone` / `summon` / `shadow` / `zombie`），前端可根据 minion 种类定制显示名。
- 新增 `MinionKindView` 枚举及从 `tswn_core::MinionKind` 的转换实现。
- 新增 `replayDisplayName()` 统一格式化回放中的显示名：clone 追加 `#playerId`，summon/shadow/zombie 分别使用“使魔”/“幻影”/“丧尸”基底名并追加 `#playerId`。
- 新增 `syntheticPlayerFromState()` 辅助函数，基于 state 数据生成可渲染的玩家对象（含 minion 显示名逻辑）。
- 新增 seed 行展示：前端从原始输入中提取 `seed:` 行并渲染到玩家列表顶部（`show-wasm.js` `extractSpecifiedSeedLine()` + `show-render.js` `seedRowHtml()`）。
- 新增 `.seed-row` / `.seed-label` / `.seed-value` CSS 样式。
- `FightReplay` 类型新增 `seedLine` 字段。

### 优化

- 暂停播放时自动切回 normal 速度模式，避免 fast/turbo 在暂停后产生突兀的播放体验。
- 重构 `show-render.js`：消除 4 处重复的幻影/分身玩家对象创建逻辑，统一使用 `syntheticPlayerFromState()`。
- `FightSession::build_frame()` 改为每帧从state实时提取玩家名，去掉缓存字段 `player_names`，避免各帧间名字不一致。
- `winnerNamesText()` 改为优先从 `replay.finalStates` 中解析胜者名，支持 minion 胜者的正确显示。

### 播放引擎重构

- `show.js` 重写播放架构：引入 `prepareReplayPlan()` 预计算渲染计划（`currentPlan` + `flatChunks`），替代旧的逐帧 `playReplay()` 循环。
- `renderPlaybackToCursor()` 重写：支持增量追加 chunk、基于检查点的回退恢复、`forceReset` 选项，不再每次回退都全量重置。
- 新增 `findNearestPlaybackCheckpointCursor()` / `restorePlaybackCheckpoint()` / `storePlaybackCheckpoint()` 检查点管理函数。
- 新增 `appendChunksBetween()` 在任意区间（startCursor ~ targetCursor）追加 chunk，替代旧的全量重置式渲染。
- 新增暂停/继续系统：`pauseBtn`、`playbackPaused` 状态、`autoplayFromCurrentCursor()` 支持被打断的延迟等待。
- 新增单步控制：4 个按钮（后退/前进一个 event、后退/前进一帧）+ 键盘快捷键（←→ 步进 event，↑↓ 步进帧），仅暂停模式下有效。
- 新增 `renderPlaybackToCursor()` 支持回退到任意位置重新渲染。
- 新增 `syncPlaybackUi()` 统一刷新所有按钮/文本的 UI 状态。
- 速度切换（fast/turbo）在暂停态下自动恢复播放。

### 战斗结算

- `show-replay.js` 新增 `buildReplayResultSummary()` 统计系统：按原版口径累加 score、归属 kills 到 root owner、记录致命一击。
- `buildReplayResultSummary()` 中记录 `finalState` 和 `iconClassId`，结算表格每行显示最终 HP 条（`summaryHpBarHtml()`），直观展示存活角色剩余血量。
- 新增 `buildReplayResultTableHtml()` 生成胜者/败者结算表格（得分/击杀/致命一击列）。
- `show.js` `renderEndPanel()` 和 `appendReplayResultBlock()` 独立出结束逻辑，回放完成后自动展示结算表。

### UI 样式

- 新增 `.icon-sprite` 统一样式类（16×16，background-image + background-size），替代原先分散在 `.sgl`、`.msg-avatar`、`.summary-actor-icon` 上的重复头像样式。
- 新增结算表格样式体系：`.result-table*`（自适应宽度，不填满）、`.summary-actor*`（角色头像+名称）。
- 新增 `.summary-actor-body` 网格布局容器，容纳角色名 + HP 条纵向排列。
- 新增 `.summary-actor.has-hp` / `.summary-actor-hp` 样式，支持结算表格中 HP 条对齐。
- 新增 `.step-controls` 网格布局（2×2 的 34px 按钮），通过 `.right-controls` 容器精确居中于暂停按钮上方。
- 新增 `.micon.is-paused` 暂停按钮背景色切换。
- 新增 `#endPanel` 宽度限制，响应式布局适配。
- 统一四个 step 按钮的 SVG 图标间距：bar 宽 2px、三角宽 8px、间距 3px。

### 按钮与快捷键

- 四个 step 按钮 title 标注对应快捷键（←→↑↓）。
- 暂停按钮不再切换图标，保持暂停符号。

### 修复

- 修复回放结束后反复按前进按钮不断重渲染玩家列表和重复追加结算表的问题：`renderPlaybackToCursor()` 中游标没动且完成状态没变时提前返回；`appendReplayResultBlock()` 追加前先移除已有 `.battle-result-block`。
- 修复回放结束后暂停按钮被禁用的问题：`syncPlaybackUi()` 中 `pauseBtn.disabled` 不再依赖 `playbackFinished`，回放完成后仍可暂停/步进操作。
- 修复 `renderPlaybackToCursor()` 早期返回守卫误拦截 `forceReset` 导致头像渲染失效的问题：将 `forceReset` 分支移到最前面，避免初始化渲染被跳过。
- 修复结算表格行高过高的问题：缩减 `.result-table th/td` 的 `padding` 为 `2px 6px`，同时保持字号 `15px`、行高 `16px`，使表格更紧凑。
- 修复结算表格中胜者只显示最后存活玩家的问题：`buildReplayResultSummary()` 改为使用 `replay.winnerIds`（获胜队伍全员 roster）区分胜者/败者，而非仅依赖 `alive` 状态。
- 修复结算表格中人名下划线被截断的问题：`.summary-actor-name` 单独设置 `line-height: 20px`，避免受 `td` 紧凑行高和 `overflow: hidden` 影响。
- 修复死亡角色在后续帧中反复播放 `oldhp` 清空动画的问题：`.player-row.is-dead .oldhp, .player-row.is-dead .healhp` 禁用 CSS 过渡，死亡帧的 HP 归零动画正常播放，后续帧不再重复。

### 示例

- `show-utils.js`：新增 `iconClassName()`、`buildIconClassCss()`、`withTeamIconClassIds()`、`renderIconSprite()` 头像 Sprite 工具函数。
- `show-render.js`：`actorToken()`、`renderPlayers()`、`syntheticPlayerFromState()` 头像渲染从 `<img>` 改为 `renderIconSprite()`。
- `show.js`：引入 `normalizeReplayPlayers()` 在回放开始时为玩家补齐 `iconClassId`；新增 `ensureIconStyleTag()` / `syncIconStyles()` 动态注入 CSS 背景图规则；播放引擎新增 `playbackCheckpoints` 检查点缓存。
- `show-replay.js`：`actorSummaryHtml()` 支持 `showHp` 选项显示 HP 条；`buildReplayResultSummary()` 中记录 `finalState` 和 `iconClassId`。
- `show.css`：头像样式从分散的 `.sgl`/`.msg-avatar`/`.summary-actor-icon` 统一为 `.icon-sprite`。
- `show-replay.js`：`renderReplayIntro()` 将 `seedLine` 写入 `playerList.dataset`，支持增量更新时保留 seed。
- `show.js`：`FightState` JSDoc 类型标注新增 `idName`、`displayName`、`minionKind`。
- `show-wasm.js`：`loadModule()` 改用 `import.meta.url` 动态解析 `pkg/` 路径，同时尝试 `../pkg/` 和 `./pkg/` 两个候选，兼容 examples/ 子目录和扁平部署两种结构。

## [0.1.0] - 2026-04-29

### 新增

- 新增 `tswn_wasm` crate，提供面向浏览器前端的 `wasm-bindgen` 封装层，不再沿用 `tswn_capi` 的裸指针 / 手动释放式接口。
- 新增顶层 wasm 导出：
  - `version()`
  - `core_version()`
  - `name_to_png_base64(name)`
  - `fight(raw_input, options?)` — 一次性跑完整局，返回 `FightReplay`
  - `fight_summary(raw_input, options?)` — 轻量摘要，返回 `FightSummary`
  - `win_rate_sync(raw_input, total_rounds, options?)` — 同步跑完胜率统计，返回 `WinRateResult`
- 新增 `FightSession`，支持逐回合推进：
  - `players()` — 返回 `PlayerMeta[]`
  - `state()` — 返回当前全量 `PlayerState[]`
  - `is_finished()` — 是否已产生胜者
  - `winner_ids()` — 获胜方 ID 列表
  - `step()` — 推进一步，返回 `RoundFrame`（含 `updates`、`states`、`total_delay`、`finished`、`winner_ids`）
  - `run_to_end(limit?)` — 推进至结束，可选限制最大帧数，返回 `FightReplay`
- 新增 `WinRateSession`，支持浏览器侧分批推进胜率统计：
  - `is_finished()`
  - `progress()` — 返回 `WinRateProgress`
  - `step(batch_size?)` — 推进指定局数（默认 100），返回 `WinRateProgress`
  - `result()` — 返回 `WinRateResult`（含 `timing` 耗时信息）
  - `eval_rq()` — 返回当前使用的 eval_rq 值
- 新增前端即用的数据模型导出：
  - 玩家元数据 `PlayerMeta`（含 `icon_png_base64`，由 `include_icons` 选项控制）
  - 玩家状态快照 `PlayerState`（含 `owner_id` 追溯召唤单位归属）
  - 渲染后的更新消息 `UpdateView`（含 `message_rendered`、`message_template`、`tone`）
  - 消息色调枚举 `MessageTone`：`Normal` / `Damage` / `Recover` / `Knockout`
  - 更新类型枚举 `UpdateTypeView`：`Win` / `None` / `NextLine`
  - 回合帧 `RoundFrame`（含 `total_delay` 供 JS 正常速度播放）
  - 完整回放 `FightReplay`
  - 轻量摘要 `FightSummary`
  - 增量胜率进度 `WinRateProgress` / 胜率结果 `WinRateResult`（含 `WinRateTiming`）
- 新增 `FightOptions`：
  - `eval_rq` — 名称评分参数
  - `include_icons` — 是否包含 PNG Base64 头像
  - `capture_replay` — 是否捕获逐帧回放数据（默认 true，设为 false 可加速 `run_to_end`）
- 新增 `WinRateOptions`：
  - `eval_rq` — 名称评分参数（默认使用 `WIN_RATE_EVAL_RQ`）
  - `thread` — 线程数（wasm 目标下恒为 1）
- 新增错误对象封装：Rust 侧错误统一转成 `{ code, message }` 风格的 JS 可读对象。错误码包括：
  - `INVALID_INPUT`
  - `INVALID_OPTIONS`
  - `RUNNER_INIT_FAILED`
  - `WIN_RATE_INVALID_GROUPS`
  - `INTERNAL_ERROR`

### 兼容性

- `WinRateSession` 内部对齐 `tswn_core` 当前 wasm 兼容策略：浏览器目标下胜率路径保持单线程，避免进入 `std::thread` 分支。
- 战斗日志在 wasm 层额外做显示名渲染（`message_rendered`），同时保留原始模板（`message_template`），不再直接暴露 `RunUpdate::msg()` 的数字 `PlrId` 替换结果。
- wasm32 目标下不测量耗时（`WinRateTiming` 中各 nanos 字段为 0），非 wasm 目标正常记录。

### 状态标签系统（本次追加）

- `PlayerState` 新增 `status_labels: Vec<String>` 字段，序列化时为空则跳过，前端可直接用于渲染玩家状态标签
- 在 `collect_states()` 中为每个玩家收集实时状态标签：
  - **技能运行时态**：`聚气`（Accumulate 激活时）、`蓄力`（Charge 激活时）、`隐匿`（Hide 激活时）、`潜行`（Assassinate 激活时）
  - **状态效果**：`狂暴`、`魅惑`、`诅咒`、`疾走`、`冰冻`、`铁壁`、`中毒`、`守护`、`迟缓`、`垂死`
  - `冰冻` 同时覆盖 `IceState` 与 `status.frozen` 两种来源，去重显示
- 新增 `push_status_label()` 去重辅助函数，避免同一标签被多次添加
- 新增 `has_active_skill()` 泛型辅助函数，通过 `skill_storage()` 遍历玩家技能，按 `debug_skill_type_name()` 后缀匹配及自定义条件筛选

### 逐段渲染系统（本次追加）

- `show-render.js` 新增 `buildFrameRows()` 函数，将一帧拆分为多个渲染 chunk（`battleRows`/`frameBody`/`row`/`delay`），支持 normal/fast 模式按 chunk delay 逐段推进播放
- `show.js` 播放逻辑从逐帧渲染改为逐段渲染：
  - normal/fast 模式按 `chunk.delay / totalFrameDelay * targetFrameDelay` 比例分配每段等待时间
  - turbo 模式保持原有批量缓冲 HTML 逻辑不变
- `show-render.js` 新增状态标签 UI 渲染：
  - `renderStatusPill()` 渲染单个标签 pill
  - `renderPlayerStatusPills()` 渲染玩家所有标签
  - `sidebarStatusLabels()` 安全读取 `state.statusLabels`
  - `statusPillTone()` 根据标签内容分类正/负面色调
- `show.js` 的 `FightState` JSDoc 类型标注同步更新，新增字段：`resistance`, `wisdom`, `point`, `allSum`, `nameFactor`, `atBoost`, `attract`, `statusLabels`

### 样式

- `show.css` 新增 `.player-effects` 布局与间距
- 新增 `.status-pill.positive`（绿色调，用于正面状态）和 `.status-pill.negative`（红色调，用于负面状态）
- 优化 `.player-effects .status-pill` 行高，适配在玩家面板中的紧凑显示

### 示例

- 新增 `examples/` 目录，提供两套静态页面 demo：
  - `demo.html` / `demo.js` / `demo.css`：快速功能验证（战斗 + 胜率）
  - `show.html` / `show.js` / `show.css`：完整对局动画展示
    - `show-wasm.js`：WASM 模块加载与初始化
    - `show-utils.js`：DOM 渲染工具函数
    - `show-render.js`：玩家状态 / 头像渲染
    - `show-replay.js`：逐帧回放逻辑
  - `examples/README.md`：运行说明
- 示例覆盖：
  - 初始化 wasm 包
  - 创建 `FightSession` 并逐步推进回合
  - 调用 `fight_summary(...)`
  - 创建 `WinRateSession` 并分批显示进度
  - 完整对局动画播放（含头像、状态条、更新消息渲染）
