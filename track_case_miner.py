#!/usr/bin/env python3
"""
tswn_case_miner 回归追踪工具
功能：运行 miner，记录 failed case 集合与 first_mismatch_idx，并与上次/存档点比较
"""

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path


DEFAULT_MODES = "1v1,2v2,3v3v3,ffa"
DEFAULT_FFA_SIZES = "4,6,8"
DEFAULT_MAX_CASES_PER_MODE = 64

PROJECT_ROOT = Path(__file__).resolve().parent


def _read_first_line(path: Path):
    try:
        return path.read_text(encoding="utf-8").splitlines()[0].strip()
    except (OSError, UnicodeDecodeError, IndexError):
        return None


def _read_gitdir(git_entry: Path):
    raw = _read_first_line(git_entry)
    if not raw or not raw.startswith("gitdir:"):
        return None
    git_dir = Path(raw.split(":", 1)[1].strip())
    if git_dir.is_absolute():
        return git_dir.resolve()
    return (git_entry.parent / git_dir).resolve()


def _detect_git_common_dir(project_root: Path) -> Path:
    git_entry = project_root / ".git"
    if git_entry.is_dir():
        return git_entry.resolve()

    git_dir = _read_gitdir(git_entry)
    if git_dir is None:
        return git_entry

    common_dir_file = git_dir / "commondir"
    common_dir = _read_first_line(common_dir_file)
    if common_dir:
        common_dir_path = Path(common_dir)
        if common_dir_path.is_absolute():
            return common_dir_path.resolve()
        return (git_dir / common_dir_path).resolve()

    return git_dir.resolve()


GIT_COMMON_DIR = _detect_git_common_dir(PROJECT_ROOT)
SHARED_REPO_ROOT = GIT_COMMON_DIR.parent if GIT_COMMON_DIR.name == ".git" else PROJECT_ROOT
PROJECT_TARGET_DIR = PROJECT_ROOT / "target"
DEFAULT_OUT_DIR = Path("target") / "ts_diff_cases"
RECORD_FILE = PROJECT_TARGET_DIR / "case_miner_regression.json"
LOG_FILE = PROJECT_TARGET_DIR / "case_miner_regression.log"
CHECKPOINT_DIR = PROJECT_TARGET_DIR / "case_miner_checkpoints"
DEFAULT_LIBRARY = SHARED_REPO_ROOT / "tests" / "sqp6000.txt"
DEFAULT_SHARED_CACHE_DIR = SHARED_REPO_ROOT / "target" / "tswn_case_miner_cache"


def _detect_default_md5_tool():
    candidates = []
    for repo_root in (PROJECT_ROOT, SHARED_REPO_ROOT):
        candidates.append(repo_root.parent / "fast-namerena" / "branch" / "latest" / "out_md5.ts")
        candidates.append(repo_root / "fast-namerena" / "branch" / "latest" / "out_md5.ts")

    seen = set()
    for candidate in candidates:
        key = str(candidate.resolve()) if candidate.exists() else str(candidate)
        if key in seen:
            continue
        seen.add(key)
        if candidate.is_file():
            return candidate.resolve()
    return None


DEFAULT_MD5_TOOL = _detect_default_md5_tool()


def _resolve_runtime_path(path: Path) -> Path:
    if path.is_absolute():
        return path
    return (PROJECT_ROOT / path).resolve()


def load_previous_records() -> dict:
    if not RECORD_FILE.exists():
        return {}
    try:
        with open(RECORD_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def save_records(records: dict):
    RECORD_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(RECORD_FILE, "w", encoding="utf-8") as f:
        json.dump(records, f, ensure_ascii=False, indent=2)


def write_log(message: str):
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG_FILE, "a", encoding="utf-8") as f:
        f.write(f"[{timestamp}] {message}\n")


def _list_checkpoint_files():
    if not CHECKPOINT_DIR.exists():
        return []
    return sorted(CHECKPOINT_DIR.glob("*.json"), reverse=True)


