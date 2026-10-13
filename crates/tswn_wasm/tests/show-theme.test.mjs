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
