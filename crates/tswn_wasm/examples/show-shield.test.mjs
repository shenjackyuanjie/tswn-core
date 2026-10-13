import test from "node:test";
import assert from "node:assert/strict";
import { actorShieldMetrics } from "./show-utils.js";
import { actorToken, buildFrameRows, renderPlayers } from "./show-render.js";

const player = { id: 1, team_index: 0, id_name: "盾手", display_name: "盾手" };
const state = { ...player, hp: 100, max_hp: 100, shield: 40, alive: true, magic_point: 0, move_point: 0 };

test("盾条与血条使用相同的数值宽度，并保留增减区段", () => {
  const recover = actorShieldMetrics(state, { ...state, shield: 20 });
  assert.equal(recover.totalWidth, 25);
  assert.equal(recover.fillWidth, 10);
  assert.equal(recover.previousWidth, 5);
  assert.equal(recover.deltaLeft, 5);
  assert.equal(recover.deltaWidth, 5);
  assert.equal(recover.deltaKind, "recover");
  const damage = actorShieldMetrics({ ...state, shield: 0 }, state);
  assert.equal(damage.fillWidth, 0);
  assert.equal(damage.previousWidth, 10);
  assert.equal(damage.deltaWidth, 10);
  assert.equal(damage.deltaKind, "damage");
});

test("盾值超过最大 HP 时完整展示，缺省和无效盾值按零处理", () => {
  const large = actorShieldMetrics({ ...state, shield: 160 });
  assert.equal(large.totalWidth, 40);
  assert.equal(large.fillWidth, 40);
  for (const shield of [undefined, -1, NaN, Infinity]) {
    const metrics = actorShieldMetrics({ ...state, shield });
    assert.equal(metrics.fillWidth, 0);
    assert.equal(metrics.deltaWidth, 0);
  }
  assert.equal(actorShieldMetrics({ ...state, alive: false }).fillWidth, 0);
  assert.equal(actorShieldMetrics({ ...state, shield: 1 }).fillWidth, 1);
  assert.equal(actorShieldMetrics(null), null);
});

test("消息只在护盾变化时生成盾条，破盾仍保留消耗区段", () => {
  const gained = actorToken(player, state, { ...state, shield: 0 }, {}, { showHp: false });
  assert.match(gained, /actor-shield/);
  assert.match(gained, /actor-shield-delta is-recover/);
  assert.doesNotMatch(gained, /class="actor-hp"/);
  const broken = actorToken(player, { ...state, shield: 0 }, state, {}, { showHp: false });
  assert.match(broken, /actor-shield-delta is-damage/);
  assert.doesNotMatch(actorToken(player, state, state, {}), /actor-shield/);
  assert.doesNotMatch(actorToken(player, { ...state, shield: 0, alive: false }, state, {}, { deathEffect: true }), /actor-shield/);
});

test("结构化片段使用每位角色的前后盾值，保持原始回放不变", () => {
  const other = { ...state, id: 2, display_name: "旁观者", shield: 0 };
  const clip = { delay: 600, sidebar_states: [state, other], sidebar_previous_states: [{ ...state, shield: 20 }, other],
    parts: [{ kind: "player", player_id: 1, text: "盾手", show_hp: false },
      { kind: "text", text: "使用技能，" }, { kind: "player", player_id: 2, text: "旁观者", show_hp: false }] };
  const frame = { states: [state, other], rows: [{ clips: [clip] }] };
  const before = JSON.stringify(frame);
  const chunks = buildFrameRows(frame, 0, [], new Map());
  const html = chunks.map(chunk => chunk.html).join("");
  assert.equal((html.match(/class="actor-shield"/g) ?? []).length, 1);
  assert.match(html, /actor-shield-delta is-recover/);
  assert.equal(chunks[0].delay, 600);
  assert.equal(JSON.stringify(frame), before);
});

test("侧栏预备独立盾条和数值提示，无盾时隐藏空条", () => {
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers([player], [state], [{ ...state, shield: 20 }], null, list, new Map());
  assert.match(list.innerHTML, /class="shieldwrap"/);
  assert.match(list.innerHTML, /title="护盾：40"/);
  assert.match(list.innerHTML, /shield-delta is-recover/);
  renderPlayers([player], [{ ...state, shield: 0 }], [{ ...state, shield: 0 }], null, list, new Map());
  assert.match(list.innerHTML, /class="shieldwrap"[^>]* hidden/);
});

test("护盾剩余值与其他正面状态一起显示，不修改规范标签", () => {
  const statusLabels = Object.freeze(["聚气", "铁壁 +500"]);
  const current = Object.freeze({ ...state, status_labels: statusLabels });
  const before = JSON.stringify(current);
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers([player], [current], [current], null, list, new Map());
  assert.match(list.innerHTML, /class="status-pill positive">护盾 \+40<\/span>/);
  assert.match(list.innerHTML, /class="status-pill positive">聚气<\/span>/);
  assert.match(list.innerHTML, /铁壁 \+500/);
  assert.equal((list.innerHTML.match(/>护盾 \+40</g) ?? []).length, 1);
  assert.equal(JSON.stringify(current), before);
  assert.deepEqual(statusLabels, ["聚气", "铁壁 +500"]);
});

test("仅有护盾时也显示状态行，无盾或死亡时保留其他状态", () => {
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers([player], [state], [state], null, list, new Map());
  assert.match(list.innerHTML, /class="detail-line player-effects">/);
  assert.match(list.innerHTML, /class="status-pill positive">护盾 \+40<\/span>/);
  for (const shield of [undefined, 0, -1, NaN, Infinity]) {
    const current = { ...state, shield, status_labels: ["聚气"] };
    renderPlayers([player], [current], [state], null, list, new Map());
    assert.doesNotMatch(list.innerHTML, />护盾 \+/);
    assert.match(list.innerHTML, /class="status-pill positive">聚气<\/span>/);
  }
  renderPlayers([player], [{ ...state, alive: false }], [state], null, list, new Map());
  assert.doesNotMatch(list.innerHTML, />护盾 \+/);
  assert.match(list.innerHTML, /class="detail-line player-effects" hidden>/);
});
