#!/usr/bin/env python3
"""定位 bun 官方 win_rate 与 tswn 胜率在具体 round 上的分叉。"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from json import JSONDecodeError
from pathlib import Path
from typing import Any

import find_bun_tswn_reply_mismatches as base


PROFILE_START = 33_554_31
DEFAULT_SKIP_ROW_ID = "Jm0MGgK4HfUAAQ4TRaxmg2ne69wB"


@dataclass(slots=True)
class BucketAnalysis:
    start_round: int
    end_round: int
    bun_start_wins: int
    bun_end_wins: int
    tswn_start_wins: int
    tswn_end_wins: int
    diff_before: int
    diff_after: int
    mismatch_rounds: list[int]


@dataclass(slots=True)
class CaseAnalysis:
    item: base.MismatchRow
    group1: str
    group2: str
    rounds: int
    md5_path_used: str
    md5_fallback_reason: str | None
    bun_cumulative_wins: list[int]
    tswn_cumulative_wins: list[int]
    mismatch_rounds: list[int]
    buckets: list[BucketAnalysis]


def parse_args(argv: list[str]) -> argparse.Namespace:
    repo_root = Path(__file__).resolve().parents[1]
    workspace_root = repo_root.parent
    parser = argparse.ArgumentParser(
        description="定位 bun 官方 win_rate 与 tswn 每局胜负分叉 round。",
    )
    parser.add_argument(
        "--dsn",
        default=os.environ.get("TSWN_PG_DSN"),
        help="PostgreSQL DSN；默认读取环境变量 TSWN_PG_DSN。",
    )
    parser.add_argument("--schema", default="eqq3695888")
    parser.add_argument("--table", default="messages")
    parser.add_argument("--sender-id", default="45620725")
    parser.add_argument("--content-like", default="最终胜率%tswn:%")
    parser.add_argument(
        "--skip-row-id",
        action="append",
        default=[DEFAULT_SKIP_ROW_ID],
        help="跳过的 mismatch row id，可重复传入。默认跳过最夸张的 1 条。",
    )
    parser.add_argument(
        "--case-id",
        action="append",
        default=[],
        help="只分析指定 case id，可重复传入。",
    )
    parser.add_argument(
        "--rounds",
        type=int,
        default=None,
        help="覆盖分析场数；默认沿用 bun 原消息最大 bucket 场数。",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=180,
        help="单个 bun/tswn 子进程超时秒数。",
    )
    parser.add_argument(
        "--md-output",
        default=str(repo_root / "docs" / "diff" / "bun_tswn_round_mismatches.md"),
        help="Markdown 报告输出路径。",
    )
    parser.add_argument(
        "--json-output",
        default=str(repo_root / "docs" / "diff" / "bun_tswn_round_mismatches.json"),
        help="JSON 报告输出路径。",
    )
    parser.add_argument(
        "--md5-path",
        default=str(workspace_root / "fast-namerena" / "branch" / "latest" / "md5.js"),
        help="md5.js 路径。",
    )
    parser.add_argument(
        "--md5-fallback",
        default=str(workspace_root / "fast-namerena" / "md5.js"),
        help="主 md5.js 失败时的 fallback 路径。",
    )
    parser.add_argument(
        "--bun-helper",
        default=str(repo_root / "scripts" / "bun_profile_trace.js"),
        help="Bun profile trace helper 路径。",
    )
    parser.add_argument(
        "--tswn-bin",
        default=None,
        help="tswn-cli 可执行文件路径；默认优先使用 target/release/tswn-cli(.exe)。",
    )
    return parser.parse_args(argv)


def ensure_utf8_stdio() -> None:
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8")


def resolve_tswn_bin(args: argparse.Namespace, repo_root: Path) -> list[str]:
    if args.tswn_bin:
        return [args.tswn_bin]

    exe_name = "tswn-cli.exe" if os.name == "nt" else "tswn-cli"
    release_bin = repo_root / "target" / "release" / exe_name
    if release_bin.exists():
        return [str(release_bin)]

    return ["cargo", "run", "--release", "--bin", "tswn-cli", "--"]


def load_cases(args: argparse.Namespace) -> tuple[list[base.MismatchRow], dict[str, base.MessageRow]]:
    schema_name = base.validate_identifier(args.schema, "schema")
    table_name = base.validate_identifier(args.table, "table")

    candidate_messages = base.fetch_candidate_messages(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        sender_id=args.sender_id,
        content_like=args.content_like,
    )
    mismatches = base.find_mismatches(candidate_messages)
    reply_ids = sorted({reply_id for item in mismatches for reply_id in item.reply_ids})
    replied_messages = base.fetch_messages_by_ids(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        message_ids=reply_ids,
    )
    mismatches = base.dedup_mismatches(mismatches, replied_messages)

    skip_ids = set(args.skip_row_id)
    if skip_ids:
        mismatches = [item for item in mismatches if item.message.row_id not in skip_ids]

    if args.case_id:
        wanted = set(args.case_id)
        mismatches = [item for item in mismatches if item.message.row_id in wanted]

    return mismatches, replied_messages


def build_bun_profile_input(group1: str, group2: str) -> str:
    return f"!test!\n\n{group1}\n\n{group2}"


def cleanup_trace_temp_modules(md5_path: Path) -> None:
    for temp_module in md5_path.parent.glob(".tswn-md5-trace-*.js"):
        temp_module.unlink(missing_ok=True)


def run_bun_profile_trace(
    bun_helper: Path,
    md5_path: Path,
    names: str,
    rounds: int,
    timeout: int,
    cwd: Path,
) -> list[int]:
    cleanup_trace_temp_modules(md5_path)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".txt", delete=False) as handle:
        handle.write(names)
        input_path = Path(handle.name)

    try:
        proc = subprocess.run(
            [
                "bun",
                str(bun_helper),
                "--input-file",
                str(input_path),
                "--rounds",
                str(rounds),
                "--md5",
                str(md5_path),
            ],
            cwd=cwd,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            check=True,
        )
    finally:
        input_path.unlink(missing_ok=True)
        cleanup_trace_temp_modules(md5_path)

    stdout_text = proc.stdout.strip()
    if not stdout_text:
        raise RuntimeError(
            f"bun trace produced empty stdout for {md5_path}: {proc.stderr[-400:]}"
        )

    try:
        payload = json.loads(stdout_text)
    except JSONDecodeError as error:
        raise RuntimeError(
            f"bun trace emitted non-json stdout for {md5_path}: {stdout_text[:200]!r}; stderr={proc.stderr[-400:]}"
        ) from error

    raw_data = payload.get("raw_data", [])
    cumulative = [int(entry["wins"]) for entry in raw_data]
    expected_rounds = list(range(1, rounds + 1))
    actual_rounds = [int(entry["round"]) for entry in raw_data]
    if actual_rounds != expected_rounds:
        raise ValueError(
            f"bun trace round 序列不连续: expected 1..{rounds}, got tail={actual_rounds[-5:]}"
        )
    return cumulative


def run_bun_profile_trace_with_fallback(
    bun_helper: Path,
    primary_md5_path: Path,
    fallback_md5_path: Path | None,
    names: str,
    rounds: int,
    timeout: int,
    cwd: Path,
) -> tuple[list[int], Path, str | None]:
    attempts: list[Path] = [primary_md5_path]
    if fallback_md5_path is not None and fallback_md5_path.resolve() != primary_md5_path.resolve():
        attempts.append(fallback_md5_path)

    last_error: Exception | None = None
    first_failure_reason: str | None = None
    for index, md5_path in enumerate(attempts):
        try:
            cumulative = run_bun_profile_trace(
                bun_helper=bun_helper,
                md5_path=md5_path,
                names=names,
                rounds=rounds,
                timeout=timeout,
                cwd=cwd,
            )
            return cumulative, md5_path, first_failure_reason
        except Exception as error:  # noqa: BLE001
            last_error = error
            if index == 0:
                first_failure_reason = str(error)

    raise RuntimeError(str(last_error) if last_error is not None else "bun trace failed")


def run_tswn_trace(
    tswn_bin: list[str],
    group1: str,
    group2: str,
    rounds: int,
    timeout: int,
) -> list[int]:
    result = base.run_tswn_win_rate(
        tswn_bin=tswn_bin,
        group1=group1,
        group2=group2,
        rounds=rounds,
        timeout=timeout,
        buckets_step=1,
    )
    if result is None:
        raise RuntimeError("tswn trace failed")

    cumulative = [wins for total, wins, _rate in result.buckets]
    totals = [total for total, _wins, _rate in result.buckets]
    expected_rounds = list(range(1, rounds + 1))
    if totals != expected_rounds:
        raise ValueError(
            f"tswn trace round 序列不连续: expected 1..{rounds}, got tail={totals[-5:]}"
        )
    return cumulative


def cumulative_to_round_results(cumulative_wins: list[int]) -> list[int]:
    round_results: list[int] = []
    previous = 0
    for wins in cumulative_wins:
        delta = wins - previous
        if delta not in (0, 1):
            raise ValueError(f"invalid round delta: {delta}")
        round_results.append(delta)
        previous = wins
    return round_results


def find_round_mismatches(bun_cumulative_wins: list[int], tswn_cumulative_wins: list[int]) -> list[int]:
    bun_rounds = cumulative_to_round_results(bun_cumulative_wins)
    tswn_rounds = cumulative_to_round_results(tswn_cumulative_wins)
    return [
        round_idx + 1
        for round_idx, (bun_win, tswn_win) in enumerate(zip(bun_rounds, tswn_rounds))
        if bun_win != tswn_win
    ]


def bucket_endpoints(rounds: int, step: int = 1000) -> list[int]:
    endpoints = list(range(step, rounds + 1, step))
    if not endpoints or endpoints[-1] != rounds:
        endpoints.append(rounds)
    return endpoints


def build_bucket_analyses(
    rounds: int,
    bun_cumulative_wins: list[int],
    tswn_cumulative_wins: list[int],
    mismatch_rounds: list[int],
) -> list[BucketAnalysis]:
    analyses: list[BucketAnalysis] = []
    prev_end = 0
    mismatch_set = set(mismatch_rounds)
    for endpoint in bucket_endpoints(rounds):
        start_round = prev_end + 1
        end_round = endpoint
        bun_start_wins = bun_cumulative_wins[start_round - 2] if start_round > 1 else 0
        bun_end_wins = bun_cumulative_wins[end_round - 1]
        tswn_start_wins = tswn_cumulative_wins[start_round - 2] if start_round > 1 else 0
        tswn_end_wins = tswn_cumulative_wins[end_round - 1]
        diff_before = tswn_start_wins - bun_start_wins
        diff_after = tswn_end_wins - bun_end_wins
        bucket_mismatches = [
            round_idx
            for round_idx in range(start_round, end_round + 1)
            if round_idx in mismatch_set
        ]
        analyses.append(
            BucketAnalysis(
                start_round=start_round,
                end_round=end_round,
                bun_start_wins=bun_start_wins,
                bun_end_wins=bun_end_wins,
                tswn_start_wins=tswn_start_wins,
                tswn_end_wins=tswn_end_wins,
                diff_before=diff_before,
                diff_after=diff_after,
                mismatch_rounds=bucket_mismatches,
            )
        )
        prev_end = endpoint
    return analyses


def seed_for_round(round_idx: int) -> str:
    if round_idx == 1:
        return "<none>"
    return str(PROFILE_START + round_idx - 1)


def compress_rounds(rounds: list[int]) -> list[str]:
    if not rounds:
        return []
    compressed: list[str] = []
    start = rounds[0]
    prev = rounds[0]
    for current in rounds[1:]:
        if current == prev + 1:
            prev = current
            continue
        compressed.append(f"{start}-{prev}" if start != prev else str(start))
        start = current
        prev = current
    compressed.append(f"{start}-{prev}" if start != prev else str(start))
    return compressed


def to_jsonable_case(analysis: CaseAnalysis) -> dict[str, Any]:
    return {
        "row_id": analysis.item.message.row_id,
        "date": analysis.item.message.date,
        "username": analysis.item.message.username,
        "rounds": analysis.rounds,
        "md5_path_used": analysis.md5_path_used,
        "md5_fallback_reason": analysis.md5_fallback_reason,
        "group1": analysis.group1,
        "group2": analysis.group2,
        "bun_reported_rate": f"{analysis.item.bun_2dp:.2f}",
        "tswn_reported_rate": f"{analysis.item.tswn:.2f}",
        "bun_exact_wins": analysis.bun_cumulative_wins[-1],
        "tswn_exact_wins": analysis.tswn_cumulative_wins[-1],
        "mismatch_rounds": analysis.mismatch_rounds,
        "mismatch_seeds": [seed_for_round(round_idx) for round_idx in analysis.mismatch_rounds],
        "buckets": [
            {
                "start_round": bucket.start_round,
                "end_round": bucket.end_round,
                "bun_start_wins": bucket.bun_start_wins,
                "bun_end_wins": bucket.bun_end_wins,
                "tswn_start_wins": bucket.tswn_start_wins,
                "tswn_end_wins": bucket.tswn_end_wins,
                "diff_before": bucket.diff_before,
                "diff_after": bucket.diff_after,
                "mismatch_rounds": bucket.mismatch_rounds,
            }
            for bucket in analysis.buckets
        ],
    }


def render_case_markdown(analysis: CaseAnalysis) -> str:
    lines: list[str] = []
    lines.append(f"## {analysis.item.message.row_id}")
    lines.append("")
    lines.append(
        f"- 时间: {analysis.item.message.date} {analysis.item.message.time} / {analysis.item.message.username}"
    )
    lines.append(
        f"- 旧消息显示: bun={analysis.item.bun_2dp:.2f}% / tswn(old)={analysis.item.tswn:.2f}% / diff={analysis.item.diff:+.2f}"
    )
    lines.append(
        f"- 本次精确结果: bun={analysis.bun_cumulative_wins[-1]}/{analysis.rounds} / tswn={analysis.tswn_cumulative_wins[-1]}/{analysis.rounds}"
    )
    lines.append(f"- bun md5.js: {analysis.md5_path_used}")
    if analysis.md5_fallback_reason:
        lines.append(f"- branch/latest 失败后 fallback: {analysis.md5_fallback_reason}")
    lines.append(f"- 队 1:\n\n```text\n{analysis.group1}\n```")
    lines.append(f"- 队 2:\n\n```text\n{analysis.group2}\n```")
    lines.append(f"- 真实分叉 round 数: {len(analysis.mismatch_rounds)}")
    if analysis.mismatch_rounds:
        exact_rounds = ", ".join(str(round_idx) for round_idx in analysis.mismatch_rounds)
        lines.append(f"- 真实分叉 round: {exact_rounds}")
        exact_seeds = ", ".join(
            f"r{round_idx}={seed_for_round(round_idx)}"
            for round_idx in analysis.mismatch_rounds
        )
        lines.append(f"- 对应 seed: {exact_seeds}")
    lines.append("")
    lines.append("### 1000 场分段")
    lines.append("")
    for bucket in analysis.buckets:
        net_change = bucket.diff_after - bucket.diff_before
        lines.append(
            f"- {bucket.start_round}-{bucket.end_round}: bun {bucket.bun_start_wins}->{bucket.bun_end_wins}, "
            f"tswn {bucket.tswn_start_wins}->{bucket.tswn_end_wins}, "
            f"累计差 {bucket.diff_before:+d}->{bucket.diff_after:+d}, 净变化 {net_change:+d}, "
            f"分叉 {len(bucket.mismatch_rounds)} 场"
        )
        if bucket.mismatch_rounds:
            lines.append(
                f"  rounds: {', '.join(compress_rounds(bucket.mismatch_rounds))}"
            )
    lines.append("")
    return "\n".join(lines)


def render_markdown(analyses: list[CaseAnalysis]) -> str:
    total_mismatch_rounds = sum(len(item.mismatch_rounds) for item in analyses)
    lines = [
        "# Bun / tswn 逐局分叉报告",
        "",
        "## 方法",
        "",
        "- bun: 对官方 md5.js::ProfileWinChance 做最小补丁，只把 callback 粒度从 100 场改成 1 场，不改胜负逻辑。",
        "- tswn: 使用 `bench auto --buckets-step 1` 输出每局累计胜场。",
        "- 对比方式: 逐 round 比较 team1 是否获胜，得到真实分叉 round。",
        "- seed 规则: 第 1 局无 seed；第 N 局 (N>1) 对应 `seed:` + (33554431 + N - 1)。",
        "",
        "## 汇总",
        "",
        f"- case 数: {len(analyses)}",
        f"- 总分叉 round 数: {total_mismatch_rounds}",
        "",
    ]
    for analysis in analyses:
        lines.append(render_case_markdown(analysis))
    return "\n".join(lines).rstrip() + "\n"


def analyze_case(
    item: base.MismatchRow,
    replied_messages: dict[str, base.MessageRow],
    bun_helper: Path,
    md5_path: Path,
    md5_fallback: Path | None,
    tswn_bin: list[str],
    rounds_override: int | None,
    timeout: int,
    repo_root: Path,
) -> CaseAnalysis:
    groups = base.resolve_retest_groups(item, replied_messages)
    if len(groups) < 2:
        raise ValueError(f"case {item.message.row_id} only has {len(groups)} groups")

    group1, group2 = groups[0], groups[1]
    rounds = rounds_override if rounds_override is not None else base.resolve_rounds_for_retest(item, None)
    bun_names = build_bun_profile_input(group1, group2)

    bun_cumulative_wins, md5_used, md5_fallback_reason = run_bun_profile_trace_with_fallback(
        bun_helper=bun_helper,
        primary_md5_path=md5_path,
        fallback_md5_path=md5_fallback,
        names=bun_names,
        rounds=rounds,
        timeout=timeout,
        cwd=repo_root,
    )
    tswn_cumulative_wins = run_tswn_trace(
        tswn_bin=tswn_bin,
        group1=group1,
        group2=group2,
        rounds=rounds,
        timeout=timeout,
    )
    mismatch_rounds = find_round_mismatches(bun_cumulative_wins, tswn_cumulative_wins)
    buckets = build_bucket_analyses(
        rounds=rounds,
        bun_cumulative_wins=bun_cumulative_wins,
        tswn_cumulative_wins=tswn_cumulative_wins,
        mismatch_rounds=mismatch_rounds,
    )
    return CaseAnalysis(
        item=item,
        group1=group1,
        group2=group2,
        rounds=rounds,
        md5_path_used=str(md5_used),
        md5_fallback_reason=md5_fallback_reason,
        bun_cumulative_wins=bun_cumulative_wins,
        tswn_cumulative_wins=tswn_cumulative_wins,
        mismatch_rounds=mismatch_rounds,
        buckets=buckets,
    )


def main(argv: list[str]) -> int:
    ensure_utf8_stdio()
    args = parse_args(argv)
    if not args.dsn:
        print("缺少 DSN，请通过 --dsn 或环境变量 TSWN_PG_DSN 提供。", file=sys.stderr)
        return 1

    repo_root = Path(__file__).resolve().parents[1]
    bun_helper = Path(args.bun_helper).resolve()
    md5_path = Path(args.md5_path).resolve()
    md5_fallback = Path(args.md5_fallback).resolve() if args.md5_fallback else None
    md_output = Path(args.md_output).resolve()
    json_output = Path(args.json_output).resolve()
    tswn_bin = resolve_tswn_bin(args, repo_root)

    mismatches, replied_messages = load_cases(args)
    analyses: list[CaseAnalysis] = []
    for index, item in enumerate(mismatches, start=1):
        print(
            f"[{index}/{len(mismatches)}] 分析 {item.message.row_id} "
            f"({item.message.date} {item.message.username})",
            file=sys.stderr,
        )
        analyses.append(
            analyze_case(
                item=item,
                replied_messages=replied_messages,
                bun_helper=bun_helper,
                md5_path=md5_path,
                md5_fallback=md5_fallback,
                tswn_bin=tswn_bin,
                rounds_override=args.rounds,
                timeout=args.timeout,
                repo_root=repo_root,
            )
        )

    md_output.parent.mkdir(parents=True, exist_ok=True)
    json_output.parent.mkdir(parents=True, exist_ok=True)
    md_output.write_text(render_markdown(analyses), encoding="utf-8")
    json_output.write_text(
        json.dumps([to_jsonable_case(item) for item in analyses], ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    print(f"markdown: {md_output}")
    print(f"json: {json_output}")
    print(f"cases: {len(analyses)}")
    print(f"mismatch_rounds: {sum(len(item.mismatch_rounds) for item in analyses)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))