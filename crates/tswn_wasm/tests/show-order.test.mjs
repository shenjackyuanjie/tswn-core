import test from "node:test";
import assert from "node:assert/strict";
import { renderPlayers } from "../examples/show-render.js";

/** 输入两组：z_last/a_first 写在前面，m_third/b_second 写在后面。 */
const inputOrder = [
  { id: 0, name: "z_last", inputGroup: 0, runtimeTeam: 1 },
  { id: 1, name: "a_first", inputGroup: 0, runtimeTeam: 1 },
  { id: 2, name: "m_third", inputGroup: 1, runtimeTeam: 0 },
  { id: 3, name: "b_second", inputGroup: 1, runtimeTeam: 0 },
];

const stateOf = ({ id, name, inputGroup, runtimeTeam }) => ({
  id,
  team_index: runtimeTeam,
  input_team_index: inputGroup,
  id_name: name,
  icon_key: name,
  display_name: name,
  hp: 100,
  max_hp: 100,
  alive: true,
  magic_point: 0,
  move_point: 0,
});

const playerOf = (entry) => {
  const state = stateOf(entry);
  return { ...state, icon_class_id: entry.id };
};

function renderList(states, players) {
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers(players, states, states, null, list, new Map());
  const names = [...list.innerHTML.matchAll(/class="name[^"]*">([^<]+)</g)].map((match) => match[1]);
  const labels = [...list.innerHTML.matchAll(/class="team-label">([^<]+)</g)].map((match) => match[1]);
  return { html: list.innerHTML, names, labels };
}

test("左侧按输入分组与输入顺序排列，与运行时分队号无关", () => {
  const states = inputOrder.map(stateOf);
  const { names, labels } = renderList(states, inputOrder.map(playerOf));
  // 运行时把先输入的一组判成 team 1，展示仍按输入顺序来。
  assert.deepEqual(names, ["z_last", "a_first", "m_third", "b_second"]);
  assert.deepEqual(labels, ["Team 1", "Team 2"]);
});

test("缺 input_team_index 的旧数据退回运行时分队号", () => {
  const legacyStates = inputOrder.map(({ id, name, runtimeTeam }) => ({
    ...stateOf({ id, name, inputGroup: runtimeTeam, runtimeTeam }),
    input_team_index: null,
  }));
  const legacyPlayers = inputOrder.map(({ id, name, runtimeTeam }) => ({
    id,
    team_index: runtimeTeam,
    id_name: name,
    icon_key: name,
    display_name: name,
    icon_class_id: id,
  }));
  const { names, labels } = renderList(legacyStates, legacyPlayers);
  assert.deepEqual(names, ["m_third", "b_second", "z_last", "a_first"], "没有输入信息时按运行时分队号排");
  assert.deepEqual(labels, ["Team 1", "Team 2"]);
});

test("召唤单位仍排在其主人之后，并沿用主人的输入分组", () => {
  const states = inputOrder.map(stateOf);
  const minion = {
    ...stateOf({ id: 9, name: "召唤物", inputGroup: 1, runtimeTeam: 0 }),
    owner_id: 2,
    minion_kind: "summon",
    icon_class_id: 2,
  };
  const { names } = renderList([...states, minion], [...inputOrder.map(playerOf), { ...minion, id_name: "summon" }]);
  assert.deepEqual(names, ["z_last", "a_first", "m_third", "召唤物", "b_second"]);
});

test("换组时重建整张表，布局键包含输入分组序号", () => {
  const states = inputOrder.map(stateOf);
  const list = { dataset: {}, querySelectorAll() { return []; }, innerHTML: "" };
  renderPlayers(inputOrder.map(playerOf), states, states, null, list, new Map());
  assert.equal(list.dataset.playerLayoutKey, "0:0|0:1|1:2|1:3");
  const regrouped = states.map((state) => ({ ...state, input_team_index: state.id < 2 ? 1 : 0 }));
  renderPlayers(inputOrder.map(playerOf), regrouped, regrouped, null, list, new Map());
  assert.equal(list.dataset.playerLayoutKey, "0:2|0:3|1:0|1:1");
  const { names } = renderList(regrouped, inputOrder.map(playerOf));
  assert.deepEqual(names, ["m_third", "b_second", "z_last", "a_first"]);
});
