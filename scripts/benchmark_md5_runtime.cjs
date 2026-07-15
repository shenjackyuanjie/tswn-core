// 使用官方 md5.js 接口测量 Node.js/Bun 的稳态 core 与矩阵调度性能。
// 计时前完成脚本加载、输入读取和 worker 预载，避免把 JS 解析时间混进逐场吞吐。

const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const {
  Worker,
  isMainThread,
  parentPort,
  workerData,
} = require("worker_threads");

const TEST_HEADER = "!test!\n\n";

function nowNanos() {
  return process.hrtime.bigint();
}

function elapsedNanos(started) {
  return Number(nowNanos() - started);
}

function runtimeInfo() {
  if (typeof Bun !== "undefined") {
    return { runtime: "bun", version: Bun.version };
  }
  return { runtime: "nodejs", version: process.versions.node };
}

function normalizeText(raw) {
  return raw.replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n");
}

function hashText(raw) {
  return crypto.createHash("sha256").update(normalizeText(raw), "utf8").digest("hex").toUpperCase();
}

function benchmarkInput(raw) {
  const normalized = normalizeText(raw).trim();
  return normalized.startsWith("!test!") ? normalized : `${TEST_HEADER}${normalized}`;
}

function winRateBenchmarkInput(raw) {
  const normalized = normalizeText(raw).trim();
  if (normalized.startsWith("!test!")) {
    return normalized;
  }

  const lines = normalized.split("\n").map((line) => line.trimEnd());
  let groups;
  if (!lines.some((line) => line.length === 0)) {
    // 名竞旧输入在完全没有空行时按“一行一个队伍”解释；加测试头前必须先固化分组。
    groups = lines.filter(Boolean).map((line) => [line]);
  } else {
    groups = [];
    let current = [];
    for (const line of lines) {
      if (line.length === 0) {
        if (current.length > 0) {
          groups.push(current);
          current = [];
        }
      } else {
        current.push(line);
      }
    }
    if (current.length > 0) {
      groups.push(current);
    }
  }
  return `${TEST_HEADER}${groups.map((group) => group.join("\n")).join("\n\n")}`;
}

function parseGroupLines(raw, doublePlus) {
  const separator = doublePlus ? "++" : "+";
  return normalizeText(raw)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.split(separator).map((name) => name.trim()).filter(Boolean));
}

function idName(rawName) {
  const noWeapon = rawName.split("+", 1)[0];
  const at = noWeapon.indexOf("@");
  if (at < 0) {
    return noWeapon;
  }
  const name = noWeapon.slice(0, at);
  const team = noWeapon.slice(at + 1);
  return team.length === 0 || team === name || team.includes(":") ? name : `${name}@${team}`;
}

function duplicateIdName(left, right) {
  const seen = new Set();
  for (const name of [...left, ...right]) {
    const identity = idName(name);
    if (seen.has(identity)) {
      return identity;
    }
    seen.add(identity);
  }
  return null;
}

function parseArgs(argv) {
  const options = {
    mode: null,
    md5Path: path.resolve(process.cwd(), "md5.js"),
    input: null,
    targets: null,
    caseDir: null,
    count: null,
    workers: 0,
    doublePlus: false,
    targetDoublePlus: false,
    label: null,
    out: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const nextValue = () => {
      const value = argv[index + 1];
      if (value == null) {
        throw new Error(`${arg} 缺少参数`);
      }
      index += 1;
      return value;
    };
    switch (arg) {
      case "--mode":
        options.mode = nextValue();
        break;
      case "--md5":
        options.md5Path = path.resolve(nextValue());
        break;
      case "--input":
        options.input = path.resolve(nextValue());
        break;
      case "--targets":
        options.targets = path.resolve(nextValue());
        break;
      case "--case-dir":
        options.caseDir = path.resolve(nextValue());
        break;
      case "--count":
        options.count = Number.parseInt(nextValue(), 10);
        break;
      case "--workers":
        options.workers = Number.parseInt(nextValue(), 10);
        break;
      case "--label":
        options.label = nextValue();
        break;
      case "--out":
        options.out = path.resolve(nextValue());
        break;
      case "--double-plus":
        options.doublePlus = true;
        break;
      case "--target-double-plus":
        options.targetDoublePlus = true;
        break;
      default:
        throw new Error(`未知参数: ${arg}`);
    }
  }

  if (!["fixed", "win-rate", "score", "matrix"].includes(options.mode)) {
    throw new Error("--mode 只支持 fixed/win-rate/score/matrix");
  }
  if (!Number.isInteger(options.count) || options.count <= 0) {
    throw new Error("--count 必须是正整数");
  }
  if (!Number.isInteger(options.workers) || options.workers < 0) {
    throw new Error("--workers 必须是非负整数");
  }
  if (options.mode === "fixed" && !options.caseDir) {
    throw new Error("fixed 模式需要 --case-dir");
  }
  if (["win-rate", "score", "matrix"].includes(options.mode) && !options.input) {
    throw new Error(`${options.mode} 模式需要 --input`);
  }
  if (options.mode === "matrix" && !options.targets) {
    throw new Error("matrix 模式需要 --targets");
  }
  return options;
}

