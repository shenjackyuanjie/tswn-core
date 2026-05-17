const fs = require("fs");
const vm = require("vm");
const path = require("path");

const filename = path.resolve(__dirname, "../../../../fast-namerena/md5.js");
let code = fs.readFileSync(filename, "utf8");
code = code.replace(/this_\.ch\s*%\s*100\s*==\s*0/g, "this_.ch % 1 == 0");
code = code.replace(/round_count\s*<\s*100/g, "round_count < 1");
if (process.argv.includes("--reset-why")) {
  code = code.replace(/profiler\.dZ\(team_1, team_2\)/g, "profiler.dZ(team_1, team_2); why_ns = 0");
}
code = code.replace(
  "return P._asyncAwait(T.start_main(flighter), $async$O)",
  "if (run_env.probe_round && this_.ch + 1 === run_env.probe_round) console.error(JSON.stringify({probe_round: this_.ch + 1, flighter})); return P._asyncAwait(T.start_main(flighter), $async$O)",
);
code = code.replace(
  "if (outer_display.includes(update_list.a(some_d.a[0]).e.gb2())) {",
  "if (run_env.probe_round && this_.ch + 1 === run_env.probe_round) { const __u = update_list.a(some_d.a[0]); console.error(JSON.stringify({probe_round: this_.ch + 1, score_update: {message: __u.d, source: __u.e && __u.e.gb2 && __u.e.gb2(), target: __u.r && __u.r.gb2 && __u.r.gb2(), affect: __u.x}})); } if (outer_display.includes(update_list.a(some_d.a[0]).e.gb2())) {",
);

const moduleObj = { exports: {} };
const context = {
  console,
  require,
  module: moduleObj,
  exports: moduleObj.exports,
  __filename: filename,
  __dirname: path.dirname(filename),
  process,
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  Buffer,
};
context.global = context;

vm.runInNewContext(code, context, { filename });

const runner = moduleObj.exports;
const input = process.argv[2] || "!test!\n\naaaaaa";
const count = Number(process.argv[3] || 1000);
runner.run_env.probe_round = Number(process.argv[4] || 0);

runner.score_callback(input, (round) => round < count).then((result) => {
  console.log(JSON.stringify(result.raw_data));
});
