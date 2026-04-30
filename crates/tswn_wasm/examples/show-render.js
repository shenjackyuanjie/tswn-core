/**
 * @fileoverview tswn_wasm 战斗回放展示页 — 渲染函数
 *
 * 包含消息行内的角色 token、模板消息渲染、玩家状态面板渲染、战斗帧 HTML 构建。
 * 所有函数通过参数接收 DOM 引用和全局状态（playersById），不直接依赖模块级变量。
 */

import {
    escapeHtml,
    iconSrc,
    actorHpMetrics,
    formatMessageText,
    statusText,
    buildStateMap,
    phantomDisplayName,
    classifyMessage,
} from './show-utils.js';

// ============================================================================
// 角色 Token 渲染（消息行里的小头像 + 名字 + HP 条）
// ============================================================================

/**
 * 渲染一个角色在消息行中的 token（小头像 + HP mini bar + 名字）。
 * @param {FightPlayer} player — 玩家对象
 * @param {FightState} state — 当前状态
 * @param {FightState} previousState — 上一帧状态
 * @param {{ showHp?: boolean }} [options] — 是否显示 HP mini bar
 * @returns {string} HTML 字符串
 */
export function actorToken(player, state, previousState, { showHp = true } = {}) {
    const hpMetrics = showHp ? actorHpMetrics(state, previousState) : null;
    const hpBar = hpMetrics
        ? `
            <span class="actor-hp" style="width:${hpMetrics.totalWidth}px">
                <span class="actor-hp-fill" style="width:${hpMetrics.fillWidth}px"></span>
                ${hpMetrics.deltaWidth > 0 ? `<span class="actor-hp-delta" style="left:${hpMetrics.deltaLeft}px;width:${hpMetrics.deltaWidth}px"></span>` : ""}
            </span>
        `
        : "";
    const hpClass = hpMetrics ? " has-hp" : "";

    return `<span class="actor-token${hpClass}"><span class="actor-avatar-wrap"><img class="msg-avatar" src="${iconSrc(player.iconPngBase64)}" alt="" aria-hidden="true">${hpBar}</span><span class="actor-name">${escapeHtml(player.displayName)}</span></span>`;
}

/**
 * 根据 playerId 渲染一个角色 token，自动处理幻影/未知角色。
 * @param {number} playerId
 * @param {Map<number, FightState>} stateMap — 当前状态 Map
 * @param {Map<number, FightState>} [previousStateMap] — 上一帧状态 Map
 * @param {Map<number, FightPlayer>} playersById — playerId → 玩家对象索引
 * @param {{ showHp?: boolean }} [options]
 * @returns {string} HTML 字符串
 */
export function renderActorById(playerId, stateMap, previousStateMap, playersById, options) {
    const player = playersById.get(playerId);
    if (!player) {
        return escapeHtml(phantomDisplayName(playerId));
    }

    const state = stateMap.get(player.id);
    const previousState = previousStateMap?.get(player.id) ?? state;
    return actorToken(player, state, previousState, options);
}

/**
 * 渲染消息模板中的 [2] 参数（目标列表或数值）。
 * @param {FrameMessage} update — 当前消息
 * @param {MessageTone} tone
 * @param {Map<number, FightState>} stateMap
 * @param {Map<number, FightState>} previousStateMap
 * @param {Map<number, FightPlayer>} playersById
 * @returns {string} HTML 字符串
 */
export function renderMessageParam(update, tone, stateMap, previousStateMap, playersById) {
    if (Array.isArray(update.targetIds) && update.targetIds.length) {
        return update.targetIds
            .map((playerId) => renderActorById(playerId, stateMap, previousStateMap, playersById, { showHp: true }))
            .join(",");
    }

    const value = update.param ?? update.score;
    if (value == null) {
        return "";
    }

    const html = escapeHtml(String(value));
    if (tone === "damage" || tone === "recover") {
        return `<span class="message-number">${html}</span>`;
    }
    return html;
}

