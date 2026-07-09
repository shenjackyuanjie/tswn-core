import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const examplesDir = dirname(fileURLToPath(import.meta.url));

async function readExampleFile(name) {
  return readFile(join(examplesDir, name), "utf8");
}

test("show page keeps v2 runtime opt-in wired to the adapter path", async () => {
  const [html, script] = await Promise.all([
    readExampleFile("show.html"),
    readExampleFile("show.js"),
  ]);

  assert.match(html, /id="runtimeModeInfo"/);
  assert.match(html, /<script type="module" src="\.\/show\.js\?v=[^"]+"><\/script>/);

  assert.match(script, /import \{ ensureApi, buildReplay, buildV2NormalizedReplay \} from "\.\/show-wasm\.js";/);
  assert.match(script, /readReplayEngineFromSearch\(window\.location\.search\)/);
  assert.match(script, /runtimeModeInfo\.textContent = replayEngineStatusText\(\);/);
  assert.match(script, /if \(replayEngine === "v2"\) \{\s*return buildV2NormalizedReplay\(rawInput, versionInfo, coreVersionInfo, modulePathInfo\);\s*\}\s*return buildReplay\(rawInput, versionInfo, coreVersionInfo, modulePathInfo\);/s);
});

test("show page preserves v2 runtime in generated share links", async () => {
  const script = await readExampleFile("show.js");

  assert.match(script, /buildShowShareUrl\(rawInput, \{\s*href: window\.location\.href,\s*runtimeV2: Boolean\(currentReplay\?\.runtime_v2\) \|\| replayEngine === "v2",\s*\}\)/s);
});
