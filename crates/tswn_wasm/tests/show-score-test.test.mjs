import assert from "node:assert/strict";
import { test } from "node:test";

import { parseScoreTestInput, runScoreTest, scoreCheckpointHtml, startScoreWorker } from "../examples/show-score-test.js";

test("识别四种评分输入，不改变普通对局入口", () => {
  assert.equal(parseScoreTestInput("alpha\n\nbeta"), null);
  assert.equal(parseScoreTestInput("!test!alpha\nbeta"), null);
  assert.equal(parseScoreTestInput("alpha\n!test!"), null);
  for (const [body, enhanced, label] of [
    ["alpha", false, "普通评分"],
    ["alpha\nalpha", false, "普通单挑"],
    ["alpha", true, "强化评分"],
    ["alpha\nalpha", true, "强化单挑"],
    ["alpha\nbeta", false, "普通评分"],
  ]) {
    const input = `\uFEFF\r\n  !test! \r\n\r\n${enhanced ? "!\r\n\r\n" : ""}${body.replaceAll("\n", "\r\n")}\r\n`;
    assert.deepEqual(parseScoreTestInput(input), { input: body, enhanced, label });
  }
  for (const raw of ["!test!", "!test!\n!", "!test!\nalpha\n \nbeta", "!test!\n!test!\nalpha", "!test!\nalpha\nseed:1"]) {
    assert.throws(() => parseScoreTestInput(raw), /请输入一组名字/);
  }
});

test("同一会话累计推进两个检查点并释放", () => {
  const steps = [];
  const checkpoints = [];
  let freed = 0;
  class Session {
    completed = 0;
    constructor(input, enhanced) {
      assert.equal(input, "alpha\nalpha");
      assert.equal(enhanced, true);
    }
    step(count) {
      steps.push(count);
      this.completed += count;
    }
    progress() {
      return { completed: this.completed, total: 10000, done: this.completed === 10000 };
    }
    free() { freed++; }
  }
  runScoreTest({ ScoreTestSession: Session }, { input: "alpha\nalpha", enhanced: true }, progress => checkpoints.push(progress.completed));
  assert.deepEqual(steps, [1000, 9000]);
  assert.deepEqual(checkpoints, [1000, 10000]);
  assert.equal(freed, 1);
});

test("评分失败时仍释放 WASM 会话", () => {
  let freed = false;
  class FailingSession {
    step() { throw new Error("runtime failed"); }
    free() { freed = true; }
  }
  assert.throws(() => runScoreTest({ ScoreTestSession: FailingSession }, { input: "alpha" }, assert.fail), /runtime failed/);
  assert.equal(freed, true);
});

test("停止后丢弃旧 Worker 的迟到消息，完成或出错时自动终止", () => {
  const input = { input: "alpha", enhanced: false };
  for (const end of ["cancel", "done", "error"]) {
    const scores = [];
    const errors = [];
    let terminated = 0;
    const worker = {
      postMessage(value) { assert.deepEqual(value, input); },
      terminate() { terminated++; },
    };
    const stop = startScoreWorker(input, { onCheckpoint: progress => scores.push(progress), onError: message => errors.push(message) }, () => worker);
    const first = { completed: 1000, done: false };
    const final = { completed: 10000, done: true };
    worker.onmessage({ data: { type: "checkpoint", progress: first } });
    if (end === "cancel") stop();
    if (end === "done") worker.onmessage({ data: { type: "checkpoint", progress: final } });
    if (end === "error") worker.onerror({ message: "worker failed" });
    worker.onmessage({ data: { type: "checkpoint", progress: final } });
    worker.onerror({ message: "late error" });
    stop();
    assert.equal(terminated, 1);
    assert.deepEqual(scores, end === "done" ? [first, final] : [first]);
    assert.deepEqual(errors, end === "error" ? ["worker failed"] : []);
  }
});

test("检查点展示累计评分、转义的动作文本和每百局频率", () => {
  const html = scoreCheckpointHtml({
    completed: 1000, total: 10000, score: 2450, wins: 245, truncated: 1,
    skills: [
      { message: "[0]发起攻击", count: 1350, frequency: 135 },
      { message: "[0]<img>[s_fire]", count: 400, frequency: 40 },
    ],
  });
  assert.match(html, /10%/);
  assert.match(html, /2450\.00/);
  assert.match(html, /截断：1 场/);
  assert.match(html, /135\.00%/);
  assert.match(html, /&lt;img&gt;/);
  assert.doesNotMatch(html, /<img>|\[s_fire\]/);
});
