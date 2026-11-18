#!/usr/bin/env node
// fixed30 多轮采样汇总：读 run_rounds.ps1 落盘的 t<threads>_r<round>.jsonl 与 runs.log，
// 按分组算中位数 / CV / MAD 离群，输出 Markdown 表格或机器可读 JSON，并与 history.json 对比。
// 用法：node summarize.mjs [--dir <目录>] [--out <json>] [--markdown] [--history <file>] [--label <名称>] [--version <版本>] [--date <日期>]

import { readFileSync, readdirSync, existsSync, writeFileSync } from 'node:fs'
import { join, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const SKILL_HISTORY = resolve(HERE, '../history.json')
const STREAM_MODES = ['ffa_6', 'ffa_8', '3v3v3']
const GROUPS = ['overall', 'core_1v1_2v2', 'one_v_one', 'two_v_two', 'stress_multi', 'ffa_4']

function parseArgs(argv) {
  const out = { markdown: false }
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i]
    if (a === '--markdown') out.markdown = true
    else if (a.startsWith('--')) out[a.slice(2)] = argv[i + 1], (i += 1)
    else throw new Error('未知参数: ' + a)
  }
  return out
}

function latestRunDir(root) {
  if (!existsSync(root)) throw new Error('找不到 ' + root + '，先跑 run_rounds.ps1')
  const dirs = readdirSync(root, { withFileTypes: true })
    .filter((d) => d.isDirectory() && /^fixed30_/.test(d.name))
    .map((d) => d.name)
    .sort()
  if (dirs.length === 0) throw new Error(root + ' 下没有 fixed30_* 目录')
  return join(root, dirs[dirs.length - 1])
}

function modeOf(input) {
  const base = input.split(/[\\/]/).pop()
  return base.replace(/^\d+_/, '').replace(/-[0-9a-f]+\.txt$/, '')
}

function groupNames(mode) {
  const names = ['overall']
  if (mode === '1v1') names.push('one_v_one', 'core_1v1_2v2')
  if (mode === '2v2') names.push('two_v_two', 'core_1v1_2v2')
  if (STREAM_MODES.includes(mode)) names.push('stress_multi')
  if (mode === 'ffa_4') names.push('ffa_4')
  return names
}

const median = (xs) => {
  const s = [...xs].sort((x, y) => x - y)
  const n = s.length
  return n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2
}

const mad = (xs) => {
  const m = median(xs)
  return median(xs.map((x) => Math.abs(x - m)))
}

function stat(values) {
  const med = median(values)
  const sigma = 1.4826 * mad(values)
  const kept = values.filter((x) => Math.abs(x - med) <= 3 * sigma)
  const mean = values.reduce((a, b) => a + b, 0) / values.length
  const sd = Math.sqrt(values.reduce((a, x) => a + (x - mean) ** 2, 0) / Math.max(values.length - 1, 1))
  return {
    median: +med.toFixed(6),
    kept_median: +median(kept).toFixed(6),
    kept_rounds: kept.length,
    rounds: values.length,
    min: +Math.min(...values).toFixed(6),
    max: +Math.max(...values).toFixed(6),
    spread_pct: +((Math.max(...values) / Math.min(...values) - 1) * 100).toFixed(2),
    cv_pct: +((sd / mean) * 100).toFixed(2),
    values: values.map((v) => +v.toFixed(6)),
  }
}

