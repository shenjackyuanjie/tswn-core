/** 本地浏览器基准测试。安装工具：npm install --prefix target/web-test-tools playwright linkedom */
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { cpus, platform, release } from 'node:os';
import { chromium } from '../target/web-test-tools/node_modules/playwright/index.mjs';
import { percentile } from '../crates/tswn_wasm/examples/show-metrics.js';

const examples = resolve('crates/tswn_wasm/examples');
const output = resolve('target/web_streaming');
await mkdir(output, { recursive: true });
const hook = `
const recordInitial = BattleMetrics.prototype.initialRendered;
BattleMetrics.prototype.initialRendered = function() {
  if (this.pulls.length !== 0) throw new Error('initial DOM must precede the first WASM pull');
  return recordInitial.call(this);
};
globalThis.battleBenchmark = {
 async start(raw) { inputName.value = raw; await startBattle({ persistInput: false }); speedMode = 'turbo';
   if (!currentBattle) throw new Error(inputStatus.textContent);
   return { frames: currentBattle.frames.length, ttis: battleMetrics.snapshot().ttis_ms }; },
 get done() { return metricsReported; },
 get snapshot() { return battleMetrics.snapshot(); },
 get samples() { return { pulls: battleMetrics.pulls, renders: battleMetrics.renders }; },
 get error() { return streamError ? formatError(streamError) : null; },
 get canonical() { return { initial: currentBattle.initial_states, frames: currentBattle.frames, result: currentBattle.result }; },
};`;
const server = createServer(async (req, res) => {
  try {
    const pathname = new URL(req.url, 'http://localhost').pathname;
    const base = pathname.startsWith('/pkg/') ? resolve(output, 'pkg') : examples;
    const relative = pathname.startsWith('/pkg/') ? pathname.slice(5) : pathname.slice(1) || 'index.html';
    const file = resolve(base, relative);
    if (!file.startsWith(base + '\\') && !file.startsWith(base + '/')) { res.writeHead(403); res.end(); return; }
    let content = await readFile(file);
    if (relative === 'show.js') content = Buffer.from(content.toString('utf8') + hook);
    res.setHeader('Content-Type', ({ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' })[extname(file)] ?? 'application/octet-stream');
    res.end(content);
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${server.address().port}/index.html?perf=1`;
const executablePath = process.env.TSWN_BROWSER_PATH || (platform() === 'win32' ? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe' : undefined);
const browser = await chromium.launch({ executablePath, headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  await page.waitForFunction(() => document.querySelector('#inputStatus').textContent.includes('可以开始'));
  const fixtures = ['1v1-0f92cb76cc37fdc5', '2v2-554f4128af707167', 'ffa_8-16d11de1ebe1df41', '3v3v3-0ace5df17b84e26a'];
  const runs = [];
  for (const fixture of fixtures) {
    const raw = await readFile(resolve('crates/tswn_test/cases/runtime_stress', fixture + '.txt'), 'utf8');
    for (let run = 0; run < 20; run++) {
      const initial = await page.evaluate(raw => battleBenchmark.start(raw), raw);
      if (initial.frames > 2 || initial.ttis == null) throw new Error('startup eagerly pulled before initial render');
      await page.waitForFunction(() => battleBenchmark.done || battleBenchmark.error, null, { timeout: 120_000 });
      const result = await page.evaluate(() => ({ ...battleBenchmark.snapshot, ...battleBenchmark.samples, error: battleBenchmark.error }));
      if (result.error || errors.length) throw new Error(JSON.stringify({ result: result.error, errors }));
      runs.push({ fixture, run, ...result });
      if (run === 0 && fixture.startsWith('2v2')) await page.screenshot({ path: resolve(output, 'streaming_result.png'), fullPage: false });
      console.log(`${fixture} ${run + 1}/20: TTIS=${result.ttis_ms.toFixed(2)} TTFE=${result.ttfe_ms.toFixed(2)} pull-p95=${result.frame_pull_p95_ms.toFixed(2)} frames=${result.frames_received}`);
    }
  }
  const environment = { browser: await browser.version(), os: `${platform()} ${release()}`, cpu: cpus()[0].model,
    viewport: '1440x1000', build: 'cargo release, wasm-bindgen --target web', mode: 'headless Edge, warm WASM module, fresh session/icons each run, turbo playback' };
  await writeFile(resolve(output, 'baseline.json'), JSON.stringify({ environment, runs }, null, 2));
  const rows = fixtures.map(fixture => {
    const group = runs.filter(run => run.fixture === fixture);
    const pulls = group.flatMap(run => run.pulls), renders = group.flatMap(run => run.renders);
    const f = x => x.toFixed(3);
    return `| ${fixture.split('-')[0]} | ${f(percentile(group.map(run => run.ttis_ms), .5))} / ${f(percentile(group.map(run => run.ttis_ms), .95))} | ${f(percentile(group.map(run => run.ttfe_ms), .5))} / ${f(percentile(group.map(run => run.ttfe_ms), .95))} | ${f(percentile(pulls, .5))} / ${f(percentile(pulls, .95))} / ${f(Math.max(...pulls))} | ${f(percentile(renders, .5))} / ${f(percentile(renders, .95))} | ${group[0].frames_received} | ${group[0].chunks_rendered} |`;
  });
  await writeFile(resolve(output, 'baseline_table.md'), rows.join('\n') + '\n');
  console.log(JSON.stringify(environment));
  console.log(rows.join('\n'));
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
