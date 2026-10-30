#!/usr/bin/env python3
"""汇总严格配对的耗时，保留逐轮数值；不删除离群值、不把 MAD 当置信区间。"""
import argparse
import csv
import json
import math
from pathlib import Path
from statistics import median


def summarize(directory):
    metadata = json.loads((directory / 'metadata.json').read_text())
    if metadata['status'] != 'complete':
        raise ValueError('comparison incomplete')
    signatures = json.loads((directory / 'signature.json').read_text())
    cases = {row['input']: row for row in signatures}
    if len(cases) != len(signatures) or any(row['errors'] for row in signatures):
        raise ValueError('invalid signatures')
    groups = {name: [name] for name in cases}
    groups['overall'] = list(cases)
    groups['ffa'] = [name for name, row in cases.items() if len(row['group_sizes']) > 2 and all(n == 1 for n in row['group_sizes'])]
    groups['duel'] = [name for name, row in cases.items() if row['group_sizes'] == [1, 1]]
    groups['team'] = [name for name in cases if name not in groups['ffa'] and name not in groups['duel']]
    for name, row in cases.items():
        sizes = row['group_sizes']
        layout = 'ffa' + str(len(sizes)) if name in groups['ffa'] else 'v'.join(map(str, sizes))
        groups.setdefault('layout_' + layout, []).append(name)
    for family in ('plain', 'boss', 'generated', 'stress'):
        selected = [name for name in cases if name.startswith(family + '__')]
        if selected:
            groups[family] = selected
            for mode in ('ffa', 'duel', 'team'):
                groups[family + '_' + mode] = [name for name in selected if name in groups[mode]]
    records = json.loads((directory / 'records.json').read_text())
    paired = {}
    for row in records:
        key = (row['round'], row['variant'])
        if key in paired or row['variant'] not in ('a', 'b') or row['warmup'] != (row['round'] == 0):
            raise ValueError('duplicate or invalid record')
        paired[key] = row
    expected = {(i, v) for i in range(metadata['measured_rounds'] + 1) for v in ('a', 'b')}
    if set(paired) != expected:
        raise ValueError('missing paired rounds')
    result, raw = [], []
    for group, names in groups.items():
        if not names:
            continue
        latencies, throughputs = [], []
        for i in range(metadata['measured_rounds'] + 1):
            a, b = (sum(paired[i, v]['seconds'][name] for name in names) for v in ('a', 'b'))
            if not math.isfinite(a) or not math.isfinite(b) or not a > 0 or not b > 0:
                raise ValueError('nonpositive duration')
            latency, throughput = (b / a - 1) * 100, (a / b - 1) * 100
            raw.append(dict(group=group, round=i, warmup=i == 0, a_seconds=a, b_seconds=b,
                            latency_change_pct=latency, throughput_gain_pct=throughput))
            if i:
                latencies.append(latency)
                throughputs.append(throughput)
        middle = median(latencies)
        result.append(dict(group=group, cases=len(names), rounds=len(latencies),
                           latency_change_pct=middle, throughput_gain_pct=median(throughputs),
                           latency_mad_pp=median(abs(value - middle) for value in latencies),
                           faster_rounds=sum(value < 0 for value in latencies)))
    return result, raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    summary, raw = summarize(args.directory)
    args.out.mkdir(parents=True, exist_ok=True)
    for filename, rows in [('summary.csv', summary), ('paired-rounds.csv', raw)]:
        with (args.out / filename).open('w', newline='') as handle:
            writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
    print(json.dumps([row for row in summary if not row['group'].endswith('.txt')], indent=2))


if __name__ == '__main__':
    main()
