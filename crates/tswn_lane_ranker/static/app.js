let currentResults = [];
let currentTargets = null;
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

function defaultCalibrationThresholdForLane(laneSize) {
  return Number(laneSize) === 1 ? 48 : 48.7;
}

function syncDefaultCalibrationThreshold(force = false) {
  const laneSize = Number(document.getElementById("laneSize").value || 1);
  const input = document.getElementById("selectionCqdThresholdInput");
  const next = defaultCalibrationThresholdForLane(laneSize).toFixed(1);
  if (force || !input.value.trim()) {
    input.value = next;
  }
  input.placeholder = next;
}

function readSelectionSettings() {
  const laneSize = Number(document.getElementById("laneSize").value || 1);
  const thresholdRaw = document.getElementById("selectionCqdThresholdInput").value.trim();
  const outerRaw = document.getElementById("outerWorkersInput").value.trim();

  if (outerRaw && !/^\d+$/.test(outerRaw)) {
    throw new Error("线程数必须是 0 或正整数。0 表示动态分配。");
  }

  if (thresholdRaw && !/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(thresholdRaw)) {
    throw new Error("校准池 Raw Score 阈值必须是数字，例如 48.2。");
  }

  const rawScoreThreshold = thresholdRaw ? Number(thresholdRaw) : defaultCalibrationThresholdForLane(laneSize);
  if (!Number.isFinite(rawScoreThreshold) || rawScoreThreshold < 0 || rawScoreThreshold > 100) {
    throw new Error("校准池 Raw Score 阈值必须在 0 到 100 之间。");
  }

  return {
    outer_workers: outerRaw ? Number(outerRaw) : 0,
    raw_score_threshold: rawScoreThreshold,
    // 与旧版后端构建兼容。新的严格 Python 校准使用 raw_score_threshold，并将相同值传给 --raw-min。
    cqd_threshold: rawScoreThreshold,
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


document.getElementById("addWinratesBtn").addEventListener("click", async () => {
  const out = document.getElementById("addWinratesOutput");
  try {
    out.textContent = "computing manual winrates...";
    const groups = document.getElementById("manualWinratesInput").value
      .split(/\r?\n/)
      .map(x => x.trim())
      .filter(Boolean);

    if (groups.length < 2) {
      out.textContent = "请至少输入两行组合。每两行组成一组对战。";
      return;
    }

    const workerSettings = readWorkerSettings();
    const body = { groups, ...workerSettings };

    const data = await postJson("/api/winrates/add", body);
    showJson(out, data);
    await loadLanes();
  } catch (err) {
    out.textContent = String(err);
  }
});

document.getElementById("refreshLanesBtn").addEventListener("click", loadLanes);
document.getElementById("loadResultsBtn").addEventListener("click", loadResults);
document.getElementById("exportResultsBtn").addEventListener("click", exportResults);
document.getElementById("runConstrainedSelectionBtn").addEventListener("click", runConstrainedSelection);
document.getElementById("exportConstrainedResultsBtn").addEventListener("click", exportConstrainedResults);
document.getElementById("generateTargetsBtn").addEventListener("click", generateTargets);
document.getElementById("exportTargetsBtn").addEventListener("click", exportTargets);
document.getElementById("showConstrainedResultsInput").addEventListener("change", event => {
  showConstrainedResults = event.target.checked;
  renderResultsTable();
});
document.getElementById("laneSize").addEventListener("change", () => {
  syncDefaultCalibrationThreshold(true);
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

  const selectedLaneSize = Number(document.getElementById("laneSize").value || 1);
  const selectedLane = lanes.find(lane => Number(lane.lane_size) === selectedLaneSize);
  const selectedProgress = selectedLane && selectedLane.progress;
  if (selectedProgress && String(selectedProgress.phase || "").startsWith("target_")
      && selectedProgress.phase !== "target_generation_ready") {
    const targetOut = document.getElementById("targetGenerationOutput");
    const done = Number(selectedProgress.rate_done || 0);
    const total = Number(selectedProgress.rate_total || 0);
    const message = String(selectedProgress.message || "");
    if (targetOut) {
      targetOut.textContent = `生成靶子中：${selectedProgress.phase} ${done}/${total}${message ? " — " + message : ""}`;
    }
  }

  el.innerHTML = lanes.map(lane => {
    const p = lane.progress;
    const progressText = p
      ? `<div class="progress">
           <strong>${escapeHtml(p.phase)}</strong>
           ${p.total_rounds ? ` round ${p.round}/${p.total_rounds}` : ""}
           ${p.rate_total ? ` rates ${p.rate_done}/${p.rate_total}` : ""}
           ${p.kicked_count ? ` archived ${p.kicked_count}` : ""}
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
  syncDefaultCalibrationThreshold(true);
  loadResults();
};

async function loadResults() {
  const el = document.getElementById("results");
  syncDefaultCalibrationThreshold(false);
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

  if (showConstrainedResults && !rows.some(hasSelectionWeightCqd)) {
    el.innerHTML = `
      <p class="empty-pair-calibration">
        当前没有校准结果。请点击“执行校准”，等状态显示完成后会自动刷新。
      </p>
    `;
    return;
  }


  const groups = buildFoldedResultGroups(rows);
  const tableHeader = showConstrainedResults
    ? `
          <th class="constrained-rank-col">C-Rank</th>
          <th class="score-col">C-Score</th>
          <th class="raw-rank-col">R-Rank</th>
          <th class="raw-col">R-Score</th>
          <th class="delta-rank-col">Δ</th>
          <th class="text-type-col">Type</th>
          <th class="name-col"><span class="fold-icon"></span>Name</th>`
    : `
          <th>R-Rank</th>
          <th class="score-col">R-Score</th>
          <th class="text-type-col">Type</th>
          <th class="name-col"><span class="fold-icon"></span>Name</th>`;

  // 高级结果 UI：
  // 从上往下扫描，每扫到一个没有被折叠的组，将下面所有与它有重复 member 的组折叠到它下面。
  // 默认折叠；点击主行展开/收起。
  el.innerHTML = `
    <table class="score-table folded-score-table ${showConstrainedResults ? "constrained-view" : "raw-view"}">
      <thead>
        <tr>
          ${tableHeader}
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
  return [...rows].sort((a, b) => Number(a.rank || 0) - Number(b.rank || 0));
}

function isConstrainedVisibleRow(row) {
  return row
    && row.selection_status !== "below_threshold"
    && hasSelectionWeightCqd(row);
}

function constrainedPresentationRows(rows) {
  return [...rows]
    .filter(isConstrainedVisibleRow)
    .sort((a, b) => {
      const aPairRank = hasFiniteNumber(selectionWeightRank(a)) ? selectionWeightRank(a) : Infinity;
      const bPairRank = hasFiniteNumber(selectionWeightRank(b)) ? selectionWeightRank(b) : Infinity;
      if (aPairRank !== bPairRank) {
        return aPairRank - bPairRank;
      }

      const pairDiff = Number(selectionWeightCqd(b)) - Number(selectionWeightCqd(a));
      if (Number.isFinite(pairDiff) && pairDiff !== 0) return pairDiff;

      return Number(a.rank || 0) - Number(b.rank || 0);
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
  const parentRankCell = showConstrainedResults ? "" : `<td>${parent.rank}</td>`;
  const parentCells = showConstrainedResults
    ? `
      <td class="constrained-rank-cell">${selectionWeightRank(parent) ?? "—"}</td>
      <td class="score-cell">${formatScore(selectionWeightCqd(parent))}</td>
      <td class="raw-rank-cell">${parent.rank ?? "—"}</td>
      <td class="raw-cell">${formatScore(parent.raw_average_cqd ?? parent.average_cqd)}</td>
      <td class="delta-rank-cell">${formatDeltaRank(parent)}</td>`
    : `<td class="score-cell">${formatScore(parent.raw_average_cqd ?? parent.average_cqd)}</td>`;

  const parentExtraCells = showConstrainedResults
    ? `<td class="text-type-cell">${escapeHtml(parent.type_label || "无")}</td>`
    : `<td class="text-type-cell">${escapeHtml(parent.type_label || "无")}</td>`;

  const parentRow = `
    <tr class="${clickableClass} ${rowClass(parent)}" data-group-index="${groupIndex}" data-expanded="false" title="${escapeHtml(rowTooltip(parent))}">
      ${parentRankCell}
      ${parentCells}
      ${parentExtraCells}
      <td class="name-cell">
        <span class="fold-icon">${icon}</span>
        ${escapeHtml(parent.canonical)}
        ${statusBadge(parent)}
        ${childBadge}
      </td>
    </tr>
  `;

  const childRows = group.children.map(child => {
    const childRankCell = showConstrainedResults ? "" : `<td>${child.rank}</td>`;
    const childCells = showConstrainedResults
      ? `
        <td class="constrained-rank-cell">${selectionWeightRank(child) ?? "—"}</td>
        <td class="score-cell">${formatScore(selectionWeightCqd(child))}</td>
        <td class="raw-rank-cell">${child.rank ?? "—"}</td>
        <td class="raw-cell">${formatScore(child.raw_average_cqd ?? child.average_cqd)}</td>
        <td class="delta-rank-cell">${formatDeltaRank(child)}</td>`
      : `<td class="score-cell">${formatScore(child.raw_average_cqd ?? child.average_cqd)}</td>`;

    const childExtraCells = showConstrainedResults
      ? `<td class="text-type-cell">${escapeHtml(child.type_label || "无")}</td>`
      : `<td class="text-type-cell">${escapeHtml(child.type_label || "无")}</td>`;

    return `
      <tr class="fold-child ${rowClass(child)}" data-parent-index="${groupIndex}" hidden title="${escapeHtml(rowTooltip(child))}">
        ${childRankCell}
        ${childCells}
        ${childExtraCells}
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




function hasFiniteNumber(value) {
  if (value === null || value === undefined || value === "") {
    return false;
  }
  const number = Number(value);
  return Number.isFinite(number);
}

function selectionWeightCqd(row) {
  if (!row) return null;
  const value = row.selection_weight_cqd ?? row.selection_weight_cqd_display ?? row.pair_score;
  return hasFiniteNumber(value) ? Number(value) : null;
}

function selectionWeightRank(row) {
  if (!row) return null;
  const value = row.selection_weight_rank_all_candidates ?? row.pair_rank;
  return hasFiniteNumber(value) ? Number(value) : null;
}

function hasSelectionWeightCqd(row) {
  return hasFiniteNumber(selectionWeightCqd(row));
}


const WINRATE_TYPE_QUALITY = Object.freeze({
  STABLE_CORE: "stable_core",
  PROBABLE: "probable",
  BOUNDARY: "boundary",
  MIXED: "mixed",
  LOW_SUPPORT: "low_support",
  UNCALIBRATED: "uncalibrated",
});

const WINRATE_TYPE_QUALITY_LABEL = Object.freeze({
  stable_core: "稳定核心",
  probable: "较稳定",
  boundary: "边界型",
  mixed: "混合型",
  low_support: "低样本",
  uncalibrated: "未校准",
});

const WINRATE_TYPE_QUALITY_EN_LABEL = Object.freeze({
  stable_core: "Stable Core",
  probable: "Probable Type",
  boundary: "Boundary Type",
  mixed: "Mixed Type",
  low_support: "Low-Support Type",
  uncalibrated: "Uncalibrated",
});

const WINRATE_TYPE_QUALITY_ORDER = Object.freeze([
  WINRATE_TYPE_QUALITY.STABLE_CORE,
  WINRATE_TYPE_QUALITY.PROBABLE,
  WINRATE_TYPE_QUALITY.BOUNDARY,
  WINRATE_TYPE_QUALITY.MIXED,
  WINRATE_TYPE_QUALITY.LOW_SUPPORT,
  WINRATE_TYPE_QUALITY.UNCALIBRATED,
]);

const WINRATE_TYPE_QUALITY_THRESHOLDS = Object.freeze({
  minClusterSize: 20,
  stableConfidence: 0.75,
  stableReclusterStability: 0.75,
  stableAssignmentEntropy: 0.30,
  probableConfidence: 0.65,
  probableReclusterStability: 0.60,
  probableAssignmentEntropy: 0.50,
  boundaryReclusterStability: 0.60,
  boundaryAssignmentEntropy: 0.50,
  boundaryMargin: 0.02,
  mixedConfidence: 0.60,
  mixedEntropy: 0.60,
});

const RESIDUAL_TYPE_QUALITY_THRESHOLDS = Object.freeze({
  ...WINRATE_TYPE_QUALITY_THRESHOLDS,
  minClusterSize: 8,
  lowVarianceBasinProbableStability: 0.70,
  lowVarianceBasinProbableAssignmentEntropy: 0.38,
  lowVarianceBasinStableStability: 0.78,
  lowVarianceBasinStableAssignmentEntropy: 0.28,
});

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

function winrateClusterSizeFromLabel(label) {
  const match = String(label || "").match(/n\s*=\s*(\d+)/);
  return match ? Number(match[1]) : null;
}

function primaryWinrateConfidence(row) {
  return numberOrNull(row && (row.winrate_profile_soft_confidence_calibrated ?? row.winrate_profile_soft_confidence));
}

function primaryWinrateEntropy(row) {
  return numberOrNull(row && (row.winrate_profile_soft_entropy_calibrated ?? row.winrate_profile_soft_entropy));
}

function primaryWinrateStability(row) {
  return numberOrNull(row && (row.winrate_profile_recluster_stability ?? row.winrate_profile_bootstrap_stability));
}

function winrateTypeQuality(row) {
  const label = row && row.winrate_type_label;
  if (!label) {
    return {
      code: WINRATE_TYPE_QUALITY.UNCALIBRATED,
      label: WINRATE_TYPE_QUALITY_LABEL.uncalibrated,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.uncalibrated,
      reason: "missing_winrate_type",
      isStableCore: false,
    };
  }

  const clusterSize = winrateClusterSizeFromLabel(label);
  const conf = primaryWinrateConfidence(row);
  const entropy = primaryWinrateEntropy(row);
  const stability = primaryWinrateStability(row);
  const assignmentEntropy = numberOrNull(row && row.winrate_profile_assignment_entropy);
  const margin = numberOrNull(row && row.winrate_profile_margin);
  const reasons = [];

  if (clusterSize != null && clusterSize < WINRATE_TYPE_QUALITY_THRESHOLDS.minClusterSize) {
    reasons.push(`cluster_size=${clusterSize}<${WINRATE_TYPE_QUALITY_THRESHOLDS.minClusterSize}`);
    return {
      code: WINRATE_TYPE_QUALITY.LOW_SUPPORT,
      label: WINRATE_TYPE_QUALITY_LABEL.low_support,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.low_support,
      reason: reasons.join(";"),
      isStableCore: false,
    };
  }

  const stableCore =
    conf != null &&
    stability != null &&
    assignmentEntropy != null &&
    conf >= WINRATE_TYPE_QUALITY_THRESHOLDS.stableConfidence &&
    stability >= WINRATE_TYPE_QUALITY_THRESHOLDS.stableReclusterStability &&
    assignmentEntropy <= WINRATE_TYPE_QUALITY_THRESHOLDS.stableAssignmentEntropy;
  if (stableCore) {
    return {
      code: WINRATE_TYPE_QUALITY.STABLE_CORE,
      label: WINRATE_TYPE_QUALITY_LABEL.stable_core,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.stable_core,
      reason: `conf>=${WINRATE_TYPE_QUALITY_THRESHOLDS.stableConfidence};stability>=${WINRATE_TYPE_QUALITY_THRESHOLDS.stableReclusterStability};assignment_entropy<=${WINRATE_TYPE_QUALITY_THRESHOLDS.stableAssignmentEntropy}`,
      isStableCore: true,
    };
  }

  const probable =
    conf != null &&
    stability != null &&
    assignmentEntropy != null &&
    conf >= WINRATE_TYPE_QUALITY_THRESHOLDS.probableConfidence &&
    stability >= WINRATE_TYPE_QUALITY_THRESHOLDS.probableReclusterStability &&
    assignmentEntropy <= WINRATE_TYPE_QUALITY_THRESHOLDS.probableAssignmentEntropy;
  if (probable) {
    return {
      code: WINRATE_TYPE_QUALITY.PROBABLE,
      label: WINRATE_TYPE_QUALITY_LABEL.probable,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.probable,
      reason: `conf>=${WINRATE_TYPE_QUALITY_THRESHOLDS.probableConfidence};stability>=${WINRATE_TYPE_QUALITY_THRESHOLDS.probableReclusterStability};assignment_entropy<=${WINRATE_TYPE_QUALITY_THRESHOLDS.probableAssignmentEntropy}`,
      isStableCore: false,
    };
  }

  if (margin != null && margin <= WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryMargin) {
    reasons.push(`margin<=${WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryMargin}`);
  }
  if (stability != null && stability < WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryReclusterStability) {
    reasons.push(`recluster_stability<${WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryReclusterStability}`);
  }
  if (assignmentEntropy != null && assignmentEntropy > WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryAssignmentEntropy) {
    reasons.push(`assignment_entropy>${WINRATE_TYPE_QUALITY_THRESHOLDS.boundaryAssignmentEntropy}`);
  }
  if (reasons.length) {
    return {
      code: WINRATE_TYPE_QUALITY.BOUNDARY,
      label: WINRATE_TYPE_QUALITY_LABEL.boundary,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.boundary,
      reason: reasons.join(";"),
      isStableCore: false,
    };
  }

  if (conf != null && conf < WINRATE_TYPE_QUALITY_THRESHOLDS.mixedConfidence) {
    reasons.push(`conf<${WINRATE_TYPE_QUALITY_THRESHOLDS.mixedConfidence}`);
  }
  if (entropy != null && entropy >= WINRATE_TYPE_QUALITY_THRESHOLDS.mixedEntropy) {
    reasons.push(`entropy>=${WINRATE_TYPE_QUALITY_THRESHOLDS.mixedEntropy}`);
  }

  return {
    code: WINRATE_TYPE_QUALITY.MIXED,
    label: WINRATE_TYPE_QUALITY_LABEL.mixed,
    englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.mixed,
    reason: reasons.length ? reasons.join(";") : "not_stable_or_probable",
    isStableCore: false,
  };
}

function winrateTypeQualityBadge(row) {
  const quality = winrateTypeQuality(row);
  return `<span class="type-quality-badge type-quality-${quality.code}" title="${escapeHtml(quality.reason)}">${escapeHtml(quality.label)}</span>`;
}

function residualTypeQuality(row) {
  const label = row && row.residual_type_label;
  if (!label) {
    return {
      code: WINRATE_TYPE_QUALITY.UNCALIBRATED,
      label: WINRATE_TYPE_QUALITY_LABEL.uncalibrated,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.uncalibrated,
      reason: "missing_residual_type",
      isStableCore: false,
    };
  }

  const clusterSize = winrateClusterSizeFromLabel(label);
  const conf = numberOrNull(row && row.residual_profile_soft_confidence_calibrated);
  const stability = numberOrNull(row && row.residual_profile_recluster_stability);
  const assignmentEntropy = numberOrNull(row && row.residual_profile_assignment_entropy);
  const margin = numberOrNull(row && row.residual_profile_margin);
  const isLowVarianceBasin = /\bvar-low\b/.test(String(label || ""));
  const reasons = [];

  if (clusterSize != null && clusterSize < RESIDUAL_TYPE_QUALITY_THRESHOLDS.minClusterSize) {
    reasons.push(`cluster_size=${clusterSize}<${RESIDUAL_TYPE_QUALITY_THRESHOLDS.minClusterSize}`);
    return {
      code: WINRATE_TYPE_QUALITY.LOW_SUPPORT,
      label: WINRATE_TYPE_QUALITY_LABEL.low_support,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.low_support,
      reason: reasons.join(";"),
      isStableCore: false,
    };
  }

  if (isLowVarianceBasin &&
      (stability == null || assignmentEntropy == null ||
       stability < RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableStability ||
       assignmentEntropy > RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableAssignmentEntropy)) {
    reasons.push(`var_low_basin_requires_stability>=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableStability}`);
    reasons.push(`var_low_basin_requires_assignment_entropy<=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableAssignmentEntropy}`);
    return {
      code: WINRATE_TYPE_QUALITY.BOUNDARY,
      label: WINRATE_TYPE_QUALITY_LABEL.boundary,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.boundary,
      reason: reasons.join(";"),
      isStableCore: false,
    };
  }

  if (conf != null && stability != null && assignmentEntropy != null &&
      conf >= RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableConfidence &&
      stability >= (isLowVarianceBasin ? RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinStableStability : RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableReclusterStability) &&
      assignmentEntropy <= (isLowVarianceBasin ? RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinStableAssignmentEntropy : RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableAssignmentEntropy)) {
    return {
      code: WINRATE_TYPE_QUALITY.STABLE_CORE,
      label: WINRATE_TYPE_QUALITY_LABEL.stable_core,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.stable_core,
      reason: `residual_conf>=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableConfidence};residual_stability>=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableReclusterStability};residual_assignment_entropy<=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.stableAssignmentEntropy}`,
      isStableCore: true,
    };
  }

  if (conf != null && stability != null && assignmentEntropy != null &&
      conf >= RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableConfidence &&
      stability >= (isLowVarianceBasin ? RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableStability : RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableReclusterStability) &&
      assignmentEntropy <= (isLowVarianceBasin ? RESIDUAL_TYPE_QUALITY_THRESHOLDS.lowVarianceBasinProbableAssignmentEntropy : RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableAssignmentEntropy)) {
    return {
      code: WINRATE_TYPE_QUALITY.PROBABLE,
      label: WINRATE_TYPE_QUALITY_LABEL.probable,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.probable,
      reason: `residual_conf>=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableConfidence};residual_stability>=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableReclusterStability};residual_assignment_entropy<=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.probableAssignmentEntropy}`,
      isStableCore: false,
    };
  }

  if (margin != null && margin <= RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryMargin) {
    reasons.push(`residual_margin<=${RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryMargin}`);
  }
  if (stability != null && stability < RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryReclusterStability) {
    reasons.push(`residual_recluster_stability<${RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryReclusterStability}`);
  }
  if (assignmentEntropy != null && assignmentEntropy > RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryAssignmentEntropy) {
    reasons.push(`residual_assignment_entropy>${RESIDUAL_TYPE_QUALITY_THRESHOLDS.boundaryAssignmentEntropy}`);
  }

  if (reasons.length) {
    return {
      code: WINRATE_TYPE_QUALITY.BOUNDARY,
      label: WINRATE_TYPE_QUALITY_LABEL.boundary,
      englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.boundary,
      reason: reasons.join(";"),
      isStableCore: false,
    };
  }

  return {
    code: WINRATE_TYPE_QUALITY.MIXED,
    label: WINRATE_TYPE_QUALITY_LABEL.mixed,
    englishLabel: WINRATE_TYPE_QUALITY_EN_LABEL.mixed,
    reason: "residual_not_stable_or_probable",
    isStableCore: false,
  };
}

function residualTypeQualityBadge(row) {
  const quality = residualTypeQuality(row);
  return `<span class="type-quality-badge type-quality-${quality.code}" title="${escapeHtml(quality.reason)}">${escapeHtml(quality.label)}</span>`;
}

function formatScore(value) {
  if (value == null) {
    return "—";
  }
  const n = Number(value);
  return Number.isFinite(n) ? n.toFixed(3) : "—";
}

function formatSigned(value) {
  if (value == null) {
    return "—";
  }
  const n = Number(value);
  if (!Number.isFinite(n)) {
    return "—";
  }
  return `${n >= 0 ? "+" : ""}${n.toFixed(3)}`;
}

function formatDeltaRank(row) {
  const rawRank = Number(row && row.rank);
  const correctRank = Number(selectionWeightRank(row));
  if (!Number.isFinite(rawRank) || !Number.isFinite(correctRank)) {
    return "—";
  }
  const delta = rawRank - correctRank;
  return `${delta >= 0 ? "+" : ""}${delta}`;
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
  // 活跃环境中未选中的行按普通行显示。
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
    `raw_rank=${row.rank ?? ""}`,
    `raw_score=${formatScore(row.raw_average_cqd ?? row.average_cqd)}`,
  ];
  if (hasSelectionWeightCqd(row)) parts.push(`selection_weight_rank=${selectionWeightRank(row) ?? ""}`);
  if (hasSelectionWeightCqd(row)) parts.push(`selection_weight_cqd=${formatScore(selectionWeightCqd(row))}`);
  if (row.type_label) parts.push(`text_type=${row.type_label}`);
  if (row.raw_delta != null) parts.push(`score_delta=${formatSigned(row.raw_delta)}`);
  return parts.join(" | ");
}


function effectiveExportScore(row) {
  return Number(row && (row.raw_average_cqd ?? row.average_cqd)) || 0;
}

function exportResults() {
  const laneSize = document.getElementById("laneSize").value;

  if (!currentResults.length) {
    alert("当前没有可导出的结果。请先点击“读取结果”。");
    return;
  }

  const rawRows = rawPresentationRows(currentResults)
    .filter(row => hasFiniteNumber(row.raw_average_cqd ?? row.average_cqd));
  const lines = [
    "R-Rank\tR-Score\tType\tName",
    ...rawRows.map(row => [
      row.rank ?? "",
      formatScore(row.raw_average_cqd ?? row.average_cqd),
      row.type_label || "",
      row.canonical || "",
    ].map(value => String(value).replace(/\t/g, " ")).join("\t")),
  ];

  downloadText(`lane_${laneSize}_raw_score.txt`, lines.join("\n"));
}

function exportConstrainedResults() {
  const laneSize = document.getElementById("laneSize").value;

  if (!currentResults.length) {
    alert("当前没有可导出的结果。请先点击“读取结果”。");
    return;
  }

  const rows = constrainedPresentationRows(currentResults)
    .filter(isConstrainedVisibleRow);

  if (!rows.length) {
    alert("当前没有校准结果。请先点击“执行校准”，等任务完成后再读取结果。");
    return;
  }

  const lines = [
    "C-Rank\tC-Score\tR-Rank\tR-Score\tΔ\tType\tName",
    ...rows.map(row => [
      selectionWeightRank(row) ?? "",
      formatScore(selectionWeightCqd(row)),
      row.rank ?? "",
      formatScore(row.raw_average_cqd ?? row.average_cqd),
      formatDeltaRank(row),
      row.type_label || "",
      row.canonical || "",
    ].map(value => String(value).replace(/\t/g, " ")).join("\t")),
  ];

  downloadText(`lane_${laneSize}_correct_score.txt`, lines.join("\n"));
}




function readTargetSettings() {
  return {};
}




async function generateTargets() {
  const laneSize = document.getElementById("laneSize").value;
  const out = document.getElementById("targetGenerationOutput");
  const button = document.getElementById("generateTargetsBtn");
  let finished = false;
  const progressTask = (async () => {
    while (!finished) {
      await sleep(1000);
      if (finished) break;
      try {
        const progress = await loadLaneProgress(laneSize);
        if (finished) break;
        const phase = progress && progress.phase ? String(progress.phase) : "";
        const message = progress && progress.message ? String(progress.message) : "";
        if (phase.startsWith("target_") && phase !== "target_generation_ready") {
          const done = Number(progress.rate_done || 0);
          const total = Number(progress.rate_total || 0);
          const countText = total > 0 ? ` ${done}/${total}` : "";
          out.textContent = `生成靶子中：${phase}${countText}${message ? " — " + message : ""}`;
        }
        await loadLanes();
      } catch (_) {
        // 目标请求仍具有权威性。短暂的进度轮询失败不得取消生成或隐藏其最终响应。
      }
    }
  })();
  try {
    button.disabled = true;
    // 此次点击从全新的零开始，而非短暂重放被中断的目标生成请求遗留的进度。
    out.textContent = "generating targets: target_preparing 0/0, 0.00 pair/s";
    const data = await postJson(`/api/lanes/${laneSize}/targets`, readTargetSettings());
    currentTargets = data;
    clearTargetPreview();
    finished = true;
    out.textContent = "靶子生成完成，可点击“导出靶子”下载。";
  } catch (err) {
    finished = true;
    out.textContent = String(err);
  } finally {
    finished = true;
    button.disabled = false;
    await progressTask;
    await loadLanes();
  }
}

function clearTargetPreview() {
  const el = document.getElementById("targetResults");
  if (el) {
    el.innerHTML = "";
  }
}




function formatWeight(value) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return "";
  return Number(value).toFixed(12);
}




function exportTargets() {
  if (!currentTargets || !Array.isArray(currentTargets.rows) || !currentTargets.rows.length) {
    alert("当前没有靶子。请先点击“生成靶子”。");
    return;
  }

  const laneSize = document.getElementById("laneSize").value;
  // Openbox 目标预设格式（TOML 风格表格）。请求的扩展名是 .html，但载荷有意匹配 newTarget2.toml。
  const tomlString = value => JSON.stringify(String(value));
  const targetLines = [];
  for (const row of currentTargets.rows) {
    // 导出原始显示身份。`player_keys` 有意将 DSU/root-team 名称用于内部所有权约束（例如 Squall 可能归一化为
    // Asunder）；暴露这些键会改写用户在目标文件中看到的名称。仅在内部保留归一化键。
    const players = String(row.canonical || "").split("+").map(s => s.trim()).filter(Boolean);
    targetLines.push(
      "[[targets]]",
      `factor = ${formatWeight(row.target_weight)}`,
      `players = [${players.map(tomlString).join(", ")}]`,
      "",
    );
  }
  downloadText(`Target${laneSize}.html`, targetLines.join("\n"));
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


function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function loadLaneProgress(laneSize) {
  const res = await fetch(`/api/lanes/${laneSize}/progress`);
  if (!res.ok) {
    return null;
  }
  return await res.json();
}

async function waitForPairwiseCalibration(laneSize, out) {
  const readyPhases = new Set([
    "winrate_type_profile_ready",
    "pairwise_calibration_ready", // compatibility with older builds / stale DB progress
    "calibration_ready",
    "constrained_selection_ready",
  ]);

  for (let attempt = 0; attempt < 1800; attempt += 1) {
    await sleep(1000);
    await loadLanes();

    const progress = await loadLaneProgress(laneSize);
    const phase = progress && progress.phase ? String(progress.phase) : "";
    const message = progress && progress.message ? String(progress.message) : "";

    if (phase) {
      out.textContent = `校准中：${phase}${message ? " — " + message : ""}`;
    }

    if (readyPhases.has(phase)) {
      out.textContent = `校准完成。${message || "结果已写入表格。"}`;
      return true;
    }

    if (/failed|error/i.test(phase) || /failed|error|panic/i.test(message)) {
      throw new Error(`校准失败：${phase}${message ? " — " + message : ""}`);
    }
  }

  out.textContent = "校准仍在运行；可以稍后点击“读取结果”或“导出校准”。";
  return false;
}





async function runConstrainedSelection() {
  const out = document.getElementById("constrainedSelectionOutput");
  const laneSize = document.getElementById("laneSize").value;
  try {
    const settings = readSelectionSettings();
    out.textContent = "queueing calibration...";
    const data = await postJson(`/api/lanes/${laneSize}/calibration`, settings);

    const checkbox = document.getElementById("showConstrainedResultsInput");
    checkbox.checked = true;
    showConstrainedResults = true;

    const queuedThreshold = Number(data.raw_score_threshold ?? data.cqd_threshold);
    out.textContent = `已排队校准：Raw Score ≥ ${queuedThreshold.toFixed(3)}，等待完成...`;
    renderResultsTable();
    await loadLanes();

    const finished = await waitForPairwiseCalibration(laneSize, out);
    if (finished) {
      await loadResults();
    }
  } catch (err) {
    out.textContent = String(err);
  }
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