async function runWinRate(md5, raw, count) {
  if (count % 100 !== 0) {
    throw new Error(`官方 win_rate_callback 只按 100 场上报，本工具要求 count 是 100 的倍数: ${count}`);
  }
  const result = await md5.win_rate_callback(
    winRateBenchmarkInput(raw),
    (round) => round < count,
  );
  return { wins: Number(result.win_count), total: count, errors: 0 };
}

async function runScore(md5, raw, count) {
  const result = await md5.score_callback(
    benchmarkInput(raw),
    (round) => round < count,
  );
  return { wins: Number(result.score), total: count, errors: 0 };
}

async function warmup(md5) {
  // 先触发静态初始化与一小段 JIT；正式计时仍会包含各 workload 自身的 matchup 准备。
  await runWinRate(md5, "warmup-left@red\n\nwarmup-right@blue", 100);
}

async function workerMain() {
  const md5 = require(workerData.md5Path);
  await warmup(md5);
  parentPort.postMessage({ type: "ready" });
  parentPort.on("message", async (message) => {
    if (message.type !== "job") {
      return;
    }
    try {
      const result = message.kind === "score"
        ? await runScore(md5, message.raw, message.count)
        : await runWinRate(md5, message.raw, message.count);
      parentPort.postMessage({ type: "result", index: message.index, result });
    } catch (error) {
      parentPort.postMessage({
        type: "job-error",
        index: message.index,
        error: error?.stack || String(error),
      });
    }
  });
}

function spawnReadyWorker(md5Path) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(__filename, {
      workerData: { role: "md5-benchmark-worker", md5Path },
    });
    const onMessage = (message) => {
      if (message.type === "ready") {
        worker.off("message", onMessage);
        resolve(worker);
      }
    };
    worker.on("message", onMessage);
    worker.once("error", reject);
    worker.once("exit", (code) => {
      if (code !== 0) {
        reject(new Error(`worker 预载阶段异常退出: ${code}`));
      }
    });
  });
}

async function runWorkerPool(jobs, count, requestedWorkers, md5Path) {
  const workerCount = Math.max(1, Math.min(requestedWorkers, jobs.length || 1));
  const workers = await Promise.all(
    Array.from({ length: workerCount }, () => spawnReadyWorker(md5Path)),
  );
  const results = new Array(jobs.length);
  let nextIndex = 0;
  let completed = 0;
  let settled = false;

  const started = nowNanos();
  try {
    await new Promise((resolve, reject) => {
      const fail = (error) => {
        if (!settled) {
          settled = true;
          reject(error);
        }
      };
      const dispatch = (worker) => {
        if (nextIndex >= jobs.length) {
          return;
        }
        const index = nextIndex;
        nextIndex += 1;
        const job = jobs[index];
        worker.postMessage({
          type: "job",
          kind: job.kind || "win-rate",
          index,
          raw: job.raw,
          count,
        });
      };

      for (const worker of workers) {
        worker.on("error", fail);
        worker.on("message", (message) => {
          if (message.type === "job-error") {
            fail(new Error(`worker job #${message.index} 失败: ${message.error}`));
            return;
          }
          if (message.type !== "result" || settled) {
            return;
          }
          results[message.index] = message.result;
          completed += 1;
          if (completed === jobs.length) {
            settled = true;
            resolve();
          } else {
            dispatch(worker);
          }
        });
        dispatch(worker);
      }
      if (jobs.length === 0) {
        settled = true;
        resolve();
      }
    });
  } finally {
    var wallNanos = elapsedNanos(started);
    await Promise.all(workers.map((worker) => worker.terminate()));
  }
  return { results, wallNanos, workers: workerCount };
}

function inferCaseMode(filename) {
  const match = filename.match(/^\d+_(1v1|2v2|3v3v3|ffa_\d+)-/);
  if (!match) {
    throw new Error(`无法从文件名解析 case mode: ${filename}`);
  }
  return match[1];
}