/**
 * 渲染带模板占位符的消息：[0]=caster, [1]=target, [2]=param。
 * @param {FrameMessage} update
 * @param {MessageTone} tone
 * @param {Map<number, FightState>} stateMap
 * @param {Map<number, FightState>} previousStateMap
 * @param {Map<number, FightPlayer>} playersById
 * @returns {string} HTML 字符串
 */
export function renderTemplateMessage(update, tone, stateMap, previousStateMap, playersById) {
    const template = `${update.messageTemplate ?? ""}`;
    if (!template) {
        return formatMessageText(`${update.messageRendered ?? ""}`, tone);
    }

    return template
        .split(/(\[[012]\])/g)
        .filter((part) => part.length > 0)
        .map((part) => {
            if (part === "[0]") {
                // 施法者 — 不显示 HP
                return renderActorById(update.casterId, stateMap, previousStateMap, playersById, { showHp: false });
            }
            if (part === "[1]") {
                // 目标 — 显示 HP
                return renderActorById(update.targetId, stateMap, previousStateMap, playersById, { showHp: true });
            }
            if (part === "[2]") {
                // 参数（目标列表或数值）
                return renderMessageParam(update, tone, stateMap, previousStateMap, playersById);
            }
            return formatMessageText(part, tone);
        })
        .join("");
}

/**
 * 高亮渲染一条消息（模板或纯文本），是 renderTemplateMessage 的别名。
 * @param {FrameMessage} update
 * @param {MessageTone} tone
 * @param {Map<number, FightState>} stateMap
 * @param {Map<number, FightState>} previousStateMap
 * @param {Map<number, FightPlayer>} playersById
 * @returns {string}
 */
export function highlightMessage(update, tone, stateMap, previousStateMap, playersById) {
    return renderTemplateMessage(update, tone, stateMap, previousStateMap, playersById);
}

// ============================================================================
// 初始 / 空闲状态渲染
// ============================================================================

/**
 * 战斗未开始时的占位内容渲染。
 * @param {HTMLElement} playerList
 * @param {HTMLElement} battleRows
 * @param {HTMLElement} plistMeta
 * @param {HTMLElement} headerMeta
 */
export function renderIdleState(playerList, battleRows, plistMeta, headerMeta) {
    playerList.innerHTML = `
        <div class="welcome">
            <div><strong>战斗还没开始。</strong></div>
            <div>左侧会按队伍显示角色状态，右侧则按原版风格逐段追加战斗记录。</div>
            <div>你可以直接用默认示例点击开始，也可以改成自己的输入。</div>
        </div>
    `;
    battleRows.innerHTML = `
        <div class="welcome">
            <div><strong>show.html 是单独的 Fight 展示页。</strong></div>
            <div>它不再混合胜率功能，而是专门模仿原始名字竞技场与 fast-namerena 的战斗观感。</div>
        </div>
    `;
    plistMeta.textContent = "输入名字后点击开始，左侧会显示角色状态，右侧自动播放整场战斗。";
    headerMeta.textContent = "目前显示的是 show 风格回放视图。";
}

// ============================================================================
// 玩家状态面板渲染
// ============================================================================

/**
 * 渲染左侧玩家状态面板。
 * — 首次渲染：全量 innerHTML
 * — 后续渲染：增量更新现有 DOM 元素属性
 *
 * @param {FightPlayer[]} players — 玩家列表
 * @param {FightState[]} states — 当前帧状态
 * @param {FightState[]} [previousStates=states] — 上一帧状态
 * @param {InvolvedSet|null} [involved=null] — 当前帧涉及的角色（用于高亮 caster/target）
 * @param {HTMLElement} playerList — 左侧面板容器 DOM
 * @param {Map<number, FightPlayer>} playersById — playerId → 玩家对象索引（会被写入幻影/分身条目）
 */
