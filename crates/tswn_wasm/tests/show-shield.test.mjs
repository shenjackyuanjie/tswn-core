import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { actorShieldMetrics } from "../examples/show-utils.js";
import { actorToken, buildFrameRows, renderPlayers } from "../examples/show-render.js";
import { buildPlayerDetailHtml } from "../examples/show-detail.js";

const player = { id: 1, team_index: 0, id_name: "盾手", display_name: "盾手" };
const state = { ...player, hp: 100, max_hp: 100, shield: 40, max_shield: 80, alive: true, magic_point: 0, move_point: 0 };

test("盾条按当前护盾熟练度确定长度，并保留增减区段", () => {
  const recover = actorShieldMetrics(state, { ...state, shield: 20 });
  assert.equal(recover.totalWidth, 20);
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

test("盾条满格取熟练度，盾值超过熟练度时由当前值撑开；旧数据回退到盾值", () => {
  const large = actorShieldMetrics({ ...state, shield: 160 });
  assert.equal(large.maxValue, 160);
  assert.equal(large.totalWidth, 40);
  assert.equal(large.fillWidth, 40);
  assert.equal(large.currentValue, 160);
  const legacy = actorShieldMetrics({ ...state, max_shield: undefined, shield: 160 });
  assert.equal(legacy.maxValue, 160);
  assert.equal(legacy.totalWidth, 40);
  const trimmed = actorShieldMetrics({ ...state, shield: 40 }, { ...state, shield: 40 });
  assert.equal(trimmed.maxValue, 80, "熟练度仍决定满格长度");
  assert.equal(trimmed.totalWidth, 20);
  for (const shield of [undefined, -1, NaN, Infinity]) {
    const metrics = actorShieldMetrics({ ...state, shield });
    assert.equal(metrics.fillWidth, 0);
    assert.equal(metrics.deltaWidth, 0);
    assert.equal(metrics.maxValue, 80);
  }
  assert.equal(actorShieldMetrics({ ...state, max_shield: 0, shield: 0 }).totalWidth, 0);
  assert.equal(actorShieldMetrics({ ...state, alive: false }).fillWidth, 0);
  assert.equal(actorShieldMetrics({ ...state, shield: 1 }).fillWidth, 1);
  assert.equal(actorShieldMetrics(null), null);
});

test("只有护盾变化时只画盾条，并占用体力条的位置", () => {
  const gained = actorToken(player, state, { ...state, shield: 0 }, {}, { showHp: false });
  assert.match(gained, /actor-shield/);
  assert.match(gained, /actor-shield-delta is-recover/);
  // 体力没变化 → 不画体力条，盾条单独占用体力条的槽位（CSS 默认 top: -2px）。
  assert.doesNotMatch(gained, /class="actor-hp"/);
  assert.doesNotMatch(gained, /has-hp/);
  assert.match(gained, /class="actor-vitals"><span class="actor-shield"/);
  assert.match(gained, /class="actor-token has-shield"/);
  const broken = actorToken(player, { ...state, shield: 0 }, state, {}, { showHp: false });
  assert.match(broken, /actor-shield-delta is-damage/);
  assert.doesNotMatch(broken, /class="actor-hp"/);
  // 护盾没变化时两侧都不出现，和体力条一样不做常驻底轨。
  assert.doesNotMatch(actorToken(player, state, state, {}, { showHp: false }), /actor-shield/);
  assert.doesNotMatch(actorToken(player, state, state, {}, { showHp: false }), /has-shield/);
  assert.doesNotMatch(actorToken(player, { ...state, shield: 0, alive: false }, state, {}, { deathEffect: true }), /actor-shield/);
});

test("体力与护盾同时变化时才叠放：体力条在下、盾条紧贴其上", () => {
  const both = actorToken(player, { ...state, hp: 60 }, { ...state, hp: 100, shield: 0 }, {}, { showHp: true });
  assert.match(both, /class="actor-token has-hp has-shield"/);
  // DOM 顺序保持 hp → shield，竖排（column-reverse）后盾条落在体力条上方。
  assert.match(both, /class="actor-vitals">[\s\S]*class="actor-hp"[\s\S]*class="actor-shield"/);
  assert.match(both, /actor-hp-delta is-damage/);
  const hpOnly = actorToken(player, { ...state, hp: 60 }, { ...state, hp: 100 }, {}, { showHp: true });
  assert.match(hpOnly, /class="actor-token has-hp"/);
  assert.doesNotMatch(hpOnly, /actor-shield/);
});

test("叠起来的盾条与体力条正好贴合，单盾条则落在体力条槽位", async () => {
  const [replay, players] = await Promise.all([
    readFile(new URL("../examples/show-replay.css", import.meta.url), "utf8"),
    readFile(new URL("../examples/show-players.css", import.meta.url), "utf8"),
  ]);
  const ruleBlock = (css, selector) => {
    const index = css.indexOf(selector);
    assert.ok(index >= 0, `缺少样式规则：${selector}`);
    const start = css.indexOf("{", index);
    return css.slice(start + 1, css.indexOf("}", start));
  };
  const px = (css, selector, property) => {
    const match = ruleBlock(css, selector).match(new RegExp(`${property}:\\s*(-?[\\d.]+)px`));
    return match ? Number(match[1]) : Number.NaN;
  };
  const shieldHeight = px(players, ".actor-shield {", "height");
  const hpHeight = px(replay, ".actor-hp {", "height");
  const vitalsTop = px(replay, ".actor-vitals {", "top");
  const stackedTop = px(replay, ".has-shield.has-hp .actor-vitals {", "top");
  const direction = ruleBlock(replay, ".has-shield.has-hp .actor-vitals {").match(/flex-direction:\s*([\w-]+)/)?.[1];
  assert.equal(shieldHeight, 2, "盾条高度");
  assert.equal(hpHeight, 2, "体力条高度");
  assert.equal(vitalsTop, -hpHeight, "没有体力条时盾条就落在体力条槽位");
  assert.equal(direction, "column-reverse", "两条同时出现时盾条排在体力条上方");
  // 容器上移量等于两条高度之和：盾条底边正好压在体力条顶边上。
  assert.equal(stackedTop, -(hpHeight + shieldHeight), "两条条形紧贴");
  assert.equal(px(players, ".actor-token.has-shield.has-hp {", "margin-top"), hpHeight + shieldHeight, "叠放时的行高留白");
  assert.equal(ruleBlock(players, ".actor-token.has-shield.has-hp {").includes("margin-top"), true);
  assert.equal(players.includes(".actor-token.has-shield {"), false, "单盾条不再额外留白");
  assert.equal(ruleBlock(players, ".actor-shield {").includes("box-shadow"), false, "盾条不再描边或加高光");
  assert.doesNotMatch(actorToken(player, { ...state, shield: 40 }, { ...state, shield: 0 }, {}), /min-width:/, "名字框不再为条形撑宽");
});

test("结构化片段使用每位角色的前后盾值，保持原始回放不变", () => {
  const other = { ...state, id: 2, display_name: "旁观者", shield: 0, max_shield: 0 };
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

test("侧栏盾条在护盾变化时显示并按熟练度定长，空盾不留痕", () => {
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers([player], [state], [{ ...state, shield: 20 }], null, list, new Map());
  assert.match(list.innerHTML, /class="shieldwrap"/);
  assert.match(list.innerHTML, /title="护盾：40 \/ 80"/);
  assert.match(list.innerHTML, /shield-delta is-recover/);
  renderPlayers([player], [{ ...state, shield: 0 }], [{ ...state, shield: 20 }], null, list, new Map());
  assert.doesNotMatch(list.innerHTML, /class="shieldwrap"[^>]* hidden/);
  renderPlayers([player], [{ ...state, shield: 0 }], [{ ...state, shield: 0 }], null, list, new Map());
  assert.match(list.innerHTML, /class="shieldwrap"[^>]* hidden/);
  // 缺熟练度时退回盾值当满格，仍有盾就照样显示。
  renderPlayers([player], [{ ...state, max_shield: 0 }], [], null, list, new Map());
  assert.doesNotMatch(list.innerHTML, /class="shieldwrap"[^>]* hidden/);
  assert.match(list.innerHTML, /title="护盾：40 \/ 40"/);
  renderPlayers([player], [{ ...state, shield: 0, max_shield: 0 }], [], null, list, new Map());
  assert.match(list.innerHTML, /class="shieldwrap"[^>]* hidden/);
});

test("熟练度变化只改满格长度，超过熟练度的盾值由当前值撑开", () => {
  assert.equal(actorShieldMetrics({ ...state, max_hp: 9999 }).totalWidth, 20);
  assert.equal(actorShieldMetrics({ ...state, max_shield: 160 }, state).totalWidth, 40);
  const raised = actorShieldMetrics({ ...state, shield: 20, max_shield: 160 }, { ...state, shield: 20 });
  assert.equal(raised.totalWidth, 40, "吞噬抬高熟练度后条更长");
  assert.equal(raised.fillWidth, 5);
  const lowered = actorShieldMetrics({ ...state, shield: 20, max_shield: 8 }, { ...state, shield: 20 });
  assert.equal(lowered.maxValue, 80, "同一对片段里沿用较大熟练度，避免动画抖动");
  const overflow = actorShieldMetrics({ ...state, shield: 160, max_shield: 8 });
  assert.equal(overflow.totalWidth, 40, "超过熟练度的叠加盾值也完整显示");
  assert.equal(overflow.fillWidth, 40);
});

test("角色详情与侧栏使用同一份实时状态标签，包含护盾且不改原数据", () => {
  const current = Object.freeze({ ...state, status_labels: Object.freeze(["聚气", "护盾 +40"]) });
  const before = JSON.stringify(current);
  const html = buildPlayerDetailHtml(player, current, false);
  assert.match(html, /<dt>状态<\/dt><dd>聚气、护盾 \+40<\/dd>/);
  assert.equal((html.match(/护盾 \+40/g) ?? []).length, 1);
  assert.equal(JSON.stringify(current), before);
  assert.match(buildPlayerDetailHtml(player, { ...state, status_labels: ["聚气"] }, false), /聚气、护盾 \+40/);
  assert.doesNotMatch(buildPlayerDetailHtml(player, { ...state, shield: 0 }, false), /护盾 \+/);
  assert.doesNotMatch(buildPlayerDetailHtml(player, { ...state, alive: false }, false), /护盾 \+/);
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
