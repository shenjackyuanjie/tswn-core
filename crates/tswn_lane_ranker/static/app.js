let currentResults = [];
let showConstrainedResults = false;

async function postJson(url, body) {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) {
    throw new Error(data.error || res.statusText);
  }
  return data;
}

function showJson(el, data) {
  el.textContent = JSON.stringify(data, null, 2);
}

function readWorkerSettings() {
  const outerRaw = document.getElementById("outerWorkersInput").value.trim();
  const skipArchived = document.getElementById("skipArchivedInput").checked;

  if (outerRaw && !/^\d+$/.test(outerRaw)) {
    throw new Error("线程数必须是 0 或正整数。0 表示动态分配。");
  }

  return {
    outer_workers: outerRaw ? Number(outerRaw) : 0,
    skip_archived: skipArchived,
  };
}

function readSelectionSettings() {
  const thresholdRaw = document.getElementById("selectionCqdThresholdInput").value.trim();
  const outerRaw = document.getElementById("outerWorkersInput").value.trim();

  if (outerRaw && !/^\d+$/.test(outerRaw)) {
    throw new Error("线程数必须是 0 或正整数。0 表示动态分配。");
  }

  if (thresholdRaw && !/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(thresholdRaw)) {
    throw new Error("环境阈值必须是数字，例如 48.5。");
  }

  const cqdThreshold = thresholdRaw ? Number(thresholdRaw) : 48.5;
  if (!Number.isFinite(cqdThreshold) || cqdThreshold < 0 || cqdThreshold > 100) {
    throw new Error("环境阈值必须在 0 到 100 之间。");
  }

  return {
    outer_workers: outerRaw ? Number(outerRaw) : 0,
    cqd_threshold: cqdThreshold,
  };
}

document.getElementById("addBtn").addEventListener("click", async () => {
  const out = document.getElementById("addOutput");
  try {
    out.textContent = "queueing...";
    const groups = document.getElementById("groupsInput").value
      .split(/\r?\n/)
      .map(x => x.trim())
      .filter(Boolean);
    const workerSettings = readWorkerSettings();
    const data = await postJson("/api/groups/add", { groups, ...workerSettings });
    showJson(out, data);
    await loadLanes();
  } catch (err) {
    out.textContent = String(err);
  }
});

async function submitBlockGroups(blocked) {
  const out = document.getElementById("blockGroupsOutput");
  try {
    out.textContent = "queueing...";
    const groups = document.getElementById("blockGroupsInput").value
      .split(/\r?\n/)
      .map(x => x.trim())
      .filter(Boolean);

    if (!groups.length) {
      out.textContent = "请先输入至少一个组合。";
      return;
    }

    const workerSettings = readWorkerSettings();
    const url = blocked ? "/api/groups/block" : "/api/groups/unblock";
    const data = await postJson(url, { groups, ...workerSettings });
    showJson(out, data);
    await loadLanes();
    await loadResults();
  } catch (err) {
    out.textContent = String(err);
  }
}

document.getElementById("blockGroupsBtn").addEventListener("click", () => submitBlockGroups(true));
document.getElementById("unblockGroupsBtn").addEventListener("click", () => submitBlockGroups(false));

document.getElementById("mergeBtn").addEventListener("click", async () => {
  const out = document.getElementById("mergeOutput");
  try {
    out.textContent = "queueing...";
    const x = document.getElementById("teamX").value.trim();
    const y = document.getElementById("teamY").value.trim();
    const workerSettings = readWorkerSettings();
    const data = await postJson("/api/teams/merge", { x, y, ...workerSettings });
    showJson(out, data);
    await loadLanes();
  } catch (err) {
    out.textContent = String(err);
  }
});

document.getElementById("refreshLanesBtn").addEventListener("click", loadLanes);
document.getElementById("loadResultsBtn").addEventListener("click", loadResults);
document.getElementById("exportResultsBtn").addEventListener("click", exportResults);
document.getElementById("purgeLowScoreBtn").addEventListener("click", purgeLowScoreGroups);
document.getElementById("runConstrainedSelectionBtn").addEventListener("click", runConstrainedSelection);
document.getElementById("exportConstrainedResultsBtn").addEventListener("click", exportConstrainedResults);
document.getElementById("showConstrainedResultsInput").addEventListener("change", event => {
  showConstrainedResults = event.target.checked;
  renderResultsTable();
});

