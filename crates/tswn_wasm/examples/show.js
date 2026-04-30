/**
 * @fileoverview tswn_wasm 战斗回放展示页 — 入口模块
 *
 * 这是一个独立的 Fight 展示页，专门模仿原始名字竞技场与 fast-namerena 的战斗观感。
 * 左侧按队伍显示角色状态（HP/速度/体力），右侧按帧逐段追加战斗记录。
 *
 * 典型的 State 结构（来自 WASM）：
 * @typedef {{
 *   id: number,
 *   hp: number,
 *   maxHp: number,
 *   mp: number,
 *   movePoint: number,
 *   speed: number,
 *   agility: number,
 *   magic: number,
 *   attack: number,
 *   defense: number,
 *   alive: boolean,
 *   frozen: boolean,
 *   teamIndex: number,
 *   ownerId?: number
 * }} FightState
 *
 * 典型的 Player 结构（来自 WASM）：
 * @typedef {{
 *   id: number,
 *   teamIndex: number,
 *   idName: string,
 *   displayName: string,
 *   iconPngBase64: string|null
 * }} FightPlayer
 *
 * 一次 Replay 的结构：
 * @typedef {{
 *   rawInput: string,
 *   players: FightPlayer[],
 *   initialStates: FightState[],
 *   frames: FrameUpdate[],
 *   winnerIds: number[],
 *   finalStates: FightState[],
 *   wasmDurationMs: number
 * }} FightReplay
 *
 * 单帧更新的结构：
 * @typedef {{
 *   updates: FrameMessage[],
 *   states: FightState[],
 *   finished: boolean,
 *   winnerIds: number[],
 *   totalDelay: number
 * }} FrameUpdate
 *
 * 单条消息的结构：
 * @typedef {{
 *   updateType?: string,
 *   messageRendered?: string,
 *   messageTemplate?: string,
 *   casterId?: number,
 *   targetId?: number,
 *   targetIds?: number[],
 *   param?: number,
 *   score?: number,
 *   delay1?: number,
 *   delay0?: number,
 *   tone?: MessageTone
 * }} FrameMessage
 *
 * HP 条的布局度量：
 * @typedef {{
 *   totalWidth: number,
 *   fillWidth: number,
 *   previousWidth: number,
 *   deltaLeft: number,
 *   deltaWidth: number
 * }} HpMetrics
 *
 * 涉及高亮的角色集合（用于 renderPlayers 的 involved 参数）：
 * @typedef {{
 *   casters: Set<number>,
 *   targets: Set<number>
 * }} InvolvedSet
 *
 * 消息色调：
 * @typedef {'normal' | 'damage' | 'recover' | 'knockout'} MessageTone
 *
 * 播放速度模式：
 * @typedef {'normal' | 'fast' | 'turbo'} SpeedMode
 */

import { formatError, sleep } from './show-utils.js';
import { renderIdleState, renderPlayers, buildFrameHtml } from './show-render.js';
import {
    renderReplayIntro,
    updateSpeedButtons,
    playbackDelay,
    winnerNamesText,
} from './show-replay.js';
import { ensureApi, buildReplay } from './show-wasm.js';

// ============================================================================
// 默认示例输入 — 可在页面中直接点击"示例"按钮填入
// ============================================================================

/** @type {string} */
const DEFAULT_RAW = `
云剑狄卡敢
白胡子

史莱姆
田一人
`.trim();

/** @type {string} localStorage 键名，用于跨会话记住用户输入 */
const INPUT_STORAGE_KEY = "tswn_wasm_show_input";

// ============================================================================
// DOM 元素引用
// ============================================================================

/** @type {HTMLElement} */
const playerList = document.querySelector("#playerList");
/** @type {HTMLElement} */
const battleRows = document.querySelector("#battleRows");
/** @type {HTMLInputElement} */
const inputName = document.querySelector("#input_name");
/** @type {HTMLElement} */
const inputPanel = document.querySelector("#inputPanel");
/** @type {HTMLElement} */
const endPanel = document.querySelector("#endPanel");
/** @type {HTMLElement} */
const inputStatus = document.querySelector("#inputStatus");
/** @type {HTMLElement} */
const plistMeta = document.querySelector("#plistMeta");
/** @type {HTMLElement} */
const headerMeta = document.querySelector("#headerMeta");
/** @type {HTMLElement} */
const winnerNames = document.querySelector("#winnerNames");
/** @type {HTMLElement} */
const winnerNote = document.querySelector("#winnerNote");

