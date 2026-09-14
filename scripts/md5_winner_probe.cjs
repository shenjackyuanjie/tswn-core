// legacy md5.js 判胜语义探针：在 Grp.dj（移出存活）与 Grp.aZ（复活/加入存活）里插桩，
// 记录真实存活队伍数、Q（alive group count）以及胜者判定所用的比较值。
// 只对内存中的源码做 patch，不写回 md5.js。
//
// 用法：
//   node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --case-dir crates/tswn_test/cases/runtime_stress
//   node scripts/md5_winner_probe.cjs ..\fast-namerena\md5.js --names tests/sqp5900.txt --battles 10000
//
// 结论与解读见 docs/mechanics/winner.md。
const fs = require("fs");
const vm = require("vm");
const path = require("path");

const md5Path = path.resolve(process.argv[2]);
const argv = process.argv.slice(3);
const caseDirArg = argv.indexOf("--case-dir");
const namesArg = argv.indexOf("--names");
const battlesArg = argv.indexOf("--battles");
const caseDir = caseDirArg >= 0 ? path.resolve(argv[caseDirArg + 1]) : null;
const namesPool = namesArg >= 0 ? path.resolve(argv[namesArg + 1]) : null;
const battlesTarget = battlesArg >= 0 ? Number(argv[battlesArg + 1]) : 2000;

let code = fs.readFileSync(md5Path, "utf8").replace(/\r\n?/g, "\n");

const patches = [
  [
    "        C.Array.U(r, a)\n        var __idx = C.Array.aT(s.c, a)",
    '        C.Array.U(r, a)\n        if (globalThis.__probe_winner) globalThis.__probe_log.push({ev:"remove", removed:a.e, emptied:this.f.length===0, group_alive_after:this.f.length, world_alive_after:r.length, true_groups_after:new Set(r.map(x=>x.y)).size, Q_before:s.Q})\n        var __idx = C.Array.aT(s.c, a)',
  ],
  [
    "            if (q.f.length === r.length) {\n                s.cy = q",
    '            if (globalThis.__probe_winner) globalThis.__probe_log.push({ev:"win_check", first_group_alive:q.f.length, world_alive:r.length, Q:s.Q})\n            if (q.f.length === r.length) {\n                if (globalThis.__probe_winner) globalThis.__probe_log.push({ev:"winner", first_group_alive:q.f.length, world_alive:r.length, Q:s.Q})\n                s.cy = q',
  ],
  [
    "        if (!q.d.includes(a)) q.d.push(a)\n        if (!q.f.includes(a)) q.f.push(a)",
    '        if (!q.d.includes(a)) q.d.push(a)\n        if (!q.f.includes(a)) q.f.push(a)\n        if (globalThis.__probe_winner) globalThis.__probe_log.push({ev:"revive", actor:a.e, group_alive_after:q.f.length, world_alive:p.e.length, true_groups_after:new Set(p.e.map(x=>x.y)).size, Q:p.Q})',
  ],
];
for (const [from, to] of patches) {
  if (!code.includes(from)) {
    throw new Error(`探针锚点未命中: ${from.slice(0, 60)}`);
  }
  code = code.replace(from, to);
}

const moduleObj = { exports: {} };
const context = {
  console,
  require,
  module: moduleObj,
  exports: moduleObj.exports,
  __filename: md5Path,
  __dirname: path.dirname(md5Path),
  process,
  setTimeout,
  clearInterval,
  clearTimeout,
  setInterval,
  Buffer,
};
context.global = context;
context.globalThis = context;
vm.runInNewContext(code, context, { filename: md5Path });
const runner = moduleObj.exports;
context.__probe_winner = true;
context.__probe_log = [];

function caseInputs() {
  return fs
    .readdirSync(caseDir)
    .filter((name) => name.endsWith(".txt"))
    .sort()
    .map((file) => ({
      file,
      raw: fs.readFileSync(path.join(caseDir, file), "utf8").replace(/\r\n?/g, "\n"),
    }));
}

function poolInputs() {
  const pool = fs
    .readFileSync(namesPool, "utf8")
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .filter((line) => line.trim() !== "" && !line.startsWith("seed:"));
  let state = 0x2545f491;
  const next = () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    return state;
  };
  const inputs = [];
  for (let index = 0; index < battlesTarget; index += 1) {
    const picked = new Set();
    while (picked.size < 6) {
      picked.add(next() % pool.length);
    }
    const names = [...picked].map((at) => pool[at]);
    const groups = [names.slice(0, 2), names.slice(2, 4), names.slice(4, 6)];
    inputs.push({
      file: `pool:${index}`,
      raw: groups.map((group) => group.join("\n")).join("\n\n"),
    });
  }
  return inputs;
}

async function main() {
  const inputs = caseDir ? caseInputs() : poolInputs();
  const started = Date.now();
  let removes = 0;
  let empties = 0;
  let revives = 0;
  let qStale = 0;
  let qOneButTwoAlive = 0;
  let winChecks = 0;
  let winners = 0;
  let badWinner = 0;
  let errors = 0;
  const staleSamples = [];
  const errorSamples = [];

  for (const item of inputs) {
    context.__probe_log.length = 0;
    let result;
    try {
      result = await runner.fight(item.raw);
    } catch (error) {
      errors += 1;
      if (errorSamples.length < 3) {
        errorSamples.push({ file: item.file, error: String(error).slice(0, 160) });
      }
      continue;
    }
    for (const event of context.__probe_log) {
      if (event.ev === "remove") {
        removes += 1;
        if (event.emptied) {
          empties += 1;
          const qAfter = event.Q_before - 1;
          if (qAfter !== event.true_groups_after) {
            qStale += 1;
            if (staleSamples.length < 3) {
              staleSamples.push({ file: item.file, ...event, q_after: qAfter });
            }
          }
          if (qAfter === 1 && event.true_groups_after >= 2) {
            qOneButTwoAlive += 1;
          }
        }
      } else if (event.ev === "revive") {
        revives += 1;
      } else if (event.ev === "win_check") {
        winChecks += 1;
      } else if (event.ev === "winner") {
        winners += 1;
        if (event.first_group_alive !== event.world_alive) {
          badWinner += 1;
        }
      }
    }
    if (result == null && errorSamples.length < 3) {
      errorSamples.push({ file: item.file, error: "fight 返回空结果" });
    }
  }

  console.log(
    JSON.stringify(
      {
        md5: md5Path,
        mode: caseDir ? "case-dir" : "pool",
        inputs: inputs.length,
        errors,
        removes,
        group_empties: empties,
        revives,
        q_stale_after_wipe: qStale,
        q_eq_1_but_two_teams_alive: qOneButTwoAlive,
        win_checks: winChecks,
        winners,
        winner_with_mismatched_counts: badWinner,
        stale_samples: staleSamples,
        error_samples: errorSamples,
        elapsed_ms: Date.now() - started,
      },
      null,
      1,
    ),
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
