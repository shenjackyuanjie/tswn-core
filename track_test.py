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
        match = re.search(r"(sampled_large_case_\d+|fight_large)", test_name)
        if match:
            case_key = match.group(1)
            case_to_test[case_key] = test_name

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
                    case_match = re.search(r"(sampled case-?\d+|fight_large)", line)
                    if case_match:
                        case_key = case_match.group(1)
                        normalized_key = case_key.replace("case-", "case_").replace("sampled ", "sampled_large_")
                        test_name = case_to_test.get(normalized_key)
                        if test_name and test_name in results:
                            results[test_name]["idx"] = idx

    return results


def compare_records(current: dict, previous: dict) -> list:
    """
    比较当前记录和上次记录
    返回变化列表
    """
    changes = []

    all_tests = set(current.keys()) | set(previous.keys())

    for test in all_tests:
        curr = current.get(test)
        prev = previous.get(test)

        if curr is None:
            changes.append({
                "test": test,
                "change": "NEW_PASS",
                "message": "测试从失败变为通过",
            })
            continue

        if prev is None:
            changes.append({
                "test": test,
                "change": "NEW_FAIL",
                "message": "新失败的测试",
                "idx": curr.get("idx", -1),
            })
            continue

        curr_idx = curr.get("idx", -1)
        prev_idx = prev.get("idx", -1)

        if curr_idx > prev_idx:
            changes.append({
                "test": test,
                "change": "IMPROVED",
                "message": f"分叉点延后 (idx: {prev_idx} -> {curr_idx})",
                "idx": curr_idx,
                "prev_idx": prev_idx,
            })
        elif curr_idx < prev_idx and curr_idx >= 0:
            changes.append({
                "test": test,
                "change": "REGRESSED",
                "message": f"分叉点提前 (idx: {prev_idx} -> {curr_idx})",
                "idx": curr_idx,
                "prev_idx": prev_idx,
            })

    return changes


def write_log(message: str):
    """写入日志"""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    log_message = f"[{timestamp}] {message}"
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG_FILE, "a", encoding="utf-8") as f:
        f.write(log_message + "\n")


def main():
    parser = argparse.ArgumentParser(description="测试回归追踪工具")
    parser.add_argument(
        "-f", "--filter",
        default="sampled_large_case fight_large",
        help="测试过滤表达式 (default: sampled_large_case fight_large)"
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
    args = parser.parse_args()

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

        save_records({})
        write_log("所有测试通过")
        return

    if not args.quiet:
        print("测试失败，分析中...")
        print()
    else:
        print("测试失败，分析中...")

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

    save_records(current_records)
    write_log(f"改进:{improved_count}, 退步:{regressed_count}, 新失败:{new_fail_count}, 修复:{fixed_count}")


if __name__ == "__main__":
    main()
