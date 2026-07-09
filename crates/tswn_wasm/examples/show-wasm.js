/**
 * @fileoverview tswn_wasm 战斗回放展示页 — WASM 模块加载与回放生成
 *
 * 负责动态加载 tswn_wasm WASM 模块（懒加载 + 缓存），
 * 以及根据用户输入调用 FightSession 或 v2 normalized run 生成回放数据。
 */

// ============================================================================
// 模块级缓存
// ============================================================================

/** @type {object|null} WASM 模块的 API 句柄，仅在首次 ensureApi() 时初始化 */
let wasmApi = null;
const MODULE_CACHE_BUST = Date.now().toString(36);
const V2_DEFAULT_MAX_ROUNDS = 2048;

function withCacheBust(url) {
    const busted = new URL(url);
    busted.searchParams.set('v', MODULE_CACHE_BUST);
    return busted;
}

// ============================================================================
// 模块加载
// ============================================================================

/**
 * 尝试加载 tswn_wasm WASM 模块。
 *
 * 基于当前脚本自身的 URL（import.meta.url）枚举多个候选路径，
 * 兼容 examples/ 子目录部署和扁平部署两种结构：
 *   - show-wasm.js 在 examples/ 下 → 尝试 ../pkg/tswn_wasm.js
 *   - show-wasm.js 与 pkg/ 同级    → 尝试 ./pkg/tswn_wasm.js
 *
 * @param {HTMLElement} modulePathInfo — 用于展示加载路径的 DOM 元素
 * @returns {Promise<{ mod: object, wasmUrl: URL }>} WASM 模块及对应 wasm URL
 * @throws {Error} 若所有候选路径均加载失败
 */
export async function loadModule(modulePathInfo) {
    const base = new URL('.', import.meta.url);
    const candidates = [
        {
            label: '../pkg/tswn_wasm.js',
            moduleUrl: withCacheBust(new URL('../pkg/tswn_wasm.js', base)),
            wasmUrl: withCacheBust(new URL('../pkg/tswn_wasm_bg.wasm', base)),
        },
        {
            label: './pkg/tswn_wasm.js',
            moduleUrl: withCacheBust(new URL('./pkg/tswn_wasm.js', base)),
            wasmUrl: withCacheBust(new URL('./pkg/tswn_wasm_bg.wasm', base)),
        },
    ];

    let lastError = null;
    for (const candidate of candidates) {
        try {
            const mod = await import(candidate.moduleUrl.href);
            modulePathInfo.textContent = `module: ${candidate.label}`;
            return { mod, wasmUrl: candidate.wasmUrl };
        } catch (error) {
            lastError = error;
        }
    }
    throw lastError;
}

/**
 * 确保 WASM API 已初始化（懒加载 + 缓存）。
 * 首次调用时会加载模块、调用 default() 初始化、记录版本信息。
 *
 * @param {HTMLElement} versionInfo — wrapper 版本展示 DOM
 * @param {HTMLElement} coreVersionInfo — core 版本展示 DOM
 * @param {HTMLElement} modulePathInfo — 模块路径展示 DOM
 * @returns {Promise<object>} WASM API 对象
 */
export async function ensureApi(versionInfo, coreVersionInfo, modulePathInfo) {
    if (wasmApi) {
        return wasmApi;
    }
    const { mod, wasmUrl } = await loadModule(modulePathInfo);
    await mod.default({ module_or_path: wasmUrl });
    versionInfo.textContent = `wrapper: ${mod.version()}`;
    coreVersionInfo.textContent = `core: ${mod.core_version()}`;
    wasmApi = mod;
    return wasmApi;
}

// ============================================================================
// 回放生成
// ============================================================================

/**
 * 从原始输入里提取显式指定的 seed 行。
 * @param {string} rawInput
 * @returns {string|null}
 */
function extractSpecifiedSeedLine(rawInput) {
    for (const line of rawInput.split(/\r?\n/)) {
        const trimmed = line.trim();
        if (/^seed:/i.test(trimmed)) {
            return trimmed;
        }
    }
    return null;
}

function normalizeUpdateType(updateType) {
    if (typeof updateType === "string") {
        const lowered = updateType.toLowerCase();
        return lowered === "nextline" ? "next_line" : lowered;
    }
    return `${updateType ?? "none"}`.toLowerCase();
}

