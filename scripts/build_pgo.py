#!/usr/bin/env python3
"""PGO 专用构建：训练集必须与评估集分离；不会改变默认源码构建。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def input_files(directory):
    paths = sorted(Path(directory).resolve().glob('*.txt'))
    if not paths:
        raise ValueError(f'没有 txt 输入：{directory}')
    return paths


def normalized_roster(path):
    # 保守去除两端空白和换行差异；不把它当作完整格式语义解析器。
    lines = [line.strip() for line in path.read_text(encoding='utf-8-sig').splitlines()]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return '\n'.join(lines)


def source_fingerprint(repo):
    # 只存路径与内容散列，不保存源码或 dirty diff；每次重新枚举以检测新增/删除。
    paths = {repo / 'Cargo.toml', repo / 'Cargo.lock',
             repo / 'crates/tswn_core/examples/perf_runtime.rs',
             repo / 'crates/tswn_core/examples/pgo_train.rs'}
    paths.update((repo / 'crates').rglob('Cargo.toml'))
    paths.update((repo / 'crates/tswn_core/src').rglob('*.rs'))
    files = {p.relative_to(repo).as_posix(): digest(p) for p in sorted(paths)}
    encoded = json.dumps(files, sort_keys=True, separators=(',', ':')).encode('utf-8')
    return {'sha256': hashlib.sha256(encoded).hexdigest(), 'files': files}


def training_results(output, expected_names, start, runs):
    """验证完整 JSONL，保留 guard 计数并进行精确配对核对。"""
    keys = ('start_inclusive', 'end_exclusive', 'group_sizes', 'total', 'wins', 'errors', 'guard_exhausted')
    results = {}
    for line in output.splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        name = Path(row['input']).name
        if name in results or name not in expected_names:
            raise ValueError('训练输出包含重复或未知输入：' + name)
        values = {key: row[key] for key in keys}
        if any(type(values[key]) is not int or values[key] < 0 for key in keys if key != 'group_sizes'):
            raise ValueError('训练输出计数必须是非负整数：' + name)
        if (not isinstance(values['group_sizes'], list) or not values['group_sizes'] or
                any(type(size) is not int or size <= 0 for size in values['group_sizes'])):
            raise ValueError('训练输出组大小无效：' + name)
        if (values['start_inclusive'] != start or values['end_exclusive'] != start + runs or
                values['total'] != runs or values['errors'] != 0 or
                values['wins'] > runs or values['guard_exhausted'] > runs):
            raise ValueError('训练输出区间/计数不符合预期：' + name)
        results[name] = values
    if set(results) != set(expected_names):
        raise ValueError('训练输出缺少输入')
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path.cwd())
    parser.add_argument('--work-dir', type=Path, required=True)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--holdout', type=Path, action='append', required=True)
    parser.add_argument('--start', type=int, default=1_000_000)
    parser.add_argument('--runs', type=int, default=10_000)
    parser.add_argument('--eval-max-rounds', type=int, default=1_000_000,
                        help='评估 [0,n) 的最大 n；实际评估必须遵守此上限')
    parser.add_argument('--jobs', type=int, default=3)
    parser.add_argument('--offline', action='store_true')
    args = parser.parse_args()
    if args.eval_max_rounds <= 0 or args.start < max(1_000_000, args.eval_max_rounds) or args.runs <= 0 or args.jobs <= 0:
        parser.error('eval-max-rounds/runs/jobs 必须为正数，start 必须 >= eval-max-rounds')
    conflicting = [key for key in os.environ if (key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER',
                              'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTC_WRAPPER',
                              'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER') or
                   key.startswith('CARGO_PROFILE_RELEASE_') or
                   key.startswith('CARGO_TARGET_') and key.endswith('_RUSTFLAGS')) and os.environ[key]]
    if conflicting:
        parser.error('拒绝继承可能冲突的编译参数；请先显式清除：' + ', '.join(conflicting))
    repo, work = args.repo.resolve(), args.work_dir.resolve()
    training = input_files(args.input)
    holdouts = [p for directory in args.holdout for p in input_files(directory)]
    held = {tuple(sorted(line for line in normalized_roster(p).splitlines() if line and not line.startswith('seed:'))) for p in holdouts}
    if any(tuple(sorted(line for line in normalized_roster(p).splitlines() if line and not line.startswith('seed:'))) in held for p in training):
        parser.error('训练输入与 holdout 内容重复')
    if work.exists():
        parser.error('work-dir 必须是不存在的新目录；本工具不会删除或覆盖旧实验')
    work.mkdir(parents=True)
    raw = work / 'raw'
    raw.mkdir()
    record = {'purpose': 'PGO build specialization, not default source speedup',
              'commands': [], 'tools': {}, 'training_start': args.start,
              'evaluation_max_rounds': args.eval_max_rounds,
              'training_runs_per_case': args.runs, 'training': [{'name': p.name, 'sha256': digest(p)} for p in training],
              'holdout': [{'name': p.name, 'sha256': digest(p)} for p in holdouts]}
    metadata = work / 'build-metadata.json'

    def save():
        metadata.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    def run(command, label, env=None, capture=False):
        # 命令 argv 原样传递，不经 shell；只记录显式构建参数，不转储宿主环境。
        record['commands'].append({'label': label, 'argv': [str(x) for x in command]})
        save()
        with (work / f'{label}.stdout').open('w', encoding='utf-8') as out, (work / f'{label}.stderr').open('w', encoding='utf-8') as err:
            completed = subprocess.run([str(x) for x in command], cwd=repo, env=env, text=True, stdout=out, stderr=err)
        record['commands'][-1]['returncode'] = completed.returncode
        save()
        if completed.returncode:
            raise RuntimeError(f'{label} 失败；请检查独立日志，不继续训练/构建')
        return (work / f'{label}.stdout').read_text(encoding='utf-8').strip() if capture else None

    try:
        verbose = run(['rustc', '-vV'], 'rustc-version', capture=True)
        record['tools']['rustc'] = verbose
        host = next(line.split(': ', 1)[1] for line in verbose.splitlines() if line.startswith('host: '))
        sysroot = Path(run(['rustc', '--print', 'sysroot'], 'rustc-sysroot', capture=True))
        suffix = '.exe' if os.name == 'nt' else ''
        profdata = sysroot / 'lib' / 'rustlib' / host / 'bin' / ('llvm-profdata' + suffix)
        if not profdata.is_file():
            raise RuntimeError('当前 rustc 工具链缺 llvm-tools-preview；请显式 rustup component add llvm-tools-preview 后用新的 work-dir 重试')
        record['source_revision'] = run(['git', 'rev-parse', 'HEAD'], 'source-revision', capture=True)
        record['cargo_lock_sha256'] = digest(repo / 'Cargo.lock')
        record['training_source_sha256'] = digest(repo / 'crates/tswn_core/examples/pgo_train.rs')
        record['source_fingerprint'] = source_fingerprint(repo)

        def build(command, label, env):
            if source_fingerprint(repo) != record['source_fingerprint']:
                raise RuntimeError('实验源码已变化；拒绝继续构建')
            run(command, label, env)
            if source_fingerprint(repo) != record['source_fingerprint']:
                raise RuntimeError('构建期间源码已变化；拒绝继续实验')
        record['tools']['cargo'] = run(['cargo', '--version'], 'cargo-version', capture=True)
        record['tools']['llvm-profdata'] = run([profdata, '--version'], 'profdata-version', capture=True)
        # 不启用 native；保留 rustc 对该固定 target 的通用默认 CPU。
        base = []
        record['target'] = host
        record['base_rustflags'] = base
        common = ['cargo', 'build', '-p', 'tswn_core', '--release', '--features', 'no_debug', '--locked', '--target', host, '--jobs', str(args.jobs)]
        if args.offline:
            common.append('--offline')
        # 编码形式避免 Windows/空格路径被 Cargo 的 RUSTFLAGS 空白切分破坏；语义等同所列 RUSTFLAGS。
        def environment(flags, target):
            env = os.environ.copy()
            env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(base + flags)
            env['CARGO_TARGET_DIR'] = str(work / target)
            return env
        control_env = environment([], 'control-target')
        build(common + ['--example', 'perf_runtime', '--example', 'pgo_train', '--bin', 'tswn-cli'], 'control-build', control_env)
        training_args = ['--input', args.input.resolve(), '--start', str(args.start), '--runs', str(args.runs)]
        control_trainer = work / 'control-target' / host / 'release' / 'examples' / ('pgo_train' + suffix)
        control_output = run([control_trainer, *training_args], 'control-training', control_env, capture=True)
        expected_names = [p.name for p in training]
        control_results = training_results(control_output, expected_names, args.start, args.runs)
        generate = [f'-Cprofile-generate={raw}']
        train_env = environment(generate, 'generate-target')
        train_env['LLVM_PROFILE_FILE'] = str(raw / '%m_%p.profraw')
        record['generate_rustflags'] = base + generate
        save()
        build(common + ['--example', 'pgo_train'], 'generate-build', train_env)
        trainer = work / 'generate-target' / host / 'release' / 'examples' / ('pgo_train' + suffix)
        training_output = run([trainer, *training_args], 'training', train_env, capture=True)
        instrumented_results = training_results(training_output, expected_names, args.start, args.runs)
        if instrumented_results != control_results:
            raise RuntimeError('插桩训练与 control 的逐输入结果不一致；拒绝使用 profile')
        record['training_comparison'] = {'status': 'exact match', 'cases': control_results,
                                         'guard_policy': 'retained; exact equality required, no exclusions'}
        save()
        profiles = sorted(raw.glob('*.profraw'))
        if not profiles:
            raise RuntimeError('没有生成 profraw，不能继续')
        merged = work / 'merged.profdata'
        run([profdata, 'merge', '-o', merged, *profiles], 'merge-profile')
        use = [f'-Cprofile-use={merged}', '-Cllvm-args=-pgo-warn-missing-function']
        record['use_rustflags'] = base + use
        record['profile_sha256'] = digest(merged)
        save()
        use_env = environment(use, 'use-target')
        build(common + ['--example', 'perf_runtime', '--bin', 'tswn-cli'], 'use-build', use_env)
        binaries = [work / 'use-target' / host / 'release' / 'examples' / ('perf_runtime' + suffix),
                    work / 'use-target' / host / 'release' / ('tswn-cli' + suffix)]
        record['binaries'] = [{'variant': 'pgo', 'name': p.name, 'sha256': digest(p)} for p in binaries]
        control_binaries = [work / 'control-target' / host / 'release' / 'examples' / ('perf_runtime' + suffix),
                            work / 'control-target' / host / 'release' / ('tswn-cli' + suffix)]
        record['binaries'] += [{'variant': 'control', 'name': p.name, 'sha256': digest(p)} for p in control_binaries]
        record['status'] = 'built; E2E timing and regression tests still required'
        save()
        print('PGO 专用构建完成；尚未证明 E2E 改善。结果目录：', work)
    except Exception as error:
        record['status'] = 'failed'
        record['error'] = str(error)
        save()
        raise


if __name__ == '__main__':
    main()