/** @type {HTMLElement} */
const versionInfo = document.querySelector("#versionInfo");
/** @type {HTMLElement} */
const coreVersionInfo = document.querySelector("#coreVersionInfo");
/** @type {HTMLElement} */
const modulePathInfo = document.querySelector("#modulePathInfo");

/** @type {HTMLButtonElement} */
const startBtn = document.querySelector("#startBtn");
/** @type {HTMLButtonElement} */
const sampleBtn = document.querySelector("#sampleBtn");
/** @type {HTMLButtonElement} */
const closeInputBtn = document.querySelector("#closeInputBtn");
/** @type {HTMLButtonElement} */
const closeEndBtn = document.querySelector("#closeEndBtn");
/** @type {HTMLButtonElement} */
const playAgainBtn = document.querySelector("#playAgainBtn");
/** @type {HTMLButtonElement} */
const editNamesBtn = document.querySelector("#editNamesBtn");
/** @type {HTMLButtonElement} */
const inputBtn = document.querySelector("#inputBtn");
/** @type {HTMLButtonElement} */
const fastBtn = document.querySelector("#fastBtn");
/** @type {HTMLButtonElement} */
const turboBtn = document.querySelector("#turboBtn");
/** @type {HTMLButtonElement} */
const refreshBtn = document.querySelector("#refreshBtn");

// ============================================================================
// 全局状态
// ============================================================================

/** @type {FightReplay|null} 当前已生成的回放数据 */
let currentReplay = null;
/** @type {number} 每次播放会话递增的 token，用于中断旧的播放循环 */
let playbackToken = 0;
/** @type {SpeedMode} 当前播放速度模式 */
let speedMode = 'normal';
/** @type {Map<number, FightPlayer>} playerId → 玩家对象的快速索引 */
let playersById = new Map();

// 页面初始化时尝试恢复上次保存的输入
restoreInputValue();

// ============================================================================
// 胶水函数（直接操作 DOM 或全局状态）
// ============================================================================

/**
 * 记住玩家列表，建立 id→player 的快速查找表。
 * 原地更新 Map（不替换引用），确保所有持有该 Map 引用的调用方都能看到最新数据。
 * @param {FightPlayer[]} players
 */
function rememberPlayers(players) {
    playersById.clear();
    for (const player of players) {
        playersById.set(player.id, player);
    }
}

/**
 * 设置输入面板下方的状态提示文本。
 * @param {string} message — 提示消息
 * @param {boolean} [isError=false] — 是否标记为错误样式
 */
function setInputStatus(message, isError = false) {
    inputStatus.textContent = message;
    inputStatus.classList.toggle("error", isError);
}

/**
 * 切换开始/示例按钮的 loading 态。
 * @param {boolean} loading
 */
function setLoading(loading) {
    startBtn.disabled = loading;
    sampleBtn.disabled = loading;
}

// ============================================================================
// localStorage 持久化
// ============================================================================

/** 从 localStorage 恢复上次输入，若无则使用默认示例 */
function restoreInputValue() {
    try {
        const savedValue = window.localStorage.getItem(INPUT_STORAGE_KEY)?.trim();
        inputName.value = savedValue ? savedValue : DEFAULT_RAW;
    } catch {
        inputName.value = DEFAULT_RAW;
    }
}

/** 将当前输入框内容持久化到 localStorage */
function persistInputValue() {
    try {
        window.localStorage.setItem(INPUT_STORAGE_KEY, inputName.value);
    } catch {
        // 即使存储不可用，内存中的输入仍然可用。
    }
}

// ============================================================================
// 面板开关
// ============================================================================

/**
 * 打开指定面板（设置 hidden=false）。
 * @param {HTMLElement} panel
 */
function openPanel(panel) {
    panel.hidden = false;
}

/**
 * 关闭指定面板（设置 hidden=true）。
 * @param {HTMLElement} panel
 */
