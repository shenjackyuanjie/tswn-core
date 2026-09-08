import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import test from "node:test";

const require = createRequire(import.meta.url);
const packagePath = resolve(process.env.TSWN_WASM_NODE_PACKAGE || "target/battle_wasm_node/tswn_wasm.js");
const wasm = require(packagePath);
const fixtures = [
  "1v1-0f92cb76cc37fdc5.txt",
  "2v2-554f4128af707167.txt",
  "ffa_8-16d11de1ebe1df41.txt",
  "3v3v3-0ace5df17b84e26a.txt",
];

function assertPlain(value) {
  if (value === null || typeof value !== "object") return;
  assert.ok(Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype);
  for (const item of Object.values(value)) assertPlain(item);
}

for (const fixture of fixtures) {
  test(`canonical BattleSession parity: ${fixture}`, () => {
    const raw = readFileSync(resolve("crates/tswn_test/cases/runtime_stress", fixture), "utf8");
    for (const max_rounds of [1, 20_000]) {
      const options = { max_rounds, include_icons: false };
      const session = new wasm.BattleSession(raw, options);
      try {
        assert.equal(session.status(), "running");
        assert.equal(session.is_failed(), false);
        assert.equal(session.stop_reason(), null);
        assert.equal(session.result(), null);
        const replay = wasm.battle_replay(raw, options);
        assertPlain(replay);
        assert.deepEqual(session.initial_states(), replay.initial_states);
        const frames = [];
        for (let frame; (frame = session.next_frame()) !== null;) frames.push(frame);
        assert.deepEqual(frames, replay.frames);
        assertPlain(frames);
        const result = session.result();
        assertPlain(result);
        for (const [key, value] of Object.entries(result)) assert.deepEqual(value, replay[key], key);
        assert.deepEqual(session.current_states(), result.final_states);
        assert.equal(session.is_done(), true);
        assert.equal(session.is_failed(), false);
        assert.equal(session.is_finished(), result.finished);
        assert.equal(session.is_truncated(), result.truncated);
        assert.equal(session.status(), result.status);
        assert.equal(session.stop_reason(), result.stop_reason);
        assert.equal(session.frames_emitted(), frames.length);
        assert.equal(session.rounds_advanced(), result.rounds_advanced);
        for (let i = 0; i < 3; i++) assert.equal(session.next_frame(), null);
        const altered = session.initial_states();
        altered[0].hp = -999;
        assert.deepEqual(session.initial_states(), replay.initial_states);
      } finally {
        session.free();
      }
    }
  });
}

test("canonical error codes are plain objects", () => {
  for (const [raw, options, code] of [
    ["", {}, "INVALID_INPUT"],
    ["a\n\nb", { max_rounds: 0 }, "INVALID_ARGUMENT"],
    ["a\n\nb", { eval_rq: NaN }, "INVALID_ARGUMENT"],
  ]) {
    assert.throws(() => new wasm.BattleSession(raw, options), (error) => {
      assert.equal(error.code, code);
      assert.equal(typeof error.message, "string");
      assertPlain(error);
      return true;
    });
  }
});

test("generated TypeScript declares precise canonical DTOs and methods", () => {
  const declaration = readFileSync(packagePath.replace(/\.js$/, ".d.ts"), "utf8");
  for (const signature of [
    "initial_states(): BattlePlayerState[]",
    "next_frame(): BattleReplayFrame | null",
    "result(): BattleResult | null",
    "status(): BattleStatus",
    "is_failed(): boolean",
    "stop_reason(): BattleStopReason | null",
    "frame_index: number", "round_index: number",
    "export interface BattleResult", "export interface BattleReplayFrame",
  ]) assert.ok(declaration.includes(signature), signature);
});

test("legacy FightSession projects canonical frame semantics", () => {
  const raw = readFileSync(resolve("crates/tswn_test/cases/runtime_stress", fixtures[3]), "utf8");
  const canonical = wasm.battle_replay(raw);
  const legacy = new wasm.FightSession(raw);
  try {
    const frames = [...legacy.run_to_end(1).frames, ...legacy.run_to_end().frames];
    assert.equal(frames.length, canonical.frames.length);
    for (let i = 0; i < frames.length; i++) {
      const actual = frames[i];
      const expected = canonical.frames[i];
      assert.equal(actual.total_delay, expected.total_delay);
      assert.deepEqual(actual.winner_ids, expected.winner_ids);
      assert.equal(actual.finished, expected.finished);
      assert.deepEqual((actual.rows || []).map(row => row.clips.map(clip =>
        (clip.parts || []).map(part => [part.text, part.show_hp, part.hp_before, part.hp_after, part.death_effect]))),
      expected.rows.map(row => row.clips.map(clip =>
        clip.parts.map(part => [part.text, part.show_hp, part.hp_before, part.hp_after, part.death_effect]))));
    }
    assert.equal(legacy.is_done(), true);
    assert.deepEqual(legacy.winner_ids(), canonical.winner_ids);
    assert.deepEqual(legacy.step().updates, []);
    const summary = wasm.fight_summary(raw, { capture_replay: false });
    assert.deepEqual(summary.winner_ids, canonical.winner_ids);
  } finally {
    legacy.free();
  }
});
