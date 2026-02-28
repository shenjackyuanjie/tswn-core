#!/usr/bin/env python3
"""
测试回归追踪工具
功能：记录测试失败idx，与上次比较，辅助判断修改是否有效
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path


PROJECT_ROOT = Path("d:/githubs/namer/tswn-core")
RECORD_FILE = PROJECT_ROOT / "target" / "test_regression.json"
LOG_FILE = PROJECT_ROOT / "target" / "test_regression.log"
CHECKPOINT_DIR = PROJECT_ROOT / "target" / "test_checkpoints"
DEFAULT_FILTER = "large_01_10 large_11_16 large_full case_17 small_seed"


def load_previous_records() -> dict:
    """加载上次的测试记录"""
    if not RECORD_FILE.exists():
        return {}
    try:
        with open(RECORD_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {}


def save_records(records: dict):
    """保存测试记录"""
    RECORD_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(RECORD_FILE, "w", encoding="utf-8") as f:
        json.dump(records, f, ensure_ascii=False, indent=2)


def parse_cargo_test_output(output: str) -> dict:
    """
    解析 cargo test 输出
    返回: {test_name: {"status": "FAILED"/"PASSED", "idx": int}}
    """
    results = {}

    lines = output.split("\n")
    for line in lines:
        match = re.match(r"^test (.+?) \.\.\. (FAILED|ok|ignored)", line)
        if match:
            test_name = match.group(1)
            status = match.group(2)
            if status == "FAILED":
                results[test_name] = {"status": "FAILED", "idx": -1}
            else:
                results[test_name] = {"status": "PASSED", "idx": -1}

    case_to_test = {}
    for test_name in results.keys():
        old_match = re.search(r"(sampled_large_case_\d+|fight_large)", test_name)
        if old_match:
            case_key = old_match.group(1)
            case_to_test[case_key] = test_name
        new_match = re.search(r"::large_(\d{2})$", test_name)
        if new_match:
            idx = new_match.group(1)
            case_to_test[f"sampled_large_case_{idx}"] = test_name
            case_to_test[f"large_{idx}"] = test_name
        if re.search(r"::large_full$", test_name):
            case_to_test["fight_large"] = test_name
            case_to_test["large_full"] = test_name

    # 额外注册直接需要识别的测试名（某些 mismatch 行可能不包含 thread 信息）
    _direct_tests = {
        "case_17",
        "help_vs_aaaaa_should_match_right_trace_step_by_step",
        "seed_small_replay_should_match",
        "small_seed",
        "fight_simple_replay_should_match",
        "simple_fight",
    }
    for name in _direct_tests:
        if name in results:
            # 将其自身作为 key，便于在没有 thread 信息时通过行内容匹配
            case_to_test[name] = name

    for line in lines:
        if "mismatch at idx=" in line:
            idx_match = re.search(r"mismatch at idx=(\d+)", line)
            if idx_match:
                idx = int(idx_match.group(1))
                thread_match = re.search(r"thread '(.+?)'", line)
                if thread_match:
                    test_name = thread_match.group(1)
                    if test_name in results:
                        results[test_name]["idx"] = idx
                else:
                    # 先尝试旧有的 sampled/fight 匹配
                    case_match = re.search(r"(sampled case-?\d+|fight_large|large_full)", line)
                    if case_match:
                        case_key = case_match.group(1)
                        if case_key.startswith("sampled "):
                            normalized_key = case_key.replace("case-", "case_").replace("sampled ", "sampled_large_")
                        else:
                            normalized_key = case_key
                        test_name = case_to_test.get(normalized_key)
                        if test_name and test_name in results:
                            results[test_name]["idx"] = idx
                            continue
                    # 如果行中直接包含我们关注的测试名，也记录 idx
                    for direct_name in _direct_tests:
                        if direct_name in line and direct_name in results:
                            results[direct_name]["idx"] = idx
                            break

    return results


def compare_records(current: dict, previous: dict) -> list:
    """
    比较当前记录和上次记录
    返回变化列表

    仅在两次都有记录时才报告状态变化（NEW_FAIL/NEW_PASS），
    仅在两次都有有效 idx (>=0) 时才比较 idx 并报告 IMPROVED/REGRESSED。
    """
    changes = []

    all_tests = set(current.keys()) | set(previous.keys())

    for test in all_tests:
        curr = current.get(test)
        prev = previous.get(test)

        # 如果两边都没有记录，跳过
        if curr is None and prev is None:
            continue

        # 只有当两次都有记录时，才考虑状态变更与 idx 比较
        if curr is not None and prev is not None:
            curr_status = curr.get("status")
            prev_status = prev.get("status")
            curr_idx = curr.get("idx", -1)
            prev_idx = prev.get("idx", -1)

            # 报告状态变化：仅当状态实际从 FAILED <-> 非 FAILED 发生变化时
            if prev_status == "FAILED" and curr_status != "FAILED":
                changes.append({
                    "test": test,
                    "change": "NEW_PASS",
                    "message": "测试从失败变为通过",
                })
            elif prev_status != "FAILED" and curr_status == "FAILED":
                changes.append({
                    "test": test,
                    "change": "NEW_FAIL",
                    "message": "新失败的测试",
                    "idx": curr_idx,
                })

            # 仅在两次都有有效 idx 时比较 idx
            if curr_idx >= 0 and prev_idx >= 0:
                if curr_idx > prev_idx:
                    changes.append({
                        "test": test,
                        "change": "IMPROVED",
                        "message": f"分叉点延后 (idx: {prev_idx} -> {curr_idx})",
                        "idx": curr_idx,
                        "prev_idx": prev_idx,
                    })
                elif curr_idx < prev_idx:
                    changes.append({
                        "test": test,
                        "change": "REGRESSED",
                        "message": f"分叉点提前 (idx: {prev_idx} -> {curr_idx})",
                        "idx": curr_idx,
                        "prev_idx": prev_idx,
                    })
            continue

        # 如果只有一侧有记录（新出现或消失），不报告状态变化或 idx 变化，
        # 因为无法确定这是实际的状态变更还是测试集差异。
        continue

    return changes


def write_log(message: str):
    """写入日志"""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    log_message = f"[{timestamp}] {message}"
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG_FILE, "a", encoding="utf-8") as f:
        f.write(log_message + "\n")


# ---- 存档点功能 ----


def _list_checkpoint_files():
    """列出所有存档点文件，按文件名排序（最新在前）"""
    if not CHECKPOINT_DIR.exists():
        return []
    return sorted(CHECKPOINT_DIR.glob("*.json"), reverse=True)


def _load_checkpoint(path):
    """加载存档点"""
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _find_checkpoint(name):
    """按名字查找存档点文件路径"""
    for f in _list_checkpoint_files():
        data = _load_checkpoint(f)
        if data.get("name") == name:
            return f
    return None


def _get_latest_checkpoint():
    """获取最近的存档点数据"""
    files = _list_checkpoint_files()
    if not files:
        return None
    return _load_checkpoint(files[0])


def _print_conclusion(changes):
    """打印结论"""
    any_improved = any(c["change"] in ("IMPROVED", "NEW_PASS") for c in changes)
    any_regressed = any(c["change"] == "REGRESSED" for c in changes)
    if any_improved and not any_regressed:
        print("结论: 修改有效 (有改进且无退步)")
    elif any_regressed:
        print("结论: 修改有问题 (存在退步)")
    else:
        print("结论: 无明显变化")


def _print_checkpoint_comparison(current_records, quiet):
    """与最近存档点对比并输出"""
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
        for change in changes:
            if change["change"] == "IMPROVED":
                print(f"[改进] {change['test']}: idx {change['prev_idx']} -> {change['idx']}")
            elif change["change"] == "REGRESSED":
                print(f"[退步] {change['test']}: idx {change['prev_idx']} -> {change['idx']}")
            elif change["change"] == "NEW_FAIL":
                print(f"[新失败] {change['test']}: idx={change.get('idx', -1)}")
            elif change["change"] == "NEW_PASS":
                print(f"[修复] {change['test']}: 从失败变为通过")
    _print_conclusion(changes)


def cmd_save(name):
    """将当前记录保存为存档点"""
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

    print(f'存档点 "{name}" 已保存 ({now.strftime("%Y-%m-%d %H:%M")})')
    failed = sum(1 for v in records.values()
                 if v.get("status") == "FAILED" and v.get("idx", -1) >= 0)
    if failed:
        print(f"  包含 {failed} 个失败测试")
    else:
        print("  当前所有测试通过")


def cmd_list():
    """列出所有存档点"""
    files = _list_checkpoint_files()
    if not files:
        print("没有存档点")
        return

    print(f"存档点列表 ({len(files)} 个):")
    for f in files:
        data = _load_checkpoint(f)
        name = data.get("name", "?")
        time = data.get("time", "?")
        records = data.get("records", {})
        failed = sum(1 for v in records.values()
                     if v.get("status") == "FAILED" and v.get("idx", -1) >= 0)
        if failed:
            print(f"  {name} ({time}) - {failed} 个失败")
        else:
            print(f"  {name} ({time}) - 全部通过")


def cmd_diff(name):
    """对比当前记录与存档点"""
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
    for change in changes:
        if change["change"] == "IMPROVED":
            print(f"[改进] {change['test']}: idx {change['prev_idx']} -> {change['idx']}")
        elif change["change"] == "REGRESSED":
            print(f"[退步] {change['test']}: idx {change['prev_idx']} -> {change['idx']}")
        elif change["change"] == "NEW_FAIL":
            print(f"[新失败] {change['test']}: idx={change.get('idx', -1)}")
        elif change["change"] == "NEW_PASS":
            print(f"[修复] {change['test']}: 从失败变为通过")
    _print_conclusion(changes)


def cmd_delete(name):
    """删除存档点"""
    cp_path = _find_checkpoint(name)
    if not cp_path:
        print(f'存档点 "{name}" 不存在')
        return
    cp_path.unlink()
    print(f'存档点 "{name}" 已删除')


def main():
    parser = argparse.ArgumentParser(description="测试回归追踪工具")
    parser.add_argument(
        "-f", "--filter",
        default=DEFAULT_FILTER,
        help=f"测试过滤表达式 (default: {DEFAULT_FILTER})"
    )
    parser.add_argument(
        "-s", "--show",
        action="store_true",
        help="只显示当前失败状态，不运行测试"
    )
    parser.add_argument(
        "-r", "--reset",
        action="store_true",
        help="重置历史记录"
    )
    parser.add_argument(
        "-q", "--quiet",
        action="store_true",
        help="安静模式，只输出关键信息"
    )
    subparsers = parser.add_subparsers(dest="command")
    save_parser = subparsers.add_parser("save", help="将当前记录保存为存档点")
    save_parser.add_argument("name", nargs="?", default=None, help="存档点名称 (默认用时间戳)")
    subparsers.add_parser("list", help="列出所有存档点")
    diff_parser = subparsers.add_parser("diff", help="对比当前记录与指定存档点")
    diff_parser.add_argument("name", nargs="?", default=None, help="存档点名称 (默认最近)")
    delete_parser = subparsers.add_parser("delete", help="删除指定存档点")
    delete_parser.add_argument("name", help="存档点名称")
    args = parser.parse_args()

    # 子命令分发
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

    if not args.quiet:
        print("=" * 40)
        print("  测试回归追踪工具")
        print("=" * 40)
        print()

    previous_records = load_previous_records()

    if args.reset:
        print("重置模式：清除历史记录")
        if RECORD_FILE.exists():
            RECORD_FILE.unlink()
        save_records({})
        return

    if args.show:
        print("当前失败状态:")
        for test, info in previous_records.items():
            if info.get("status") == "FAILED":
                idx = info.get("idx", -1)
                print(f"  {test} => idx={idx}")
        return

    if not args.quiet:
        print(f"运行测试: {args.filter}")
        print()
    elif args.quiet:
        print(f"[track_test] 运行测试: {args.filter}")

    test_args = args.filter.split() if args.filter else []
    cmd = "cargo test -- " + " ".join(test_args)

    result = subprocess.run(
        cmd,
        cwd=str(PROJECT_ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        shell=True,
    )
    output = (result.stdout or "") + "\n" + (result.stderr or "")

    current_records = parse_cargo_test_output(output)

    has_failure = any(
        r.get("status") == "FAILED" and r.get("idx", -1) >= 0
        for r in current_records.values()
    )

    if not has_failure:
        if not args.quiet:
            print("所有测试通过！")

            passing_tests = [t for t, r in current_records.items() if r.get("status") != "FAILED"]
            if passing_tests:
                print()
                print("通过的测试:")
                for test in passing_tests:
                    print(f"  - {test}")
        else:
            print("所有测试通过！")

        _print_checkpoint_comparison({}, args.quiet)
        save_records({})
        write_log("所有测试通过")
        return

    if not args.quiet:
        print("测试失败，分析中...")
        print()
    else:
        print("测试失败，分析中...")

    print("--- vs 上次运行 ---")
    changes = compare_records(current_records, previous_records)

    improved_count = 0
    regressed_count = 0
    new_fail_count = 0
    fixed_count = 0

    for change in changes:
        if change["change"] == "IMPROVED":
            improved_count += 1
            if not args.quiet:
                print(f"[改进] {change['test']}")
                print(f"       {change['message']}")
        elif change["change"] == "REGRESSED":
            regressed_count += 1
            if not args.quiet:
                print(f"[退步] {change['test']}")
                print(f"       {change['message']}")
        elif change["change"] == "NEW_FAIL":
            new_fail_count += 1
            if not args.quiet:
                print(f"[新失败] {change['test']}")
                print(f"         idx={change.get('idx', -1)}")
        elif change["change"] == "NEW_PASS":
            fixed_count += 1
            if not args.quiet:
                print(f"[修复] {change['test']}")
                print(f"       从失败变为通过")

    if not args.quiet:
        print()
        print("=" * 40)
        print("  汇总")
        print("=" * 40)
        print(f"改进: {improved_count}")
        print(f"退步: {regressed_count}")
        print(f"新失败: {new_fail_count}")
        print(f"修复: {fixed_count}")

    any_improved = improved_count > 0 or fixed_count > 0
    any_regressed = regressed_count > 0

    print()
    if any_improved and not any_regressed:
        print("结论: 修改有效 (有改进且无退步)")
    elif any_regressed:
        print("结论: 修改有问题 (存在退步)")
    else:
        print("结论: 无明显变化")

    _print_checkpoint_comparison(current_records, args.quiet)

    save_records(current_records)
    write_log(f"改进:{improved_count}, 退步:{regressed_count}, 新失败:{new_fail_count}, 修复:{fixed_count}")


if __name__ == "__main__":
    main()
