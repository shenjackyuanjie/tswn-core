import { escapeHtml } from "./show-utils.js";

const SCORE_TEST_CHECKPOINTS = [1000, 10000];

export function parseScoreTestInput(raw) {
  const [header, ...body] = raw.trim().replace(/\r\n?/g, "\n").split("\n");
  if (header.trim() !== "!test!") return null;

  const lines = body.join("\n").trim().split("\n");
  const enhanced = lines[0].trim() === "!";
  const input = (enhanced ? lines.slice(1).join("\n") : lines.join("\n")).trim();
  const names = input.split("\n").map(line => line.trimEnd());
  if (names.some(name => !name || /^seed:/i.test(name.trimStart()) || ["!", "!test!"].includes(name.trim()))) {
    throw new Error("!test! 后请输入一组名字，不要用空行分队或指定 seed。");
  }

  const duel = names.length === 2 && names[0] === names[1];
  return { input, enhanced, label: (enhanced ? "强化" : "普通") + (duel ? "单挑" : "评分") };
}

export function runScoreTest(api, { input, enhanced }, onCheckpoint) {
  if (typeof api.ScoreTestSession !== "function") throw new Error("当前 WASM 不支持评分会话，请更新 WASM 或刷新页面。");
  const session = new api.ScoreTestSession(input, enhanced);
  const total = SCORE_TEST_CHECKPOINTS.at(-1);
  try {
    let completed = 0;
    for (const checkpoint of SCORE_TEST_CHECKPOINTS) {
      session.step(checkpoint - completed);
      const progress = session.progress();
      if (progress.completed !== checkpoint || progress.total !== total || progress.done !== (checkpoint === total)) {
        throw new Error("评分进度与预期不符，请更新 WASM 或刷新页面。");
      }
      onCheckpoint(progress);
      completed = checkpoint;
    }
  } finally {
    session.free();
  }
}

export function startScoreWorker(input, { onCheckpoint, onError }, createWorker = () =>
  new Worker(new URL("./show-score-worker.js", import.meta.url), { type: "module" })) {
  const worker = createWorker();
  let stopped = false;
  function stop() {
    if (stopped) return;
    stopped = true;
    worker.terminate();
  }
  function fail(message) {
    if (stopped) return;
    stop();
    onError(message);
  }
  worker.onmessage = ({ data }) => {
    if (stopped) return;
    if (data.type === "error") {
      fail(data.message);
    } else if (data.type === "checkpoint") {
      if (data.progress.done) stop();
      onCheckpoint(data.progress);
    }
  };
  worker.onerror = event => fail(event.message || "后台计算异常");
  worker.onmessageerror = () => fail("无法读取后台评分结果。");
  try {
    worker.postMessage(input);
  } catch (error) {
    stop();
    throw error;
  }
  return stop;
}

export function scoreCheckpointHtml(progress) {
  const percent = progress.completed * 100 / progress.total;
  const skills = [...progress.skills].sort((first, second) => second.count - first.count || first.message.localeCompare(second.message));
  const rows = skills.map(skill => {
    const message = escapeHtml(skill.message.replace(/\[[012]\]|\[s_[a-z0-9_]+\]/g, "").trim());
    return `<tr><td>${message}</td><td>${skill.count}</td><td>${skill.frequency.toFixed(2)}%</td></tr>`;
  }).join("");
  return `<section class="score-test-result">
    <h3>${percent}% · ${progress.completed.toLocaleString()} 场</h3>
    <p>实力评分：<strong>${progress.score.toFixed(2)}</strong> / 10000 · 胜 ${progress.wins.toLocaleString()} 场</p>
    ${progress.truncated ? `<p>回合上限截断：${progress.truncated} 场（计入总场数，不记胜）</p>` : ""}
    <table><thead><tr><th scope="col">技能／动作</th><th scope="col">累计次数</th><th scope="col">频率</th></tr></thead>
    <tbody>${rows || '<tr><td colspan="3">尚无技能／动作记录</td></tr>'}</tbody></table>
    <p>频率 = 被测角色本体的累计消息次数 ÷ 已完成场数 × 100%，可超过 100%。</p>
  </section>`;
}
