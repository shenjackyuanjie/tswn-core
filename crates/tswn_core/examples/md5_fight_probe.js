const fs = require("fs");
const vm = require("vm");
const path = require("path");

const filename = path.resolve(__dirname, "../../../../fast-namerena/md5.js");
let code = fs.readFileSync(filename, "utf8");
code = code.replace(
  /lazy_old\(\$, "vr", "rq", function \(\) \{\s*return 4\s*\}\)/,
  'lazy_old($, "vr", "rq", function () { return 6 })',
);
code = code.replace(
  "reset_fight_log_data()\n            run_env.capture_fight_log = true",
  "reset_fight_log_data()\n            $.vr = 6\n            run_env.capture_fight_log = true",
);
code = code.replace(
  'finish_trigger.once("done_fight", (data) => {\n                resolve(fmt_RunUpdate(data));',
  '$.vr = 6\n            finish_trigger.once("done_fight", (data) => {\n                resolve(fmt_RunUpdate(data));',
);
code = code.replace(
  "p.fx = q.fx",
  "p.fx = q.fx; if (run_env.probe_clone && String(this_.r && this_.r.e).startsWith(run_env.probe_clone)) console.error(JSON.stringify({clone_owner:this_.r.e, clone:p.e, q:p.q, hp:p.fx, atk:p.ch, def:p.cx, spd:p.cy, agi:p.db, mag:p.dx, mdf:p.dy, wis:p.fr, maxhp:p.fy, mp:p.go, move:p.l, factor:p.x}))",
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
context.globalThis = context;
context.__probe_getAt = process.argv.includes("--get-at");
const actionArg = process.argv.find((arg) => arg.startsWith("--action="));
if (actionArg) {
  context.__probe_action_skill_target = actionArg.slice("--action=".length);
}
vm.runInNewContext(code, context, { filename });

function buildScoreMatchInput(modifier, round) {
  const base = 33554431 + (round - 1) * 3;
  return `aaaaaa\n${base}@${modifier}\n\n${base + 1}@${modifier}\n${base + 2}@${modifier}`;
}

const modifier = process.argv[2] || "\u0002";
const round = Number(process.argv[3] || 1);
const input = buildScoreMatchInput(modifier, round);

const method = process.argv[4] || "fight_log";
moduleObj.exports.run_env.probe_clone = process.argv[5] || "";

moduleObj.exports[method](input).then((result) => {
  console.log(JSON.stringify(result));
});