function loadRounds(dir) {
  const runs = []
  const logPath = join(dir, 'runs.log')
  const meta = new Map()
  if (existsSync(logPath)) {
    for (const line of readFileSync(logPath, 'utf8').trim().split(/\r?\n/)) {
      const m = line.match(/threads=(\d) round=(\d+) wall_s=([\d.]+) start=(\S+) end=(\S+) load_before=(\d+) load_after=(\d+) exit=(-?\d+)/)
      if (m) meta.set(m[1] + '_' + m[2], { wall_s: Number(m[3]), start: m[4], end: m[5], load_before: Number(m[6]), load_after: Number(m[7]), exit: Number(m[8]) })
    }
  }
  const files = readdirSync(dir).filter((f) => /^t\d+_r\d+\.jsonl$/.test(f))
  if (files.length === 0) throw new Error(dir + ' 下没有 t<threads>_r<round>.jsonl')
  for (const f of files.sort()) {
    const m = f.match(/^t(\d+)_r(\d+)\.jsonl$/)
    const threads = Number(m[1])
    const round = Number(m[2])
    const rows = readFileSync(join(dir, f), 'utf8').trim().split(/\r?\n/).filter(Boolean).map((l) => JSON.parse(l))
    const agg = {}
    const bump = (name, r) => {
      agg[name] ??= { us: 0, total: 0, wins: 0, errors: 0, guard: 0 }
      const a = agg[name]
      a.us += r.elapsed_seconds * 1e6
      a.total += r.total
      a.wins += r.wins
      a.errors += r.errors
      a.guard += r.guard_exhausted
    }
    for (const r of rows) for (const name of groupNames(modeOf(r.input))) bump(name, r)
    for (const a of Object.values(agg)) a.us_per_game = a.us / a.total
    runs.push({ threads, round, cases: rows, agg, ...(meta.get(m[1] + '_' + m[2]) ?? {}) })
  }
  return runs
}

function summarize(runs) {
  const result = { groups: {}, cases: [], invariants: {} }
  for (const t of [...new Set(runs.map((r) => r.threads))].sort((a, b) => b - a)) {
    const rs = runs.filter((r) => r.threads === t).sort((a, b) => a.round - b.round)
    result.groups['threads_' + t] = Object.fromEntries(GROUPS.filter((g) => rs[0].agg[g]).map((g) => [g, stat(rs.map((r) => r.agg[g].us_per_game))]))
  }
  const st = runs.filter((r) => r.threads === Math.max(...runs.map((r) => r.threads)))
  const template = st[0].cases
  for (const c of template) {
    const base = c.input.split(/[\\/]/).pop()
    const values = st.map((r) => {
      const row = r.cases.find((x) => x.input.split(/[\\/]/).pop() === base)
      return (row.elapsed_seconds / row.total) * 1e6
    })
    result.cases.push({ input: base, mode: modeOf(c.input), total_games: c.total, wins: c.wins, ...stat(values) })
  }
  const all = runs.map((r) => r.agg.overall)
  result.invariants = {
    rounds: runs.length,
    total_games_per_round: [...new Set(all.map((a) => a.total))],
    wins_per_round: [...new Set(all.map((a) => a.wins))],
    errors_total: all.reduce((s, a) => s + a.errors, 0),
    guard_exhausted_total: all.reduce((s, a) => s + a.guard, 0),
    non_zero_exit: runs.filter((r) => r.exit !== undefined && r.exit !== 0).length,
  }
  return result
}

function fmtPct(cur, prev) {
  const d = ((cur / prev - 1) * 100)
  return (d >= 0 ? '+' : '') + d.toFixed(1) + '%'
}