async function loadLanes() {
  const el = document.getElementById("lanes");
  const previousStickiness = {};
  el.querySelectorAll(".stickiness-input").forEach(input => {
    previousStickiness[input.id] = input.value;
  });

  const res = await fetch("/api/lanes");
  const lanes = await res.json();
  if (!lanes.length) {
    el.textContent = "暂无赛道";
    return;
  }

  el.innerHTML = lanes.map(lane => {
    const p = lane.progress;
    const progressText = p
      ? `<div class="progress">
           <strong>${escapeHtml(p.phase)}</strong>
           ${p.total_rounds ? ` round ${p.round}/${p.total_rounds}` : ""}
           ${p.rate_total ? ` rates ${p.rate_done}/${p.rate_total}` : ""}
           ${p.kicked_count ? ` changed ${p.kicked_count}` : ""}
           <br><span>${escapeHtml(p.message || "")}</span>
         </div>`
      : "";

    const stickinessId = `stickiness-${lane.lane_size}`;
    const stickinessValue = previousStickiness[stickinessId] || "";

    return `<div class="lane-card">
      <div class="lane-actions">
        <button onclick="selectLane(${lane.lane_size})">选择 ${lane.lane_size} 人赛道</button>
        <button onclick="recomputeLane(${lane.lane_size})">重算</button>
        <label class="stickiness-label">粘性
          <input
            id="${stickinessId}"
            class="stickiness-input"
            type="number"
            min="1"
            step="1"
            placeholder="默认 ${lane.lane_size * 10}"
            value="${escapeHtml(stickinessValue)}"
          />
        </label>
        <span class="badge ${escapeHtml(lane.status)}">${escapeHtml(lane.status)}</span>
        <span>groups: ${lane.group_count}</span>
      </div>
      ${progressText}
    </div>`;
  }).join("");
}

window.recomputeLane = async function(size) {
  const input = document.getElementById(`stickiness-${size}`);
  const raw = input ? input.value.trim() : "";
  const body = {};

  if (raw) {
    if (!/^[1-9]\d*$/.test(raw)) {
      alert("粘性必须是正整数；留空则使用默认值。");
      return;
    }
    body.stickiness = Number(raw);
  }

  try {
    const workerSettings = readWorkerSettings();
    await postJson(`/api/lanes/${size}/recompute`, { ...body, ...workerSettings });
    await loadLanes();
  } catch (err) {
    alert(String(err));
  }
};

window.selectLane = function(size) {
  document.getElementById("laneSize").value = size;
  loadResults();
};

async function loadResults() {
  const el = document.getElementById("results");
  const laneSize = document.getElementById("laneSize").value;
  const res = await fetch(`/api/lanes/${laneSize}/results`);
  const rows = await res.json();
  currentResults = rows;

  if (!rows.length) {
    el.textContent = "waiting";
    return;
  }

  renderResultsTable();
}

function renderResultsTable() {
  const el = document.getElementById("results");
  if (!currentResults.length) {
    el.textContent = "waiting";
    return;
  }

  const rows = showConstrainedResults
    ? constrainedPresentationRows(currentResults)
    : rawPresentationRows(currentResults);
  const groups = buildFoldedResultGroups(rows);
  const constrainedHeader = showConstrainedResults
    ? `<th class="constrained-rank-col">P-Rank</th><th class="score-col">P-Score</th><th class="raw-rank-col">R-Rank</th><th class="raw-col">R-Score</th><th class="delta-col">Δ</th>`
    : `<th class="score-col">Score</th>`;

  // 高级结果 UI：
  // 从上往下扫描，每扫到一个没有被折叠的组，将下面所有与它有重复 member 的组折叠到它下面。
  // 默认折叠；点击主行展开/收起。
  el.innerHTML = `
    <table class="score-table folded-score-table ${showConstrainedResults ? "constrained-view" : "raw-view"}">
      <thead>
        <tr>
          ${showConstrainedResults ? "" : "<th>Rank</th>"}
          ${constrainedHeader}
          <th class="type-col">Type</th>
          <th class="name-col">Name</th>
        </tr>
      </thead>
      <tbody>
        ${groups.map((group, groupIndex) => renderFoldedGroup(group, groupIndex)).join("")}
      </tbody>
    </table>
  `;

  document.querySelectorAll(".fold-toggle.has-children").forEach(row => {
    row.addEventListener("click", () => {
      if (!row.classList.contains("has-children")) {
        return;
      }

      const groupIndex = row.dataset.groupIndex;
      const expanded = row.dataset.expanded === "true";
      const nextExpanded = !expanded;

      row.dataset.expanded = String(nextExpanded);
      row.classList.toggle("expanded", nextExpanded);

      const icon = row.querySelector(".fold-icon");
      if (icon) {
        icon.textContent = nextExpanded ? "▾" : "▸";
      }

      document
        .querySelectorAll(`.fold-child[data-parent-index="${groupIndex}"]`)
        .forEach(child => {
          child.hidden = !nextExpanded;
        });
    });
  });
}

