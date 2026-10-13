import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const examplesDir = dirname(fileURLToPath(import.meta.url));

async function readExampleFile(name) {
  return readFile(join(examplesDir, name), "utf8");
}

test("show page starts a canonical streaming session", async () => {
  const [html, script] = await Promise.all([readExampleFile("index.html"), readExampleFile("show.js")]);
  assert.match(html, /使用 BattleSession 边计算边播放战斗。/);
  assert.match(html, /show\.js\?v=20261013a/);
  assert.match(script, /await createBattleStreamSource\(/);
  assert.match(script, /new BattleStreamController\(/);
  assert.doesNotMatch(script, /buildMainNormalizedReplay|battle_replay|FightSession/);
});

test("show page removes runtime choice from generated share links", async () => {
  const script = await readExampleFile("show.js");

  assert.match(script, /buildShowShareUrl\(rawInput, \{\s*href: window\.location\.href,\s*\}\)/s);
  assert.doesNotMatch(script, /runtimeEngine/);
});

test("right controls expose shortcuts for every button", async () => {
  const script = await readExampleFile("show.js");
  const shortcutMap = script.match(/const BUTTON_SHORTCUTS = new Map\(\[([\s\S]*?)\n\]\);/);
  assert.ok(shortcutMap);

  const shortcuts = [...shortcutMap[1].matchAll(/\["([a-z])", (\w+)\]/g)].map((match) => [match[1], match[2]]);
  assert.deepEqual(shortcuts, [
    ["c", "toggleControlsBtn"],
    ["p", "normalBtn"],
    ["f", "fastBtn"],
    ["i", "inputBtn"],
    ["t", "turboBtn"],
    ["r", "refreshBtn"],
    ["s", "shareBtn"],
    ["d", "themeBtn"],
    ["b", "shieldBtn"],
  ]);
  assert.match(script, /shortcutButton\.click\(\)/);
  assert.match(script, /isEditableKeyTarget\(event\.target\)/);
  assert.match(script, /setAttribute\("aria-keyshortcuts", "Space"\)/);
  assert.match(script, /setAttribute\("aria-keyshortcuts", "ArrowLeft"\)/);
});

test("detail panel keeps readable colors in light theme", async () => {
  const css = await readExampleFile("show.css");

  assert.match(css, /\.detail-card\s*\{[^}]*background: var\(--surface-soft\);[^}]*color: var\(--ink\);[^}]*\}/s);
  assert.match(css, /\.detail-subtitle\s*\{[^}]*color: var\(--muted\);[^}]*\}/s);
  assert.match(css, /\.detail-grid dt\s*\{[^}]*color: var\(--muted\);[^}]*\}/s);
  assert.match(css, /\.detail-grid dd\s*\{[^}]*color: var\(--ink\);[^}]*\}/s);
  assert.doesNotMatch(css, /\.detail-subtitle\s*\{[^}]*color: rgba\(255, 255, 255, 0\.9\);[^}]*\}/s);
  assert.doesNotMatch(css, /\.detail-grid dd\s*\{[^}]*color: white;[^}]*\}/s);
});

test("盾条样式仅在显式开启后显示，默认与脚本加载前都关闭", async () => {
  const [html, script, css] = await Promise.all([readExampleFile("index.html"), readExampleFile("show.js"), readExampleFile("show.css")]);
  assert.match(html, /id="shieldBtn"[^>]*aria-pressed="false"/);
  assert.match(script, /let showShield = false;/);
  assert.match(script, /localStorage.getItem\(SHIELD_STORAGE_KEY\) === "true"/);
  assert.match(css, /\.shieldwrap,\s*\.actor-shield\s*\{[^}]*display: none;/s);
  assert.match(css, /:root\[data-show-shield="true"\] \.shieldwrap:not\(\[hidden\]\),\s*:root\[data-show-shield="true"\] \.actor-shield\s*\{[^}]*display: block;/s);
});

function colorLuminance(hex) {
  const channels = hex.slice(1).match(/../g).map(channel => parseInt(channel, 16) / 255);
  const [red, green, blue] = channels.map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return red * 0.2126 + green * 0.7152 + blue * 0.0722;
}

function contrast(first, second) {
  const values = [colorLuminance(first), colorLuminance(second)].sort((a, b) => b - a);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

test("深色名字框有可辨边界，存活和死亡名字均保持可读对比度", async () => {
  const css = await readExampleFile("show.css");
  const palette = css.match(/:root\[data-theme="dark"\]\s*\{([^}]+)\}/)[1];
  const colors = Object.fromEntries([...palette.matchAll(/--([\w-]+): (#[\da-f]{6});/g)].map(match => [match[1], match[2]]));
  assert.ok(contrast(colors["message-text"], colors["message-bg"]) >= 7);
  assert.ok(contrast(colors["dead-text"], colors["dead-bg"]) >= 5);
  assert.ok(contrast(colors["name-bg"], colors.paper) >= 1.7, "名字框与页面底色区分");
  assert.match(palette, /--message-shadow: transparent;/);
  assert.match(css, /:root\[data-theme="dark"\] \.actor-name\s*\{[^}]*box-shadow: inset/s);
  const iconRule = css.match(/:root\[data-theme="dark"\] \.icon-sprite\s*\{([^}]+)\}/)[1];
  const iconBackground = iconRule.match(/background-color: (#[\da-f]{6});/)[1];
  assert.ok(contrast("#404040", iconBackground) >= 6, "原图深色边缘与图标衬底区分");
  assert.doesNotMatch(iconRule, /invert|grayscale/);
});
