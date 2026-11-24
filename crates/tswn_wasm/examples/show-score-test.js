import { escapeHtml } from "./show-utils.js";

export const SCORE_TEST_TOTAL = 10000;
export const SCORE_TEST_CHECKPOINTS = [1000, 10000];

/** !test! 是首行指令，不作为参战名字；普通输入不进入评分路径。
 *  空行分隔多组时改为胜率测试：各组轮流作为被测方，对其余各组算胜率。 */
export function parseScoreTestInput(raw) {
  const lines = raw.trim().replace(/\r\n?/g, "\n").split("\n");
  if (lines[0].trim() !== "!test!") return null;
  const body = lines.slice(1).join("\n").trim();
  // 单独的 ! 是强化指令，不作为被测名字；同名两行沿用引擎的单挑规则。
  const enhanced = body.split("\n")[0].trim() === "!";
  const input = (enhanced ? body.split("\n").slice(1).join("\n") : body).trim();
  if (!input || input.split("\n").some(line => ["!", "!test!"].includes(line.trim()) || /^seed:/i.test(line.trimStart()))) {
    throw new Error("!test! 后请输入名字；空行分隔多组时按组互相计算胜率。");
  }
  const groups = input.split(/\n\s*\n/).map(group => group.split("\n").map(line => line.trim()).filter(Boolean)).filter(group => group.length > 0);
  if (groups.length > 1) {
    return { input, enhanced, groups, label: (enhanced ? "强化" : "普通") + "胜率测试" };
  }
  const names = groups[0] ?? [];
  const duel = names.length === 2 && names[0] === names[1];
  return { input, enhanced, groups, label: (enhanced ? "强化" : "普通") + (duel ? "单挑" : "评分") };
}

/** 仅输出两个累计检查点；每次 step 的一场都由所选策略执行。 */
export async function runScoreTest(api, input, strategy, onCheckpoint, yieldControl = () => new Promise(resolve => setTimeout(resolve, 0))) {
  if (typeof api.ScoreTestSession !== "function") throw new Error("评分引擎版本过旧，请刷新页面。");
  // 兼容直接传名字的调用方；页面与 Worker 传递包含强化标记的解析结果。
  const options = typeof input === "string" ? { input, enhanced: false } : input;
  const session = new api.ScoreTestSession(options.input, strategy === "search", options.enhanced,
    strategy === "search" && options.legacyOpponents === true);
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