function rawPresentationRows(rows) {
  return [...rows].sort((a, b) => {
    const scoreDiff = rawScoreOf(b) - rawScoreOf(a);
    if (Number.isFinite(scoreDiff) && scoreDiff !== 0) return scoreDiff;
    return String(a.canonical || "").localeCompare(String(b.canonical || ""));
  }).map((row, idx) => ({ ...row, true_raw_rank: idx + 1 }));
}

function rawScoreOf(row) {
  const raw = Number(row && row.raw_average_cqd);
  if (Number.isFinite(raw) && raw !== 0) return raw;
  const avg = Number(row && row.average_cqd);
  return Number.isFinite(avg) ? avg : Number.NEGATIVE_INFINITY;
}

function trueRawRankMap(rows) {
  const map = new Map();
  rawPresentationRows(rows).forEach((row, idx) => {
    map.set(row.group_id, idx + 1);
  });
  return map;
}

function trueRawRankOf(row) {
  if (row && Number.isFinite(Number(row.true_raw_rank))) {
    return Number(row.true_raw_rank);
  }
  if (row && Number.isFinite(Number(row.raw_rank))) {
    return Number(row.raw_rank);
  }
  return Number(row && row.rank) || "—";
}

function constrainedPresentationRows(rows) {
  const rawRanks = trueRawRankMap(rows);
  return [...rows].map(row => ({
    ...row,
    true_raw_rank: rawRanks.get(row.group_id) || row.rank,
  })).sort((a, b) => {
    const aHasPair = Number.isFinite(Number(a.pair_score));
    const bHasPair = Number.isFinite(Number(b.pair_score));
    if (aHasPair !== bHasPair) return aHasPair ? -1 : 1;

    const ar = Number(a.pair_rank ?? Number.POSITIVE_INFINITY);
    const br = Number(b.pair_rank ?? Number.POSITIVE_INFINITY);
    if (Number.isFinite(ar) && Number.isFinite(br) && ar !== br) return ar - br;

    const pairDiff = Number(b.pair_score ?? -Infinity) - Number(a.pair_score ?? -Infinity);
    if (Number.isFinite(pairDiff) && pairDiff !== 0) return pairDiff;

    return Number(trueRawRankOf(a) || 0) - Number(trueRawRankOf(b) || 0);
  });
}

function buildFoldedResultGroups(rows) {
  const consumed = new Array(rows.length).fill(false);
  const pending = new Map();
  const parsed = rows.map(row => ({
    row,
    members: parseGroupMembers(row.canonical),
  }));

  const groups = [];

  for (let i = 0; i < rows.length; i++) {
    if (consumed[i]) {
      continue;
    }

    const parent = parsed[i];

    // 新折叠规则：如果一个被屏蔽组合下面有未屏蔽且成员重复的组合，
    // 它不再当父行，而是折叠到那个更低的未屏蔽组合下。
    if (isBlockedRow(parent.row)) {
      const targetIndex = findLowerUnblockedOverlap(parsed, consumed, i, parent.members);
      if (targetIndex >= 0) {
        addPendingChild(pending, targetIndex, parent.row);
        consumed[i] = true;
        continue;
      }
    }

    consumed[i] = true;
    const children = takePendingChildren(pending, i);

    for (let j = i + 1; j < rows.length; j++) {
      if (consumed[j]) {
        continue;
      }

      const child = parsed[j];
      if (!hasMemberOverlap(parent.members, child.members)) {
        continue;
      }

      if (isBlockedRow(child.row)) {
        const targetIndex = findLowerUnblockedOverlap(parsed, consumed, j, child.members);
        if (targetIndex >= 0) {
          addPendingChild(pending, targetIndex, child.row);
          consumed[j] = true;
          continue;
        }
      }

      consumed[j] = true;
      children.push(child.row);
      children.push(...takePendingChildren(pending, j));
    }

    groups.push({
      parent: parent.row,
      children,
    });
  }

  return groups;
}