def _load_checkpoint(path: Path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _find_checkpoint(name: str):
    for file in _list_checkpoint_files():
        data = _load_checkpoint(file)
        if data.get("name") == name:
            return file
    return None


def _get_latest_checkpoint():
    files = _list_checkpoint_files()
    if not files:
        return None
    return _load_checkpoint(files[0])


def load_summary(path: Path) -> dict:
    if not path.exists():
        raise FileNotFoundError(f"找不到 summary.json: {path}")
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def summarize_run(summary: dict, args) -> dict:
    failed_cases = {}
    for case in summary.get("failed_cases", []):
        failed_cases[case["id"]] = {
            "mode": case.get("mode", "?"),
            "idx": int(case.get("first_mismatch_idx", -1)),
            "diff_signature": case.get("diff_signature", ""),
            "input": case.get("input", ""),
            "ts": case.get("ts", ""),
            "rust": case.get("rust", ""),
            "diff": case.get("diff", ""),
            "meta": case.get("meta", ""),
        }

    return {
        "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "config": {
            "library": str(args.library_path) if args.library_path else None,
            "md5_tool": str(args.md5_tool_path) if args.md5_tool_path else None,
            "out_dir": str(args.out_dir_path),
            "shared_cache_dir": str(args.shared_cache_dir_path),
            "bun_cache_dir": str(args.bun_cache_dir_path),
            "modes": args.modes,
            "ffa_sizes": args.ffa_sizes,
            "max_cases_per_mode": args.max_cases_per_mode,
            "keep_going": args.keep_going,
        },
        "summary": {
            "total_generated": summary.get("total_generated", 0),
            "unique_inputs": summary.get("unique_inputs", 0),
            "executed": summary.get("executed", 0),
            "ts_failures": summary.get("ts_failures", 0),
            "rust_failures": summary.get("rust_failures", 0),
            "diff_failures": summary.get("diff_failures", 0),
            "deduped_diff_failures": summary.get("deduped_diff_failures", 0),
            "per_mode_generated": summary.get("per_mode_generated", {}),
            "per_mode_failures": summary.get("per_mode_failures", {}),
        },
        "failed_cases": failed_cases,
    }


def compare_records(current: dict, previous: dict) -> list:
    changes = []
    current_cases = current.get("failed_cases", {})
    previous_cases = previous.get("failed_cases", {})
    all_case_ids = sorted(set(current_cases.keys()) | set(previous_cases.keys()))

    for case_id in all_case_ids:
        curr = current_cases.get(case_id)
        prev = previous_cases.get(case_id)

        if curr is None and prev is not None:
            changes.append(
                {
                    "case": case_id,
                    "change": "FIXED_CASE",
                    "message": "failed case 已修复",
                    "mode": prev.get("mode", "?"),
                    "prev_idx": prev.get("idx", -1),
                }
            )
            continue

        if curr is not None and prev is None:
            changes.append(
                {
                    "case": case_id,
                    "change": "NEW_FAILED_CASE",
                    "message": "新增 failed case",
                    "mode": curr.get("mode", "?"),
                    "idx": curr.get("idx", -1),
                }
            )
            continue

        if curr is None or prev is None:
            continue

        curr_idx = curr.get("idx", -1)
        prev_idx = prev.get("idx", -1)
        if curr_idx > prev_idx:
            changes.append(
                {
                    "case": case_id,
                    "change": "IMPROVED",
                    "message": f"分叉点延后 (idx: {prev_idx} -> {curr_idx})",
                    "mode": curr.get("mode", "?"),
                    "idx": curr_idx,
                    "prev_idx": prev_idx,
                }
            )
        elif curr_idx < prev_idx:
            changes.append(
                {
                    "case": case_id,
                    "change": "REGRESSED",
                    "message": f"分叉点提前 (idx: {prev_idx} -> {curr_idx})",
                    "mode": curr.get("mode", "?"),
                    "idx": curr_idx,
                    "prev_idx": prev_idx,
                }
            )

    return changes