function closePanel(panel) {
    panel.hidden = true;
}

/**
 * 打开输入编辑面板，可选是否全选文本。
 * @param {boolean} [selectAll=false] — 是否自动全选输入框内容
 */
function openInputEditor(selectAll = false) {
    openPanel(inputPanel);
    window.requestAnimationFrame(() => {
        inputName.focus();
        if (selectAll) {
            inputName.select();
        }
    });
}

// ============================================================================
// 回放播放主循环
// ============================================================================

/**
 * 自动播放整场回放。
 * — normal/fast 模式：逐帧渲染 DOM 并等待 delay
 * — turbo 模式：批量缓冲 HTML，约每 16ms 写入一次 DOM 并让出主线程
 *
 * @param {FightReplay} replay
 * @returns {Promise<void>}
 */
async function playReplay(replay) {
    const token = ++playbackToken;
    const frontendStart = performance.now();
    closePanel(endPanel);
    renderReplayIntro(replay, speedMode, playerList, battleRows, plistMeta, headerMeta, playersById, rememberPlayers);
    let previousStates = replay.initialStates;

    let htmlBuffer = "";
    let lastRenderTime = performance.now();

    for (const [index, frame] of replay.frames.entries()) {
        // 如果播放 token 已变更（用户触发了新的播放），中止当前循环
        if (token !== playbackToken) {
            return;
        }

        const frameHtml = buildFrameHtml(frame, index, previousStates, playersById);

        if (speedMode !== 'turbo') {
            // 正常/快进模式：逐帧渲染 DOM 并等待
            if (frameHtml) {
                battleRows.insertAdjacentHTML("beforeend", frameHtml);
                const hbody = battleRows.closest(".hbody");
                if (hbody) hbody.scrollTop = hbody.scrollHeight;
            }

            // 构建当前帧涉及的角色集合，用于左侧高亮
            const involved = { casters: new Set(), targets: new Set() };
            for (const update of frame.updates) {
                if (update.casterId != null) involved.casters.add(update.casterId);
                if (update.targetId != null) involved.targets.add(update.targetId);
                if (Array.isArray(update.targetIds)) update.targetIds.forEach((id) => involved.targets.add(id));
            }
            renderPlayers(replay.players, frame.states, previousStates, involved, playerList, playersById);
            previousStates = frame.states;
            await sleep(playbackDelay(frame, speedMode));

        } else {
            // Turbo 模式：批量缓冲 HTML，取消帧间 sleep
            if (frameHtml) htmlBuffer += frameHtml;
            previousStates = frame.states;

            const now = performance.now();
            // 每 ~16ms（约 60FPS 的间隔）才进行一次 DOM 实际写入和 UI 释放，防止页面卡死
            if (now - lastRenderTime > 16) {
                if (htmlBuffer) {
                    battleRows.insertAdjacentHTML("beforeend", htmlBuffer);
                    htmlBuffer = "";
                    const hbody = battleRows.closest(".hbody");
                    if (hbody) hbody.scrollTop = hbody.scrollHeight;
                }
                // 左侧面板也只在这个切片点进行增量更新（turbo 下不高亮具体角色）
                renderPlayers(replay.players, frame.states, previousStates, null, playerList, playersById);

                await sleep(0); // 短暂让出执行权，让浏览器绘制画面
                lastRenderTime = performance.now();
            }
        }
    }

    if (token !== playbackToken) {
        return;
    }

    // 循环结束后，清空可能残余的 buffer
    if (speedMode === 'turbo' && htmlBuffer) {
        battleRows.insertAdjacentHTML("beforeend", htmlBuffer);
        const hbody = battleRows.closest(".hbody");
        if (hbody) hbody.scrollTop = hbody.scrollHeight;
    }

    // 最后刷新一次左侧面板（使用最终状态）
    renderPlayers(replay.players, replay.finalStates, previousStates, null, playerList, playersById);
    const frontendDurationMs = performance.now() - frontendStart;
    winnerNames.textContent = winnerNamesText(replay);
    winnerNote.innerHTML = [
        `共播放 ${replay.frames.length} 帧，winnerIds=${JSON.stringify(replay.winnerIds)}。`,
        `WASM 战斗计算: ${replay.wasmDurationMs?.toFixed(1) ?? '?'} ms`,
        `前端展示耗时: ${frontendDurationMs.toFixed(1)} ms（含等待）`,
    ].join('<br>');
    openPanel(endPanel);
}