function addPendingChild(pending, targetIndex, row) {
  if (!pending.has(targetIndex)) {
    pending.set(targetIndex, []);
  }
  pending.get(targetIndex).push(row);
}

function takePendingChildren(pending, targetIndex) {
  const children = pending.get(targetIndex) || [];
  pending.delete(targetIndex);
  return children;
}

function findLowerUnblockedOverlap(parsed, consumed, startIndex, members) {
  for (let i = startIndex + 1; i < parsed.length; i++) {
    if (consumed[i] || isBlockedRow(parsed[i].row)) {
      continue;
    }
    if (hasMemberOverlap(members, parsed[i].members)) {
      return i;
    }
  }
  return -1;
}

function isBlockedRow(row) {
  return Boolean(row && row.is_blocked);
}

function parseGroupMembers(canonical) {
  return String(canonical)
    .split("+")
    .map(x => x.trim())
    .filter(Boolean);
}

function hasMemberOverlap(a, b) {
  if (!a.length || !b.length) {
    return false;
  }

  const seen = new Set(a);
  return b.some(member => seen.has(member));
}

function renderFoldedGroup(group, groupIndex) {
  const childCount = group.children.length;
  const parent = group.parent;
  const clickableClass = childCount ? "fold-toggle has-children" : "fold-row";
  const icon = childCount ? "▸" : "";
  const childBadge = childCount ? `<span class="fold-count">+${childCount}</span>` : "";
  const parentCells = showConstrainedResults
    ? `
      <td class="constrained-rank-cell">${parent.pair_rank ?? "—"}</td>
      <td class="score-cell">${formatScore(parent.pair_score)}</td>
      <td class="raw-rank-cell">${trueRawRankOf(parent)}</td>
      <td class="raw-cell">${formatScore(rawScoreOf(parent))}</td>
      <td class="delta-cell">${formatSigned(parent.raw_delta)}</td>`
    : `<td>${trueRawRankOf(parent)}</td><td class="score-cell">${formatScore(rawScoreOf(parent))}</td>`;

  const parentRow = `
    <tr class="${clickableClass} ${rowClass(parent)}" data-group-index="${groupIndex}" data-expanded="false" title="${escapeHtml(rowTooltip(parent))}">
      ${parentCells}
      <td class="type-cell">${escapeHtml(parent.type_label || "无")}</td>
      <td class="name-cell">
        <span class="fold-icon">${icon}</span>
        ${escapeHtml(parent.canonical)}
        ${statusBadge(parent)}
        ${childBadge}
      </td>
    </tr>
  `;

  const childRows = group.children.map(child => {
    const childCells = showConstrainedResults
      ? `
        <td class="constrained-rank-cell">${child.pair_rank ?? "—"}</td>
        <td class="score-cell">${formatScore(child.pair_score)}</td>
        <td class="raw-rank-cell">${trueRawRankOf(child)}</td>
        <td class="raw-cell">${formatScore(rawScoreOf(child))}</td>
        <td class="delta-cell">${formatSigned(child.raw_delta)}</td>`
      : `<td>${trueRawRankOf(child)}</td><td class="score-cell">${formatScore(rawScoreOf(child))}</td>`;

    return `
      <tr class="fold-child ${rowClass(child)}" data-parent-index="${groupIndex}" hidden title="${escapeHtml(rowTooltip(child))}">
        ${childCells}
        <td class="type-cell">${escapeHtml(child.type_label || "无")}</td>
        <td class="name-cell child-name">
          <span class="fold-child-marker">↳</span>
          ${escapeHtml(child.canonical)}
          ${statusBadge(child)}
        </td>
      </tr>
    `;
  }).join("");

  return parentRow + childRows;
}

function formatScore(value) {
  const n = Number(value);
  return Number.isFinite(n) ? n.toFixed(3) : "—";
}

