import test from "node:test";
import assert from "node:assert/strict";
import { formatGlobalPlayerName, replayDisplayName, phantomDisplayName } from "../examples/show-utils.js";
import { actorToken, buildFrameRows, renderPlayers } from "../examples/show-render.js";
import { buildPlayerDetailHtml } from "../examples/show-detail.js";
import { winnerNamesText, buildReplayResultSummary, buildReplayResultTableHtml } from "../examples/show-replay.js";
import { BattleDisplay } from "../examples/show-display.js";

const state = (id, name = "同名", kind = null) => ({
  id, display_name: name, id_name: name, team_index: id % 2, input_team_index: 0,
  minion_kind: kind, display_index: 1, hp: 100, max_hp: 100, alive: true,
  magic_point: 0, move_point: 0,
});
const rendered = (frame, cache = new Map()) => buildFrameRows(frame, 0, [], cache).map(chunk => chunk.html).join("");
const frameOf = (states, parts, winner = false) => ({ states, rows: [{ clips: [{
  delay: 0, sidebar_states: states, sidebar_previous_states: states, winner, parts,
}] }] });

function fullList(states) {
  const list = { dataset: {}, querySelectorAll: () => [], innerHTML: "" };
  renderPlayers(states, states, states, null, list, new Map());
  return list.innerHTML;
}

