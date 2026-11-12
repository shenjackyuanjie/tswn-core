#!/usr/bin/env python3
"""串行交替配对 namer-pf 四项 CLI E2E；第零轮预热，输出必须完全一致。"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import time


def cases(repo):
    pool = (repo / 'docs/perf/cqp/sqp6000_first20.txt').read_text().splitlines()
    pool = [name for name in pool if name.strip()]
    groups = [('mario', ['mario']), ('unicode', ['琉音芽玲亚 #AIBSXOQHSK@无惨']),
              ('clone-regression', ['艾泽莉娅 #IAPBVEKLIO@无惨']),
              ('haste-regression', ['龟钧募犀红@Hell'])]
    for size in (1, 2, 4, 8):
        for offset in (0, 8):
            groups.append((f'pool-{size}-{offset}', pool[offset:offset + size]))
    groups.extend([('profile-clan-bang', ['33554433@!']),
                   ('profile-clan-normal', ['33554433@\x02'])])
    return groups


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--a', type=Path, required=True)
    parser.add_argument('--b', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=2000)
    parser.add_argument('--rounds', type=int, default=7)
    parser.add_argument('--threads', type=int, default=1)
    parser.add_argument('--keep-rq', action='store_true')
    parser.add_argument('--exclude-case', action='append', default=[], help='显式排除基线无法运行的病例；记录在汇总中')
    args = parser.parse_args()
    if min(args.runs, args.rounds, args.threads) < 1:
        parser.error('场数、轮数、线程数必须为正数')
    binaries = {'a': args.a.resolve(), 'b': args.b.resolve()}
    hashes = {key: digest(path) for key, path in binaries.items()}
    groups = cases(Path(__file__).resolve().parents[1])
    unknown = set(args.exclude_case) - {label for label, _ in groups}
    if unknown:
        parser.error(f'未知排除病例: {sorted(unknown)}')
    groups = [(label, group) for label, group in groups if label not in args.exclude_case]
    if not groups:
        parser.error('不能排除全部病例')
    args.out.mkdir(parents=True, exist_ok=False)
    records, expected = [], {}
    for round_index in range(args.rounds + 1):
        for mode_index, mode in enumerate(('pp', 'pd', 'qp', 'qd')):
            for case_index, (label, group) in enumerate(groups):
                key = f'{mode}/{label}'
                order = ('a', 'b') if (round_index + mode_index + case_index) % 2 == 0 else ('b', 'a')
                for variant in order:
                    binary = binaries[variant]
                    if digest(binary) != hashes[variant]:
                        raise RuntimeError('计时中二进制发生变化')
                    command = [str(binary), 'namer-pf', '-r', '+'.join(group), '-n', str(args.runs),
                               '--thread', str(args.threads), '--precision', '9', '--metric', mode]
                    if args.keep_rq:
                        command.append('--keep-rq')
                    start = time.perf_counter()
                    result = subprocess.run(command, capture_output=True, check=True)
                    elapsed = time.perf_counter() - start
                    output = result.stdout.decode().strip()
                    if not output or f' {mode}:' not in output:
                        raise RuntimeError(f'{key}: 未发现指标输出')
                    if key in expected and output != expected[key]:
                        raise RuntimeError(f'{key}: 行为不一致')
                    expected[key] = output
                    records.append({'round': round_index, 'variant': variant, 'mode': mode,
                                    'case': label, 'elapsed_seconds': elapsed})
        (args.out / 'records.json').write_text(json.dumps(records, indent=2) + '\n')
        print(f'完成 {round_index}/{args.rounds}', flush=True)
    summary = {}
    for mode in ('pp', 'pd', 'qp', 'qd', 'all'):
        improvements = []
        for round_index in range(1, args.rounds + 1):
            totals = {variant: sum(row['elapsed_seconds'] for row in records
                                  if row['round'] == round_index and row['variant'] == variant
                                  and (mode == 'all' or row['mode'] == mode)) for variant in ('a', 'b')}
            improvements.append(100 * (totals['a'] / totals['b'] - 1))
        median = statistics.median(improvements)
        summary[mode] = {'throughput_gain_percent': median,
                         'mad_percentage_points': statistics.median(abs(value - median) for value in improvements),
                         'paired_round_gains_percent': improvements}
    per_case = {}
    for mode in ('pp', 'pd', 'qp', 'qd'):
        for label, group in groups:
            gains = []
            for round_index in range(1, args.rounds + 1):
                seconds = {row['variant']: row['elapsed_seconds'] for row in records
                           if row['round'] == round_index and row['mode'] == mode and row['case'] == label}
                gains.append(100 * (seconds['a'] / seconds['b'] - 1))
            median = statistics.median(gains)
            per_case[f'{mode}/{label}'] = {
                'target_names': len(group), 'throughput_gain_percent': median,
                'mad_percentage_points': statistics.median(abs(value - median) for value in gains),
                'paired_round_gains_percent': gains,
            }
    metadata = {'binary_sha256': hashes, 'runs_per_metric': args.runs, 'threads': args.threads,
                'keep_rq': args.keep_rq, 'excluded_cases': args.exclude_case, 'precision': 9, 'warmup_round': 0,
                'measured_rounds': args.rounds, 'cases': dict(groups),
                'measurement': '完整 CLI 进程墙钟，包括启动、解析、准备、战斗和输出；无逐场计时',
                'output_sha256': {key: hashlib.sha256(value.encode()).hexdigest() for key, value in expected.items()},
                'summary': summary, 'per_case': per_case}
    (args.out / 'summary.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(summary, indent=2), flush=True)


if __name__ == '__main__':
    main()
