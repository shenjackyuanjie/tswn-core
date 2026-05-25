const fs = require("fs");
const path = require("path");

const SCORE_ROUND_PATTERN = /if \(outer_display\.includes\(update_list\.a\(some_d\.a\[0\]\)\.e\.gb2\(\)\)\) \{\r?\n\s*\/\/ 胜利场\r?\n\s*\+\+this_\.Q\r?\n\s*\};\r?\n\s*\+\+round_count;\r?\n\s*\+\+this_\.ch/;
const SCORE_ROUND_INSERT = `{
                            const __score_round_update = update_list.a(some_d.a[0]);
                            const __score_round_source = __score_round_update.e.gb2();
                            const __score_round_target = __score_round_update.f == null ? null : __score_round_update.f.gb2();
                            const __score_round_message = __score_round_update.d;
                            const __score_round_win = outer_display.includes(__score_round_source) ? 1 : 0;
                            if (__score_round_win) {
                                ++this_.Q;
                            }
                            ++round_count;
                            ++this_.ch;
                            if (run_env.from_code) finish_trigger.emit("__score_trace_round", this_.ch, this_.Q, __score_round_win, __score_round_source, __score_round_target, __score_round_message);
                        }`;
const SCORE_TRACE_EXPORT = `
if (run_env.from_code) {
    runner.score_trace = (names, target_round) => {
        return new Promise((resolve) => {
            let round_datas = [];
            finish_trigger.removeAllListeners("__score_trace_round");
            finish_trigger.on("__score_trace_round", (run_round, score, round_win, source, target, message) => {
                round_datas.push({ round: run_round, score: score, round_win: round_win, source: source, target: target, message: message });
                if (run_round >= target_round) {
                    stop_bomb = true;
                    resolve({ score: score, raw_data: round_datas });
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
        // 临时 trace 模块清理失败不影响主流程，尽量清掉即可。
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
            options.rounds = Number.parseInt(argv[index + 1] ?? "", 10);
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

function patchScoreTrace(sourceText) {
    const matchCount = [...sourceText.matchAll(new RegExp(SCORE_ROUND_PATTERN, "g"))].length;
    if (matchCount !== 1) {
        throw new Error(`expected exactly one score trace token, found ${matchCount}`);
    }
    return sourceText.replace(SCORE_ROUND_PATTERN, SCORE_ROUND_INSERT) + SCORE_TRACE_EXPORT;
}

function createTempModule(md5Path) {
    const resolvedMd5Path = path.resolve(md5Path);
    const md5Dir = path.dirname(resolvedMd5Path);
    const sourceText = fs.readFileSync(resolvedMd5Path, "utf8");
    const patched = patchScoreTrace(sourceText);
    const tempDir = path.resolve(process.cwd(), "target", "trace-temp");
    fs.mkdirSync(tempDir, { recursive: true });
    const sourceAssets = path.resolve(md5Dir, "assets");
    const tempAssets = path.resolve(tempDir, "assets");
    if (fs.existsSync(sourceAssets) && !fs.existsSync(tempAssets)) {
        try {
            fs.symlinkSync(sourceAssets, tempAssets, "junction");
        } catch (_error) {
            fs.cpSync(sourceAssets, tempAssets, { recursive: true });
        }
    }
    const tempModulePath = path.resolve(
        tempDir,
        `tswn-md5-score-trace-${process.pid}-${Date.now()}.js`,
    );
    fs.writeFileSync(tempModulePath, patched, "utf8");
    currentTempModulePath = tempModulePath;
    return { tempModulePath };
}

async function main() {
    const options = parseArgs(process.argv.slice(2));
    const names = fs.readFileSync(options.inputFile, "utf8");
    const { tempModulePath } = createTempModule(options.md5Path);

    try {
        const md5 = require(tempModulePath);
        if (typeof md5.score_trace !== "function") {
            throw new Error("instrumented md5.js does not expose score_trace()");
        }
        const result = await md5.score_trace(names, options.rounds);
        process.stdout.write(
            `${JSON.stringify({
                score: result.score,
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
