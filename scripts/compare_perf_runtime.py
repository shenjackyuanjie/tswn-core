#!/usr/bin/env python3
"""逐输入串行配对确认：日志保留在本地，固定顺序，使用 API 内部耗时。

第 0 轮预热。每轮按排序后的 *.txt 输入执行，A/B 顺序仅由
(round + case_index) % 2 决定。不得与其他性能测试同时运行。
"""
import argparse
import datetime
import hashlib
import json
import math
from pathlib import Path
import subprocess
import time


SIGNATURE_KEYS = ('group_sizes', 'total', 'wins', 'errors', 'guard_exhausted')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    temporary.replace(path)


def parse_result(text, case, runs):
    rows = [json.loads(line) for line in text.splitlines() if line.strip()]
    if len(rows) != 1:
        raise ValueError(f'Expected exactly one result for {case.name}')
    row = rows[0]
    if Path(row['input']).resolve() != case:
        raise ValueError(f'Unexpected input for {case.name}')
    sizes = row['group_sizes']
    if (not isinstance(sizes, list) or not sizes or
            any(type(size) is not int or size <= 0 for size in sizes)):
        raise ValueError(f'Invalid group sizes for {case.name}')
    for key in SIGNATURE_KEYS[1:]:
        if type(row[key]) is not int or row[key] < 0:
            raise ValueError(f'Invalid counter {key} for {case.name}')
    if (row['total'] != runs or row['errors'] != 0 or row['wins'] > runs or
            row['guard_exhausted'] > runs or row['runs_per_matchup'] != runs or
            row['threads'] != 1 or row['mode'] != 'win-rate' or row['timed'] is not False):
        raise ValueError(f'Unexpected benchmark settings/counters for {case.name}')
    elapsed = row['elapsed_seconds']
    if isinstance(elapsed, bool) or not isinstance(elapsed, (int, float)) or not math.isfinite(elapsed) or elapsed <= 0:
        raise ValueError(f'Invalid elapsed time for {case.name}')
    signature = {'input': case.name, **{key: row[key] for key in SIGNATURE_KEYS}}
    return signature, elapsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--a', type=Path, required=True)
    parser.add_argument('--b', type=Path, required=True)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=52_000)
    parser.add_argument('--rounds', type=int, default=5)
    args = parser.parse_args()
    if not 1 <= args.runs <= 1_000_000 or args.rounds <= 0:
        parser.error('runs must be 1..1000000 and rounds must be positive')
    source = args.input.resolve()
    if not source.is_dir():
        parser.error('input must be a directory')
    cases = sorted(path.resolve() for path in source.glob('*.txt') if path.is_file())
    if not cases or len({path.name for path in cases}) != len(cases):
        parser.error('input requires nonempty, uniquely named txt cases')
    binaries = {'a': args.a.resolve(), 'b': args.b.resolve()}
    binary_hashes = {variant: digest(path) for variant, path in binaries.items()}
    input_hashes = {path.name: digest(path) for path in cases}
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    metadata = {
        'status': 'running',
        'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'runs': args.runs, 'measured_rounds': args.rounds, 'warmup_round': 0,
        'threads': 1, 'mode': 'win-rate', 'timed': False,
        'order_rule': 'sorted cases; a,b if (round+case_index)%2 == 0, otherwise b,a',
        'measurement': 'sum of perf_runtime elapsed_seconds; process wall stored separately',
        'binaries': {variant: {'path': str(path), 'sha256': binary_hashes[variant]}
                     for variant, path in binaries.items()},
        'input_sha256': input_hashes,
        'case_order': [case.name for case in cases],
        'script_sha256': digest(Path(__file__)),
    }
    write_json(out / 'metadata.json', metadata)
    records, expected = [], {}
    try:
        for round_index in range(args.rounds + 1):
            elapsed_by_variant = {'a': {}, 'b': {}}
            wall_by_variant = {'a': {}, 'b': {}}
            for case_index, case in enumerate(cases):
                order = ('a', 'b') if (round_index + case_index) % 2 == 0 else ('b', 'a')
                for variant in order:
                    if digest(case) != input_hashes[case.name] or digest(binaries[variant]) != binary_hashes[variant]:
                        raise RuntimeError('Input or binary changed during comparison')
                    log_dir = out / f'round-{round_index:02d}' / variant
                    log_dir.mkdir(parents=True, exist_ok=True)
                    stdout_path = log_dir / (case.name + '.stdout.jsonl')
                    stderr_path = log_dir / (case.name + '.stderr.txt')
                    command = [str(binaries[variant]), '-i', str(case), '-n', str(args.runs),
                               '--threads', '1', '--mode', 'win-rate']
                    with stdout_path.open('xb') as stdout, stderr_path.open('xb') as stderr:
                        started = time.perf_counter()
                        result = subprocess.run(command, stdout=stdout, stderr=stderr, check=False)
                        wall = time.perf_counter() - started
                    write_json(log_dir / (case.name + '.process.json'),
                               {'returncode': result.returncode, 'wall_seconds': wall})
                    if result.returncode != 0:
                        raise RuntimeError(f'Benchmark failed: round={round_index} variant={variant} case={case.name}')
                    signature, elapsed = parse_result(stdout_path.read_text(encoding='utf-8'), case, args.runs)
                    if case.name not in expected:
                        expected[case.name] = signature
                    elif signature != expected[case.name]:
                        raise RuntimeError(f'Behavior mismatch: round={round_index} variant={variant} case={case.name}')
                    elapsed_by_variant[variant][case.name] = elapsed
                    wall_by_variant[variant][case.name] = wall
            for variant in ('a', 'b'):
                seconds = dict(elapsed_by_variant[variant])
                seconds['overall'] = sum(seconds.values())
                seconds['ffa'] = sum(elapsed_by_variant[variant][name] for name, row in expected.items()
                                     if len(row['group_sizes']) > 2 and all(size == 1 for size in row['group_sizes']))
                records.append({'round': round_index, 'variant': variant, 'warmup': round_index == 0,
                                'seconds': seconds,
                                'process_wall_seconds': {'overall': sum(wall_by_variant[variant].values()),
                                                         'per_case': wall_by_variant[variant]}})
            write_json(out / 'signature.json', [expected[case.name] for case in cases])
            write_json(out / 'records.json', records)
            print(f'Completed paired round {round_index}/{args.rounds}', flush=True)
        metadata['status'] = 'complete'
    except BaseException as error:
        metadata['status'] = 'failed'
        metadata['error'] = str(error)
        raise
    finally:
        metadata['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        write_json(out / 'metadata.json', metadata)


if __name__ == '__main__':
    main()
