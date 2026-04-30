const DEFAULT_RAW = `
云剑狄卡敢
白胡子

史莱姆
田一人
`.trim();

const INPUT_STORAGE_KEY = "tswn_wasm_show_input";

const playerList = document.querySelector("#playerList");
const battleRows = document.querySelector("#battleRows");
const inputName = document.querySelector("#input_name");
const inputPanel = document.querySelector("#inputPanel");
const endPanel = document.querySelector("#endPanel");
const inputStatus = document.querySelector("#inputStatus");
const plistMeta = document.querySelector("#plistMeta");
const headerMeta = document.querySelector("#headerMeta");
const winnerNames = document.querySelector("#winnerNames");
const winnerNote = document.querySelector("#winnerNote");

const versionInfo = document.querySelector("#versionInfo");
const coreVersionInfo = document.querySelector("#coreVersionInfo");
const modulePathInfo = document.querySelector("#modulePathInfo");

const startBtn = document.querySelector("#startBtn");
const sampleBtn = document.querySelector("#sampleBtn");
const closeInputBtn = document.querySelector("#closeInputBtn");
const closeEndBtn = document.querySelector("#closeEndBtn");
const playAgainBtn = document.querySelector("#playAgainBtn");
const editNamesBtn = document.querySelector("#editNamesBtn");
const inputBtn = document.querySelector("#inputBtn");
const fastBtn = document.querySelector("#fastBtn");
const refreshBtn = document.querySelector("#refreshBtn");

let wasmApi = null;
let currentReplay = null;
let playbackToken = 0;
let fastMode = false;
let playersByEscapedName = new Map();
let playerNamePattern = null;

restoreInputValue();

function escapeHtml(text) {
    return String(text)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}