function collectRawPlayers(rawInput) {
    const players = [];
    let teamIndex = 0;
    let currentTeamHasPlayer = false;
    for (const line of rawInput.replace(/\r\n?/g, "\n").split("\n")) {
        const trimmed = line.trim();
        if (!trimmed) {
            if (currentTeamHasPlayer) {
                teamIndex += 1;
                currentTeamHasPlayer = false;
            }
            continue;
        }
        if (/^seed:/i.test(trimmed)) {
            continue;
        }
        players.push({
            id_name: trimmed,
            icon_key: trimmed,
            display_name: trimmed,
            team_index: teamIndex,
        });
        currentTeamHasPlayer = true;
    }
    return players;
}

function maxHpByEntity(run) {
    const maxHpById = new Map();
    for (const round of run.rounds ?? []) {
        for (const [index, entityId] of (round.entity_ids ?? []).entries()) {
            const id = Number(entityId);
            const hp = Number(round.hp?.[index] ?? 0);
            maxHpById.set(id, Math.max(maxHpById.get(id) ?? 1, hp, 1));
        }
    }
    return maxHpById;
}

function buildV2Players(rawInput, run) {
    const rawPlayers = collectRawPlayers(rawInput);
    const firstRound = run.rounds?.[0] ?? null;
    const ids = firstRound?.entity_ids ?? [];
    const count = rawPlayers.length || ids.length;
    return Array.from({ length: count }, (_, index) => {
        const id = Number(ids[index] ?? index);
        const rawPlayer = rawPlayers[index] ?? {};
        const displayName = rawPlayer.display_name ?? `#${id}`;
        return {
            id,
            team_index: Number(rawPlayer.team_index ?? firstRound?.teams?.[index] ?? 0),
            id_name: rawPlayer.id_name ?? `entity_${id}`,
            icon_key: rawPlayer.icon_key ?? rawPlayer.id_name ?? `entity_${id}`,
            display_name: displayName,
            icon_png_base64: null,
        };
    });
}

function buildV2States(outcome, playersById, maxHpById) {
    return (outcome?.entity_ids ?? []).map((entityId, index) => {
        const id = Number(entityId);
        const player = playersById.get(id);
        const hp = Number(outcome.hp?.[index] ?? 0);
        const maxHp = Math.max(1, Number(maxHpById.get(id) ?? hp));
        return {
            id,
            team_index: Number(outcome.teams?.[index] ?? player?.team_index ?? 0),
            id_name: player?.id_name ?? `entity_${id}`,
            icon_key: player?.icon_key ?? player?.id_name ?? `entity_${id}`,
            display_name: player?.display_name ?? `#${id}`,
            display_index: 0,
            icon_png_base64: player?.icon_png_base64 ?? null,
            hp,
            max_hp: maxHp,
            magic_point: Number(outcome.magic_point?.[index] ?? 0),
            move_point: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            attack: 0,
            defense: Number(outcome.defense?.[index] ?? 0),
            resistance: Number(outcome.resistance?.[index] ?? 0),
            wisdom: 0,
            point: 0,
            all_sum: 0,
            name_factor: 0,
            at_boost: 0,
            attract: 0,
            alive: Boolean(outcome.alive?.[index]),
            frozen: false,
            status_labels: [],
        };
    });
}

function buildStateNameMap(states) {
    return new Map(states.map((state) => [state.id, state.display_name ?? `#${state.id}`]));
}

function v2PlayerPart(playerId, namesById) {
    const id = Number(playerId);
    return {
        kind: "player",
        text: namesById.get(id) ?? `#${id}`,
        player_id: id,
        show_hp: false,
    };
}

function v2PartsFromMessage(message, update, namesById) {
    const parts = [];
    const template = `${message ?? ""}`;
    const tokenRe = /\[(\d+)\]/g;
    let cursor = 0;
    let match;
    while ((match = tokenRe.exec(template)) != null) {
        if (match.index > cursor) {
            parts.push({ kind: "text", text: template.slice(cursor, match.index) });
        }
        const placeholder = Number(match[1]);
        if (placeholder === 0 && update.caster_id != null) {
            parts.push(v2PlayerPart(update.caster_id, namesById));
        } else if (placeholder === 1 && update.target_id != null) {
            parts.push(v2PlayerPart(update.target_id, namesById));
        } else if (placeholder === 2 && update.param != null) {
            parts.push({ kind: "data", text: `${update.param}` });
        } else {
            parts.push({ kind: "text", text: match[0] });
        }
        cursor = tokenRe.lastIndex;
    }
    if (cursor < template.length) {
        parts.push({ kind: "text", text: template.slice(cursor) });
    }
    return parts;
}

