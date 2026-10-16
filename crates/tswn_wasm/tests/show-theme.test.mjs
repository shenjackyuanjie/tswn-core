import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

function colorLuminance(hex) {
  const channels = hex.slice(1).match(/../g).map(channel => parseInt(channel, 16) / 255);
  const [red, green, blue] = channels.map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return red * 0.2126 + green * 0.7152 + blue * 0.0722;
}

function contrast(first, second) {
  const values = [colorLuminance(first), colorLuminance(second)].sort((a, b) => b - a);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

test("深色配色的文字、名字框与图标衬底保持可读对比度", async () => {
  const css = await readFile(new URL("../examples/show-base.css", import.meta.url), "utf8");
  const palette = css.match(/:root\[data-theme="dark"\]\s*\{([^}]+)\}/)[1];
  const colors = Object.fromEntries([...palette.matchAll(/--([\w-]+): (#[\da-f]{6});/g)].map(match => [match[1], match[2]]));
  assert.ok(contrast(colors["message-text"], colors["message-bg"]) >= 7);
  assert.ok(contrast(colors["dead-text"], colors["dead-bg"]) >= 5);
  assert.ok(contrast(colors["name-bg"], colors.paper) >= 1.7, "名字框与页面底色区分");
  const iconRule = css.match(/:root\[data-theme="dark"\] \.icon-sprite\s*\{([^}]+)\}/)[1];
  const iconBackground = iconRule.match(/background-color: (#[\da-f]{6});/)[1];
  assert.ok(contrast("#404040", iconBackground) >= 6, "原图深色边缘与图标衬底区分");
});

test("浅色模式的新增护盾区段能与底轨及已有护盾区分", async () => {
  const css = await readFile(new URL("../examples/show-base.css", import.meta.url), "utf8");
  const palette = css.match(/:root\s*\{([^}]+)\}/)[1];
  const colors = Object.fromEntries([...palette.matchAll(/--([\w-]+): (#[\da-f]{6});/g)].map(match => [match[1], match[2]]));
  // 新增区段画在底轨上：与底轨的对比要求 3:1，保证浅色模式下看得见。
  assert.ok(contrast(colors["shield-recover"], colors["shield-background"]) >= 3);
  // 新增区段叠在已有护盾填充之上（填充区左侧、增量区右侧），只要明显更深即可区分。
  assert.ok(contrast(colors["shield-recover"], colors.shield) >= 2.5);
});
