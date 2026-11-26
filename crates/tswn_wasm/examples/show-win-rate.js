import { escapeHtml } from "./show-utils.js";

/** 每组对全体其它组的胜率测试场数。 */
export const WIN_RATE_ROUNDS = 2000;

/** 把分组按“被测组在前、其余组在后”重排：WinRateSession 统计第一组对其余组的胜率。 */
export function rotateGroups(groups, index) {
  const order = [groups[index], ...groups.filter((_, i) => i !== index)];
  return order.map(group => group.join("\n")).join("\n\n");
}

/** 逐组跑胜率；每组跑完回调一次，便于边算边显示。 */
export async function runGroupWinRates(api, groups, onProgress, yieldControl = () => new Promise(resolve => setTimeout(resolve, 0))) {
  if (typeof api.WinRateSession !== "function") throw new Error("胜率引擎版本过旧，请刷新页面。");
  const results = [];
  for (let index = 0; index < groups.length; index++) {
    const session = new api.WinRateSession(rotateGroups(groups, index), WIN_RATE_ROUNDS, { thread: 1 });
    try {
      let progress = session.progress();
      while (!progress.done) {
        progress = session.step(64);
        if (progress.done) break;
      }
      const result = session.result();
      results.push({
        label: groups[index].join(" / "),
        wins: result.wins,
        rounds: result.rounds_done,
        percent: result.percent,
      });
      onProgress(results.slice());
    } finally {
      session.free();
    }
    await yieldControl();
  }
  return results;
}

/** 胜率表格：队伍、胜场与百分比。 */
export function winRateTableHtml(results) {
  const rows = results
    .map(row => `<tr><td>${escapeHtml(row.label)}</td><td>${row.wins.toLocaleString()}</td><td>${row.rounds.toLocaleString()}</td><td>${row.percent.toFixed(2)}%</td></tr>`)
    .join("");
  return `<section class="score-test-result">
    <h3>胜率测试 · 每组对全体其余组 ${WIN_RATE_ROUNDS.toLocaleString()} 场</h3>
    <table><thead><tr><th scope="col">队伍</th><th scope="col">胜场</th><th scope="col">场数</th><th scope="col">胜率</th></tr></thead><tbody>${rows}</tbody></table>
    <p class="input-note">同名两行视为一个两人队；空行分隔各组，逐组作为被测方对其余全部组计算胜率。</p>
  </section>`;
}
