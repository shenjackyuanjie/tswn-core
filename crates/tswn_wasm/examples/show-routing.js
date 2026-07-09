import { formatError } from "./show-utils.js";

/** @type {string[]} URL 参数名，值为 URL-safe Base64 编码后的原始对局输入 */
export const STATIC_INPUT_PARAM_NAMES = ["input", "replay", "data"];
/** @type {string[]} URL 参数名，用于显式选择 replay runtime */
export const REPLAY_ENGINE_PARAM_NAMES = ["engine", "runtime"];
/** @type {'legacy'|'v2'} show 页面未显式指定 runtime 时使用的默认 replay runtime */
export const DEFAULT_REPLAY_ENGINE = "v2";

const REPLAY_ENGINE_V2_VALUES = new Set(["v2", "runtime_v2", "normalized", "normalized_v2"]);
const REPLAY_ENGINE_LEGACY_VALUES = new Set(["legacy", "v1", "fightsession", "fight_session"]);

/**
 * 从 URL-safe Base64 解码 UTF-8 原始输入。
 * @param {string} encoded
 * @returns {string}
 */
export function decodeBase64UrlUtf8(encoded) {
  const compact = encoded.trim();
  if (!compact) {
    throw new Error("URL 参数为空。");
  }
  if (!/^[A-Za-z0-9_-]+={0,2}$/.test(compact)) {
    throw new Error("不是合法的 URL-safe Base64。");
  }

  const base64 = compact.replace(/-/g, "+").replace(/_/g, "/");
  if (base64.length % 4 === 1) {
    throw new Error("Base64 长度不合法。");
  }

  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = globalThis.atob(padded);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

/**
 * 将 UTF-8 字符串编码成 URL-safe Base64。
 * @param {string} input
 * @returns {string}
 */
export function encodeBase64UrlUtf8(input) {
  const bytes = new TextEncoder().encode(input);
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return globalThis.btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/**
 * 为当前对局输入生成分享链接。
 * @param {string} rawInput
 * @param {{ href: string, runtimeV2?: boolean, runtimeEngine?: 'legacy'|'v2'|null }} options
 * @returns {string}
 */
export function buildShowShareUrl(rawInput, { href, runtimeV2 = false, runtimeEngine = null }) {
  const url = new URL(href);
  for (const paramName of STATIC_INPUT_PARAM_NAMES) {
    url.searchParams.delete(paramName);
  }
  for (const paramName of REPLAY_ENGINE_PARAM_NAMES) {
    url.searchParams.delete(paramName);
  }
  url.searchParams.set("input", encodeBase64UrlUtf8(rawInput));
  const engine = runtimeEngine ?? (runtimeV2 ? "v2" : null);
  if (engine === "v2") {
    url.searchParams.set("engine", "v2");
  } else if (engine === "legacy") {
    url.searchParams.set("engine", "legacy");
  }
  url.hash = "";
  return url.href;
}

/**
 * 从 URL search 中读取静态对局输入参数。
 * @param {string} search
 * @returns {{ ok: true, input: string, paramName: string }|{ ok: false, message: string }|null}
 */
export function readStaticReplayInputFromSearch(search) {
  const params = new URLSearchParams(search);
  for (const paramName of STATIC_INPUT_PARAM_NAMES) {
    if (!params.has(paramName)) {
      continue;
    }
    try {
      return {
        ok: true,
        input: decodeBase64UrlUtf8(params.get(paramName) ?? ""),
        paramName,
      };
    } catch (error) {
      return {
        ok: false,
        message: `URL 参数 ${paramName} 解码失败：${formatError(error)}`,
      };
    }
  }
  return null;
}

/**
 * 从 URL search 中读取 replay runtime。未指定时返回 null，由页面默认 runtime 决定。
 * @param {string} search
 * @returns {{ engine: 'legacy'|'v2', paramName: string, message?: string }|null}
 */
export function readReplayEngineFromSearch(search) {
  const params = new URLSearchParams(search);
  for (const paramName of REPLAY_ENGINE_PARAM_NAMES) {
    if (!params.has(paramName)) {
      continue;
    }
    const value = `${params.get(paramName) ?? ""}`.trim().toLowerCase();
    if (REPLAY_ENGINE_V2_VALUES.has(value)) {
      return { engine: "v2", paramName };
    }
    if (!value || REPLAY_ENGINE_LEGACY_VALUES.has(value)) {
      return { engine: "legacy", paramName };
    }
    return {
      engine: DEFAULT_REPLAY_ENGINE,
      paramName,
      message: `URL 参数 ${paramName}=${value} 未识别，已回退 v2 normalized run。`,
    };
  }
  return null;
}

/**
 * @param {'legacy'|'v2'} replayEngine
 * @returns {string}
 */
export function replayEngineStatusText(replayEngine) {
  return replayEngine === "v2"
    ? "使用 v2 normalized run 生成 replay 适配视图。"
    : "自动使用 FightSession 捕获 replay，并按帧播放。";
}