// ============================================================================
// 用户操作入口
// ============================================================================

/**
 * 开始一场新战斗：校验输入 → 生成回放 → 自动播放。
 * @returns {Promise<void>}
 */
async function startBattle() {
    const rawInput = inputName.value.trim();
    if (!rawInput) {
        setInputStatus("请输入至少一个名字。", true);
        openInputEditor();
        return;
    }

    persistInputValue();
    playbackToken += 1;
    setLoading(true);
    setInputStatus("正在生成回放，请稍候...");
    closePanel(endPanel);

    try {
        currentReplay = await buildReplay(rawInput, versionInfo, coreVersionInfo, modulePathInfo);
        setInputStatus("回放已生成，开始自动播放。");
        closePanel(inputPanel);
        await playReplay(currentReplay);
    } catch (error) {
        setInputStatus(formatError(error), true);
        openInputEditor();
    } finally {
        setLoading(false);
    }
}

/**
 * 重播当前回放（不重新生成）。
 * @returns {Promise<void>}
 */
async function replayCurrent() {
    if (!currentReplay) {
        openInputEditor();
        return;
    }
    await playReplay(currentReplay);
}

// ============================================================================
// 事件绑定
// ============================================================================

// 示例按钮：填入默认示例输入
sampleBtn.addEventListener("click", () => {
    inputName.value = DEFAULT_RAW;
    persistInputValue();
    setInputStatus("已填入示例输入。");
    openInputEditor(true);
});

// 开始按钮：启动战斗
startBtn.addEventListener("click", () => {
    void startBattle();
});

// 再来一局：关闭结束面板，重播当前回放
playAgainBtn.addEventListener("click", () => {
    closePanel(endPanel);
    void replayCurrent();
});

// 刷新按钮：重播当前回放
refreshBtn.addEventListener("click", () => {
    void replayCurrent();
});

// 输入按钮：打开输入编辑面板
inputBtn.addEventListener("click", () => {
    openInputEditor();
});

// 编辑名字按钮：关闭结束面板并打开输入编辑
editNamesBtn.addEventListener("click", () => {
    closePanel(endPanel);
    openInputEditor(true);
});

// 快进按钮：切换 normal ↔ fast
fastBtn.addEventListener("click", () => {
    speedMode = speedMode === 'fast' ? 'normal' : 'fast';
    updateSpeedButtons(fastBtn, turboBtn, speedMode, currentReplay, headerMeta);
});

// 极速按钮：切换 normal ↔ turbo
turboBtn.addEventListener("click", () => {
    speedMode = speedMode === 'turbo' ? 'normal' : 'turbo';
    updateSpeedButtons(fastBtn, turboBtn, speedMode, currentReplay, headerMeta);
});

// 关闭输入面板（仅在已有回放时允许关闭）
closeInputBtn.addEventListener("click", () => {
    if (currentReplay) {
        closePanel(inputPanel);
    }
});

// 关闭结束面板
closeEndBtn.addEventListener("click", () => {
    closePanel(endPanel);
});

// 输入框内容变化时自动持久化
inputName.addEventListener("input", () => {
    persistInputValue();
});

// Ctrl+Enter / Cmd+Enter 快捷开始
inputName.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        void startBattle();
    }
});

// ============================================================================
// 入口
// ============================================================================

/**
 * 页面入口：渲染空闲 UI，初始化 WASM 模块。
 * @returns {Promise<void>}
 */
async function main() {
    renderIdleState(playerList, battleRows, plistMeta, headerMeta);
    updateSpeedButtons(fastBtn, turboBtn, speedMode, currentReplay, headerMeta);
    setInputStatus("会使用 show 风格自动播放整场战斗。");
    openInputEditor();

    try {
        await ensureApi(versionInfo, coreVersionInfo, modulePathInfo);
        setInputStatus("tswn_wasm 已初始化，可以开始。");
    } catch (error) {
        setInputStatus(`模块加载失败: ${formatError(error)}`, true);
    }
}

void main();