function renderedV2Message(message, update, namesById) {
    return v2PartsFromMessage(message, update, namesById)
        .map((part) => part.text ?? "")
        .join("");
}

function classifyV2Tone(frame) {
    const message = `${frame.message ?? ""}`;
    if (/击败|死亡|死了|消失/.test(message)) {
        return "knockout";
    }
    if (/恢复|回复|治疗/.test(message)) {
        return "recover";
    }
    if ((frame.score ?? 0) > 0 || /伤害|攻击/.test(message)) {
        return "damage";
    }
    if (/解除|结束/.test(message)) {
        return "status_exit";
    }
    return "normal";
}

function v2UpdateFromFrame(frame, namesById) {
    const update = {
        score: Number(frame.score ?? 0),
        delay0: Number(frame.delay0 ?? 0),
        delay1: Number(frame.delay1 ?? 0),
        caster_id: Number(frame.caster ?? 0),
        target_id: Number(frame.target ?? 0),
        target_ids: (frame.targets ?? []).map(Number),
        update_type: normalizeUpdateType(frame.update_type),
        message_template: `${frame.message ?? ""}`,
        param: frame.param == null ? null : Number(frame.param),
        hp_delta: null,
        status_change_tokens: [],
        tone: classifyV2Tone(frame),
    };
    update.message_rendered = renderedV2Message(update.message_template, update, namesById);
    return update;
}

function v2ClipFromUpdate(update, states, previousStates, namesById) {
    return {
        delay: Math.max(0, Number(update.delay0 ?? 0) + Number(update.delay1 ?? 0)),
        text_template: update.message_template,
        color: null,
        tone: update.tone,
        player_id: update.caster_id,
        data: update.param,
        show_hp: false,
        hp_before: null,
        hp_after: null,
        death_effect: update.tone === "knockout",
        emoji: null,
        parts: v2PartsFromMessage(update.message_template, update, namesById),
        caster_ids: update.caster_id == null ? [] : [update.caster_id],
        target_ids: update.target_ids?.length ? update.target_ids : [update.target_id],
        sidebar_states: states,
        sidebar_previous_states: previousStates,
        winner: null,
    };
}

function v2RowsFromUpdates(updates, states, previousStates, namesById) {
    const rows = [{ indent: 0, clips: [] }];
    for (const update of updates) {
        const currentRow = rows[rows.length - 1];
        if (update.update_type === "next_line" && currentRow.clips.length > 0) {
            rows.push({ indent: 0, clips: [] });
        }
        rows[rows.length - 1].clips.push(v2ClipFromUpdate(update, states, previousStates, namesById));
    }
    return rows.filter((row) => row.clips.length > 0);
}

function winnerIdsFromOutcome(outcome) {
    const winnerTeam = outcome?.winner_team;
    if (winnerTeam == null) {
        return [];
    }
    const winners = [];
    for (const [index, entityId] of (outcome.entity_ids ?? []).entries()) {
        if (outcome.teams?.[index] === winnerTeam && outcome.alive?.[index]) {
            winners.push(Number(entityId));
        }
    }
    return winners;
}

function buildV2Frame(outcome, previousStates, playersById, maxHpById) {
    const states = buildV2States(outcome, playersById, maxHpById);
    const namesById = buildStateNameMap(states);
    const updates = (outcome.frames ?? []).map((frame) => v2UpdateFromFrame(frame, namesById));
    const rows = v2RowsFromUpdates(updates, states, previousStates, namesById);
    const totalDelay = rows
        .flatMap((row) => row.clips)
        .reduce((sum, clip) => sum + Number(clip.delay ?? 0), 0);
    return {
        finished: outcome.winner_team != null,
        winner_ids: winnerIdsFromOutcome(outcome),
        updates,
        rows,
        states,
        total_delay: totalDelay,
    };
}

