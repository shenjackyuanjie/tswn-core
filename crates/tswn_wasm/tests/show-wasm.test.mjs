import test from "node:test";
import assert from "node:assert/strict";

import { actorToken, buildFrameRows } from "../examples/show-render.js";
import { createBattleStreamSource } from "../examples/show-wasm.js";
import { actorHpMetrics } from "../examples/show-utils.js";
import { BattleStreamController } from "../examples/show-stream.js";

// 真实流适配器与历史控制器共同验证采样缓存的生命周期。
test("state rates finish ten thousand samples once per frame and reuse history", async () => {
  let samples = 0, freed = 0;
  const states = [{id: 0, team_index: 0, owner_id: null}, {id: 1, team_index: 1, owner_id: null}];
  const api = { BattleSession: class {
    constructor() { this.frames = 0; }
    initial_states() { return states; }
    next_frame() { return {frame_index: this.frames++, states, rows: []}; }
    current_win_rate_batch(start, count) { samples += count; return [count, 0, 0]; }
    is_done() { return this.frames === 2; }
    result() { return {status: 'finished', final_states: states}; }
    free() { freed++; }
  }};
  const makeSource = () => createBattleStreamSource('a\n\nb', null, null, null, {api, strategy: 'legacy', stateRates: true});
  const controller = new BattleStreamController(await makeSource(), {paused: true});
  const first = await controller.ensureFrame(0);
  assert.deepEqual(first.state_rates, {counts: [10000, 0, 0], rounds: 10000});
  await controller.ensureFrame(1);
  assert.equal(samples, 20000);
  controller.setPlaybackFrame(-1);
  assert.equal(await controller.ensureFrame(0), first);
  controller.setPlaybackFrame(0);
  await controller.ensureFrame(1);
  assert.equal(samples, 20000);
  controller.dispose();
  assert.equal(controller.history.length, 0);
  const nextBattle = new BattleStreamController(await makeSource(), {paused: true});
  const newFirst = await nextBattle.ensureFrame(0);
  assert.notEqual(newFirst, first);
  assert.equal(samples, 30000);
  nextBattle.dispose();
  assert.equal(freed, 2);
});

function streamApi({ failAt = -1, initialFailure = false } = {}) {
  const calls = { pulls: 0, frees: 0, eager: 0, options: null };
  const initial = [{ id: 0, hp: 100, team_index: 0, display_name: "alpha", owner_id: null }];
  const frames = [{ frame_index: 0, states: initial, rows: [] }, { frame_index: 1, states: initial, rows: [] }];
  const result = { status: "finished", stop_reason: "winner", frames_emitted: 2 };
  return { calls, initial, frames, result, api: {
    BattleSession: class {
      constructor(raw, options) { calls.options = options; calls.raw = raw; }
      initial_states() { if (initialFailure) throw new Error("initial failed"); return initial; }
      next_frame() {
        if (calls.pulls === failAt) throw new Error("runtime failed");
        return frames[calls.pulls++] ?? null;
      }
      is_done() { return calls.pulls >= frames.length; }
      result() { return result; }
      free() { calls.frees += 1; }
    },
    battle_replay() { calls.eager += 1; throw new Error("must not eagerly run battle"); },
    name_to_png_base64(key) { return `icon:${key}`; },
  } };
}

test("stream source creates BattleSession and returns initial without pulling", async () => {
  const fake = streamApi();
  const source = await createBattleStreamSource("seed:42@!\nalpha", null, null, null, { api: fake.api, maxRounds: 8 });
  assert.equal(fake.calls.pulls, 0);
  assert.equal(fake.calls.eager, 0);
  assert.deepEqual(fake.calls.options, { include_icons: false, max_rounds: 8 });
  assert.equal(source.initial_states, fake.initial);
  assert.equal(source.players[0].hp, 100);
  assert.equal(source.seed_line, "seed:42@!");
  assert.equal(source.result(), null);
  assert.equal(source.loadIcon("alpha"), "icon:alpha");
  assert.equal(await source.nextFrame(), fake.frames[0]);
  assert.equal(await source.nextFrame(), fake.frames[1]);
  assert.equal(source.result(), fake.result);
  assert.equal(source.isDone(), true);
  assert.equal(fake.calls.frees, 1);
  assert.equal(await source.nextFrame(), null);
  source.dispose();
  source.dispose();
  assert.equal(fake.calls.frees, 1);
});

test("stream source frees an aborted session exactly once", async () => {
  const fake = streamApi();
  const source = await createBattleStreamSource("alpha", null, null, null, { api: fake.api });
  source.dispose();
  assert.equal(await source.nextFrame(), null);
  assert.equal(source.result(), null);
  assert.equal(fake.calls.pulls, 0);
  assert.equal(fake.calls.frees, 1);
});