function summarizeRows(label, rows, predicate = () => true) {
  const selected = rows.filter(predicate);
  const battles = selected.reduce((sum, row) => sum + row.total, 0);
  const wins = selected.reduce((sum, row) => sum + row.wins, 0);
  const errors = selected.reduce((sum, row) => sum + row.errors, 0);
  const nanos = selected.reduce((sum, row) => sum + row.elapsed_nanos, 0);
  const seconds = nanos / 1e9;
  return {
    group: label,
    cases: selected.length,
    battles,
    wins,
    errors,
    elapsed_nanos: nanos,
    us_per_battle: nanos / 1e3 / Math.max(1, battles),
    battles_per_second: battles / Math.max(Number.EPSILON, seconds),
  };
}

async function runFixed(options, md5, md5Version) {
  const allFilenames = fs.readdirSync(options.caseDir)
    .filter((filename) => filename.endsWith(".txt"))
    .sort();
  // 官方 win_rate 只支持恰好两个对战组；多组 case 必须明确跳过，不能等待一个永远不会发出的事件。
  const filenames = allFilenames.filter((filename) => ["1v1", "2v2"].includes(inferCaseMode(filename)));
  const skippedCases = allFilenames
    .filter((filename) => !filenames.includes(filename))
    .map((filename) => ({ filename, mode: inferCaseMode(filename), reason: "official win_rate only supports two groups" }));
  const cases = [];
  for (const filename of filenames) {
    const raw = normalizeText(fs.readFileSync(path.join(options.caseDir, filename), "utf8"));
    const started = nowNanos();
    const result = await runWinRate(md5, raw, options.count);
    cases.push({
      filename,
      mode: inferCaseMode(filename),
      input_sha256: hashText(raw),
      ...result,
      elapsed_nanos: elapsedNanos(started),
    });
  }
  const summaries = [
    summarizeRows("overall", cases),
    summarizeRows("core_1v1_2v2", cases, (item) => ["1v1", "2v2"].includes(item.mode)),
    summarizeRows("one_v_one", cases, (item) => item.mode === "1v1"),
    summarizeRows("two_v_two", cases, (item) => item.mode === "2v2"),
  ];
  const overall = summaries[0];
  return {
    schema_version: 1,
    ...runtimeInfo(),
    md5_version: md5Version,
    workload: "fixed30-two-team-single-thread",
    label: options.label || "fixed30",
    case_dir: options.caseDir,
    count_per_case: options.count,
    total_case_count: allFilenames.length,
    benchmarked_case_count: filenames.length,
    skipped_cases: skippedCases,
    total: overall.battles,
    wins: overall.wins,
    errors: overall.errors,
    elapsed_nanos: overall.elapsed_nanos,
    us_per_battle: overall.us_per_battle,
    battles_per_second: overall.battles_per_second,
    timing_scope: "sequential core wall; excludes process startup, md5.js load, warmup, input read and report serialization",
    cases,
    summaries,
  };
}

async function runSingleWinRate(options, md5, md5Version) {
  const raw = normalizeText(fs.readFileSync(options.input, "utf8"));
  const started = nowNanos();
  const result = await runWinRate(md5, raw, options.count);
  const wallNanos = elapsedNanos(started);
  return {
    schema_version: 1,
    ...runtimeInfo(),
    md5_version: md5Version,
    workload: "win-rate-single-thread",
    label: options.label || "win-rate",
    input: options.input,
    input_sha256: hashText(raw),
    count: options.count,
    ...result,
    elapsed_nanos: wallNanos,
    us_per_battle: wallNanos / 1e3 / options.count,
    battles_per_second: options.count / (wallNanos / 1e9),
    timing_scope: "core wall; excludes process startup, md5.js load, warmup, input read and report serialization",
  };
}

async function runScoreBatch(options, md5, md5Version) {
  const inputText = fs.readFileSync(options.input, "utf8");
  const groups = parseGroupLines(inputText, options.doublePlus);
  const rows = [];
  const batchStarted = nowNanos();
  for (let index = 0; index < groups.length; index += 1) {
    const started = nowNanos();
    const result = await runScore(md5, groups[index].join("\n"), options.count);
    rows.push({ index, players: groups[index], ...result, elapsed_nanos: elapsedNanos(started) });
  }
  const wallNanos = elapsedNanos(batchStarted);
  const total = rows.reduce((sum, row) => sum + row.total, 0);
  const wins = rows.reduce((sum, row) => sum + row.wins, 0);
  return {
    schema_version: 1,
    ...runtimeInfo(),
    md5_version: md5Version,
    workload: "score-single-thread",
    label: options.label || "score",
    input: options.input,
    input_sha256: hashText(inputText),
    count_per_group: options.count,
    group_count: groups.length,
    total,
    wins,
    errors: 0,
    elapsed_nanos: wallNanos,
    us_per_battle: wallNanos / 1e3 / Math.max(1, total),
    battles_per_second: total / (wallNanos / 1e9),
    timing_scope: "sequential core batch wall; excludes process startup, md5.js load, warmup, input read and report serialization",
    groups: rows,
  };
}

