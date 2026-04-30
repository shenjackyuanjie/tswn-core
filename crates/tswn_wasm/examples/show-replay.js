/**
 * @fileoverview tswn_wasm 战斗回放展示页 — 回放播放辅助
 *
 * 提供回放介绍渲染、速度按钮状态更新、帧延迟计算、获胜者文本等辅助函数。
 * playReplay 主循环因需要频繁读写全局状态（playbackToken 等），保留在 show.js 中。
 */

import { renderPlayers } from './show-render.js';

// ============================================================================
// 回放介绍与速度控制
// ============================================================================

/**
 * 渲染回放开始前的介绍信息（角色数量、队伍数、帧数等）。
 *
 * @param {FightReplay} replay
 * @param {SpeedMode} speedMode — 当前播放速度
 * @param {HTMLElement} playerList
 * @param {HTMLElement} battleRows
 * @param {HTMLElement} plistMeta
 * @param {HTMLElement} headerMeta
 * @param {Map<number, FightPlayer>} playersById — 会被 rememberPlayers 覆写
 * @param {(players: FightPlayer[]) => void} rememberPlayers — 更新 playersById 的回调
 */
export function renderReplayIntro(replay, speedMode, playerList, battleRows, plistMeta, headerMeta, playersById, rememberPlayers) {
    const teamCount = new Set(replay.players.map((player) => player.teamIndex)).size;
    rememberPlayers(replay.players);
    plistMeta.textContent = `${replay.players.length} 名角色 · ${teamCount} 支队伍 · ${replay.frames.length} 帧回放。`;
    const labels = { normal: '正常速度', fast: '快进模式', turbo: '极速模式（无延时）' };
    headerMeta.textContent = `当前是${labels[speedMode]}，会自动推进 ${replay.frames.length} 帧。`;
    battleRows.innerHTML = `
        <div class="welcome">
            <div><strong>战斗已经开始。</strong></div>
            <div>下面会按回合逐段追加战斗事件，左侧状态栏会同步刷新 HP、MP 与存活状态。</div>
            <div>右下角两个速度按钮可切换快进或极速模式。</div>
        </div>
    `;
    renderPlayers(replay.players, replay.initialStates, replay.initialStates, null, playerList, playersById);
}

/**
 * 根据当前 speedMode 更新快进/极速按钮的激活样式。
 *
 * @param {HTMLButtonElement} fastBtn
 * @param {HTMLButtonElement} turboBtn
 * @param {SpeedMode} speedMode
 * @param {FightReplay|null} currentReplay
 * @param {HTMLElement} headerMeta
 */
export function updateSpeedButtons(fastBtn, turboBtn, speedMode, currentReplay, headerMeta) {
    fastBtn.classList.toggle("is-active", speedMode === 'fast');
    turboBtn.classList.toggle("is-active", speedMode === 'turbo');
    if (currentReplay) {
        const labels = { normal: '正常速度', fast: '快进模式', turbo: '极速模式（无延时）' };
        headerMeta.textContent = `当前是${labels[speedMode]}，会自动推进 ${currentReplay.frames.length} 帧。`;
    }
}

/**
 * 根据当前速度模式和帧的延迟配置，计算本帧应等待的毫秒数。
 * turbo 模式返回 0；fast 模式固定 40ms；normal 取帧内所有 update 的 delay 累加。
 *
 * @param {FrameUpdate} frame
 * @param {SpeedMode} speedMode
 * @returns {number} 等待毫秒数
 */
export function playbackDelay(frame, speedMode) {
    if (speedMode === 'turbo') {
        return 0;
    }
    if (speedMode === 'fast') {
        return 40;
    }
    return frame.updates.reduce((value, update) => value + (update.delay1 || update.delay0 || 0), 0);
}

/**
 * 根据回放中的 winnerIds 拼接获胜者名字。
 *
 * @param {FightReplay} replay
 * @returns {string} 如 "张三、李四" 或 "未分出胜负"
 */
export function winnerNamesText(replay) {
    const playersById = new Map(replay.players.map((player) => [player.id, player]));
    const names = replay.winnerIds.map((winnerId) => playersById.get(winnerId)?.displayName ?? `#${winnerId}`);
    return names.length ? names.join("、") : "未分出胜负";
}