def print_current_status(records: dict):
    summary = records.get("summary", {})
    failed_cases = records.get("failed_cases", {})
    print("当前 miner 失败状态:")
    print(f"  diff_failures={summary.get('diff_failures', len(failed_cases))}")
    print(f"  deduped_diff_failures={summary.get('deduped_diff_failures', len(failed_cases))}")
    for mode, count in summary.get("per_mode_failures", {}).items():
        print(f"  {mode}: {count}")
    if failed_cases:
        print()
        print("failed cases:")
        for case_id, info in sorted(failed_cases.items()):
            print(f"  {case_id} => mode={info.get('mode', '?')} idx={info.get('idx', -1)}")


def _print_conclusion(changes):
    any_improved = any(c["change"] in ("IMPROVED", "FIXED_CASE") for c in changes)
    any_regressed = any(c["change"] in ("REGRESSED", "NEW_FAILED_CASE") for c in changes)
    if any_improved and not any_regressed:
        print("结论: 修改有效 (有改进且无退步)")
    elif any_regressed:
        print("结论: 修改有问题 (存在退步)")
    else:
        print("结论: 无明显变化")


def _print_checkpoint_comparison(current_records: dict, quiet: bool):
    cp = _get_latest_checkpoint()
    if cp is None:
        return

    cp_name = cp.get("name", "?")
    cp_time = cp.get("time", "?")
    cp_records = cp.get("records", {})
    changes = compare_records(current_records, cp_records)

    print()
    print(f'--- vs 存档点 "{cp_name}" ({cp_time}) ---')
    if not quiet:
        print_changes(changes)
    _print_conclusion(changes)


def print_changes(changes: list):
    for change in changes:
        if change["change"] == "IMPROVED":
            print(f"[改进] {change['case']} ({change['mode']}): idx {change['prev_idx']} -> {change['idx']}")
        elif change["change"] == "REGRESSED":
            print(f"[退步] {change['case']} ({change['mode']}): idx {change['prev_idx']} -> {change['idx']}")
        elif change["change"] == "NEW_FAILED_CASE":
            print(f"[新失败] {change['case']} ({change['mode']}): idx={change.get('idx', -1)}")
        elif change["change"] == "FIXED_CASE":
            print(f"[修复] {change['case']} ({change['mode']}): 上次 idx={change.get('prev_idx', -1)}")


def run_miner(args):
    cmd = [
        "cargo",
        "run",
        "--quiet",
        "--release",
        "--features",
        "no_debug",
        "--bin",
        "tswn_case_miner",
        "--",
        "--library",
        str(args.library_path),
        "--md5-tool",
        str(args.md5_tool_path),
        "--out-dir",
        str(args.out_dir_path),
        "--modes",
        args.modes,
        "--ffa-sizes",
        args.ffa_sizes,
        "--max-cases-per-mode",
        str(args.max_cases_per_mode),
    ]
    if args.keep_going:
        cmd.append("--keep-going")

    args.shared_cache_dir_path.mkdir(parents=True, exist_ok=True)
    args.bun_cache_dir_path.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["TSWN_CASE_MINER_TS_CACHE_DIR"] = str(args.shared_cache_dir_path / "ts_trace")
    env["TSWN_CASE_MINER_BUN_CACHE_DIR"] = str(args.bun_cache_dir_path)

    result = subprocess.run(
        cmd,
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
    )
    return result


