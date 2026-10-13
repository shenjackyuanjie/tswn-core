import test from "node:test";
import assert from "node:assert/strict";
import { watchDarkReader } from "../examples/show-theme-compat.js";

function page(theme = "dark") {
  const attributes = new Map();
  const root = { dataset: { theme }, getAttribute(name) { return attributes.get(name) ?? null; } };
  const notice = { hidden: true, textContent: "" };
  const scheme = { content: "light dark" };
  let observer;
  class Observer {
    constructor(callback) { this.callback = callback; this.connected = false; observer = this; }
    observe(target) { assert.equal(target, root); this.connected = true; }
    disconnect() { this.connected = false; }
  }
  const document = { documentElement: root, defaultView: { MutationObserver: Observer },
    querySelector(selector) { return selector === "#themeNotice" ? notice : scheme; } };
  const stop = watchDarkReader(document);
  const mutate = (name, value) => {
    if (name === "data-theme") root.dataset.theme = value;
    else if (value === null) attributes.delete(name);
    else attributes.set(name, value);
    if (observer.connected) observer.callback();
  };
  return { root, notice, scheme, stop, mutate };
}

test("原生明暗主题不会误报 Dark Reader，主题声明跟随切换", () => {
  const current = page();
  assert.equal(current.scheme.content, "dark");
  assert.equal(current.notice.hidden, true);
  assert.equal(current.notice.textContent, "");
  current.mutate("data-theme", "light");
  assert.equal(current.scheme.content, "light");
  assert.equal(current.notice.hidden, true);
  current.stop();
});

test("Dark Reader 深色转换出现和移除时同步提醒，不改变原生主题", () => {
  const current = page("light");
  current.mutate("data-darkreader-scheme", "dark");
  assert.equal(current.notice.hidden, true, "不完整标记不足以判断启用");
  current.mutate("data-darkreader-mode", "dynamic");
  assert.equal(current.notice.hidden, false);
  assert.match(current.notice.textContent, /Dark Reader.*二次转换.*重影/);
  assert.equal(current.root.dataset.theme, "light");
  current.mutate("data-darkreader-mode", null);
  assert.equal(current.notice.hidden, true);
  assert.equal(current.notice.textContent, "");
  current.stop();
});

test("Dark Reader dimmed 模式不当作深色转换提醒", () => {
  const current = page();
  current.mutate("data-darkreader-mode", "dynamic");
  current.mutate("data-darkreader-scheme", "dimmed");
  assert.equal(current.notice.hidden, true);
  current.stop();
});

test("断开监听后不再更新提醒或主题声明", () => {
  const current = page();
  current.stop();
  current.stop();
  current.mutate("data-darkreader-mode", "dynamic");
  current.mutate("data-darkreader-scheme", "dark");
  current.mutate("data-theme", "light");
  assert.equal(current.notice.hidden, true);
  assert.equal(current.scheme.content, "dark");
});
