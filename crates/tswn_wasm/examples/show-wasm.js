/**
 * @fileoverview tswn_wasm 战斗回放展示页 — WASM 模块加载与流式战斗
 *
 * 负责动态加载 tswn_wasm WASM 模块（懒加载 + 缓存），
 * 以及创建 BattleSession、逐帧拉取和显式释放 WASM 资源。
 */

// ============================================================================
// 模块级缓存
// ============================================================================

/** @type {object|null} WASM 模块的 API 句柄，仅在首次 ensureApi() 时初始化 */
let wasmApi = null;
const MODULE_CACHE_BUST = Date.now().toString(36);

/** 固定角色的 Emoji 徽章优先于 WASM 生成的像素图标。 */
const QUILT_EMOJI = [
    ["quilt-icon", "🛏️"],
    ["quilt-monkey", "🐵"],
    ["quilt-fish", "🐟️"],
    ["smdd-leopard", "🐆"],
];
const quiltEmojiCache = new Map();

/** 咦 Boss 直接使用原图（不缩到 32x32），由浏览器按图标尺寸缩放。 */
const YI_BOSS_ICON = `data:image/png;base64,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`;

function quiltEmojiIcon(iconKey) {
    // 本体会收到规范的 quilt@!，而非旧版 quilt-icon 别名。
    const raw = String(iconKey ?? "");
    const key = /^quilt@!(?:@|\+|$)/.test(raw) ? "quilt-icon" : raw;
    if (key === "咦@!" || key === "咦") return YI_BOSS_ICON;
    const hit = QUILT_EMOJI.find(([prefix]) => key === prefix || key.startsWith(`${prefix}@`));
    if (!hit) return null;
    if (quiltEmojiCache.has(hit[1])) return quiltEmojiCache.get(hit[1]);
    const canvas = document.createElement("canvas");
    canvas.width = 48;
    canvas.height = 48;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.font = '40px "Segoe UI Emoji", "Apple Color Emoji", "Noto Color Emoji", sans-serif';
    ctx.fillText(hit[1], 24, 26);
    const url = canvas.toDataURL("image/png");
    quiltEmojiCache.set(hit[1], url);
    return url;
}

/** 挑战选择器、战斗回放与导出共用 Emoji 优先的图标入口。 */
export function loadBattleIcon(api, iconKey) {
    return quiltEmojiIcon(iconKey) ?? api.name_to_png_base64(iconKey);
}

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

function playersFromBattleReplayStates(states) {
    return (states ?? [])
        .filter((state) => state.owner_id == null)
        .map((state) => ({
            ...state,
            id: Number(state.id),
            team_index: Number(state.team_index ?? 0),
            // 输入分组序号：左侧列表按它排序与分组，保证与输入的名字顺序一致。
            input_team_index: state.input_team_index == null ? null : Number(state.input_team_index),
            id_name: state.id_name ?? state.base_name ?? `entity_${state.id}`,
            icon_key: state.icon_key ?? state.id_name ?? `entity_${state.id}`,
            display_name: state.display_name ?? state.base_name ?? "角色",
            icon_png_base64: state.icon_png_base64 ?? null,
        }));
}

/** 单场固定正整数采样量，限制在 WASM u32 可表示的范围内。 */
export function normalizeStateRateRounds(value) {
    const rounds = Number(value);
    return Number.isSafeInteger(rounds) && rounds > 0 ? Math.min(rounds, 0xffffffff) : 10000;
}

/** 创建一个增量驱动的 WASM 会话。启动时不拉取任何帧。
 * `api` 是供适配器测试使用的可选依赖注入点。
 */
export async function createBattleStreamSource(rawInput, versionInfo, coreVersionInfo, modulePathInfo, options = {}) {
    const loadStart = performance.now();
    const api = options.api ?? await ensureApi(versionInfo, coreVersionInfo, modulePathInfo);
    options.metrics?.wasmLoaded(performance.now() - loadStart);
    const createStart = performance.now();
    if (typeof api.BattleSession !== "function") {
        throw new Error("当前 tswn_wasm 包未导出 BattleSession");
    }
    let session = new api.BattleSession(rawInput, {
        include_icons: false,
        ...(options.maxRounds == null ? {} : { max_rounds: options.maxRounds }),
        ...(options.evalRq == null ? {} : { eval_rq: options.evalRq }),
    });
    let terminalResult = null;
    let disposed = false;
    let failure = null;
    function release() {
        const owned = session;
        session = null;
        owned?.free();
    }
    function captureResult() {
        if (session?.is_done()) {
            terminalResult = session.result();
            if (terminalResult == null) throw new Error("BattleSession 终止但未返回 result");
            release();
        }
    }
    try {
        if (options.strategy === "search") {
            if (typeof session.set_search !== "function") throw new Error("当前引擎未提供 Clever 搜索，请刷新页面加载新版。");
            session.set_search(16, 128, 4, 1);
        }
        const initialStates = session.initial_states();
        const stateRateRounds = normalizeStateRateRounds(options.stateRateRounds);
        const measureRates = options.stateRates === true && options.strategy !== "search" && new Set(initialStates.map(s => s.team_index)).size === 2;
        captureResult();
        options.metrics?.sessionCreated(performance.now() - createStart);
        return {
            raw_input: rawInput,
            seed_line: extractSpecifiedSeedLine(rawInput),
            players: playersFromBattleReplayStates(initialStates),
            initial_states: initialStates,
            async nextFrame() {
                if (failure) throw failure;
                if (disposed || terminalResult != null) return null;
                const pullStart = performance.now();
                try {
                    const frame = session.next_frame();
                    // 完整采样后发布该步；分批让出主线程，取消后不再访问会话。
                    if (frame && measureRates) {
                        const counts = [0, 0, 0];
                        let yieldedAt = performance.now();
                        for (let start = 0; start < stateRateRounds; start += 8) {
                            if (disposed) return null;
                            // 尾批只取剩余局数，任意正整数均不超采样。
                            session.current_win_rate_batch(start, Math.min(8, stateRateRounds - start)).forEach((n, i) => counts[i] += n);
                            if (performance.now() - yieldedAt >= 12) {
                                await new Promise(resolve => setTimeout(resolve, 0));
                                yieldedAt = performance.now();
                            }
                        }
                        if (disposed) return null;
                        frame.state_rates = { counts, rounds: stateRateRounds };
                    }
                    captureResult();
                    if (frame == null && terminalResult == null) {
                        throw new Error("BattleSession 返回空 frame 但尚未终止");
                    }
                    return frame ?? null;
                } catch (error) {
                    failure = error;
                    release();
                    throw error;
                } finally {
                    options.metrics?.framePulled(performance.now() - pullStart);
                }
            },
            result() { return terminalResult; },
            isDone() { return terminalResult != null; },
            dispose() {
                if (disposed) return;
                disposed = true;
                release();
            },
            loadIcon(iconKey) { return loadBattleIcon(api, iconKey); },
        };
    } catch (error) {
        release();
        throw error;
    }
}