function markdown(summary, runs, history, label) {
  const out = []
  const threadKeys = Object.keys(summary.groups).sort((a, b) => Number(b.split('_')[1]) - Number(a.split('_')[1]))
  for (const key of threadKeys) {
    const threads = key.split('_')[1]
    const g = summary.groups[key]
    out.push('## ' + (threads === '1' ? '单线程' : threads === '0' ? '自动线程' : 'threads=' + threads) + '（' + g.overall.rounds + ' 轮）', '')
    out.push('| 分组 | wall µs/场 | 范围 | CV | MAD 保留 |')
    out.push('| --- | ---: | ---: | ---: | ---: |')
    for (const name of GROUPS.filter((n) => g[n])) {
      const s = g[name]
      out.push('| ' + name + ' | ' + s.median.toFixed(3) + ' | ' + s.min.toFixed(3) + ' – ' + s.max.toFixed(3) + ' | ' + s.cv_pct.toFixed(2) + '% | ' + s.kept_rounds + '/' + s.rounds + ' |')
    }
    out.push('')
  }
  const latest = summary.groups.threads_1 ?? Object.values(summary.groups)[0]
  if (history) {
    const cur = { label: label ?? '本次', single: Object.fromEntries(GROUPS.filter((n) => latest[n]).map((n) => [n, latest[n].median])), auto: summary.groups.threads_0 ? { overall: summary.groups.threads_0.overall.median } : null }
    const last = history.entries[history.entries.length - 1]
    const sameAsLast = last !== undefined && Math.abs(last.single.overall - cur.single.overall) < 0.05 && Math.abs(last.single.core_1v1_2v2 - cur.single.core_1v1_2v2) < 0.05
    const rows = sameAsLast ? history.entries : [...history.entries, cur]
    out.push('## 趋势（单线程 µs/场）', '')
    out.push('| 版本 | 日期 | overall | vs 上版 | core 1v1/2v2 | vs 上版 | stress_multi | vs 上版 | 自动线程 |')
    out.push('| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |')
    let prev = null
    for (const e of rows) {
      const single = e.single
      const auto = e.auto ? ' ' + e.auto.overall + ' |' : ' — |'
      const sign = (k) => (prev === null ? '—' : fmtPct(single[k], prev.single[k]))
      out.push('| ' + e.label + ' | ' + (e.date ?? '') + ' | ' + single.overall.toFixed(3) + ' | ' + sign('overall') + ' | ' + single.core_1v1_2v2.toFixed(3) + ' | ' + sign('core_1v1_2v2') + ' | ' + single.stress_multi.toFixed(3) + ' | ' + sign('stress_multi') + ' |' + auto)
      prev = e
    }
    out.push('')
  }
  if (history) {
    const last = history.entries[history.entries.length - 1]
    const sameAsLast = last !== undefined && Math.abs(last.single.overall - latest.overall.median) < 0.05 && Math.abs(last.single.core_1v1_2v2 - latest.core_1v1_2v2.median) < 0.05
    if (sameAsLast) {
      out.push('本次结果与最后一条留档（' + last.label + '）一致，趋势表未重复追加。', '')
    } else {
      out.push('## 相对上一条留档（' + last.label + '）', '')
      for (const name of GROUPS.filter((n) => latest[n] && last.single[n])) {
        out.push('- ' + name + ': ' + latest[name].median.toFixed(3) + ' vs ' + last.single[name] + ' → ' + fmtPct(latest[name].median, last.single[name]))
      }
      out.push('')
    }
  }
  out.push('## 一致性', '')
  out.push('- 每轮场数: ' + summary.invariants.total_games_per_round.join(', '))
  out.push('- 每轮 wins: ' + summary.invariants.wins_per_round.join(', '))
  out.push('- errors 合计: ' + summary.invariants.errors_total + '；guard 合计: ' + summary.invariants.guard_exhausted_total + '；非零退出轮: ' + summary.invariants.non_zero_exit)
  return out.join('\n')
}

const args = parseArgs(process.argv.slice(2))
const dir = args.dir ? resolve(args.dir) : latestRunDir(resolve('target'))
const runs = loadRounds(dir)
const summary = summarize(runs)
const history = existsSync(args.history ?? SKILL_HISTORY) ? JSON.parse(readFileSync(args.history ?? SKILL_HISTORY, 'utf8')) : null

if (args.markdown || !args.out) {
  console.log(markdown(summary, runs, history, args.label))
}

if (args.out) {
  const payload = {
    meta: {
      generated_at: new Date().toISOString(),
      commit: args.commit ?? null,
      version: args.version ?? null,
      label: args.label ?? null,
      dir,
      rounds: runs.length,
      command: 'perf_runtime -i docs/perf/fixed_cases_30 -n 13000 --threads 1|0',
    },
    invariants: summary.invariants,
    rounds: runs.map((r) => ({ threads: r.threads, round: r.round, wall_s: r.wall_s ?? null, start: r.start ?? null, end: r.end ?? null, load_before: r.load_before ?? null, load_after: r.load_after ?? null, exit: r.exit ?? null, overall_us_per_game: +r.agg.overall.us_per_game.toFixed(6), core_us_per_game: +(r.agg.core_1v1_2v2?.us_per_game ?? 0).toFixed(6), stress_us_per_game: +(r.agg.stress_multi?.us_per_game ?? 0).toFixed(6), total_games: r.agg.overall.total, wins: r.agg.overall.wins, errors: r.agg.overall.errors, guard: r.agg.overall.guard })),
    groups: summary.groups,
    cases: summary.cases,
  }
  writeFileSync(resolve(args.out), JSON.stringify(payload, null, 2) + '\n')
  console.log('JSON: ' + resolve(args.out))
}
