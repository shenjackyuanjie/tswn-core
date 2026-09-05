import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const examplesDir = dirname(fileURLToPath(import.meta.url));

async function readExampleFile(name) {
  return readFile(join(examplesDir, name), "utf8");
}

test("show page only wires the shared replay view path", async () => {
  const [html, script, wasmScript] = await Promise.all([
    readExampleFile("index.html"),
    readExampleFile("show.js"),
    readExampleFile("show-wasm.js"),
  ]);

  assert.match(html, /id="runtimeModeInfo"/);
  assert.match(html, /使用 battle_replay 生成 replay 适配视图。/);
  assert.match(html, /<script type="module" src="\.\/show\.js\?v=20260904a"><\/script>/);

  assert.match(script, /import \{ ensureApi, buildMainNormalizedReplay \} from "\.\/show-wasm\.js";/);
  assert.match(script, /return buildMainNormalizedReplay\(rawInput, versionInfo, coreVersionInfo, modulePathInfo\);/);
  assert.doesNotMatch(script, /FightSession|readReplayEngineFromSearch|DEFAULT_REPLAY_ENGINE/);
  assert.match(
    wasmScript,
    /export async function buildMainNormalizedReplay[\s\S]*api\.battle_replay\(rawInput, \{[\s\S]*include_icons: true,[\s\S]*max_rounds: maxRounds,[\s\S]*\}\);[\s\S]*return buildMainReplayFromBattleReplay\(rawInput, replay, wasmDurationMs\);/,
  );
  assert.doesNotMatch(wasmScript, /new api\.FightSession|export async function buildReplay\(/);
});

test("show page removes runtime choice from generated share links", async () => {
  const script = await readExampleFile("show.js");

  assert.match(script, /buildShowShareUrl\(rawInput, \{\s*href: window\.location\.href,\s*\}\)/s);
  assert.doesNotMatch(script, /runtimeEngine/);
});
