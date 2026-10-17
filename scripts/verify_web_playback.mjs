import { readFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { parseHTML } from '../target/web-test-tools/node_modules/linkedom/esm/index.js';

const root = resolve('crates/tswn_wasm/examples');
const { window } = parseHTML(await readFile(resolve(root, 'index.html'), 'utf8'));
window.matchMedia = () => ({ matches: false, addEventListener() {} });
window.requestAnimationFrame = fn => setTimeout(fn, 0);
window.location = { href: 'http://localhost/index.html', search: '' };
const savedSettings = new Map([["tswn-show-shield", "false"]]);
window.localStorage = { getItem(key) { return savedSettings.get(key) ?? null; }, setItem(key, value) { savedSettings.set(key, value); } };
window.HTMLElement.prototype.focus = function () {};
window.HTMLElement.prototype.select = function () {};
let zeroSleeps = 0;
const nativeSetTimeout = setTimeout;
const timedSetTimeout = (fn, ms) => { if (ms === 0) zeroSleeps++; return nativeSetTimeout(fn, ms); };
window.setTimeout = timedSetTimeout;
const sandbox = { window, document: window.document, localStorage: window.localStorage,
  navigator: {}, performance, setTimeout: timedSetTimeout, clearTimeout, URL, URLSearchParams, console,
  Element: window.Element, HTMLElement: window.HTMLElement, HTMLStyleElement: window.HTMLStyleElement,
  HTMLButtonElement: window.HTMLButtonElement, Node: window.Node };
const context = vm.createContext(sandbox);
let nextSource;
const modules = new Map();
async function load(path) {
  if (modules.has(path)) return modules.get(path);
  if (path.endsWith('show-wasm.js')) {
    const mod = new vm.SyntheticModule(['ensureApi', 'createBattleStreamSource'], function () {
      this.setExport('ensureApi', async () => ({}));
      this.setExport('createBattleStreamSource', async () => nextSource);
    }, { context });
    modules.set(path, mod);
    return mod;
  }
  let code = await readFile(path, 'utf8');
  if (path.endsWith('show.js')) code = code.replace('void main();', '') + `
export const pageTest = {
 startBattle, pausePlayback, resumePlayback, replayCurrent, stepPlaybackTo, stepPlaybackForward,
 nextVisibleCursor, nextFrameCursor, previousVisibleCursor, previousFrameCursor, openPlayerDetail, saveCurrentNickname,
 get battle() { return currentBattle; }, get plan() { return currentPlan; },
 get cursor() { return playbackCursor; }, get finished() { return playbackFinished; },
 setSpeed(value) { speedMode = value; },
};`;
  const mod = new vm.SourceTextModule(code, { context, identifier: path });
  const linked = mod.link((specifier, parent) => load(resolve(dirname(parent.identifier), specifier))).then(() => mod);
  modules.set(path, linked);
  return linked;
}
const main = await load(resolve(root, 'show.js'));
await main.evaluate();
const page = main.namespace.pageTest;
const rootElement = window.document.documentElement;

const state = id => ({ id, id_name: `player${id}`, icon_key: `player${id}`, display_name: `player${id}`,
  team_index: id, hp: 100, max_hp: 100, shield: 0, max_shield: 80, alive: true, status_labels: [], move_point: 0, magic_point: 0 });
const initial = [state(1), state(2)];
const frame = index => ({ frame_index: index, round_index: index + 1, states: initial, updates: [], total_delay: 0,
  rows: [{ indent: false, clips: [{ delay: 0, color: '000000', tone: 'normal', parts: [{ kind: 'text', text: `event${index}` }] }] }] });
const flush = () => new Promise(resolve => setTimeout(resolve, 5));

// 使用真实 DOM 属性变化验证提醒，不读取浏览器扩展的全局设置。
const themeCompatibility = await load(resolve(root, "show-theme-compat.js"));
await themeCompatibility.evaluate();
const themeNotice = window.document.querySelector("#themeNotice");
const schemeMeta = window.document.querySelector('meta[name="color-scheme"]');
assert.equal(themeNotice.hidden, true);
rootElement.dataset.theme = "dark";
await flush();
assert.equal(schemeMeta.content, "dark");
rootElement.setAttribute("data-darkreader-mode", "dynamic");
rootElement.setAttribute("data-darkreader-scheme", "dark");
await flush();
assert.equal(themeNotice.hidden, false);
assert.match(themeNotice.textContent, /Dark Reader/);
assert.equal(rootElement.dataset.theme, "dark");
rootElement.setAttribute("data-darkreader-scheme", "dimmed");
await flush();
assert.equal(themeNotice.hidden, true);
rootElement.removeAttribute("data-darkreader-mode");
rootElement.removeAttribute("data-darkreader-scheme");
rootElement.dataset.theme = "light";
await flush();
assert.equal(schemeMeta.content, "light");
assert.equal(themeNotice.hidden, true);
console.log("PASS: 原生主题声明、Dark Reader 转换提醒与移除，不改变页面主题或播放状态");

function source(count) {
  let n = 0, freed = 0;
  const waiting = [];
  return {
    raw_input: 'player1\n\nplayer2', players: initial.map(x => ({ ...x })), initial_states: initial,
    get pulls() { return n; }, get freed() { return freed; }, waiting,
    nextFrame() { const index = n++; return new Promise(resolve => waiting.push(() => resolve(frame(index)))); },
    isDone() { return n >= count && waiting.length === 0; },
    result() { return { final_states: initial, winner_ids: [1] }; },
    dispose() { freed++; }, loadIcon() { return ''; },
    release() { const fn = waiting.shift(); assert.ok(fn); fn(); },
  };
}

nextSource = source(3);
await page.startBattle();
assert.match(window.document.querySelector('#playerList').textContent, /player1/);
assert.equal(window.document.querySelector('#inputPanel').hidden, true);
assert.equal(window.document.querySelector('#startBtn').disabled, false);
assert.equal(page.battle.frames.length, 0);
assert.equal(page.finished, false, 'known empty tail is not terminal');
assert.equal(nextSource.pulls, 1);
page.setSpeed('fast');
nextSource.release();
await flush();
assert.match(window.document.querySelector('#battleRows').textContent, /event0/);
assert.equal(page.finished, false);
nextSource.release();
await flush();
nextSource.release();
await flush();
assert.equal(page.battle.frames.length, 3);
assert.equal(page.finished, true);
assert.equal(nextSource.freed, 1);
assert.ok(window.document.querySelector('.battle-result-block'));
console.log('PASS: initial DOM before first frame; live tail; streamed terminal result; source release');

// 重播已完成战斗时使用同一份已接收历史。
const completedSource = nextSource;
await page.replayCurrent();
await flush();
assert.equal(completedSource.pulls, 3);
assert.equal(page.finished, true);
page.stepPlaybackTo(0);
await page.stepPlaybackForward();
assert.equal(page.cursor, page.nextVisibleCursor(0));
page.stepPlaybackTo(page.previousVisibleCursor(page.cursor));
assert.equal(page.cursor, 0);
await page.stepPlaybackForward(true);
assert.equal(page.cursor, page.plan.frames[0].end);
page.stepPlaybackTo(page.previousFrameCursor(page.cursor));
assert.equal(page.cursor, 0);

nextSource = source(30);
await page.startBattle();
page.pausePlayback();
nextSource.release();
await flush();
assert.equal(nextSource.pulls, 1, 'pause allows in-flight completion but no prefetch');
await page.stepPlaybackForward(true);
assert.equal(page.cursor, page.plan.frames[0].end);
const oneFrame = page.stepPlaybackForward(true);
await flush();
assert.equal(nextSource.pulls, 2, 'step at live tail requests exactly one frame');
nextSource.release();
await oneFrame;
await flush();
assert.equal(nextSource.pulls, 2);
assert.equal(page.cursor, page.plan.frames[1].end);
page.stepPlaybackTo(0);
page.resumePlayback();
page.setSpeed('fast');
await flush();
assert.equal(nextSource.pulls, 3, 'history is consumed before the next live pull');
page.pausePlayback();
nextSource.release();
await flush();
await page.stepPlaybackForward(true);
for (let i = 3; i < 23; i++) {
  const pending = page.stepPlaybackForward(true);
  await flush();
  nextSource.release();
  await pending;
}
const savedHtml = window.document.querySelector('#battleRows').innerHTML;
const savedCursor = page.cursor;
page.stepPlaybackTo(page.plan.frames[20].end);
page.stepPlaybackTo(savedCursor);
assert.equal(window.document.querySelector('#battleRows').innerHTML, savedHtml, 'checkpoint seek restores exact display');
assert.equal(nextSource.pulls, 23);
console.log('PASS: event/frame navigation; pause; one-frame demand; history resume; checkpoint; replay without recomputation');

// 来自已中止战斗的迟到帧不得进入替换后的历史。
const staleSource = source(2);
nextSource = staleSource;
await page.startBattle();
const replacement = source(2);
nextSource = replacement;
await page.startBattle();
page.pausePlayback();
staleSource.release();
await flush();
assert.equal(staleSource.freed, 1);
assert.equal(page.battle.frames.length, 0);
replacement.release();
await flush();
assert.equal(page.battle.frames.length, 1);

// 待处理的源创建也相互隔离，包括加载状态。
let finishCreate;
nextSource = new Promise(resolve => { finishCreate = resolve; });
const pendingStart = page.startBattle();
const oldCreation = source(2);
nextSource = source(2);
await page.startBattle();
page.pausePlayback();
finishCreate(oldCreation);
await pendingStart;
assert.equal(oldCreation.freed, 1);
assert.equal(page.battle.frames.length, 0);
nextSource.release();
await flush();

// 流式传输失败时保留已渲染历史，且绝不渲染伪造结果。
await page.stepPlaybackForward(true);
nextSource.nextFrame = async () => { throw new Error('test streaming failure'); };
await page.stepPlaybackForward(true);
assert.equal(page.battle.frames.length, 1);
assert.equal(page.battle.result, null);
assert.match(window.document.querySelector('#headerMeta').textContent, /test streaming failure/);
assert.match(window.document.querySelector('#battleRows').textContent, /event0/);
assert.equal(window.document.querySelector('.battle-result-block'), null);
page.stepPlaybackTo(0);
await page.stepPlaybackForward(true);
assert.match(window.document.querySelector('#battleRows').textContent, /event0/);

// Turbo 根据可见块让出执行权，而非根据绝对游标取模。
let emitted = 0;
nextSource = {
  ...source(30),
  nextFrame() { return Promise.resolve(frame(emitted++)); },
  isDone() { return emitted === 30; },
};
await page.startBattle();
page.setSpeed('turbo');
const sleepsBefore = zeroSleeps;
for (let i = 0; !page.finished && i < 100; i++) await flush();
assert.equal(page.finished, true);
const visibleChunks = page.plan.flatChunks.filter(chunk => chunk.visible).length;
assert.equal(zeroSleeps - sleepsBefore, Math.floor(visibleChunks / 24));
assert.equal(emitted, 30);
console.log('PASS: abort/generation isolation; source creation race; streaming error history; turbo visible-chunk yields');

const canonicalBefore = JSON.stringify(page.battle);
page.openPlayerDetail(1);
window.document.querySelector('#nicknameInput').value = '新的昵称';
page.saveCurrentNickname();
assert.match(window.document.querySelector('#playerList').textContent, /新的昵称/);
assert.equal(JSON.stringify(page.battle), canonicalBefore, 'editing a nickname cannot modify the battle DTO');
page.stepPlaybackTo(0);
assert.match(window.document.querySelector('#playerList').textContent, /新的昵称/);
assert.equal(JSON.stringify(page.battle), canonicalBefore);
console.log('PASS: page nickname edit rebuilds display and keeps canonical history unchanged');

let normalEmitted = 0;
nextSource = { ...source(1), nextFrame() { normalEmitted++; return Promise.resolve(frame(0)); }, isDone() { return normalEmitted === 1; } };
await page.startBattle();
await flush();
assert.equal(page.finished, true);
assert.equal(window.document.querySelector('.battle-result-block'), null, 'normal result waits 1500 ms');
page.pausePlayback();
page.resumePlayback();
const resultDeadline = performance.now() + 4000;
while (!window.document.querySelector('.battle-result-block') && performance.now() < resultDeadline) await flush();
assert.ok(window.document.querySelector('.battle-result-block'), 'resume during result delay eventually reveals result');
assert.match(window.document.querySelector('#plistMeta').textContent, /已接收 1 帧/);
console.log('PASS: normal result delay resumes after pause; failed history remains navigable; frame count stays current');
// 侧栏完整渲染与增量更新使用同一盾值计算，破盾只显示一次消耗区段。
const renderModule = await load(resolve(root, "show-render.js"));
await renderModule.evaluate();
const list = window.document.createElement("div");
const shieldPlayer = { ...state(9), status_labels: ["聚气"] };
const shieldStates = shield => [{ ...shieldPlayer, shield }];
const renderShield = (shield, previousShield) => renderModule.namespace.renderPlayers(
  [shieldPlayer], shieldStates(shield), shieldStates(previousShield), null, list, new Map());
renderShield(40, 0);
const playerRow = list.querySelector("tr[data-player-id]");
let shieldTrack = list.querySelector(".shieldwrap");
assert.equal(shieldTrack.hidden, false);
assert.equal(shieldTrack.title, "护盾：40 / 80");
assert.equal(shieldTrack.querySelector(".shield-fill").style.width, "10px");
const effects = playerRow.querySelector(".player-effects");
assert.equal(effects.hidden, false);
assert.equal(effects.querySelectorAll(".status-pill.positive").length, 2);
assert.equal(effects.textContent, "聚气护盾 +40");
renderShield(20, 40);
assert.equal(list.querySelector("tr[data-player-id]"), playerRow, "更新护盾不重建玩家行");
assert.equal(shieldTrack.querySelector(".shield-delta").className, "shield-delta is-damage");
assert.equal(shieldTrack.querySelector(".shield-fill").style.width, "5px");
assert.equal(playerRow.querySelector(".player-effects"), effects);
assert.equal(effects.textContent, "聚气护盾 +20");
renderShield(0, 20);
assert.equal(shieldTrack.hidden, false);
assert.equal(shieldTrack.title, "护盾：0 / 80");
assert.equal(effects.textContent, "聚气", "破盾移除护盾标签，保留其他状态");
assert.equal(effects.hidden, false);
assert.equal(shieldTrack.querySelector(".shield-delta").style.width, "5px");
renderShield(0, 0);
assert.equal(shieldTrack.hidden, true, "破盾归零后侧栏不留空条");
renderModule.namespace.renderPlayers([shieldPlayer], [{ ...shieldPlayer, shield: 0, max_shield: 160 }], shieldStates(10), null, list, new Map());
assert.equal(shieldTrack.style.width, "40px", "吞噬后底轨扩展");
assert.equal(shieldTrack.hidden, false, "上一片段仍有盾时保留消耗区段");
assert.equal(list.querySelector("tr[data-player-id]"), playerRow);
renderShield(40, 0);
renderModule.namespace.renderPlayers([shieldPlayer], [{ ...shieldPlayer, hp: 0, alive: false, shield: 40 }], shieldStates(40), null, list, new Map());
assert.equal(effects.textContent, "聚气", "死亡快照中的残留盾值不展示标签");
renderModule.namespace.renderPlayers([shieldPlayer], [{ ...shieldPlayer, shield: undefined }], shieldStates(0), null, list, new Map());
assert.equal(effects.textContent, "聚气", "旧数据缺盾字段时不制造标签");
assert.equal(list.querySelector("tr[data-player-id]"), playerRow);
// 正文 token：只有护盾变化时只画盾条并占用体力条槽位；体力与护盾同时变化时才叠放。
const shieldOnlyToken = renderModule.namespace.actorToken(
  { id: 9, display_name: "盾手" }, { ...shieldPlayer, shield: 40 }, { ...shieldPlayer, shield: 0 }, {}, { showHp: false });
assert.match(shieldOnlyToken, /class="actor-token has-shield"/);
assert.doesNotMatch(shieldOnlyToken, /class="actor-hp"/);
assert.match(shieldOnlyToken, /class="actor-vitals"><span class="actor-shield"/);
const stackedToken = renderModule.namespace.actorToken(
  { id: 9, display_name: "盾手" }, { ...shieldPlayer, hp: 60, shield: 40 }, { ...shieldPlayer, hp: 100, shield: 0 }, {}, { showHp: true });
assert.match(stackedToken, /class="actor-token has-hp has-shield"/);
assert.match(stackedToken, /class="actor-vitals">[\s\S]*class="actor-hp"[\s\S]*class="actor-shield"/);
const quietToken = renderModule.namespace.actorToken(
  { id: 9, display_name: "盾手" }, { ...shieldPlayer, shield: 40 }, { ...shieldPlayer, shield: 40 }, {}, { showHp: false });
assert.doesNotMatch(quietToken, /actor-shield/, "护盾没变化时正文不画盾条");
console.log("PASS: 侧栏盾条与正面护盾标签完整渲染、增量更新、破盾和死亡归零，正文盾条单显占体力条槽位、双显叠放，旧数据兼容");

nextSource = { ...source(1), players: [shieldPlayer], initial_states: shieldStates(40) };
await page.startBattle();
page.pausePlayback();
page.openPlayerDetail(9);
assert.match(window.document.querySelector("#detailContent").textContent, /状态聚气、护盾 \+40/);
assert.equal(window.document.querySelector(".shieldwrap").hidden, false);
assert.equal(savedSettings.get("tswn-show-shield"), "false");
assert.equal(JSON.stringify(page.battle.initial_states), JSON.stringify(shieldStates(40)));
console.log("PASS: 真实详情面板读取当前护盾标签，盾条不再有隐藏开关且不修改回放数据");
