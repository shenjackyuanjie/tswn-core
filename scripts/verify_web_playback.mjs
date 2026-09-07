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
const sandbox = { window, document: window.document, localStorage: window.localStorage,
  navigator: {}, performance, setTimeout, clearTimeout, URL, console,
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
 startBattle, pausePlayback, resumePlayback, replayCurrent, stepPlaybackTo,
 nextVisibleCursor, nextFrameCursor, previousVisibleCursor, previousFrameCursor,
 get battle() { return currentBattle; }, get plan() { return currentPlan; },
 get cursor() { return playbackCursor; }, get finished() { return playbackFinished; },
 setSpeed(value) { speedMode = value; },
};`;
  const mod = new vm.SourceTextModule(code, { context, identifier: path });
  modules.set(path, mod);
  await mod.link((specifier, parent) => load(resolve(dirname(parent.identifier), specifier)));
  return mod;
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
