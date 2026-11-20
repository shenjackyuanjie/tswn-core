import { escapeHtml } from "./show-utils.js";

export const SCORE_TEST_TOTAL = 10000;
export const SCORE_TEST_CHECKPOINTS = [1000, 10000];

/** !test! 是首行指令，不作为参战名字；普通输入不进入评分路径。 */
export function parseScoreTestInput(raw) {
  const lines = raw.trim().replace(/\r\n?/g, "\n").split("\n");
  if (lines[0].trim() !== "!test!") return null;
  const input = lines.slice(1).join("\n").trim();
  if (!input || input.split(/\n\s*\n/).length !== 1 || input.split("\n").some(line => line.trim() === "!test!")) {
    throw new Error("!test! 后请输入一个名字或同一队伍的名字，名字之间不要用空行分队。");
  }
  return input;
}

/** 仅输出两个累计检查点；每次 step 的一场都由所选策略执行。 */
export async function runScoreTest(api, input, strategy, onCheckpoint, yieldControl = () => new Promise(resolve => setTimeout(resolve, 0))) {
  if (typeof api.ScoreTestSession !== "function") throw new Error("评分引擎版本过旧，请刷新页面。");
  const session = new api.ScoreTestSession(input, strategy === "search");
  try {
    if (session.progress().total !== SCORE_TEST_TOTAL) throw new Error("评分场数不匹配，请刷新页面。");
    let yieldedAt = performance.now();
    for (let completed = 1; completed <= SCORE_TEST_TOTAL; completed++) {
      session.step();
      if (SCORE_TEST_CHECKPOINTS.includes(completed)) {
        const progress = session.progress();
        if (progress.completed !== completed || progress.done !== (completed === SCORE_TEST_TOTAL)) throw new Error("评分进度异常");
        onCheckpoint(progress);
      }
      if (performance.now() - yieldedAt > 30) {
        await yieldControl();
        yieldedAt = performance.now();
      }
    }
  } finally {
    session.free();
  }
}

export function scoreCheckpointHtml(progress) {
  const skills = [...progress.skills].sort((a, b) => b.count - a.count || a.message.localeCompare(b.message));
  const rows = skills.map(skill => `<tr><td>${escapeHtml(skill.message.replace(/\[[012]\]|\[s_[a-z0-9_]+\]/g, "").trim())}</td><td>${skill.count}</td><td>${skill.frequency.toFixed(2)}%</td></tr>`).join("");
  return `<section class="score-test-result">
    <h3>${progress.percent}% · ${progress.completed.toLocaleString()} 场</h3>
    <p>实力评分：<strong>${progress.score.toFixed(2)}</strong> / 10000 · 胜 ${progress.wins.toLocaleString()} 场</p>
    ${progress.truncated ? `<p>回合上限截断：${progress.truncated} 场（计入总场数，不记胜场）</p>` : ""}
    <table><thead><tr><th scope="col">技能／动作</th><th scope="col">累计次数</th><th scope="col">频率</th></tr></thead><tbody>${rows || '<tr><td colspan="3">尚无技能／动作记录</td></tr>'}</tbody></table>
    <p class="input-note">频率 = 该名字本体的真实技能／动作消息次数 ÷ 已完成场数 × 100%；每百局可发动多次，因此可能超过 100%。搜索试算不计入。</p>
  </section>`;
}