export function renderPlayers(players, states, previousStates = states, involved = null, playerList, playersById) {
    const stateMap = buildStateMap(states);
    const previousStateMap = buildStateMap(previousStates);

    // 补上 states 里有但初始 players 里没有的召唤单位（幻影/分身）
    const knownIds = new Set(players.map((p) => p.id));
    const allPlayers = [...players];
    for (const state of states) {
        if (!knownIds.has(state.id)) {
            knownIds.add(state.id);
            // 幻影/分身使用本体头像
            let icon = null;
            if (state.ownerId != null) {
                const ownerPlayer = playersById.get(state.ownerId);
                if (ownerPlayer) {
                    icon = ownerPlayer.iconPngBase64;
                }
            }
            const phantomPlayer = {
                id: state.id,
                teamIndex: state.teamIndex ?? 0,
                idName: `player_${state.id}`,
                displayName: phantomDisplayName(state.id),
                iconPngBase64: icon,
            };
            allPlayers.push(phantomPlayer);
            playersById.set(state.id, phantomPlayer);
        }
    }

    const existingRows = playerList.querySelectorAll('tr[data-player-id]');
    if (existingRows.length !== allPlayers.length) {
        // —— 全量渲染（首次或玩家数量变化时） ——
        const teams = new Map();
        for (const player of allPlayers) {
            const items = teams.get(player.teamIndex) ?? [];
            items.push(player);
            teams.set(player.teamIndex, items);
        }

        const sortedTeams = [...teams.entries()].sort((left, right) => left[0] - right[0]);
        const firstTeamIsSingle = sortedTeams.length > 0 && sortedTeams[0][1].length === 1;

        const teamHtml = sortedTeams
            .map(([teamIndex, teamPlayers]) => {
                const members = teamPlayers
                    .map((player) => {
                        const state = stateMap.get(player.id);
                        const previous = previousStateMap.get(player.id) ?? state;
                        if (!state) {
                            return "";
                        }

                        const hpMetrics = actorHpMetrics(state, previous);
                        const totalWidth = hpMetrics?.totalWidth ?? 0;
                        const fillWidth = hpMetrics?.fillWidth ?? 0;
                        const previousWidth = hpMetrics?.previousWidth ?? 0;
                        const healStart = Math.min(previousWidth, fillWidth);
                        const healWidth = Math.max(0, fillWidth - previousWidth);
                        const deadClass = state.alive ? "" : " is-dead";
                        const involvedClass = involved
                            ? (involved.casters.has(player.id) && involved.targets.has(player.id) ? " is-caster is-target"
                                : involved.casters.has(player.id) ? " is-caster"
                                : involved.targets.has(player.id) ? " is-target"
                                : "")
                            : "";
                        const nameClass = state.alive ? "name" : "name namedie";
                        const stateClass = !state.alive ? "status-pill dead" : state.frozen ? "status-pill frozen" : "status-pill";

                        const maxMp = state.magic > 0 ? state.magic : (state.mp > 0 ? state.mp : 1);
                        const mpPercent = state.alive
                            ? Math.max(0, Math.min(100, (state.mp / maxMp) * 100))
                            : 0;

                        return `
                        <tr class="player-row${deadClass}${involvedClass}" data-player-id="${player.id}" title="id: ${escapeHtml(player.idName)} · playerId: ${player.id}">
                            <td class="player-name-cell">
                                <div class="player-name-wrap">
                                    <img class="sgl" src="${iconSrc(player.iconPngBase64)}" alt="${escapeHtml(player.displayName)}">
                                    <span class="${nameClass}">${escapeHtml(player.displayName)}</span>
                                </div>
                                <div class="hpwrap compact" style="width:${totalWidth}px">
                                    <div class="maxhp" style="width:${totalWidth}px"></div>
                                    <div class="oldhp" style="width:${previousWidth}px"></div>
                                    <div class="healhp" style="left:${healStart}px;width:${healWidth}px"></div>
                                    <div class="hp" style="width:${fillWidth}px"></div>
                                </div>
                                <div class="mpwrap">
                                    <div class="mp" style="width:${mpPercent.toFixed(2)}%"></div>
                                </div>
                            </td>
                            <td class="player-stat-cell player-hp-cell">${state.hp}/${state.maxHp}</td>
                            <td class="player-stat-cell">${state.mp}/${state.magic}</td>
                            <td class="player-stat-cell">${state.attack}/${state.defense}</td>
                            <td class="player-state-cell"><span class="${stateClass}">${statusText(state)}</span></td>
                        </tr>
                    `;
                    })
                    .join("");

                const isSingle = teamPlayers.length === 1;
                const labelHtml = !isSingle ? `<div class="team-label">Team ${teamIndex + 1}</div>` : "";
                const theadHtml = !isSingle ? `
                        <thead>
                            <tr>
                                <th class="player-name-head">角色</th>
                                <th class="player-hp-head">HP</th>
                                <th class="player-mix-head">MP/魔</th>
                                <th class="player-mix-head">攻/防</th>
                                <th class="player-state-head">状态</th>
                            </tr>
                        </thead>` : "";
                return `
                <section class="team-block">
                    ${labelHtml}
                    <table class="player-table">
                        <colgroup>
                            <col class="player-name-head">
                            <col class="player-hp-head">
                            <col class="player-mix-head">
                            <col class="player-mix-head">
                            <col class="player-state-head">
                        </colgroup>
                        ${theadHtml}
                        <tbody>
                            ${members}
                        </tbody>
                    </table>
                </section>
            `;
            })
            .join("");

        // 单队伍时在顶部渲染列头
        const columnHeader = firstTeamIsSingle ? `
        <table class="player-table column-headers">
            <colgroup>
                <col class="player-name-head">
                <col class="player-hp-head">
                <col class="player-mix-head">
                <col class="player-mix-head">
                <col class="player-state-head">
            </colgroup>
            <thead>
                <tr>
                    <th class="player-name-head">角色</th>
                    <th class="player-hp-head">HP</th>
                    <th class="player-mix-head">MP/魔</th>
                    <th class="player-mix-head">攻/防</th>
                    <th class="player-state-head">状态</th>
                </tr>
            </thead>
        </table>` : "";
        playerList.innerHTML = columnHeader + teamHtml;
    } else {
        // —— 增量更新：直接修改现有 DOM，避免 innerHTML 全量替换造成闪烁 ——
        for (const player of allPlayers) {
            const state = stateMap.get(player.id);
            const previous = previousStateMap.get(player.id) ?? state;
            if (!state) continue;

            const row = playerList.querySelector(`tr[data-player-id="${player.id}"]`);
            if (!row) continue;

            const hpMetrics = actorHpMetrics(state, previous);
            const totalWidth = hpMetrics?.totalWidth ?? 0;
            const fillWidth = hpMetrics?.fillWidth ?? 0;
            const previousWidth = hpMetrics?.previousWidth ?? 0;
            const healStart = Math.min(previousWidth, fillWidth);
            const healWidth = Math.max(0, fillWidth - previousWidth);
            const deadClass = state.alive ? "" : " is-dead";
            const involvedClass = involved
                ? (involved.casters.has(player.id) && involved.targets.has(player.id) ? " is-caster is-target"
                    : involved.casters.has(player.id) ? " is-caster"
                    : involved.targets.has(player.id) ? " is-target"
                    : "")
                : "";
            const nameClass = state.alive ? "name" : "name namedie";
            const stateClass = !state.alive ? "status-pill dead" : state.frozen ? "status-pill frozen" : "status-pill";
            const maxMp = state.magic > 0 ? state.magic : (state.mp > 0 ? state.mp : 1);
            const mpPercent = state.alive
                ? Math.max(0, Math.min(100, (state.mp / maxMp) * 100))
                : 0;

            row.className = `player-row${deadClass}${involvedClass}`;

            const nameEl = row.querySelector('.player-name-wrap .name, .player-name-wrap .namedie');
            if (nameEl) nameEl.className = nameClass;

            const hpwrapEl = row.querySelector('.hpwrap');
            if (hpwrapEl) hpwrapEl.style.width = totalWidth + 'px';
            const maxhpEl = row.querySelector('.maxhp');
            if (maxhpEl) maxhpEl.style.width = totalWidth + 'px';
            const hpEl = row.querySelector('.hp');
            if (hpEl) hpEl.style.width = fillWidth + 'px';
            const oldhpEl = row.querySelector('.oldhp');
            if (oldhpEl) oldhpEl.style.width = previousWidth + 'px';
            const healhpEl = row.querySelector('.healhp');
            if (healhpEl) {
                healhpEl.style.left = healStart + 'px';
                healhpEl.style.width = healWidth + 'px';
            }

            const mpEl = row.querySelector('.mp');
            if (mpEl) mpEl.style.width = mpPercent.toFixed(2) + '%';

            const statCells = row.querySelectorAll('.player-stat-cell');
            if (statCells.length >= 3) {
                statCells[0].textContent = `${state.hp}/${state.maxHp}`;
                statCells[1].textContent = `${state.mp}/${state.magic}`;
                statCells[2].textContent = `${state.attack}/${state.defense}`;
            }

            const stateEl = row.querySelector('.player-state-cell span');
            if (stateEl) {
                stateEl.className = stateClass;
                stateEl.textContent = statusText(state);
            }
        }
    }
}