def cmd_save(name):
    records = load_previous_records()
    now = datetime.now()
    if name is None:
        name = now.strftime("%Y%m%d_%H%M%S")

    existing = _find_checkpoint(name)
    if existing:
        print(f'存档点 "{name}" 已存在，覆盖')
        existing.unlink()

    timestamp = now.strftime("%Y%m%d_%H%M%S")
    filename = f"{timestamp}_{name}.json"
    CHECKPOINT_DIR.mkdir(parents=True, exist_ok=True)
    data = {
        "name": name,
        "time": now.strftime("%Y-%m-%d %H:%M:%S"),
        "records": records,
    }
    with open(CHECKPOINT_DIR / filename, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

    failed = len(records.get("failed_cases", {}))
    print(f'存档点 "{name}" 已保存 ({now.strftime("%Y-%m-%d %H:%M")})')
    print(f"  包含 {failed} 个 failed case")


def cmd_list():
    files = _list_checkpoint_files()
    if not files:
        print("没有存档点")
        return

    print(f"存档点列表 ({len(files)} 个):")
    for file in files:
        data = _load_checkpoint(file)
        name = data.get("name", "?")
        time = data.get("time", "?")
        failed = len(data.get("records", {}).get("failed_cases", {}))
        print(f"  {name} ({time}) - {failed} 个 failed case")


def cmd_diff(name):
    if name:
        cp_path = _find_checkpoint(name)
        if not cp_path:
            print(f'存档点 "{name}" 不存在')
            return
        cp_data = _load_checkpoint(cp_path)
    else:
        cp_data = _get_latest_checkpoint()
        if not cp_data:
            print("没有存档点")
            return

    current = load_previous_records()
    cp_records = cp_data.get("records", {})
    cp_name = cp_data.get("name", "?")
    cp_time = cp_data.get("time", "?")
    changes = compare_records(current, cp_records)

    print(f'--- vs 存档点 "{cp_name}" ({cp_time}) ---')
    print_changes(changes)
    _print_conclusion(changes)


def cmd_delete(name):
    cp_path = _find_checkpoint(name)
    if not cp_path:
        print(f'存档点 "{name}" 不存在')
        return
    cp_path.unlink()
    print(f'存档点 "{name}" 已删除')


def main():
    parser = argparse.ArgumentParser(description="tswn_case_miner 回归追踪工具")
    parser.add_argument("--library", type=Path, default=DEFAULT_LIBRARY, help=f"号库文件路径 (default: {DEFAULT_LIBRARY})")
    parser.add_argument(
        "--md5-tool",
        type=Path,
        default=DEFAULT_MD5_TOOL,
        help=f"out_md5.ts 路径 (default: {DEFAULT_MD5_TOOL if DEFAULT_MD5_TOOL else '自动推导'})",
    )
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR, help=f"miner 输出目录 (default: {DEFAULT_OUT_DIR})")
    parser.add_argument(
        "--shared-cache-dir",
        type=Path,
        default=DEFAULT_SHARED_CACHE_DIR,
        help=f"共享 bun/TS 缓存目录 (default: {DEFAULT_SHARED_CACHE_DIR})",
    )
    parser.add_argument("--modes", default=DEFAULT_MODES, help=f"对战模式 (default: {DEFAULT_MODES})")
    parser.add_argument("--ffa-sizes", default=DEFAULT_FFA_SIZES, help=f"ffa 人数列表 (default: {DEFAULT_FFA_SIZES})")
    parser.add_argument(
        "--max-cases-per-mode",
        type=int,
        default=DEFAULT_MAX_CASES_PER_MODE,
        help=f"每种模式最多生成多少 case (default: {DEFAULT_MAX_CASES_PER_MODE})",
    )
    parser.add_argument("--keep-going", action="store_true", help="单个 case 失败时继续")
    parser.add_argument("-s", "--show", action="store_true", help="只显示当前失败状态，不运行 miner")
    parser.add_argument("-r", "--reset", action="store_true", help="重置历史记录")
    parser.add_argument("-q", "--quiet", action="store_true", help="安静模式，只输出关键信息")
    subparsers = parser.add_subparsers(dest="command")
    save_parser = subparsers.add_parser("save", help="将当前记录保存为存档点")
    save_parser.add_argument("name", nargs="?", default=None, help="存档点名称 (默认用时间戳)")
    subparsers.add_parser("list", help="列出所有存档点")
    diff_parser = subparsers.add_parser("diff", help="对比当前记录与指定存档点")
    diff_parser.add_argument("name", nargs="?", default=None, help="存档点名称 (默认最近)")
    delete_parser = subparsers.add_parser("delete", help="删除指定存档点")
    delete_parser.add_argument("name", help="存档点名称")
    args = parser.parse_args()

    if args.command:
        if args.command == "save":
            cmd_save(args.name)
        elif args.command == "list":
            cmd_list()
        elif args.command == "diff":
            cmd_diff(args.name)
        elif args.command == "delete":
            cmd_delete(args.name)
        return

    if args.reset:
        if RECORD_FILE.exists():
            RECORD_FILE.unlink()
        save_records({})
        print("已重置 miner 历史记录")
        return

    if args.show:
        print_current_status(load_previous_records())
        return

    args.library_path = _resolve_runtime_path(args.library)
    args.out_dir_path = _resolve_runtime_path(args.out_dir)
    args.shared_cache_dir_path = _resolve_runtime_path(args.shared_cache_dir)
    args.bun_cache_dir_path = args.shared_cache_dir_path / "bun"
    args.md5_tool_path = _resolve_runtime_path(args.md5_tool) if args.md5_tool else None

    if not args.library_path.is_file():
        parser.error(f"号库文件不存在: {args.library_path}")
    if args.md5_tool_path is None:
        parser.error("运行 miner 时必须提供 --md5-tool，或保证默认 fast-namerena 路径可自动推导")
    if not args.md5_tool_path.is_file():
        parser.error(f"md5 工具文件不存在: {args.md5_tool_path}")

    if not args.quiet:
        print("=" * 40)
        print("  tswn_case_miner 回归追踪工具")
        print("=" * 40)
        print()
        print(f"运行 miner: library={args.library_path}")
        print(f"md5 tool: {args.md5_tool_path}")
        print(f"shared cache: {args.shared_cache_dir_path}")
        print(f"bun cache: {args.bun_cache_dir_path}")
        print()
    else:
        print(f"[track_case_miner] 运行 miner: {args.library_path}")

    result = run_miner(args)
    if result.returncode != 0:
        print("miner 运行失败")
        if result.stdout:
            print(result.stdout.rstrip())
        if result.stderr:
            print(result.stderr.rstrip())
        sys.exit(result.returncode)

    summary_path = args.out_dir_path / "summary.json"
    try:
        summary = load_summary(summary_path)
    except FileNotFoundError as exc:
        print(str(exc))
        sys.exit(1)

    current_records = summarize_run(summary, args)
    previous_records = load_previous_records()
    changes = compare_records(current_records, previous_records)

    if not args.quiet:
        print("--- vs 上次运行 ---")
        print_changes(changes)
        print()
        print("=" * 40)
        print("  汇总")
        print("=" * 40)
        print(f"failed case: {current_records['summary'].get('diff_failures', 0)}")
        print(f"deduped failed case: {current_records['summary'].get('deduped_diff_failures', 0)}")
        print(f"TS failures: {current_records['summary'].get('ts_failures', 0)}")
        print(f"Rust failures: {current_records['summary'].get('rust_failures', 0)}")
    else:
        print("--- vs 上次运行 ---")

    _print_conclusion(changes)
    _print_checkpoint_comparison(current_records, args.quiet)

    save_records(current_records)
    improved_count = sum(1 for c in changes if c["change"] == "IMPROVED")
    regressed_count = sum(1 for c in changes if c["change"] == "REGRESSED")
    fixed_count = sum(1 for c in changes if c["change"] == "FIXED_CASE")
    new_failed_count = sum(1 for c in changes if c["change"] == "NEW_FAILED_CASE")
    write_log(
        f"improved:{improved_count}, regressed:{regressed_count}, fixed:{fixed_count}, new_failed:{new_failed_count}, diff_failures:{current_records['summary'].get('diff_failures', 0)}"
    )


if __name__ == "__main__":
    main()
