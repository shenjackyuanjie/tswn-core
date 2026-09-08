// 为 benchmark_py_battle_session.py 提供同机比较；从 stdin 读取 JSON 请求，将 JSON 结果写入 stdout。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";

const request = JSON.parse(readFileSync(0, "utf8"));
assert.ok(Number.isInteger(request.sessions) && request.sessions >= 100);
assert.ok(Number.isInteger(request.warmup) && request.warmup >= 1);
const wasm = createRequire(import.meta.url)(request.package);

function run(job) {
  const begin = performance.now();
  const session = new wasm.BattleSession(job.raw, job.options);
  const created = performance.now();
  try {
    const samples = [];
    let frames = 0;
    for (;;) {
      const start = performance.now();
      const frame = session.next_frame();
      samples.push(performance.now() - start);
      if (frame === null) break;
      frames++;
    }
    const end = performance.now();
    const result = session.result();
    assert.ok(session.is_done() && !session.is_failed());
    assert.ok(result.finished && result.stop_reason === "winner");
    assert.equal(frames, result.frames_emitted);
    return {
      session_create_ms: created - begin,
      next_frame_wall_ms: samples,
      total_session_ms: end - begin,
      frames,
      winner_ids: result.winner_ids,
      rounds_advanced: result.rounds_advanced,
    };
  } finally {
    session.free();
  }
}

const records = request.jobs.map(job => {
  for (let i = 0; i < request.warmup; i++) run(job);
  return { fixture: job.fixture, runs: Array.from({ length: request.sessions }, () => run(job)) };
});
process.stdout.write(JSON.stringify({ node: process.version, records }));
