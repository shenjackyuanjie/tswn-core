#!/usr/bin/env python3
"""筛出 bun / tswn `/namer-pf` 四项评分不一致的消息。

默认只读取同时包含 `pp|pd|qp|qd` 和 `tswn:` 的新版 bot 输出，并从
输出里的 bun 段与 tswn 段重新计算 diff；不依赖消息里已有的 `diff:`
文本。
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import find_bun_tswn_reply_mismatches as base


SCORE_LINE_RE = re.compile(
    r"^\s*"
    r"(?P<pp>\d+)\|(?P<pd>\d+)\|(?P<qp>\d+)\|(?P<qd>\d+)\|(?P<total>\d+)"
    r"(?:-\s*(?P<elapsed>\d+(?:\.\d+)?)s)?"
    r"\s*$"
)
TSWN_SECTION_RE = re.compile(r"(?im)^\s*tswn:\s*$")
NAMER_PF_COMMAND_RE = re.compile(r"^\s*/namer-pf(?:\s+(?P<rest>.*))?\s*$")
SCORE_FIELDS = ("pp", "pd", "qp", "qd", "total")
RETEST_TIMEOUT = 60


@dataclass(slots=True)
class PfScore:
    pp: int
    pd: int
    qp: int
    qd: int
    total: int
    elapsed: str | None = None


@dataclass(slots=True)
class PfDiffRow:
    index: int
    input_line: str
    bun: PfScore | None
    tswn: PfScore | None
    signed: dict[str, int]
    absolute: dict[str, int]


@dataclass(slots=True)
class PfCase:
    message: base.MessageRow
    bun_scores: list[PfScore]
    tswn_scores: list[PfScore]
    diff_rows: list[PfDiffRow]
    reply_ids: list[str]
    inline_reply_contents: list[str]


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="查找 bun / tswn `/namer-pf` 四项评分不一致的消息。",
    )
    parser.add_argument(
        "--dsn",
        default=os.environ.get("TSWN_PG_DSN"),
        help="PostgreSQL DSN；默认读取环境变量 TSWN_PG_DSN。",
    )
    parser.add_argument("--schema", default="eqq3695888")
    parser.add_argument("--table", default="messages")
    parser.add_argument("--sender-id", default="45620725")
    parser.add_argument(
        "--content-like",
        default="%pp|pd|qp|qd%tswn:%diff%",
        help="content LIKE 条件；默认只扫新版 `/namer-pf` 对照输出。",
    )
    parser.add_argument(
        "--case-id",
        action="append",
        default=[],
        help="只输出指定消息 id，可重复传入。",
    )
    parser.add_argument(
        "--include-zero",
        action="store_true",
        help="同时输出 diff=0 的记录。默认只输出不一致记录。",
    )
    parser.add_argument(
        "--dedup",
        action="store_true",
        help="按回复中的 `/namer-pf` 输入去重，保留第一条。",
    )
    parser.add_argument("--json", action="store_true", help="输出 JSON。")
    parser.add_argument("--output", default=None, help="可选：输出到指定 UTF-8 文件。")

    parser.add_argument(
        "--retest",
        action="store_true",
        help="用当前 tswn-cli namer-pf 重跑，并与原 bun 分数对比。",
    )
    parser.add_argument(
        "--tswn-bin",
        default=None,
        help="tswn-cli 可执行文件路径；默认优先使用 target/release/tswn-cli(.exe)。",
    )
    parser.add_argument(
        "--retest-count",
        type=int,
        default=10000,
        help="重测每个评分项的场数，默认 10000。",
    )
    parser.add_argument(
        "--retest-timeout",
        type=int,
        default=RETEST_TIMEOUT,
        help=f"单次 tswn 调用超时秒数，默认 {RETEST_TIMEOUT}。",
    )
    parser.add_argument(
        "--retest-thread",
        type=int,
        default=None,
        help="传给 tswn-cli namer-pf 的 --thread。",
    )
    parser.add_argument(
        "--retest-only-diff",
        action="store_true",
        help="重测时只显示当前仍和 bun 不一致的 case。",
    )
    return parser.parse_args(argv)


def ensure_utf8_stdio() -> None:
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8")


def parse_score_lines(text: str) -> list[PfScore]:
    scores: list[PfScore] = []
    for line in text.splitlines():
        match = SCORE_LINE_RE.match(line)
        if not match:
            continue
        scores.append(
            PfScore(
                pp=int(match.group("pp")),
                pd=int(match.group("pd")),
                qp=int(match.group("qp")),
                qd=int(match.group("qd")),
                total=int(match.group("total")),
                elapsed=match.group("elapsed"),
            )
        )
    return scores


def parse_namer_pf_sections(content: str) -> tuple[list[PfScore], list[PfScore]]:
    parts = TSWN_SECTION_RE.split(content, maxsplit=1)
    if len(parts) != 2:
        return [], []
    bun_text, tswn_text = parts
    return parse_score_lines(bun_text), parse_score_lines(tswn_text)


def score_to_dict(score: PfScore | None) -> dict[str, Any] | None:
    if score is None:
        return None
    return {
        "pp": score.pp,
        "pd": score.pd,
        "qp": score.qp,
        "qd": score.qd,
        "sum": score.total,
        "elapsed": score.elapsed,
    }


def score_value(score: PfScore, field: str) -> int:
    if field == "total":
        return score.total
    return int(getattr(score, field))


def score_diff(bun: PfScore | None, tswn: PfScore | None) -> tuple[dict[str, int], dict[str, int]]:
    if bun is None or tswn is None:
        return {}, {}

    signed = {
        field: score_value(tswn, field) - score_value(bun, field)
        for field in SCORE_FIELDS
    }
    absolute = {field: abs(value) for field, value in signed.items()}
    return signed, absolute


def has_nonzero_diff(row: PfDiffRow) -> bool:
    return row.bun is None or row.tswn is None or any(row.absolute.values())


def extract_namer_pf_raw(raw_content: str) -> str:
    if not raw_content:
        return ""

    lines = raw_content.strip().splitlines()
    if not lines:
        return ""

    first_non_empty = 0
    while first_non_empty < len(lines) and not lines[first_non_empty].strip():
        first_non_empty += 1
    if first_non_empty >= len(lines):
        return ""

    first = lines[first_non_empty]
    match = NAMER_PF_COMMAND_RE.match(first)
    if not match:
        return "\n".join(line.strip() for line in lines if line.strip())

    result_lines: list[str] = []
    rest = (match.group("rest") or "").strip()
    if rest:
        result_lines.append(rest)
    result_lines.extend(line.strip() for line in lines[first_non_empty + 1 :] if line.strip())
    return "\n".join(result_lines)


def input_lines_from_raw(raw: str) -> list[str]:
    return [line.strip() for line in raw.splitlines() if line.strip()]


def resolve_primary_reply_content(
    case: PfCase,
    replied_messages: dict[str, base.MessageRow],
) -> str:
    reply_id = case.reply_ids[0] if case.reply_ids else ""
    if reply_id and reply_id in replied_messages:
        return replied_messages[reply_id].content
    if case.inline_reply_contents:
        return case.inline_reply_contents[0]
    return ""


def build_diff_rows(
    bun_scores: list[PfScore],
    tswn_scores: list[PfScore],
    input_lines: list[str],
) -> list[PfDiffRow]:
    rows: list[PfDiffRow] = []
    max_len = max(len(bun_scores), len(tswn_scores), len(input_lines))
    for index in range(max_len):
        bun = bun_scores[index] if index < len(bun_scores) else None
        tswn = tswn_scores[index] if index < len(tswn_scores) else None
        signed, absolute = score_diff(bun, tswn)
        rows.append(
            PfDiffRow(
                index=index + 1,
                input_line=input_lines[index] if index < len(input_lines) else "",
                bun=bun,
                tswn=tswn,
                signed=signed,
                absolute=absolute,
            )
        )
    return rows


def build_cases(
    messages: list[base.MessageRow],
    replied_messages: dict[str, base.MessageRow],
) -> list[PfCase]:
    cases: list[PfCase] = []
    for message in messages:
        bun_scores, tswn_scores = parse_namer_pf_sections(message.content)
        if not bun_scores and not tswn_scores:
            continue

        reply_ids, inline_reply_contents = base.extract_reply_metadata(message.reply_message)
        placeholder = PfCase(
            message=message,
            bun_scores=bun_scores,
            tswn_scores=tswn_scores,
            diff_rows=[],
            reply_ids=reply_ids,
            inline_reply_contents=inline_reply_contents,
        )
        raw_input = extract_namer_pf_raw(
            resolve_primary_reply_content(placeholder, replied_messages)
        )
        diff_rows = build_diff_rows(
            bun_scores=bun_scores,
            tswn_scores=tswn_scores,
            input_lines=input_lines_from_raw(raw_input),
        )
        cases.append(
            PfCase(
                message=message,
                bun_scores=bun_scores,
                tswn_scores=tswn_scores,
                diff_rows=diff_rows,
                reply_ids=reply_ids,
                inline_reply_contents=inline_reply_contents,
            )
        )
    return cases


def dedup_cases(
    cases: list[PfCase],
    replied_messages: dict[str, base.MessageRow],
) -> list[PfCase]:
    seen: set[str] = set()
    deduped: list[PfCase] = []
    dup_count = 0
    for case in cases:
        key = extract_namer_pf_raw(resolve_primary_reply_content(case, replied_messages))
        if key in seen:
            dup_count += 1
            continue
        seen.add(key)
        deduped.append(case)
    if dup_count:
        print(f"去重: 移除 {dup_count} 条重复输入，剩余 {len(deduped)} 条", file=sys.stderr)
    return deduped


def case_has_diff(case: PfCase) -> bool:
    return any(has_nonzero_diff(row) for row in case.diff_rows)


def compact_diff(diff: dict[str, int], *, signed: bool) -> str:
    if not diff:
        return "<missing>"
    parts = []
    for field in SCORE_FIELDS:
        value = diff[field]
        if value == 0:
            continue
        label = "sum" if field == "total" else field
        parts.append(f"{label}={value:+d}" if signed else f"{label}={value}")
    return ", ".join(parts) if parts else "0"


def format_score(score: PfScore | None) -> str:
    if score is None:
        return "<missing>"
    elapsed = f"-{score.elapsed}s" if score.elapsed else ""
    return f"{score.pp}|{score.pd}|{score.qp}|{score.qd}|{score.total}{elapsed}"


def case_to_record(
    case: PfCase,
    replied_messages: dict[str, base.MessageRow],
    retest_scores: list[PfScore] | None = None,
) -> dict[str, Any]:
    raw_input = extract_namer_pf_raw(resolve_primary_reply_content(case, replied_messages))
    record: dict[str, Any] = {
        "_id": case.message.row_id,
        "username": case.message.username,
        "senderId": case.message.sender_id,
        "date": case.message.date,
        "time": case.message.time,
        "diff": case_has_diff(case),
        "row_count": len(case.diff_rows),
        "input": raw_input,
        "rows": [
            {
                "index": row.index,
                "input": row.input_line,
                "bun": score_to_dict(row.bun),
                "tswn": score_to_dict(row.tswn),
                "signed": {
                    ("sum" if key == "total" else key): value
                    for key, value in row.signed.items()
                },
                "absolute": {
                    ("sum" if key == "total" else key): value
                    for key, value in row.absolute.items()
                },
                "diff": has_nonzero_diff(row),
            }
            for row in case.diff_rows
        ],
        "content": case.message.content,
        "reply_ids": case.reply_ids,
    }

    if retest_scores is not None:
        retest_rows = build_diff_rows(
            bun_scores=case.bun_scores,
            tswn_scores=retest_scores,
            input_lines=input_lines_from_raw(raw_input),
        )
        record["retest_rows"] = [
            {
                "index": row.index,
                "input": row.input_line,
                "bun": score_to_dict(row.bun),
                "tswn_now": score_to_dict(row.tswn),
                "signed": {
                    ("sum" if key == "total" else key): value
                    for key, value in row.signed.items()
                },
                "absolute": {
                    ("sum" if key == "total" else key): value
                    for key, value in row.absolute.items()
                },
                "diff": has_nonzero_diff(row),
            }
            for row in retest_rows
        ]
        record["retest_diff"] = any(has_nonzero_diff(row) for row in retest_rows)

    return record


def render_text_report(
    cases: list[PfCase],
    replied_messages: dict[str, base.MessageRow],
    *,
    candidate_count: int,
    parsed_count: int,
) -> str:
    lines = [
        f"candidate_count={candidate_count}",
        f"parsed_count={parsed_count}",
        f"output_count={len(cases)}",
    ]
    for case_index, case in enumerate(cases, start=1):
        raw_input = extract_namer_pf_raw(resolve_primary_reply_content(case, replied_messages))
        lines.append(
            f"\n[{case_index}] {case.message.date} {case.message.time} "
            f"{case.message.username} id={case.message.row_id}"
        )
        lines.append(f"  diff={case_has_diff(case)} rows={len(case.diff_rows)}")
        if raw_input:
            lines.append("  input:")
            lines.append(base.indent_block(raw_input, "    "))
        for row in case.diff_rows:
            if not has_nonzero_diff(row):
                continue
            label = f"row[{row.index}]"
            if row.input_line:
                label += f" {row.input_line}"
            lines.append(f"  {label}")
            lines.append(f"    bun : {format_score(row.bun)}")
            lines.append(f"    tswn: {format_score(row.tswn)}")
            lines.append(f"    signed(tswn-bun): {compact_diff(row.signed, signed=True)}")
            lines.append(f"    abs: {compact_diff(row.absolute, signed=False)}")
    return "\n".join(lines) + "\n"


def resolve_tswn_bin(args: argparse.Namespace, repo_root: Path) -> list[str]:
    if args.tswn_bin:
        return [args.tswn_bin]

    exe_name = "tswn-cli.exe" if os.name == "nt" else "tswn-cli"
    release_bin = repo_root / "target" / "release" / exe_name
    if release_bin.exists():
        return [str(release_bin)]

    return ["cargo", "run", "--release", "--bin", "tswn-cli", "--"]


def run_tswn_namer_pf(
    tswn_bin: list[str],
    raw_input: str,
    count: int,
    timeout: int,
    thread: int | None,
) -> list[PfScore] | None:
    escaped = raw_input.replace("\\", "\\\\").replace("\n", "\\n")
    cmd = [*tswn_bin, "namer-pf", "-r", escaped, "-n", str(count)]
    if thread is not None:
        cmd.extend(["--thread", str(thread)])

    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        print(f"  [retest] 超时 ({timeout}s): {' '.join(cmd[:5])}...", file=sys.stderr)
        return None
    except FileNotFoundError:
        print(f"  [retest] 找不到 tswn-cli: {tswn_bin[0]}", file=sys.stderr)
        return None

    if proc.returncode != 0:
        stderr_tail = proc.stderr.strip().splitlines()[-3:]
        print(f"  [retest] tswn-cli 返回非零 ({proc.returncode}): {stderr_tail}", file=sys.stderr)
        return None

    scores = parse_score_lines(proc.stdout)
    if not scores:
        print(f"  [retest] 无法解析 namer-pf 输出: {proc.stdout.strip()[-200:]}", file=sys.stderr)
        return None
    return scores


def run_retest(
    cases: list[PfCase],
    replied_messages: dict[str, base.MessageRow],
    args: argparse.Namespace,
    repo_root: Path,
) -> None:
    tswn_bin = resolve_tswn_bin(args, repo_root)
    try:
        probe = subprocess.run(
            [*tswn_bin, "--version"],
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
        )
    except Exception as error:  # noqa: BLE001
        print(f"无法启动 tswn-cli: {' '.join(tswn_bin)} ({error})", file=sys.stderr)
        return

    print(f"tswn-cli: {probe.stdout.strip() or probe.stderr.strip()}")
    print(f"case 总数: {len(cases)}")
    print()

    tested = 0
    still_diff = 0
    now_match = 0
    errors = 0

    for index, case in enumerate(cases, start=1):
        raw_input = extract_namer_pf_raw(resolve_primary_reply_content(case, replied_messages))
        if not raw_input:
            errors += 1
            print(f"[{index}/{len(cases)}] id={case.message.row_id} 跳过: 找不到 /namer-pf 输入")
            continue

        scores = run_tswn_namer_pf(
            tswn_bin=tswn_bin,
            raw_input=raw_input,
            count=args.retest_count,
            timeout=args.retest_timeout,
            thread=args.retest_thread,
        )
        tested += 1
        if scores is None:
            errors += 1
            continue

        retest_rows = build_diff_rows(
            bun_scores=case.bun_scores,
            tswn_scores=scores,
            input_lines=input_lines_from_raw(raw_input),
        )
        is_diff = any(has_nonzero_diff(row) for row in retest_rows)
        if is_diff:
            still_diff += 1
        else:
            now_match += 1

        if args.retest_only_diff and not is_diff:
            continue

        print(
            f"[{index}/{len(cases)}] {case.message.date} {case.message.username} "
            f"id={case.message.row_id}"
        )
        print(f"  当前重测: {'仍不一致' if is_diff else '已一致'}")
        for row in retest_rows:
            if not has_nonzero_diff(row) and args.retest_only_diff:
                continue
            label = f"row[{row.index}]"
            if row.input_line:
                label += f" {row.input_line}"
            print(f"  {label}")
            print(f"    bun     : {format_score(row.bun)}")
            print(f"    tswn_now: {format_score(row.tswn)}")
            print(f"    signed(tswn-bun): {compact_diff(row.signed, signed=True)}")
            print(f"    abs: {compact_diff(row.absolute, signed=False)}")
        print()

    print("=" * 50)
    print(f"测试: {tested}  仍不一致: {still_diff}  已一致: {now_match}  错误: {errors}")


def main(argv: list[str]) -> int:
    ensure_utf8_stdio()
    args = parse_args(argv)
    if not args.dsn:
        print("缺少 DSN，请通过 --dsn 或环境变量 TSWN_PG_DSN 提供。", file=sys.stderr)
        return 1

    repo_root = Path(__file__).resolve().parents[1]
    schema_name = base.validate_identifier(args.schema, "schema")
    table_name = base.validate_identifier(args.table, "table")

    candidate_messages = base.fetch_candidate_messages(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        sender_id=args.sender_id,
        content_like=args.content_like,
    )
    if args.case_id:
        wanted = set(args.case_id)
        candidate_messages = [row for row in candidate_messages if row.row_id in wanted]

    reply_ids: list[str] = []
    for message in candidate_messages:
        ids, _inline = base.extract_reply_metadata(message.reply_message)
        reply_ids.extend(ids)
    replied_messages = base.fetch_messages_by_ids(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        message_ids=sorted(set(reply_ids)),
    )

    cases = build_cases(candidate_messages, replied_messages)
    if args.dedup:
        cases = dedup_cases(cases, replied_messages)
    parsed_count = len(cases)
    if not args.include_zero:
        cases = [case for case in cases if case_has_diff(case)]

    if args.retest:
        run_retest(cases, replied_messages, args, repo_root)
        return 0

    if args.json:
        payload = json.dumps(
            {
                "candidate_count": len(candidate_messages),
                "parsed_count": parsed_count,
                "output_count": len(cases),
                "records": [case_to_record(case, replied_messages) for case in cases],
            },
            ensure_ascii=False,
            indent=2,
        )
    else:
        payload = render_text_report(
            cases,
            replied_messages,
            candidate_count=len(candidate_messages),
            parsed_count=parsed_count,
        )

    if args.output:
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
