let currentResults = [];

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
  const innerRaw = document.getElementById("innerWorkersInput").value.trim();
  const skipArchived = document.getElementById("skipArchivedInput").checked;

  if (outerRaw && !/^\d+$/.test(outerRaw)) {
    throw new Error("Outer Worker 必须是 0 或正整数。0 表示动态分配。 ");
  }
  if (innerRaw && !/^\d+$/.test(innerRaw)) {
    throw new Error("Inner Worker 必须是 0 或正整数。0 表示 tswn-core 自动。 ");
  }

  return {
    outer_workers: outerRaw ? Number(outerRaw) : 0,
    inner_workers: innerRaw ? Number(innerRaw) : 0,
    skip_archived: skipArchived,
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

  const groups = buildFoldedResultGroups(rows);

  // 高级结果 UI：
  // 从上往下扫描，每扫到一个没有被折叠的组，将下面所有与它有重复 member 的组折叠到它下面。
  // 默认折叠；点击主行展开/收起。
  el.innerHTML = `
    <table class="score-table folded-score-table">
      <thead>
        <tr>
          <th>Rank</th>
          <th class="score-col">Score</th>
          <th class="odds-col">Odds</th>
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

function buildFoldedResultGroups(rows) {
  const consumed = new Array(rows.length).fill(false);
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
    consumed[i] = true;

    const children = [];
    for (let j = i + 1; j < rows.length; j++) {
      if (consumed[j]) {
        continue;
      }

      if (hasMemberOverlap(parent.members, parsed[j].members)) {
        consumed[j] = true;
        children.push(parsed[j].row);
      }
    }

    groups.push({
      parent: parent.row,
      children,
    });
  }

  return groups;
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

  const parentRow = `
    <tr class="${clickableClass}" data-group-index="${groupIndex}" data-expanded="false">
      <td>${parent.rank}</td>
      <td class="score-cell">${parent.average_cqd.toFixed(3)}</td>
      <td class="odds-cell">${parent.golden_rate.toFixed(3)}</td>
      <td class="name-cell">
        <span class="fold-icon">${icon}</span>
        ${escapeHtml(parent.canonical)}
        ${childBadge}
      </td>
    </tr>
  `;

  const childRows = group.children.map(child => `
    <tr class="fold-child" data-parent-index="${groupIndex}" hidden>
      <td>${child.rank}</td>
      <td class="score-cell">${child.average_cqd.toFixed(3)}</td>
      <td class="odds-cell">${child.golden_rate.toFixed(3)}</td>
      <td class="name-cell child-name">
        <span class="fold-child-marker">↳</span>
        ${escapeHtml(child.canonical)}
      </td>
    </tr>
  `).join("");

  return parentRow + childRows;
}

function exportResults() {
  const laneSize = document.getElementById("laneSize").value;

  if (!currentResults.length) {
    alert("当前没有可导出的结果。请先点击“读取结果”。");
    return;
  }

  // 导出格式：CQD 名字，一行一个。
  const content = currentResults
    .map(row => `${row.average_cqd.toFixed(3)} ${row.canonical}`)
    .join("\n");

  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");

  a.href = url;
  a.download = `lane_${laneSize}_score.txt`;
  document.body.appendChild(a);
  a.click();
  a.remove();

  URL.revokeObjectURL(url);
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
