#!/usr/bin/env python3
"""预先固定收窄候选的十四项 E2E 确认；要求 Linux CPU 2–5 可用。"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

from benchmark_namer_pf import cases, digest


def selected_cases():
    selected = [f't1-rq6/{mode}/pool-8-{offset}'
                for mode in ('pp', 'pd', 'qp', 'qd') for offset in (0, 8)]
    selected += ['t1-rq6/qd/unicode', 't1-rq6/qd/clone-regression']
    selected += [f't4-rq4/qp/{label}'
                 for label in ('pool-2-8', 'pool-4-0', 'pool-8-0', 'pool-8-8')]
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if not {2, 3, 4, 5}.issubset(os.sched_getaffinity(0)):
        parser.error('此确认协议需要 CPU 2–5')
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    binaries = {'a': args.baseline.resolve(), 'c': args.candidate.resolve()}
    hashes = {variant: digest(path) for variant, path in binaries.items()}
    groups = dict(cases(Path(__file__).resolve().parents[1]))
    selected = selected_cases()
    records, expected = [], {}
    for round_index in range(6):
        for case_index, key in enumerate(selected):
            context, mode, label = key.split('/')
            parallel = context == 't4-rq4'
            os.sched_setaffinity(0, {2, 3, 4, 5} if parallel else {2})
            order = ('a', 'c') if (round_index + case_index) % 2 == 0 else ('c', 'a')
            for variant in order:
                binary = binaries[variant]
                if digest(binary) != hashes[variant]:
                    raise RuntimeError('计时中二进制发生变化')
                command = [str(binary), 'namer-pf', '-r', '+'.join(groups[label]), '-n', '13000',
                           '--thread', '4' if parallel else '1', '--precision', '9', '--metric', mode]
                if parallel:
                    command.append('--keep-rq')
                start = time.perf_counter()
                result = subprocess.run(command, capture_output=True, check=True)
                elapsed = time.perf_counter() - start
                if not result.stdout or f' {mode}:'.encode() not in result.stdout:
                    raise RuntimeError(f'{key}: 未发现指标输出')
                if key in expected and expected[key] != result.stdout:
                    raise RuntimeError(f'{key}: 输出不一致')
                expected[key] = result.stdout
                records.append({'round': round_index, 'case': key, 'variant': variant,
                                'elapsed_seconds': elapsed})
        (out / 'records.json').write_text(json.dumps(records, indent=2) + '\n')
        print(f'完成 {round_index}/5', flush=True)
    summary = {}
    for key in selected:
        gains = []
        for round_index in range(1, 6):
            seconds = {row['variant']: row['elapsed_seconds'] for row in records
                       if row['round'] == round_index and row['case'] == key}
            gains.append(100 * (seconds['a'] / seconds['c'] - 1))
        median = statistics.median(gains)
        summary[key] = {'median_percent': median,
                        'mad_pp': statistics.median(abs(gain - median) for gain in gains),
                        'gains_percent': gains}
    metadata = {'runs': 13000, 'rounds': 5, 'warmup_round': 0,
                'selected_before_run': selected, 'binary_sha256': hashes, 'summary': summary}
    (out / 'summary.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