test("stream errors propagate and release WASM without inventing a result", async () => {
  const fake = streamApi({ failAt: 1 });
  const source = await createBattleStreamSource("alpha", null, null, null, { api: fake.api });
  await source.nextFrame();
  await assert.rejects(source.nextFrame(), /runtime failed/);
  await assert.rejects(source.nextFrame(), /runtime failed/);
  assert.equal(source.result(), null);
  assert.equal(fake.calls.frees, 1);
  source.dispose();
  assert.equal(fake.calls.frees, 1);
});

test("stream constructor failure after allocation releases the handle", async () => {
  const fake = streamApi({ initialFailure: true });
  await assert.rejects(createBattleStreamSource("alpha", null, null, null, { api: fake.api }), /initial failed/);
  assert.equal(fake.calls.frees, 1);
});

test("revive HP bars render only the blue recovery segment", () => {
  const metrics = actorHpMetrics(
    { max_hp: 100, hp: 40, alive: true },
    { max_hp: 100, hp: 0, alive: false },
  );

  assert.equal(metrics.fillWidth, 0);
  assert.equal(metrics.previousWidth, 0);
  assert.equal(metrics.deltaLeft, 0);
  assert.equal(metrics.deltaWidth, 10);
  assert.equal(metrics.deltaKind, "recover");
});

test("explicitly disabled death effect keeps a normal actor name", () => {
  const html = actorToken(
    { id: 2, display_name: "zombie", icon_class_id: 2 },
    { max_hp: 100, hp: 0, alive: false },
    { max_hp: 100, hp: 0, alive: false },
    { tone: "normal" },
    { showHp: false, deathEffect: false },
  );

  assert.doesNotMatch(html, /namedie/);
  assert.match(html, /actor-name/);
});

test("stream players retain all sidebar detail fields", async () => {
  const fake = streamApi();
  Object.assign(fake.initial[0], { attack: 55, defense: 41, speed: 184, team_index: 0, input_team_index: 3 });
  const source = await createBattleStreamSource("alpha", null, null, null, { api: fake.api });
  assert.equal(source.players[0].attack, 55);
  assert.equal(source.players[0].defense, 41);
  assert.equal(source.players[0].speed, 184);
  assert.equal(source.players[0].team_index, 0);
  assert.equal(source.players[0].input_team_index, 3, "输入分组序号随玩家一起下发");
  source.dispose();
});

test("canonical clips preserve multi-target recovery, death, delay and dynamic entities", () => {
  const states = [
    { id: 1, display_name: "healer", hp: 80, max_hp: 100 },
    { id: 2, display_name: "target", hp: 0, max_hp: 100 },
    { id: 3, display_name: "summon", icon_class_id: 9, minion_kind: "summon", owner_id: 1, hp: 60, max_hp: 60 },
  ];
  const frame = { states, updates: [{ message_template: "MUST NOT INFER", hp_delta: 999 }], rows: [{
    indent: false, clips: [{ delay: 120, tone: "normal", color: "000000", sidebar_states: states,
      parts: [
        { kind: "player", player_id: 1, text: "healer", show_hp: true, hp_before: 40, hp_after: 80, death_effect: false },
        { kind: "player", player_id: 2, text: "target", show_hp: true, hp_before: 30, hp_after: 0, death_effect: true },
        { kind: "player", player_id: 3, text: "summon", show_hp: false, death_effect: false },
      ],
    }, { delay: 70, parts: [] }],
  }] };
  const players = new Map();
  const chunks = buildFrameRows(frame, 0, [], players);
  const html = chunks.map(chunk => chunk.html).join("");
  assert.match(html, /is-recover/);
  assert.match(html, /is-damage/);
  assert.match(html, /namedie/);
  assert.match(html, /summon/);
  assert.match(html, /icon_9/);
  assert.doesNotMatch(html, /MUST NOT INFER|999/);
  assert.equal(chunks.reduce((sum, chunk) => sum + chunk.delay, 0), 190);
  assert.equal(players.get(3).minion_kind, "summon");
  assert.deepEqual(buildFrameRows({ states, updates: frame.updates }, 0, [], new Map()), []);
});

test("search strategy configures the session before reading initial state", async () => {
  const fake = streamApi();
  fake.api.BattleSession.prototype.set_search = function (...budget) {
    fake.calls.budget = budget;
    assert.equal(fake.calls.pulls, 0);
  };
  const source = await createBattleStreamSource("alpha", null, null, null, { api: fake.api, strategy: "search" });
  assert.deepEqual(fake.calls.budget, [16, 128, 4, 1]);
  assert.equal(fake.calls.pulls, 0);
  source.dispose();
  assert.equal(fake.calls.frees, 1);
});

test("search refuses stale WASM and releases its session", async () => {
  const fake = streamApi();
  await assert.rejects(createBattleStreamSource("alpha", null, null, null, { api: fake.api, strategy: "search" }), /Clever/);
  assert.equal(fake.calls.frees, 1);
});