function formatSigned(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) {
    return "—";
  }
  return `${n >= 0 ? "+" : ""}${n.toFixed(3)}`;
}

function rowClass(row) {
  const classes = [];
  if (isBlockedRow(row)) {
    classes.push("blocked-row");
  }
  return classes.join(" ");
}

function statusBadge(row) {
  if (!row) {
    return "";
  }
  if (isBlockedRow(row)) {
    return `<span class="blocked-badge">blocked</span>`;
  }
  if (showConstrainedResults && row.selection_status === "below_threshold") {
    return `<span class="threshold-badge">below threshold</span>`;
  }
  return "";
}

function rowTooltip(row) {
  if (!row) {
    return "";
  }
  const parts = [
    `status=${row.selection_status || "unknown"}`,
    `raw=${formatScore(rawScoreOf(row))}`,
  ];
  if (row.pair_score != null) parts.push(`pair=${formatScore(row.pair_score)}`);
  if (row.pair_rank != null) parts.push(`pair_rank=${row.pair_rank}`);
  if (row.raw_delta != null) parts.push(`delta=${formatSigned(row.raw_delta)}`);
  if (row.pair_score_std != null) parts.push(`score_std=${formatScore(row.pair_score_std)}`);
  if (row.pair_rank_std != null) parts.push(`rank_std=${formatScore(row.pair_rank_std)}`);
  if (row.delta_std != null) parts.push(`delta_std=${formatScore(row.delta_std)}`);
  if (row.edge_count_mean != null) parts.push(`edge_mean=${formatScore(row.edge_count_mean)}`);
  if (row.stability_flag) parts.push(`stability=${row.stability_flag}`);
  return parts.join(" | ");
}

function effectiveExportScore(row) {
  const raw = rawScoreOf(row);
  return Number.isFinite(raw) ? raw : 0;
}

function exportResults() {
  const laneSize = document.getElementById("laneSize").value;

  if (!currentResults.length) {
    alert("当前没有可导出的结果。请先点击“读取结果”。");
    return;
  }

  // Raw 导出保持旧格式：只按默认 Score/CQD 输出，不混入约束选号结果。
  const rawRows = rawPresentationRows(currentResults);
  const resultLines = rawRows
    .map(row => `${effectiveExportScore(row).toFixed(3)} ${row.canonical}`);

  const content = resultLines.join("\n");

  downloadText(`lane_${laneSize}_score.txt`, content);
}

function exportConstrainedResults() {
  const laneSize = document.getElementById("laneSize").value;

  if (!currentResults.length) {
    alert("当前没有可导出的结果。请先点击“读取结果”。");
    return;
  }

  const allRows = constrainedPresentationRows(currentResults);
  const calibratedRows = allRows.filter(row => row.pair_score != null || row.pair_rank != null);

  if (!calibratedRows.length) {
    alert("当前没有修正结果。请先点击“执行修正”，等任务完成后再读取结果。");
    return;
  }

  const foldedGroups = buildFoldedResultGroups(calibratedRows);
  const envThreshold = currentEnvironmentThreshold();
  const pairSkillTotalLines = buildSkillEquivalentSummary(foldedGroups, envThreshold, row => row.pair_score);

  const content = [
    "P-Rank	P-Score	R-Rank	R-Score	Δ	Type	Name	pair_score_std	delta_std	pair_rank_std	edge_count_mean	stability_flag	uncertainty	status",
    ...calibratedRows.map(row => [
      row.pair_rank ?? "",
      formatScore(row.pair_score),
      trueRawRankOf(row),
      formatScore(rawScoreOf(row)),
      formatSigned(row.raw_delta),
      row.type_label || "无",
      row.canonical,
      formatScore(row.pair_score_std),
      formatScore(row.delta_std),
      formatScore(row.pair_rank_std),
      formatScore(row.edge_count_mean),
      row.stability_flag || "",
      formatScore(row.uncertainty),
      row.selection_status || "",
    ].join("\t")),
    "",
    "# Full Raw/Calibration Table",
    "P-Rank	P-Score	R-Rank	R-Score	Δ	Type	Name	pair_score_std	delta_std	pair_rank_std	edge_count_mean	stability_flag	uncertainty	status",
    ...allRows.map(row => [
      row.pair_rank ?? "",
      formatScore(row.pair_score),
      trueRawRankOf(row),
      formatScore(rawScoreOf(row)),
      formatSigned(row.raw_delta),
      row.type_label || "无",
      row.canonical,
      formatScore(row.pair_score_std),
      formatScore(row.delta_std),
      formatScore(row.pair_rank_std),
      formatScore(row.edge_count_mean),
      row.stability_flag || "",
      formatScore(row.uncertainty),
      row.selection_status || "",
    ].join("\t")),
    "",
    `# Skill Equivalent Sum for unfolded, unblocked groups with pair_score >= ${formatThreshold(envThreshold)}`,
    ...pairSkillTotalLines,
  ].join("\n");

  downloadText(`lane_${laneSize}_stability.txt`, content);
}


