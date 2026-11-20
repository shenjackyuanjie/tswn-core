import test from "node:test";
import assert from "node:assert/strict";
import { parseScoreTestInput, runScoreTest, scoreCheckpointHtml } from "../examples/show-score-test.js";

test("test directive strips the command and preserves full names", () => {
  assert.equal(parseScoreTestInput("\n !test!\r\n\r\n名字@队伍\r\n"), "名字@队伍");
  assert.equal(parseScoreTestInput("alpha\nbeta"), null);
  assert.equal(parseScoreTestInput("!test!\nalpha\nbeta"), "alpha\nbeta");
  assert.throws(() => parseScoreTestInput("!test!"));
  assert.throws(() => parseScoreTestInput("!test!\nalpha\n\nbeta"));
});

test("chosen strategy is passed to every test session, only 10 and 100 percent are emitted", async () => {
  for (const strategy of ["search", "legacy"]) {
    let calls = 0, frees = 0;
    const api = { ScoreTestSession: class {
      constructor(input, search) { assert.equal(input, "alpha"); assert.equal(search, strategy === "search"); }
      step() { calls++; assert.ok(calls <= 10000); }
      progress() { return { total: 10000, completed: calls, done: calls === 10000 }; }
      free() { frees++; }
    } };
    const checkpoints = [];
    await runScoreTest(api, "alpha", strategy, p => checkpoints.push(p.completed));
    assert.deepEqual(checkpoints, [1000, 10000]);
    assert.equal(calls, 10000);
    assert.equal(frees, 1);
  }
});

test("a failed score test releases the session and never emits a final result", async () => {
  let freed = false;
  const api = { ScoreTestSession: class {
    progress() { return { total: 10000 }; }
    step() { throw new Error("battle failed"); }
    free() { freed = true; }
  } };
  await assert.rejects(runScoreTest(api, "alpha", "search", () => assert.fail()), /battle failed/);
  assert.ok(freed);
});

test("checkpoint rendering escapes skill messages and permits rates above 100 percent", () => {
  const html = scoreCheckpointHtml({ percent: 10, completed: 1000, score: 3200, wins: 320, truncated: 0,
    skills: [{ message: "[0]<script>使用[火球术]", count: 2500, frequency: 250 }] });
  assert.match(html, /10%/);
  assert.match(html, /3200\.00/);
  assert.match(html, /250\.00%/);
  assert.match(html, /&lt;script&gt;/);
  assert.doesNotMatch(html, /<script>/);
});
