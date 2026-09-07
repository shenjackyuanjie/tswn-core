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
window.localStorage = { getItem() { return null; }, setItem() {} };
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
const state = id => ({ id, id_name: `player${id}`, icon_key: `player${id}`, display_name: `player${id}`,
  team_index: id, hp: 100, max_hp: 100, alive: true, status_labels: [], move_point: 0, magic_point: 0 });
const initial = [state(1), state(2)];
const frame = index => ({ frame_index: index, round_index: index + 1, states: initial, updates: [], total_delay: 0,
  rows: [{ indent: false, clips: [{ delay: 0, color: '000000', tone: 'normal', parts: [{ kind: 'text', text: `event${index}` }] }] }] });
const flush = () => new Promise(resolve => setTimeout(resolve, 5));
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

// Replaying a completed battle uses the same received history.
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

// A late frame from an aborted battle must not enter the replacement history.
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

// A pending source creation is also isolated, including loading state.
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

// Streaming failure retains rendered history and never renders a fake result.
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

// Turbo yields according to visible chunks, not absolute cursor modulo.
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