async function runMatrix(options, md5Version) {
  const playerText = fs.readFileSync(options.input, "utf8");
  const targetText = fs.readFileSync(options.targets, "utf8");
  const players = parseGroupLines(playerText, options.doublePlus);
  const targets = parseGroupLines(targetText, options.targetDoublePlus);
  const jobs = [];
  const skippedMatchups = [];
  for (let playerIndex = 0; playerIndex < players.length; playerIndex += 1) {
    for (let targetIndex = 0; targetIndex < targets.length; targetIndex += 1) {
      const duplicate = duplicateIdName(players[playerIndex], targets[targetIndex]);
      if (duplicate != null) {
        skippedMatchups.push({ player_index: playerIndex, target_index: targetIndex, duplicate_id_name: duplicate });
        continue;
      }
      jobs.push({
        playerIndex,
        targetIndex,
        raw: `${players[playerIndex].join("\n")}\n\n${targets[targetIndex].join("\n")}`,
      });
    }
  }
  const logical = typeof os.availableParallelism === "function"
    ? os.availableParallelism()
    : os.cpus().length;
  const automaticWorkers = options.count <= 100 ? Math.ceil(logical * 1.5) : logical * 2;
  const requestedWorkers = options.workers || automaticWorkers;
  const pool = await runWorkerPool(jobs, options.count, requestedWorkers, options.md5Path);
  const rows = pool.results.map((result, index) => ({
    player_index: jobs[index].playerIndex,
    target_index: jobs[index].targetIndex,
    ...result,
  }));
  const total = rows.reduce((sum, row) => sum + row.total, 0);
  const wins = rows.reduce((sum, row) => sum + row.wins, 0);
  return {
    schema_version: 1,
    ...runtimeInfo(),
    md5_version: md5Version,
    workload: "cqp-matrix-auto",
    label: options.label || "cqp-matrix",
    players: options.input,
    targets: options.targets,
    player_sha256: hashText(playerText),
    target_sha256: hashText(targetText),
    count_per_matchup: options.count,
    player_groups: players.length,
    target_groups: targets.length,
    requested_matchups: players.length * targets.length,
    matchups: jobs.length,
    skipped_duplicate_matchups: skippedMatchups,
    workers: pool.workers,
    total,
    wins,
    errors: 0,
    elapsed_nanos: pool.wallNanos,
    us_per_battle: pool.wallNanos / 1e3 / Math.max(1, total),
    battles_per_second: total / (pool.wallNanos / 1e9),
    timing_scope: "dynamic matrix wall; excludes process startup, worker startup, md5.js load, warmup, input read and report serialization",
    results: rows,
  };
}

function writeReport(report, out) {
  const json = `${JSON.stringify(report, null, 2)}\n`;
  if (!out) {
    process.stdout.write(json);
    return;
  }
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, json, "utf8");
  process.stdout.write(
    `${report.runtime} ${report.workload}: ${(report.elapsed_nanos / 1e9).toFixed(6)}s -> ${out}\n`,
  );
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!fs.existsSync(options.md5Path)) {
    throw new Error(`md5.js 不存在: ${options.md5Path}`);
  }

  let report;
  if (options.mode === "matrix") {
    // 主线程只负责调度；每个 worker 自己加载一份官方 md5.js。
    const probe = require(options.md5Path);
    const md5Version = probe.run_env?.version ?? null;
    report = await runMatrix(options, md5Version);
  } else {
    const md5 = require(options.md5Path);
    await warmup(md5);
    const md5Version = md5.run_env?.version ?? null;
    if (options.mode === "fixed") {
      report = await runFixed(options, md5, md5Version);
    } else if (options.mode === "win-rate") {
      report = await runSingleWinRate(options, md5, md5Version);
    } else {
      report = await runScoreBatch(options, md5, md5Version);
    }
  }
  writeReport(report, options.out);
}

if (!isMainThread && workerData?.role === "md5-benchmark-worker") {
  workerMain().catch((error) => {
    parentPort.postMessage({ type: "job-error", index: -1, error: error?.stack || String(error) });
    process.exitCode = 1;
  });
} else {
  main().catch((error) => {
    process.stderr.write(`${error?.stack || String(error)}\n`);
    process.exit(1);
  });
}