function downloadText(filename, content) {
  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");

  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();

  URL.revokeObjectURL(url);
}

async function purgeLowScoreGroups() {
  const laneSize = document.getElementById("laneSize").value;
  if (!laneSize) {
    alert("请先选择赛道。");
    return;
  }

  if (!confirm("将从数据库物理删除该赛道所有 R-Score < 45 的组合；45.000 不删。这个操作不是封存，不能直接撤销。继续吗？")) {
    return;
  }

  try {
    const workerSettings = readWorkerSettings();
    const data = await postJson(`/api/lanes/${laneSize}/purge-low-score`, workerSettings);
    alert(`已物理删除 ${data.deleted_count} 个 R-Score < ${formatThreshold(data.threshold)} 的组合。${data.queued_lanes?.length ? "已自动提交重算。" : "没有需要删除的组合。"}`);
    await loadLanes();
    await loadResults();
  } catch (err) {
    alert(String(err));
  }
}

async function runConstrainedSelection() {
  const out = document.getElementById("constrainedSelectionOutput");
  const laneSize = document.getElementById("laneSize").value;
  try {
    const settings = readSelectionSettings();
    out.textContent = "queueing edge-bagging stability...";
    const data = await postJson(`/api/lanes/${laneSize}/constrained-selection`, settings);
    out.textContent = "已提交修正任务。";
    document.getElementById("showConstrainedResultsInput").checked = true;
    showConstrainedResults = true;
    await loadLanes();
  } catch (err) {
    out.textContent = String(err);
  }
}


function buildTeamOddsSummary(rows) {
  const teamOdds = new Map();

  for (const row of rows) {
    const odds = Number(row.golden_rate || 0);
    if (!Number.isFinite(odds) || odds <= 0) {
      continue;
    }

    const team = String(row.root_team_name || row.team_name || extractTeamName(row.canonical) || "").trim();
    if (!team) {
      continue;
    }

    teamOdds.set(team, (teamOdds.get(team) || 0) + odds);
  }

  return Array.from(teamOdds.entries())
    .filter(([, odds]) => odds > 0)
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([team, odds]) => `${odds.toFixed(3)} ${team}`);
}

function skillEquivalentThresholdForLane(laneSize) {
  return Number(laneSize) === 1 ? 48.0 : 48.5;
}

function currentEnvironmentThreshold() {
  const raw = document.getElementById("selectionCqdThresholdInput")?.value?.trim();
  const value = raw ? Number(raw) : 48.5;
  return Number.isFinite(value) ? value : 48.5;
}

function formatThreshold(value) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function buildSkillEquivalentSummary(foldedGroups, threshold, scoreSelector = row => row.average_cqd) {
  const totals = new Map();

  for (const group of foldedGroups) {
    const row = group.parent;
    if (!row || isBlockedRow(row) || Number(scoreSelector(row) || 0) < threshold) {
      continue;
    }

    for (const item of row.skill_totals || []) {
      const name = String(item.name || "").trim();
      const value = Number(item.value || 0);
      if (!name || !Number.isFinite(value)) {
        continue;
      }
      totals.set(name, (totals.get(name) || 0) + value);
    }
  }

  return Array.from(totals.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([name, value]) => `${value.toFixed(2)} ${name}`);
}

function extractTeamName(canonical) {
  const firstMember = String(canonical || "").split("+")[0] || "";
  const at = firstMember.lastIndexOf("@");
  if (at < 0 || at === firstMember.length - 1) {
    return "";
  }
  return firstMember.slice(at + 1);
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

loadLanes();
setInterval(loadLanes, 2000);
