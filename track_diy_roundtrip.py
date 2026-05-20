#!/usr/bin/env python3
"""
DIY 往返验证工具 — 验证 DIY 玩家的初始状态（八围、技能等级）与原始玩家一致。

工作流：
1. 从 tswn_case_miner 或已有目录读取 case 的 input.txt
2. 对每个玩家运行 `tswn-cli to-diy` 获取 DIY 格式名
3. 对比原始玩家和 DIY 玩家 build 后的初始状态
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent
CLI_EXE = PROJECT_ROOT / "target" / "debug" / "tswn-cli.exe"
if not CLI_EXE.exists():
    CLI_EXE = PROJECT_ROOT / "target" / "release" / "tswn-cli.exe"

DEFAULT_OUT_DIR = PROJECT_ROOT / "target" / "diy_roundtrip"
DEFAULT_LIBRARY = PROJECT_ROOT / "tests" / "sqp6000.txt"
DEFAULT_FIGHT_TIMEOUT = 60


def run_cli(*args, timeout=30):
    """运行 tswn-cli，返回 stdout。"""
    try:
        result = subprocess.run(
            [str(CLI_EXE)] + list(args),
            capture_output=True,
            timeout=timeout,
            cwd=str(PROJECT_ROOT),
            encoding="utf-8",
            errors="replace",
        )
        return result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return "", "TIMEOUT"
    except Exception as e:
        return "", str(e)


# ---- 玩家状态解析 ----

STATUS_RE = re.compile(
    r"-\s*(.+?)\s*\(id=(\d+)\):\s*HP=(\d+)/(\d+),\s*move_point:(\d+)\s*"
    r"ATK=(\d+),\s*DEF=(\d+),\s*SPD=(\d+),\s*AGI=(\d+),\s*MAG=(\d+),\s*"
    r"MP=(\d+),\s*MDF=(\d+),\s*ITL=(\d+),\s*all_sum=(\d+)\s*"
    r"[^\d]*:\s*([\d.\-]+)"
)


def parse_player_status(line: str) -> dict | None:
    """解析 status 行，返回 dict。"""
    m = STATUS_RE.search(line)
    if not m:
        return None
    return {
        "name": m.group(1),
        "id": int(m.group(2)),
        "hp": int(m.group(3)),
        "max_hp": int(m.group(4)),
        "move_point": int(m.group(5)),
        "atk": int(m.group(6)),
        "def": int(m.group(7)),
        "spd": int(m.group(8)),
        "agi": int(m.group(9)),
        "mag": int(m.group(10)),
        "mp": int(m.group(11)),
        "mdf": int(m.group(12)),
        "itl": int(m.group(13)),
        "all_sum": int(m.group(14)),
        "name_factor": float(m.group(15)),
    }


def get_player_statuses(input_text: str) -> list[dict]:
    """通过运行 fight 获取每个玩家的初始状态。"""
    tmp = DEFAULT_OUT_DIR / "_tmp_status_input.txt"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    tmp.write_text(input_text, encoding="utf-8")
    stdout, _ = run_cli("fight", "-f", str(tmp), timeout=30)
    if not stdout:
        return []
    statuses = []
    for line in stdout.split("\n"):
        s = parse_player_status(line)
        if s:
            statuses.append(s)
    return statuses


def get_fight_lines(input_text: str, timeout: int) -> tuple[list[str], str | None]:
    """运行对战并返回日志行列表。"""
    tmp = DEFAULT_OUT_DIR / "_tmp_fight_input.txt"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    tmp.write_text(input_text, encoding="utf-8")
    stdout, stderr = run_cli("fight", "--out-raw", "-f", str(tmp), timeout=timeout)
    if not stdout:
        return [], stderr or "对战输出为空"
    return stdout.splitlines(), None


# ---- 玩家名解析 ----

def parse_players_from_input(input_text: str) -> list[list[str]]:
    """解析 namerena 输入，返回 groups。"""
    groups = []
    current = []
    for line in input_text.strip().split("\n"):
        line = line.strip()
        if line == "":
            if current:
                groups.append(current)
                current = []
        else:
            current.append(line)
    if current:
        groups.append(current)
    return groups


def get_diy_name(player_raw: str) -> str | None:
    """调用 tswn-cli to-diy 获取 DIY/OL 格式名字（优先 ol）。"""
    stdout, stderr = run_cli("to-diy", player_raw, timeout=15)
    if not stdout:
        return None
    fallback = None
    for line in stdout.split("\n"):
        line = line.strip()
        if "+ol:" in line:
            return line
        if "+diy[" in line:
            fallback = line
    return fallback


def build_diy_input(original_input: str) -> tuple[str, dict[str, str], list[str]]:
    """将原始输入转换为 DIY 版输入。返回 (diy_input, name_map, errors)。"""
    groups = parse_players_from_input(original_input)
    name_map = {}
    errors = []

    diy_groups = []
    for group in groups:
        diy_group = []
        for player in group:
            diy_name = get_diy_name(player)
            if diy_name:
                name_map[player] = diy_name
                diy_group.append(diy_name)
            else:
                errors.append(f"无法转换: {player[:60]}...")
                diy_group.append(player)
        diy_groups.append(diy_group)

    lines = []
    for i, group in enumerate(diy_groups):
        for player in group:
            lines.append(player)
        if i < len(diy_groups) - 1:
            lines.append("")

    return "\n".join(lines), name_map, errors


# ---- 比对逻辑 ----

def compare_statuses(orig_statuses: list[dict], diy_statuses: list[dict]) -> list[str]:
    """比对两组玩家状态，返回差异列表。"""
    diffs = []
    if len(orig_statuses) != len(diy_statuses):
        diffs.append(f"玩家数量不同: orig={len(orig_statuses)}, diy={len(diy_statuses)}")
        return diffs

    def index_by_id(items: list[dict]) -> dict[int, dict] | None:
        indexed = {}
        for item in items:
            pid = item.get("id")
            if pid is None or pid in indexed:
                return None
            indexed[pid] = item
        return indexed

    orig_by_id = index_by_id(orig_statuses)
    diy_by_id = index_by_id(diy_statuses)

    if orig_by_id is None or diy_by_id is None:
        diffs.append("玩家 id 缺失或重复，无法按 id 对齐")
        return diffs

    orig_ids = sorted(orig_by_id.keys())
    diy_ids = sorted(diy_by_id.keys())
    if orig_ids != diy_ids:
        diffs.append(f"玩家 id 集合不同: orig={orig_ids}, diy={diy_ids}")
        return diffs

    for pid in orig_ids:
        os_ = orig_by_id[pid]
        ds = diy_by_id[pid]
        # 比对关键字段
        fields = ["hp", "max_hp", "atk", "def", "spd", "agi", "mag", "mdf", "itl", "all_sum"]
        for f in fields:
            ov = os_.get(f)
            dv = ds.get(f)
            if ov != dv:
                diffs.append(f"  player[id={pid}] {f}: orig={ov}, diy={dv}")
        # name_factor
        nf_o = os_.get("name_factor", 0)
        nf_d = ds.get("name_factor", 0)
        if abs(nf_o - nf_d) > 0.001:
            diffs.append(
                f"  player[id={pid}] name_factor: orig={nf_o:.6f}, diy={nf_d:.6f}"
            )

    return diffs


def compare_fight_lines(orig_lines: list[str], diy_lines: list[str], context: int = 2) -> tuple[list[str], list[str]]:
    """比对对战过程日志，返回摘要 diff 和详细 diff 文本。"""
    if orig_lines == diy_lines:
        return [], []

    max_len = max(len(orig_lines), len(diy_lines))
    mismatch_idx = 0
    for i in range(max_len):
        o = orig_lines[i] if i < len(orig_lines) else None
        d = diy_lines[i] if i < len(diy_lines) else None
        if o != d:
            mismatch_idx = i
            break

    summary = [
        f"  fight: mismatch at line {mismatch_idx} (orig_lines={len(orig_lines)}, diy_lines={len(diy_lines)})"
    ]

    detail_lines = [
        f"orig_lines={len(orig_lines)}",
        f"diy_lines={len(diy_lines)}",
        f"first_mismatch={mismatch_idx}",
        "",
    ]
    start = max(0, mismatch_idx - context)
    end = min(max_len, mismatch_idx + context + 1)
    for i in range(start, end):
        o = orig_lines[i] if i < len(orig_lines) else "<EOF>"
        d = diy_lines[i] if i < len(diy_lines) else "<EOF>"
        prefix = ">>" if i == mismatch_idx else "  "
        detail_lines.append(f"{prefix} [{i}] orig: {o}")
        detail_lines.append(f"{prefix} [{i}] diy : {d}")
    return summary, detail_lines


# ---- 主流程 ----

def run_case(orig_input: str, case_id: str, out_dir: Path, compare_fight: bool, fight_timeout: int) -> dict:
    """处理单个 case。"""
    result = {
        "case_id": case_id,
        "success": False,
        "diffs": [],
    }

    case_dir = out_dir / case_id
    case_dir.mkdir(parents=True, exist_ok=True)

    # 1. 生成 DIY 输入
    print(f"  [{case_id}] 生成 DIY 输入...")
    diy_input, name_map, errors = build_diy_input(orig_input)
    if errors:
        result["warnings"] = errors
    if not name_map:
        result["error"] = "无法生成任何 DIY 名字"
        return result

    (case_dir / "input_orig.txt").write_text(orig_input, encoding="utf-8")
    (case_dir / "input_diy.txt").write_text(diy_input, encoding="utf-8")

    # 2. 获取原始玩家状态
    print(f"  [{case_id}] 获取原始状态...")
    orig_statuses = get_player_statuses(orig_input)
    if not orig_statuses:
        result["error"] = "原始对局无 status 输出"
        return result
    with open(case_dir / "status_orig.json", "w", encoding="utf-8") as f:
        json.dump(orig_statuses, f, ensure_ascii=False, indent=2)

    # 3. 获取 DIY 玩家状态
    print(f"  [{case_id}] 获取 DIY 状态...")
    diy_statuses = get_player_statuses(diy_input)
    if not diy_statuses:
        result["error"] = "DIY 对局无 status 输出"
        return result
    with open(case_dir / "status_diy.json", "w", encoding="utf-8") as f:
        json.dump(diy_statuses, f, ensure_ascii=False, indent=2)

    # 4. 比对
    diffs = compare_statuses(orig_statuses, diy_statuses)
    result["diffs"] = diffs
    result["success"] = len(diffs) == 0

    # 5. 对战过程比对
    if compare_fight:
        print(f"  [{case_id}] 比对对战过程...")
        fight_orig, err_orig = get_fight_lines(orig_input, fight_timeout)
        if err_orig:
            result["error"] = f"原始对战输出失败: {err_orig}"
            return result
        fight_diy, err_diy = get_fight_lines(diy_input, fight_timeout)
        if err_diy:
            result["error"] = f"DIY 对战输出失败: {err_diy}"
            return result

        (case_dir / "fight_orig.txt").write_text("\n".join(fight_orig), encoding="utf-8")
        (case_dir / "fight_diy.txt").write_text("\n".join(fight_diy), encoding="utf-8")

        fight_diffs, fight_detail = compare_fight_lines(fight_orig, fight_diy)
        if fight_detail:
            (case_dir / "fight_diff.txt").write_text("\n".join(fight_detail), encoding="utf-8")
        if fight_diffs:
            diffs.extend(fight_diffs)
            result["success"] = False

    if diffs:
        (case_dir / "diff.txt").write_text("\n".join(diffs), encoding="utf-8")

    result["player_count"] = len(orig_statuses)
    return result


def main():
    parser = argparse.ArgumentParser(description="DIY 往返验证工具")
    parser.add_argument("--cases-dir", type=Path, help="已有 case 目录")
    parser.add_argument("--library", type=Path, default=DEFAULT_LIBRARY, help="号库文件")
    parser.add_argument("--max-cases", type=int, default=64, help="最大 case 数")
    parser.add_argument("--case-offset", type=int, default=0, help="跳过前 N 个 case")
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUT_DIR, help="输出目录")
    parser.add_argument("--skip-fight", action="store_true", help="跳过对战过程比对（仅比初始状态）")
    parser.add_argument("--fight-timeout", type=int, default=DEFAULT_FIGHT_TIMEOUT, help="单场对战超时（秒）")
    parser.add_argument("-q", "--quiet", action="store_true", help="安静模式")
    args = parser.parse_args()

    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    cases = []
    if args.cases_dir and args.cases_dir.exists():
        for case_dir in sorted(args.cases_dir.iterdir()):
            if not case_dir.is_dir():
                continue
            input_file = case_dir / "input.txt"
            if input_file.exists():
                cases.append((case_dir.name, input_file.read_text(encoding="utf-8")))
    else:
        # 从号库生成简单 case
        if not args.library.exists():
            print(f"错误: 号库文件不存在: {args.library}")
            sys.exit(1)
        # 直接从号库读取名字，生成 1v1 case
        names = []
        with open(args.library, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#"):
                    names.append(line)
        # 生成简单 1v1 case
        for i in range(0, min(len(names) - 1, args.max_cases * 2), 2):
            if i + 1 >= len(names):
                break
            case_id = f"case_{i//2:04d}"
            input_text = f"{names[i]}\n\n{names[i+1]}"
            cases.append((case_id, input_text))

    if not cases:
        print("错误: 没有可用的 case")
        sys.exit(1)

    cases = cases[args.case_offset:args.case_offset + args.max_cases]
    print(f"共 {len(cases)} 个 case 待测试")

    results = []
    passed = 0
    failed = 0
    errors = 0

    for i, (case_id, orig_input) in enumerate(cases):
        print(f"\n[{i+1}/{len(cases)}] {case_id}")
        result = run_case(orig_input, case_id, out_dir, not args.skip_fight, args.fight_timeout)
        results.append(result)

        if result.get("error"):
            errors += 1
            print(f"  错误: {result['error']}")
        elif result["success"]:
            passed += 1
            print(f"  通过 ({result.get('player_count', 0)} 玩家一致)")
        else:
            failed += 1
            print(f"  失败 ({len(result['diffs'])} 处差异)")
            for diff in result["diffs"][:6]:
                print(diff)

    summary = {
        "time": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "results": {"total": len(cases), "passed": passed, "failed": failed, "errors": errors},
        "failed_cases": [
            {"case_id": r["case_id"], "diffs": r.get("diffs", [])}
            for r in results if not r["success"] and not r.get("error")
        ],
    }

    summary_path = out_dir / "summary.json"
    with open(summary_path, "w", encoding="utf-8") as f:
        json.dump(summary, f, ensure_ascii=False, indent=2)

    print(f"\n{'='*50}")
    print(f"总计: {len(cases)} | 通过: {passed} | 失败: {failed} | 错误: {errors}")
    print(f"Summary: {summary_path}")
    sys.exit(0 if failed == 0 and errors == 0 else 1)


if __name__ == "__main__":
    main()
