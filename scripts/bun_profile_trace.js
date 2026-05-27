const fs = require("fs");
const path = require("path");

const TRACE_EVENT_PATTERN = /(\+\+this_\.z\s*\r?\n)(\s*async_goto = 3)/;
const TRACE_EVENT_INSERT =
  '$1                    if (run_env.from_code) finish_trigger.emit("__trace_round", this_.z, this_.y, C.Array.w(o, n.a(f.a[0]).e.gb2()) ? 1 : 0);\n$2';
const PROFILE_TRACE_EXPORT = `
if (run_env.from_code) {
    runner.profile_trace = (names, target_round) => {
        return new Promise((resolve) => {
            let round_datas = [];
            finish_trigger.removeAllListeners('__trace_round');
            finish_trigger.on('__trace_round', (run_round, win_count, round_win) => {
                round_datas.push({ round: run_round, wins: win_count, round_win: round_win });
                if (run_round >= target_round) {
                    stop_bomb = true;
                    resolve({ win_count: win_count, raw_data: round_datas });
                }
            });
            main(names);
        });
    };
}
`;

let currentTempModulePath = null;

function cleanupTempModule() {
  if (!currentTempModulePath) {
    return;
  }
  try {
    fs.rmSync(currentTempModulePath, { force: true });
  } catch (_error) {
    // Some Bun failure paths can abort normal control flow; best-effort cleanup is enough here.
  } finally {
    currentTempModulePath = null;
  }
}

process.on("exit", cleanupTempModule);
process.on("uncaughtException", (error) => {
  cleanupTempModule();
  process.stderr.write(`${error.stack || String(error)}\n`);
  process.exit(1);
});
process.on("unhandledRejection", (reason) => {
  cleanupTempModule();
  process.stderr.write(`${reason?.stack || String(reason)}\n`);
  process.exit(1);
});

function parseArgs(argv) {
  const options = {
    inputFile: null,
    rounds: null,
    md5Path: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--input-file") {
      options.inputFile = argv[index + 1] ?? null;
      index += 1;
      continue;
    }
    if (arg === "--rounds") {
      const raw = argv[index + 1] ?? "";
      options.rounds = Number.parseInt(raw, 10);
      index += 1;
      continue;
    }
    if (arg === "--md5") {
      options.md5Path = argv[index + 1] ?? null;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${arg}`);
  }

  if (!options.inputFile) {
    throw new Error("missing --input-file");
  }
  if (!Number.isInteger(options.rounds) || options.rounds <= 0) {
    throw new Error(`invalid --rounds: ${options.rounds}`);
  }
  if (!options.md5Path) {
    throw new Error("missing --md5");
  }
  return options;
}

function patchProfileTrace(sourceText) {
  const matchCount = [...sourceText.matchAll(new RegExp(TRACE_EVENT_PATTERN, "g"))].length;
  if (matchCount !== 1) {
    throw new Error(`expected exactly one profile trace token, found ${matchCount}`);
  }
  return sourceText.replace(TRACE_EVENT_PATTERN, TRACE_EVENT_INSERT) + PROFILE_TRACE_EXPORT;
}

function createTempModule(md5Path) {
  const resolvedMd5Path = path.resolve(md5Path);
  const sourceText = fs.readFileSync(resolvedMd5Path, "utf8");
  const patched = patchProfileTrace(sourceText);
  const md5Dir = path.dirname(resolvedMd5Path);
  const tempModulePath = path.resolve(md5Dir, `tswn-md5-trace-${process.pid}-${Date.now()}.js`);
  fs.writeFileSync(tempModulePath, patched, "utf8");
  currentTempModulePath = tempModulePath;
  return {
    tempModulePath,
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const names = fs.readFileSync(options.inputFile, "utf8");
  const { tempModulePath } = createTempModule(options.md5Path);

  try {
    const md5 = require(tempModulePath);
    if (typeof md5.profile_trace !== "function") {
      throw new Error("instrumented md5.js does not expose profile_trace()");
    }
    const result = await md5.profile_trace(names, options.rounds);

    process.stdout.write(
      `${JSON.stringify({
        win_count: result.win_count,
        raw_data: result.raw_data,
      })}\n`,
    );
  } finally {
    cleanupTempModule();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || String(error)}\n`);
  process.exit(1);
});