/**
 * 将 v2 normalized run 转成当前 show.html 可消费的 replay shape。
 *
 * @param {string} rawInput
 * @param {object} run
 * @param {number} [wasmDurationMs=0]
 * @returns {FightReplay}
 */
export function buildV2ReplayFromNormalizedRun(rawInput, run, wasmDurationMs = 0) {
    const players = buildV2Players(rawInput, run);
    const playersById = new Map(players.map((player) => [player.id, player]));
    const maxHpById = maxHpByEntity(run);
    const firstOutcome = run.rounds?.[0] ?? null;
    const finalOutcome = run.rounds?.[run.rounds.length - 1] ?? firstOutcome;
    const initial_states = buildV2States(firstOutcome, playersById, maxHpById);
    const frames = [];
    let previousStates = initial_states;
    for (const outcome of run.rounds ?? []) {
        const frame = buildV2Frame(outcome, previousStates, playersById, maxHpById);
        frames.push(frame);
        previousStates = frame.states;
    }
    const final_states = buildV2States(finalOutcome, playersById, maxHpById);
    return {
        raw_input: rawInput,
        seed_line: extractSpecifiedSeedLine(rawInput),
        players,
        initial_states,
        frames,
        winner_ids: winnerIdsFromOutcome(finalOutcome),
        final_states,
        runtime_v2: true,
        winner_team: run.winner_team ?? null,
        guard_exhausted: Boolean(run.guard_exhausted),
        total_score: Number(run.total_score ?? 0),
        wasm_duration_ms: wasmDurationMs,
    };
}

/**
 * 根据原始输入文本生成完整回放数据。
 *
 * @param {string} rawInput — 原始输入文本（每行一个名字，空行分隔队伍）
 * @param {HTMLElement} versionInfo
 * @param {HTMLElement} coreVersionInfo
 * @param {HTMLElement} modulePathInfo
 * @returns {Promise<FightReplay>}
 */
export async function buildReplay(rawInput, versionInfo, coreVersionInfo, modulePathInfo) {
    const api = await ensureApi(versionInfo, coreVersionInfo, modulePathInfo);
    const session = new api.FightSession(rawInput, { include_icons: true, capture_replay: true });
    const players = session.players();
    const initial_states = session.state();
    const wasmStart = performance.now();
    const replay = session.run_to_end();
    const wasmDurationMs = performance.now() - wasmStart;
    return {
        raw_input: rawInput,
        seed_line: extractSpecifiedSeedLine(rawInput),
        players,
        initial_states,
        frames: replay.frames,
        winner_ids: replay.winner_ids,
        final_states: replay.final_states,
        wasm_duration_ms: wasmDurationMs,
    };
}

/**
 * 使用 v2 default custom profile 的 normalized run 构造 show-compatible replay。
 *
 * 这是 Phase H 迁移用的显式入口：当前 show.html 默认仍走 FightSession，
 * 后续可以用这个结果和 legacy replay view 做 DOM/golden 对账后再切换默认路径。
 *
 * @param {string} rawInput
 * @param {HTMLElement} versionInfo
 * @param {HTMLElement} coreVersionInfo
 * @param {HTMLElement} modulePathInfo
 * @param {{ maxRounds?: number }} [options]
 * @returns {Promise<FightReplay>}
 */
export async function buildV2NormalizedReplay(rawInput, versionInfo, coreVersionInfo, modulePathInfo, options = {}) {
    const api = await ensureApi(versionInfo, coreVersionInfo, modulePathInfo);
    if (typeof api.default_custom_runtime_v2_normalized_run !== "function") {
        throw new Error("当前 tswn_wasm 包未导出 default_custom_runtime_v2_normalized_run");
    }
    const maxRounds = Math.max(1, Number(options.maxRounds ?? V2_DEFAULT_MAX_ROUNDS) || V2_DEFAULT_MAX_ROUNDS);
    const wasmStart = performance.now();
    const run = api.default_custom_runtime_v2_normalized_run(rawInput, maxRounds);
    const wasmDurationMs = performance.now() - wasmStart;
    return buildV2ReplayFromNormalizedRun(rawInput, run, wasmDurationMs);
}