test("同名本体与各自第一个分身均按真实全局ID区分，缺号不重排", () => {
  const all = [state(0, "A"), state(2, "B"), state(4, "A", "clone"), state(7, "B", "clone")];
  assert.deepEqual(all.map(item => replayDisplayName(item)), ["A#0", "B#2", "A#4", "B#7"]);
  const html = fullList([all[3], all[0], all[2]]);
  for (const label of ["A#0", "A#4", "B#7"]) assert(html.includes(`>${label}</span>`));
  assert.doesNotMatch(html, /class="player-id"/);
  assert.doesNotMatch(html, /A #1|B #1/);
});

test("所有实体类型统一附加全局编号，完全忽略display_index", () => {
  for (const kind of [null, "clone", "summon", "shadow", "zombie"]) {
    assert.equal(replayDisplayName(state(7, "名字", kind)), "名字#7");
    assert.equal(replayDisplayName({ ...state(7, "名字", kind), display_index: 999 }), "名字#7");
  }
  assert.equal(replayDisplayName({ id: 9, minion_kind: "shadow" }), "幻影#9");
  assert.equal(replayDisplayName({ id: 12, minion_kind: "summon" }), "使魔#12");
  assert.equal(replayDisplayName({ id: 14, minion_kind: "zombie" }), "丧尸#14");
});

test("原名中的#数字保留，显式display_label保证跨适配层幂等", () => {
  assert.equal(formatGlobalPlayerName("A#7", 4), "A#7#4");
  assert.equal(replayDisplayName(state(4, "A#7", "clone")), "A#7#4");
  const labeled = { ...state(4, "A#7#4", "clone"), display_label: "A#7#4" };
  assert.equal(replayDisplayName(labeled), "A#7#4");
  assert.equal(replayDisplayName({ ...labeled, display_name: replayDisplayName(labeled) }), "A#7#4");
});

test("缺状态可用真实fallback0，state自身ID优先，不伪造编号", () => {
  assert.equal(replayDisplayName(undefined, 0), "幻影#0");
  assert.equal(phantomDisplayName(0), "幻影#0");
  assert.equal(replayDisplayName({ id: 0 }, 9), "角色#0");
  assert.equal(replayDisplayName({ display_name: "A" }, 9), "A#9");
  assert.equal(replayDisplayName(undefined), "未知角色");
});

test("actorToken和角色详情使用相同全局名称并转义原名", () => {
  const actor = state(0, "<A>#7");
  assert.match(actorToken(actor, actor, actor, {}, { showHp: false }), /&lt;A&gt;#7#0/);
  assert.match(buildPlayerDetailHtml(actor, actor, false), /class="detail-name">&lt;A&gt;#7#0</);
  assert.match(buildPlayerDetailHtml(actor, null, false), /class="detail-name">&lt;A&gt;#7#0</);
});

test("结构化片段重复渲染不叠加后缀，也不改规范快照", () => {
  const all = [state(0, "A"), state(4, "A", "clone"), state(7, "B", "clone")];
  const frame = frameOf(all, all.map(item => ({ kind: "player", player_id: item.id, text: item.display_name })));
  const before = JSON.stringify(frame);
  const cache = new Map();
  const first = rendered(frame, cache);
  assert.equal(rendered(frame, cache), first);
  for (const label of ["A#0", "A#4", "B#7"]) assert(first.includes(`>${label}</span>`));
  assert.doesNotMatch(first, /#0#0|#4#4|#7#7/);
  assert.equal(JSON.stringify(frame), before);
});

test("instant已格式化sidebar和无sidebar的显式part标签都不会二次加号", () => {
  const labeled = { ...state(4, "A#4", "clone"), display_label: "A#4" };
  const parts = [{ kind: "player", player_id: 4, text: "A#4", display_label: "A#4" }];
  const cache = new Map([[4, { ...state(4, "旧名"), display_label: "旧名#4" }]]);
  assert.match(rendered(frameOf([labeled], parts), cache), />A#4</);
  const noState = frameOf([], [{ ...parts[0], display_label: "新名#4" }]);
  const html = rendered(noState, cache);
  assert.match(html, />新名#4</);
  assert.doesNotMatch(html, /新名#4#4/);
});

test("胜者片段、胜者概况、结算名字和致命一击均保留全局编号", () => {
  const a = state(0, "A");
  const b = { ...state(2, "B"), hp: 0, alive: false };
  const clone = { ...state(7, "A", "clone"), owner_id: 0 };
  const winnerFrame = frameOf([a, clone], [
    { kind: "player", player_id: 7, text: "A" }, { kind: "text", text: "获胜" },
  ], true);
  assert.match(rendered(winnerFrame), /A#7获胜/);
  const replay = { players: [a, b], initial_states: [a, { ...b, hp: 100, alive: true }],
    final_states: [a, b, clone], winner_ids: [0, 7], frames: [{
      states: [a, b, clone], updates: [{ caster_id: 7, target_id: 2, tone: "damage", score: 10 }],
    }] };
  assert.equal(winnerNamesText(replay), "A#0、A#7");
  const summary = buildReplayResultSummary(replay);
  assert.equal(summary.losers[0].killed_by.display_label, "A#7");
  const html = buildReplayResultTableHtml(replay);
  assert.match(html, />A#0</);
  assert.match(html, />B#2</);
  assert.match(html, />A#7</);
});

test("昵称和显式标签的装饰可重复，清除昵称后恢复原名的同一编号", () => {
  const canonical = state(0, "A#7");
  let nickname = "昵称";
  const display = new BattleDisplay([canonical], () => null, () => nickname);
  const decorated = display.state(canonical);
  assert.equal(decorated.display_name, "昵称");
  assert.equal(decorated.display_label, "昵称#0");
  assert.equal(display.state(decorated).display_label, "昵称#0");
  nickname = "";
  assert.equal(display.state(decorated).display_label, "A#7#0");
  assert.equal(canonical.display_name, "A#7");
});


test("缺帧快照时仍给原始玩家引用使用昵称和全局0号", () => {
  const actor = state(0, "原名");
  const display = new BattleDisplay([actor], () => null, () => "昵称");
  const canonical = frameOf([], [{ kind: "player", player_id: 0, text: "原名" }]);
  const decorated = display.frame(canonical);
  assert.equal(decorated.rows[0].clips[0].parts[0].display_label, "昵称#0");
  assert.match(rendered(decorated), />昵称#0</);
  assert.equal(canonical.rows[0].clips[0].parts[0].text, "原名");
});