function escapeRegExp(text) {
    return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function iconSrc(iconPngBase64) {
    if (!iconPngBase64) {
        return "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    }
    return iconPngBase64.startsWith("data:")
        ? iconPngBase64
        : `data:image/png;base64,${iconPngBase64}`;
}

function formatError(error) {
    if (!error) {
        return "未知错误";
    }
    if (typeof error === "string") {
        return error;
    }
    if (error.code || error.message) {
        return `${error.code ?? "ERROR"}: ${error.message ?? ""}`.trim();
    }
    if (error instanceof Error) {
        return error.message;
    }
    try {
        return JSON.stringify(error, null, 2);
    } catch {
        return String(error);
    }
}

function rememberPlayers(players) {
    const names = [];
    playersByEscapedName = new Map();

    for (const player of players) {
        const escapedName = escapeHtml(player.displayName);
        if (!escapedName || playersByEscapedName.has(escapedName)) {
            continue;
        }
        playersByEscapedName.set(escapedName, player);
        names.push(escapedName);
    }

    names.sort((left, right) => right.length - left.length);
    playerNamePattern = names.length
        ? new RegExp(names.map((name) => escapeRegExp(name)).join("|"), "g")
        : null;
}

function setInputStatus(message, isError = false) {
    inputStatus.textContent = message;
    inputStatus.classList.toggle("error", isError);
}

function setLoading(loading) {
    startBtn.disabled = loading;
    sampleBtn.disabled = loading;
}

function restoreInputValue() {
    try {
        const savedValue = window.localStorage.getItem(INPUT_STORAGE_KEY)?.trim();
        inputName.value = savedValue ? savedValue : DEFAULT_RAW;
    } catch {
        inputName.value = DEFAULT_RAW;
    }
}

function persistInputValue() {
    try {
        window.localStorage.setItem(INPUT_STORAGE_KEY, inputName.value);
    } catch {
        // Keep the in-memory input usable even if storage is unavailable.
    }
}

function openPanel(panel) {
    panel.hidden = false;
}

function closePanel(panel) {
    panel.hidden = true;
}

function openInputEditor(selectAll = false) {
    openPanel(inputPanel);
    window.requestAnimationFrame(() => {
        inputName.focus();
        if (selectAll) {
            inputName.select();
        }
    });
}

function sleep(ms) {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function buildStateMap(states) {
    return new Map(states.map((state) => [state.id, state]));
}

function statusText(state) {
    if (!state.alive) {
        return "死亡";
    }
    if (state.frozen) {
        return "冻结";
    }
    return "存活";
}

function classifyMessage(message) {
    if (message.includes("回复体力")) {
        return "recover";
    }
    if (message.includes("被击倒")) {
        return "knockout";
    }
    if (message.includes("点伤害")) {
        return "damage";
    }
    return "normal";
}

function actorToken(player) {
    return `<span class="actor-token"><img class="msg-avatar" src="${iconSrc(player.iconPngBase64)}" alt="" aria-hidden="true"><span class="actor-name">${escapeHtml(player.displayName)}</span></span>`;
}

function injectActorIcons(html) {
    if (!playerNamePattern) {
        return html;
    }

    return html.replace(playerNamePattern, (matched) => {
        const player = playersByEscapedName.get(matched);
        return player ? actorToken(player) : matched;
    });
}

function highlightMessage(message, tone) {
    let html = escapeHtml(message);
    html = html.replace(/(\[[^\]]+\])/g, '<span class="skill-token">$1</span>');

    if (tone === "damage") {
        html = html.replace(/(\d+)(?=点伤害)/g, '<span class="message-number">$1</span>');
    }
    if (tone === "recover") {
        html = html.replace(/(\d+)(?=点)/g, '<span class="message-number">$1</span>');
    }
    return injectActorIcons(html);
}

function renderIdleState() {
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

function renderPlayers(players, states, previousStates = states) {
    const stateMap = buildStateMap(states);
    const previousStateMap = buildStateMap(previousStates);

    // 补上 states 里有但初始 players 里没有的召唤单位
    const knownIds = new Set(players.map((p) => p.id));
    const allPlayers = [...players];
    for (const state of states) {
        if (!knownIds.has(state.id)) {
            knownIds.add(state.id);
            allPlayers.push({
                id: state.id,
                teamIndex: 0,
                idName: `player_${state.id}`,
                displayName: `#${state.id}`,
                iconPngBase64: null,
            });
        }
    }

    const teams = new Map();
    for (const player of allPlayers) {
        const items = teams.get(player.teamIndex) ?? [];
        items.push(player);
        teams.set(player.teamIndex, items);
    }

    const teamHtml = [...teams.entries()]
        .sort((left, right) => left[0] - right[0])
        .map(([teamIndex, teamPlayers]) => {
            const members = teamPlayers
                .map((player) => {
                    const state = stateMap.get(player.id);
                    const previous = previousStateMap.get(player.id) ?? state;
                    if (!state) {
                        return "";
                    }

                    const hpPercent = state.maxHp > 0 ? Math.max(0, Math.min(100, (state.hp / state.maxHp) * 100)) : 0;
                    const previousPercent = previous.maxHp > 0 ? Math.max(0, Math.min(100, (previous.hp / previous.maxHp) * 100)) : hpPercent;
                    const healStart = Math.min(previousPercent, hpPercent);
                    const healWidth = Math.max(0, hpPercent - previousPercent);
                    const deadClass = state.alive ? "" : " is-dead";
                    const nameClass = state.alive ? "name" : "name namedie";
                    const stateClass = !state.alive ? "status-pill dead" : state.frozen ? "status-pill frozen" : "status-pill";

                    // MP 蓝条：用 magic 作为 max 参考
                    const maxMp = state.magic > 0 ? state.magic : (state.mp > 0 ? state.mp : 1);
                    const mpPercent = Math.max(0, Math.min(100, (state.mp / maxMp) * 100));

                    return `
                        <tr class="player-row${deadClass}" title="id: ${escapeHtml(player.idName)} · playerId: ${player.id}">
                            <td class="player-name-cell">
                                <div class="player-name-wrap">
                                    <span class="sgl-wrap">
                                        <img class="sgl" src="${iconSrc(player.iconPngBase64)}" alt="${escapeHtml(player.displayName)}">
                                        <span class="sgl-hp"><span class="sgl-hp-fill" style="width:${hpPercent.toFixed(0)}%"></span></span>
                                    </span>
                                    <span class="${nameClass}">${escapeHtml(player.displayName)}</span>
                                </div>
                                <div class="hpwrap compact">
                                    <div class="maxhp"></div>
                                    <div class="oldhp" style="width:${previousPercent.toFixed(2)}%"></div>
                                    <div class="healhp" style="left:${healStart.toFixed(2)}%;width:${healWidth.toFixed(2)}%"></div>
                                    <div class="hp" style="width:${hpPercent.toFixed(2)}%"></div>
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

            return `
                <section class="team-block">
                    <div class="team-label">Team ${teamIndex + 1}</div>
                    <table class="player-table">
                        <thead>
                            <tr>
                                <th class="player-name-head">角色</th>
                                <th class="player-hp-head">HP</th>
                                <th class="player-mix-head">MP/魔</th>
                                <th class="player-mix-head">攻/防</th>
                                <th class="player-state-head">状态</th>
                            </tr>
                        </thead>
                        <tbody>
                            ${members}
                        </tbody>
                    </table>
                </section>
            `;
        })
        .join("");

    playerList.innerHTML = teamHtml;
}

function appendFrame(frame, roundIndex) {
    const rows = [];
    let segments = [];

    function flushRow() {
        if (!segments.length) {
            return;
        }
        rows.push(`<div class="row">${segments.join('<span class="msg-sep">，</span>')}</div>`);
        segments = [];
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
        segments.push(`<span class="msg ${tone}">${highlightMessage(message, tone)}</span>`);
    }

    flushRow();

    if (!rows.length && !frame.finished) {
        return;
    }

    const winnerLine = frame.finished
        ? `<div class="row winner-line"><span class="winner-row">winnerIds=${escapeHtml(JSON.stringify(frame.winnerIds))}</span></div>`
        : "";

    battleRows.insertAdjacentHTML(
        "beforeend",
        `
            <section class="round-block">
                <div class="frame-sidebar"><span class="frame-chip">frame ${roundIndex}</span></div>
                <div class="frame-body">
                    ${rows.join("")}
                    ${winnerLine}
                </div>
            </section>
        `,
    );
    const hbody = battleRows.closest(".hbody");
    if (hbody) {
        hbody.scrollTop = hbody.scrollHeight;
    }
}

function renderReplayIntro(replay) {
    const teamCount = new Set(replay.players.map((player) => player.teamIndex)).size;
    rememberPlayers(replay.players);
    plistMeta.textContent = `${replay.players.length} 名角色 · ${teamCount} 支队伍 · ${replay.frames.length} 帧回放。`;
    headerMeta.textContent = fastMode
        ? `当前是快进模式，会更快推进 ${replay.frames.length} 帧。`
        : `当前是正常播放模式，会自动推进 ${replay.frames.length} 帧。`;
    battleRows.innerHTML = `
        <div class="welcome">
            <div><strong>战斗已经开始。</strong></div>
            <div>下面会按回合逐段追加战斗事件，左侧状态栏会同步刷新 HP、MP 与存活状态。</div>
            <div>右下角的快进按钮可以在播放中途切换速度。</div>
        </div>
    `;
    renderPlayers(replay.players, replay.initialStates, replay.initialStates);
}

function updateFastButton() {
    fastBtn.classList.toggle("is-active", fastMode);
    fastBtn.title = fastMode ? "切回正常速度" : "切换快进";
    fastBtn.setAttribute("aria-label", fastBtn.title);
    if (currentReplay) {
        headerMeta.textContent = fastMode
            ? `当前是快进模式，会更快推进 ${currentReplay.frames.length} 帧。`
            : `当前是正常播放模式，会自动推进 ${currentReplay.frames.length} 帧。`;
    }
}

function playbackDelay(frame) {
    if (fastMode) {
        return 60;
    }
    const maxDelay = frame.updates.reduce((value, update) => Math.max(value, update.delay1 ?? update.delay0 ?? 0), 0);
    return Math.max(180, Math.min(520, Math.round(maxDelay / 4) + 120));
}

function winnerNamesText(replay) {
    const playersById = new Map(replay.players.map((player) => [player.id, player]));
    const names = replay.winnerIds.map((winnerId) => playersById.get(winnerId)?.displayName ?? `#${winnerId}`);
    return names.length ? names.join("、") : "未分出胜负";
}

async function playReplay(replay) {
    const token = ++playbackToken;
    closePanel(endPanel);
    renderReplayIntro(replay);
    let previousStates = replay.initialStates;

    for (const [index, frame] of replay.frames.entries()) {
        if (token !== playbackToken) {
            return;
        }
        appendFrame(frame, index);
        renderPlayers(replay.players, frame.states, previousStates);
        previousStates = frame.states;
        await sleep(playbackDelay(frame));
    }

    if (token !== playbackToken) {
        return;
    }

    renderPlayers(replay.players, replay.finalStates, previousStates);
    winnerNames.textContent = winnerNamesText(replay);
    winnerNote.textContent = `共播放 ${replay.frames.length} 帧，winnerIds=${JSON.stringify(replay.winnerIds)}。`;
    openPanel(endPanel);
}

async function loadModule() {
    const candidates = [
        { label: "../pkg/tswn_wasm.js", path: "../pkg/tswn_wasm.js" },
        { label: "../dist/wasm/pkg/tswn_wasm.js", path: "../dist/wasm/pkg/tswn_wasm.js" },
    ];

    let lastError = null;
    for (const candidate of candidates) {
        try {
            const mod = await import(candidate.path);
            modulePathInfo.textContent = `module: ${candidate.label}`;
            return mod;
        } catch (error) {
            lastError = error;
        }
    }
    throw lastError;
}

async function ensureApi() {
    if (wasmApi) {
        return wasmApi;
    }
    const mod = await loadModule();
    await mod.default();
    versionInfo.textContent = `wrapper: ${mod.version()}`;
    coreVersionInfo.textContent = `core: ${mod.core_version()}`;
    wasmApi = mod;
    return wasmApi;
}

async function buildReplay(rawInput) {
    const api = await ensureApi();
    const session = new api.FightSession(rawInput, { includeIcons: true, captureReplay: true });
    const players = session.players();
    const initialStates = session.state();
    const replay = session.run_to_end();
    return {
        rawInput,
        players,
        initialStates,
        frames: replay.frames,
        winnerIds: replay.winnerIds,
        finalStates: replay.finalStates,
    };
}

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
        currentReplay = await buildReplay(rawInput);
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

async function replayCurrent() {
    if (!currentReplay) {
        openInputEditor();
        return;
    }
    await playReplay(currentReplay);
}

sampleBtn.addEventListener("click", () => {
    inputName.value = DEFAULT_RAW;
    persistInputValue();
    setInputStatus("已填入示例输入。");
    openInputEditor(true);
});

startBtn.addEventListener("click", () => {
    void startBattle();
});

playAgainBtn.addEventListener("click", () => {
    closePanel(endPanel);
    void replayCurrent();
});

refreshBtn.addEventListener("click", () => {
    void replayCurrent();
});

inputBtn.addEventListener("click", () => {
    openInputEditor();
});

editNamesBtn.addEventListener("click", () => {
    closePanel(endPanel);
    openInputEditor(true);
});

fastBtn.addEventListener("click", () => {
    fastMode = !fastMode;
    updateFastButton();
});

closeInputBtn.addEventListener("click", () => {
    if (currentReplay) {
        closePanel(inputPanel);
    }
});

closeEndBtn.addEventListener("click", () => {
    closePanel(endPanel);
});

inputName.addEventListener("input", () => {
    persistInputValue();
});

inputName.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        void startBattle();
    }
});

async function main() {
    renderIdleState();
    updateFastButton();
    setInputStatus("会使用 show 风格自动播放整场战斗。");
    openInputEditor();

    try {
        await ensureApi();
        setInputStatus("tswn_wasm 已初始化，可以开始。");
    } catch (error) {
        setInputStatus(`模块加载失败: ${formatError(error)}`, true);
    }
}

void main();
