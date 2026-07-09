import test from "node:test";
import assert from "node:assert/strict";

import {
  buildShowShareUrl,
  decodeBase64UrlUtf8,
  encodeBase64UrlUtf8,
  readReplayEngineFromSearch,
  readStaticReplayInputFromSearch,
  replayEngineStatusText,
} from "./show-routing.js";

test("show routing decodes static replay input aliases", () => {
  const rawInput = "seed: fixed\nleft@red\n\nright@blue\n";
  const encoded = encodeBase64UrlUtf8(rawInput);

  assert.equal(decodeBase64UrlUtf8(encoded), rawInput);
  assert.deepEqual(readStaticReplayInputFromSearch(`?replay=${encoded}`), {
    ok: true,
    input: rawInput,
    paramName: "replay",
  });
});

test("show routing reports invalid static replay input", () => {
  const result = readStaticReplayInputFromSearch("?input=*not-base64*");

  assert.equal(result?.ok, false);
  assert.match(result?.message ?? "", /URL 参数 input 解码失败/);
  assert.match(result?.message ?? "", /URL-safe Base64/);
});

test("show routing recognizes v2 and legacy engine aliases", () => {
  assert.deepEqual(readReplayEngineFromSearch("?engine=v2"), { engine: "v2", paramName: "engine" });
  assert.deepEqual(readReplayEngineFromSearch("?runtime=normalized_v2"), { engine: "v2", paramName: "runtime" });
  assert.deepEqual(readReplayEngineFromSearch("?engine=fight_session"), { engine: "legacy", paramName: "engine" });
  assert.deepEqual(readReplayEngineFromSearch("?engine="), { engine: "legacy", paramName: "engine" });
  assert.equal(readReplayEngineFromSearch("?input=abc"), null);
});

test("show routing falls back to legacy for unknown engine values", () => {
  const result = readReplayEngineFromSearch("?runtime=experimental");

  assert.equal(result?.engine, "legacy");
  assert.equal(result?.paramName, "runtime");
  assert.match(result?.message ?? "", /未识别/);
});

test("show share URL preserves v2 engine only for v2 replay paths", () => {
  const rawInput = "left@red\n\nright@blue\n";
  const legacyUrl = buildShowShareUrl(rawInput, {
    href: "https://example.test/show.html?runtime=v2&data=old#section",
  });
  const v2Url = buildShowShareUrl(rawInput, {
    href: "https://example.test/show.html?runtime=legacy&replay=old#section",
    runtimeV2: true,
  });

  const legacyParsed = new URL(legacyUrl);
  assert.equal(legacyParsed.hash, "");
  assert.equal(legacyParsed.searchParams.has("runtime"), false);
  assert.equal(legacyParsed.searchParams.has("data"), false);
  assert.equal(legacyParsed.searchParams.get("engine"), null);
  assert.equal(decodeBase64UrlUtf8(legacyParsed.searchParams.get("input") ?? ""), rawInput);

  const v2Parsed = new URL(v2Url);
  assert.equal(v2Parsed.hash, "");
  assert.equal(v2Parsed.searchParams.has("runtime"), false);
  assert.equal(v2Parsed.searchParams.has("replay"), false);
  assert.equal(v2Parsed.searchParams.get("engine"), "v2");
  assert.equal(decodeBase64UrlUtf8(v2Parsed.searchParams.get("input") ?? ""), rawInput);
});

test("show routing exposes engine status text", () => {
  assert.match(replayEngineStatusText("legacy"), /FightSession/);
  assert.match(replayEngineStatusText("v2"), /v2 normalized run/);
});