// ============================================================================
// 右侧战斗帧 HTML 构建
// ============================================================================

/**
 * 构建单帧的战斗记录 HTML。
 * 每帧内部的多条消息用" ，"分隔，换行消息（next_line）触发新行。
 * HP 条会基于当前帧内累计伤害/回复进行模拟变化。
 *
 * @param {FrameUpdate} frame — 当前帧数据
 * @param {number} roundIndex — 帧序号
 * @param {FightState[]} [previousStates=frame.states] — 上一帧的状态（默认同当前帧）
 * @param {Map<number, FightPlayer>} playersById — playerId → 玩家对象索引
 * @returns {string} 帧的 HTML 字符串，无有效行时返回空字符串
 */
export function buildFrameHtml(frame, roundIndex, previousStates = frame.states, playersById) {
    const previousStateMap = buildStateMap(previousStates);
    /** @type {Map<number, FightState>} 帧内逐步更新的模拟 HP 状态 */
    let running = new Map(previousStateMap);
    const rows = [];
    let segments = [];

    /**
     * 将当前累积的消息片段刷入一个新行。
     */
    function flushRow() {
        if (!segments.length) {
            return;
        }
        rows.push(`<div class="row">${segments.join('<span class="msg-sep">，</span>')}</div>`);
        segments = [];
    }

    /**
     * 对 running 中的某个角色施加 HP 变化（伤害或回复）。
     * @param {number} id — 角色 id
     * @param {Map<number, FightState>} hitState — 当前帧内模拟状态 Map
     * @param {MessageTone} tone
     * @param {number} value — 变化量
     */
    function applyDelta(id, hitState, tone, value) {
        const cur = hitState.get(id);
        if (!cur || cur.maxHp <= 0) return;
        if (tone === 'damage') {
            hitState.set(id, { ...cur, hp: Math.max(0, cur.hp - value) });
        } else if (tone === 'recover') {
            hitState.set(id, { ...cur, hp: Math.min(cur.maxHp, cur.hp + value) });
        }
    }

    for (const update of frame.updates) {
        if (update.updateType === "next_line") {
            flushRow();
            continue;
        }

        const message = `${update.messageRendered ?? ""}`.trim();
        if (!message) {
            continue;
        }

        const tone = classifyMessage(message);
        const hitState = new Map(running);
        const value = update.param ?? update.score ?? 0;
        if (value > 0) {
            if (update.targetId != null) applyDelta(update.targetId, hitState, tone, value);
            if (Array.isArray(update.targetIds)) update.targetIds.forEach((id) => applyDelta(id, hitState, tone, value));
        }
        segments.push(`<span class="msg ${tone}">${highlightMessage(update, tone, hitState, running, playersById)}</span>`);
        running = hitState;
    }

    flushRow();

    if (!rows.length && !frame.finished) {
        return "";
    }

    const winnerLine = frame.finished
        ? `<div class="row winner-line"><span class="winner-row">winnerIds=${escapeHtml(JSON.stringify(frame.winnerIds))}</span></div>`
        : "";

    return `
        <section class="round-block">
            <div class="frame-sidebar"><span class="frame-chip">frame ${roundIndex}</span></div>
            <div class="frame-body">
                ${rows.join("")}
                ${winnerLine}
            </div>
        </section>
    `;
}
