#!/usr/bin/env python3
"""筛出 bun / tswn 胜率不一致的消息，并回查其回复的原始消息内容。

--retest 模式：对每个 mismatch 用当前 tswn-cli 重跑胜率，对比是否仍然不一致。
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from decimal import Decimal, ROUND_HALF_UP
from pathlib import Path
from typing import Any

import psycopg
from psycopg import sql
from psycopg.rows import dict_row


BUN_RATE_RE = re.compile(r"最终胜率:\|(?P<rate>\d+\.\d+)%\|")
TSWN_RATE_RE = re.compile(r"tswn: 胜率: (?P<rate>\d+\.\d+)%")
BUN_BUCKET_RE = re.compile(r"^(?P<rate>\d+\.\d+)%\((?P<rounds>\d+)\)$", re.MULTILINE)
TSWN_OUTPUT_SUMMARY_RE = re.compile(
    r"胜率:\s*(?P<rate>\d+\.\d+)%\s+\((?P<wins>\d+)/(?P<total>\d+)\)"
)
TSWN_BUCKET_RE = re.compile(
    r"胜率\(分段\):\s*(?P<rate>\d+\.\d+)%\s+\((?P<wins>\d+)/(?P<total>\d+)\)"
)
SAFE_IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
REPLY_ID_KEYS = {"_id", "id", "messageId", "msgId"}
INLINE_CONTENT_KEYS = {"content", "text", "message"}

# tswn-cli retest 默认超时（秒）
RETEST_TIMEOUT = 60


@dataclass(slots=True)
class MessageRow:
    row_id: str
    username: str
    sender_id: str
    date: str
    time: int
    content: str
    reply_message: str


@dataclass(slots=True)
class MismatchRow:
    message: MessageRow
    bun_raw: Decimal
    bun_2dp: Decimal
    tswn: Decimal
    diff: Decimal
    bun_buckets: list[tuple[int, Decimal]]
    reply_ids: list[str]
    inline_reply_contents: list[str]


@dataclass(slots=True)
class TswnWinRateResult:
    rate: Decimal
    wins: int
    total: int
    buckets: list[tuple[int, int, Decimal]]  # (cumulative_total, cumulative_wins, cumulative_rate)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="查找 bun / tswn 胜率不一致的消息，并回查回复的原始内容。",
    )
    parser.add_argument(
        "--dsn",
        default=os.environ.get("TSWN_PG_DSN"),
        help="PostgreSQL DSN；默认读取环境变量 TSWN_PG_DSN。",
    )
    parser.add_argument(
        "--schema",
        default="eqq3695888",
        help="schema 名，默认 eqq3695888。",
    )
    parser.add_argument(
        "--table",
        default="messages",
        help="table 名，默认 messages。",
    )
    parser.add_argument(
        "--sender-id",
        default="45620725",
        help="senderId 过滤值，默认 45620725。",
    )
    parser.add_argument(
        "--content-like",
        default="最终胜率%tswn:%",
        help="content LIKE 条件，默认 最终胜率%%tswn:%%。",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="输出 JSON，便于后续处理。",
    )
    parser.add_argument(
        "--output",
        default=None,
        help="可选：把结果写到指定文件路径（UTF-8）。",
    )

    # -- retest 相关参数 --
    parser.add_argument(
        "--retest",
        action="store_true",
        help="对每个 mismatch 用当前 tswn-cli 重跑胜率，输出新旧对比。",
    )
    parser.add_argument(
        "--tswn-bin",
        default=None,
        help="tswn-cli 可执行文件路径；默认使用 `cargo run --release --bin tswn-cli --`。",
    )
    parser.add_argument(
        "--retest-rounds",
        type=int,
        default=None,
        help="重测场数；默认取各 mismatch 的 bun_buckets 最大轮数（通常 10000）。",
    )
    parser.add_argument(
        "--retest-timeout",
        type=int,
        default=RETEST_TIMEOUT,
        help=f"单次 tswn 调用超时秒数（默认 {RETEST_TIMEOUT}）。",
    )
    parser.add_argument(
        "--retest-only-diff",
        action="store_true",
        help="只显示重测后依然 diff != 0 的 case。",
    )
    parser.add_argument(
        "--retest-buckets-step",
        type=int,
        default=1000,
        help="重测时启用分段胜率输出，每 N 场输出一次累积胜率（默认 1000）。设为 0 禁用。",
    )

    return parser.parse_args(argv)


def ensure_utf8_stdio() -> None:
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            reconfigure(encoding="utf-8")


def validate_identifier(name: str, desc: str) -> str:
    if not SAFE_IDENTIFIER_RE.match(name):
        raise ValueError(f"非法 {desc}: {name!r}")
    return name


def fetch_candidate_messages(
    dsn: str,
    schema_name: str,
    table_name: str,
    sender_id: str,
    content_like: str,
) -> list[MessageRow]:
    query = sql.SQL(
        """
        select _id, username, "senderId", date, time, content, "replyMessage"
        from {}.{}
        where "senderId" = %s
          and content like %s
        order by time desc
        """
    ).format(sql.Identifier(schema_name), sql.Identifier(table_name))

    with psycopg.connect(dsn, row_factory=dict_row) as conn:
        conn.execute("set client_encoding = 'UTF8'")
        with conn.cursor() as cur:
            cur.execute(query, (sender_id, content_like))
            rows = cur.fetchall()

    return [
        MessageRow(
            row_id=row["_id"],
            username=row["username"] or "",
            sender_id=row["senderId"] or "",
            date=row["date"] or "",
            time=int(row["time"]),
            content=row["content"] or "",
            reply_message=row["replyMessage"] or "",
        )
        for row in rows
    ]


def fetch_messages_by_ids(
    dsn: str,
    schema_name: str,
    table_name: str,
    message_ids: list[str],
) -> dict[str, MessageRow]:
    if not message_ids:
        return {}

    query = sql.SQL(
        """
        select _id, username, "senderId", date, time, content, "replyMessage"
        from {}.{}
        where _id = any(%s)
        """
    ).format(sql.Identifier(schema_name), sql.Identifier(table_name))

    with psycopg.connect(dsn, row_factory=dict_row) as conn:
        conn.execute("set client_encoding = 'UTF8'")
        with conn.cursor() as cur:
            cur.execute(query, (sorted(set(message_ids)),))
            rows = cur.fetchall()

    return {
        row["_id"]: MessageRow(
            row_id=row["_id"],
            username=row["username"] or "",
            sender_id=row["senderId"] or "",
            date=row["date"] or "",
            time=int(row["time"]),
            content=row["content"] or "",
            reply_message=row["replyMessage"] or "",
        )
        for row in rows
    }


def extract_reply_metadata(reply_message: str) -> tuple[list[str], list[str]]:
    if not reply_message.strip():
        return [], []

    try:
        payload = json.loads(reply_message)
    except json.JSONDecodeError:
        stripped = reply_message.strip()
        if stripped:
            return [stripped], []
        return [], []

    found_ids: list[str] = []
    inline_contents: list[str] = []

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            for key, value in node.items():
                if key in REPLY_ID_KEYS and isinstance(value, str) and value not in found_ids:
                    found_ids.append(value)
                if key in INLINE_CONTENT_KEYS and isinstance(value, str) and value not in inline_contents:
                    inline_contents.append(value)
                walk(value)
            return

        if isinstance(node, list):
            for item in node:
                walk(item)

    walk(payload)
    return found_ids, inline_contents


def extract_bun_buckets(content: str) -> list[tuple[int, Decimal]]:
    buckets: list[tuple[int, Decimal]] = []
    for match in BUN_BUCKET_RE.finditer(content):
        buckets.append((int(match.group("rounds")), Decimal(match.group("rate"))))
    buckets.sort(key=lambda item: item[0])
    return buckets


def find_mismatches(rows: list[MessageRow]) -> list[MismatchRow]:
    mismatches: list[MismatchRow] = []
    for row in rows:
        bun_match = BUN_RATE_RE.search(row.content)
        tswn_match = TSWN_RATE_RE.search(row.content)
        if not bun_match or not tswn_match:
            continue

        bun_raw = Decimal(bun_match.group("rate"))
        bun_2dp = bun_raw.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
        tswn = Decimal(tswn_match.group("rate"))
        if bun_2dp == tswn:
            continue

        reply_ids, inline_reply_contents = extract_reply_metadata(row.reply_message)
        mismatches.append(
            MismatchRow(
                message=row,
                bun_raw=bun_raw,
                bun_2dp=bun_2dp,
                tswn=tswn,
                diff=tswn - bun_2dp,
                bun_buckets=extract_bun_buckets(row.content),
                reply_ids=reply_ids,
                inline_reply_contents=inline_reply_contents,
            )
        )
    return mismatches


def dedup_mismatches(
    mismatches: list[MismatchRow],
    replied_messages: dict[str, MessageRow],
) -> list[MismatchRow]:
    """按 namerena 输入去重，保留每个唯一输入的第一个 mismatch。"""
    seen: set[str] = set()
    deduped: list[MismatchRow] = []
    dup_count = 0

    for item in mismatches:
        key = resolve_primary_reply_content(item, replied_messages).strip()
        if key in seen:
            dup_count += 1
            continue
        seen.add(key)
        deduped.append(item)

    if dup_count:
        print(f"去重: 移除 {dup_count} 条重复输入，剩余 {len(deduped)} 条")
        print()
    return deduped


def resolve_primary_reply_content(
    item: MismatchRow,
    replied_messages: dict[str, MessageRow],
) -> str:
    """返回用于复盘或去重的原始回复内容。"""
    reply_id = item.reply_ids[0] if item.reply_ids else ""
    if reply_id:
        reply = replied_messages.get(reply_id)
        if reply:
            return reply.content

    if item.inline_reply_contents:
        return item.inline_reply_contents[0]

    return ""


def resolve_replied_message_rows(
    item: MismatchRow,
    replied_messages: dict[str, MessageRow],
) -> list[MessageRow]:
    return [
        replied_messages[reply_id]
        for reply_id in item.reply_ids
        if reply_id in replied_messages
    ]


def resolve_retest_groups(
    item: MismatchRow,
    replied_messages: dict[str, MessageRow],
) -> list[str]:
    return parse_namerena_groups(resolve_primary_reply_content(item, replied_messages))


def build_output(
    mismatches: list[MismatchRow],
    replied_messages: dict[str, MessageRow],
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for item in mismatches:
        resolved_replies = resolve_replied_message_rows(item, replied_messages)
        records.append(
            {
                "_id": item.message.row_id,
                "username": item.message.username,
                "senderId": item.message.sender_id,
                "date": item.message.date,
                "time": item.message.time,
                "bun_raw": f"{item.bun_raw:.4f}",
                "bun_2dp": f"{item.bun_2dp:.2f}",
                "tswn": f"{item.tswn:.2f}",
                "diff": f"{item.diff:+.2f}",
                "bun_buckets": [
                    {
                        "rounds": rounds,
                        "rate": f"{rate:.2f}",
                    }
                    for rounds, rate in item.bun_buckets
                ],
                "content": item.message.content,
                "reply_ids": item.reply_ids,
                "inline_reply_contents": item.inline_reply_contents,
                "resolved_replies": [
                    {
                        "_id": reply.row_id,
                        "username": reply.username,
                        "senderId": reply.sender_id,
                        "date": reply.date,
                        "time": reply.time,
                        "content": reply.content,
                    }
                    for reply in resolved_replies
                ],
            }
        )
    return records


def indent_block(text: str, prefix: str) -> str:
    return "\n".join(f"{prefix}{line}" for line in text.splitlines())


def _get_best_group(group_str: str) -> str:
    """把一个 namerena 组字符串标准化：去除首尾空白，保留内部换行。"""
    return group_str.strip()


def parse_namerena_groups(raw_content: str) -> list[str]:
    """从回复的原始 namerena 内容中解析出队伍分组。

    输入形如:
        /namerena
        !test!

        player1 #key@clan
        player2 #key@clan

        player3 #key@clan
        player4 #key@clan

    返回每个组（玩家之间以 \\n 分隔）的字符串列表。单组 / 无法解析时返回空或少组。
    """
    if not raw_content:
        return []

    # 先按行拆分，跳过 /namerena 和 !test! 标记行
    lines = raw_content.strip().split("\n")
    start = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped in ("/namerena", "!test!"):
            start = i + 1
        else:
            break

    body = "\n".join(lines[start:]).strip()
    if not body:
        return []

    # 按连续空行（一个或多个空行）拆分队伍
    groups = re.split(r"\n\s*\n", body)
    result = [_get_best_group(g) for g in groups if _get_best_group(g)]

    # 兜底：只有 1 组且恰好 2 个玩家 → 视为 1v1（原消息漏了空行）
    if len(result) == 1:
        lines_in_group = [line for line in result[0].split("\n") if line.strip()]
        if len(lines_in_group) == 2:
            result = list(lines_in_group)

    return result


def resolve_tswn_bin(args: argparse.Namespace) -> list[str]:
    """根据参数确定 tswn-cli 的启动命令。"""
    if args.tswn_bin:
        return [args.tswn_bin]

    # 默认通过 cargo run
    return ["cargo", "run", "--release", "--bin", "tswn-cli", "--"]


def run_tswn_win_rate(
    tswn_bin: list[str],
    group1: str,
    group2: str,
    rounds: int,
    timeout: int,
    buckets_step: int | None = None,
) -> TswnWinRateResult | None:
    """用 tswn-cli 计算两个队伍之间的胜率。

    tswn_bin 是命令行前缀列表（例如 ["cargo", "run", "--release", "--bin", "tswn-cli", "--"]）。
    通过 `bench auto -r <escaped> -n <rounds>` 传入原始 namerena 输入。
    若指定 buckets_step，会同时解析分段累积胜率。
    返回 TswnWinRateResult，失败返回 None。
    """
    raw_input = f"{group1}\n\n{group2}"
    # decode_raw 会把字面量 \n 转为真实换行，所以这里把真实换行替换为 \n
    escaped = raw_input.replace("\\", "\\\\").replace("\n", "\\n")

    cmd = [*tswn_bin, "bench", "auto", "-r", escaped, "-n", str(rounds)]
    if buckets_step:
        cmd.extend(["--buckets-step", str(buckets_step)])
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=timeout,
            cwd=None,  # inherit workspace
        )
    except subprocess.TimeoutExpired:
        print(
            f"        [retest] 超时 ({timeout}s): {' '.join(cmd[:5])}...",
            file=sys.stderr,
        )
        return None
    except FileNotFoundError:
        print(
            f"        [retest] 找不到 tswn-cli: {tswn_bin[0]}",
            file=sys.stderr,
        )
        return None

    stdout = proc.stdout
    if stdout is None:
        print(
            "        [retest] tswn-cli 无 stdout 输出",
            file=sys.stderr,
        )
        return None
    if proc.returncode != 0:
        stderr_tail = proc.stderr.strip().split("\n")[-3:]
        print(
            f"        [retest] tswn-cli 返回非零 ({proc.returncode}): {stderr_tail}",
            file=sys.stderr,
        )
        return None

    match = TSWN_OUTPUT_SUMMARY_RE.search(stdout)
    if not match:
        # 可能是单组评分输出，没有 "胜率:"
        if "实力评分测试" in stdout:
            return None  # 评分模式，不算胜率
        print(
            f"        [retest] 无法从 tswn 输出解析胜率。stdout 尾部: {stdout.strip()[-200:]}",
            file=sys.stderr,
        )
        return None

    rate = Decimal(match.group("rate"))
    wins = int(match.group("wins"))
    total = int(match.group("total"))
    buckets: list[tuple[int, int, Decimal]] = []
    for bucket_match in TSWN_BUCKET_RE.finditer(stdout):
        buckets.append(
            (
                int(bucket_match.group("total")),
                int(bucket_match.group("wins")),
                Decimal(bucket_match.group("rate")),
            )
        )

    return TswnWinRateResult(rate=rate, wins=wins, total=total, buckets=buckets)


def infer_exact_wins_from_displayed_rate(rate: Decimal, total: int) -> int | None:
    """从显示到两位小数的百分比反推唯一胜场数。"""
    if total <= 0:
        return 0

    nominal = int(
        (rate * Decimal(total) / Decimal("100")).to_integral_value(
            rounding=ROUND_HALF_UP
        )
    )
    matches: list[int] = []
    lower = max(0, nominal - 3)
    upper = min(total, nominal + 3)
    for wins in range(lower, upper + 1):
        displayed = (
            Decimal(wins) * Decimal("100") / Decimal(total)
        ).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)
        if displayed == rate:
            matches.append(wins)

    if len(matches) == 1:
        return matches[0]
    return None


def exact_rate_percent(wins: int, total: int) -> Decimal:
    if total <= 0:
        return Decimal("0")
    return Decimal(wins) * Decimal("100") / Decimal(total)


def format_exact_rate(rate: Decimal) -> str:
    return f"{rate.quantize(Decimal('0.0001'), rounding=ROUND_HALF_UP):.4f}%"


def format_signed_exact_rate(rate: Decimal) -> str:
    return f"{rate.quantize(Decimal('0.0001'), rounding=ROUND_HALF_UP):+.4f}%"


def _print_exact_summary_comparison(bun_rate: Decimal, result: TswnWinRateResult) -> None:
    bun_wins = infer_exact_wins_from_displayed_rate(bun_rate, result.total)
    if bun_wins is None:
        print("  实际汇总: <无法从 bun 显示值唯一反推胜场>")
        return

    bun_exact = exact_rate_percent(bun_wins, result.total)
    tswn_exact = exact_rate_percent(result.wins, result.total)
    delta_wins = result.wins - bun_wins
    delta_rate = tswn_exact - bun_exact
    print(
        "  实际汇总: "
        f"bun={bun_wins}/{result.total} ({format_exact_rate(bun_exact)}) → "
        f"tswn={result.wins}/{result.total} ({format_exact_rate(tswn_exact)}) "
        f"Δwins={delta_wins:+d} Δrate={format_signed_exact_rate(delta_rate)}"
    )


def _print_exact_bucket_comparison(
    bun_buckets: list[tuple[int, Decimal]],
    tswn_buckets: list[tuple[int, int, Decimal]],
) -> None:
    """输出按累计场数对齐后的真实胜场差。"""
    all_rounds = sorted({rounds for rounds, _rate in bun_buckets} | {total for total, _wins, _rate in tswn_buckets})
    if not all_rounds:
        return

    bun_map = {rounds: rate for rounds, rate in bun_buckets}
    tswn_map = {total: (wins, rate) for total, wins, rate in tswn_buckets}

    print("  实际分段差 (bun → tswn):")
    printed = False
    for rounds in all_rounds:
        bun_rate = bun_map.get(rounds)
        tswn_bucket = tswn_map.get(rounds)
        if bun_rate is None or tswn_bucket is None:
            continue

        bun_wins = infer_exact_wins_from_displayed_rate(bun_rate, rounds)
        if bun_wins is None:
            print(f"    {rounds:>6}场: <无法从 bun {bun_rate}% 唯一反推胜场>")
            printed = True
            continue

        tswn_wins, tswn_rate = tswn_bucket
        delta_wins = tswn_wins - bun_wins
        if delta_wins == 0:
            continue

        bun_exact = exact_rate_percent(bun_wins, rounds)
        tswn_exact = exact_rate_percent(tswn_wins, rounds)
        delta_rate = tswn_exact - bun_exact
        print(
            f"    {rounds:>6}场: "
            f"bun={bun_wins}/{rounds} ({format_exact_rate(bun_exact)}) → "
            f"tswn={tswn_wins}/{rounds} ({format_exact_rate(tswn_exact)}) "
            f"Δwins={delta_wins:+d} Δrate={format_signed_exact_rate(delta_rate)}"
        )
        printed = True

    if not printed:
        print("    <全部分段胜场一致>")


def _print_bucket_comparison(
    bun_buckets: list[tuple[int, Decimal]],
    tswn_buckets: list[tuple[int, int, Decimal]],
) -> None:
    """并排输出 bun / tswn 的分段累积胜率对比。"""
    # 对齐同一个累积场数进行比较
    all_rounds = sorted(
        {rounds for rounds, _rate in bun_buckets}
        | {total for total, _wins, _rate in tswn_buckets}
    )
    if not all_rounds:
        return

    bun_map = {r: rate for r, rate in bun_buckets}
    tswn_map = {total: rate for total, _wins, rate in tswn_buckets}

    print("  分段对比 (bun → tswn):")
    for rounds in all_rounds:
        bun_rate = bun_map.get(rounds)
        tswn_rate = tswn_map.get(rounds)
        bun_str = f"{bun_rate}%" if bun_rate is not None else "---"
        tswn_str = f"{tswn_rate}%" if tswn_rate is not None else "---"
        if bun_rate is not None and tswn_rate is not None:
            delta = tswn_rate - bun_rate
            delta_str = f"({delta:+.2f})"
        else:
            delta_str = ""
        print(f"    {rounds:>6}场: {bun_str:>8} → {tswn_str:>8} {delta_str}")


def resolve_rounds_for_retest(item: MismatchRow, cli_rounds: int | None) -> int:
    """确定重测使用的场数。"""
    if cli_rounds is not None:
        return cli_rounds
    if item.bun_buckets:
        return max(rounds for rounds, _rate in item.bun_buckets)
    return 10000


def run_retest(
    mismatches: list[MismatchRow],
    replied_messages: dict[str, MessageRow],
    args: argparse.Namespace,
) -> None:
    """对每个 mismatch 重跑 tswn-cli，输出新旧对比。"""
    tswn_bin = resolve_tswn_bin(args)

    # 快速检查 tswn-cli 是否可用
    try:
        probe = subprocess.run(
            [*tswn_bin, "--version"],
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
        )
    except Exception:
        print(
            f"无法启动 tswn-cli: {' '.join(tswn_bin)}",
            file=sys.stderr,
        )
        return

    print(f"tswn-cli: {probe.stdout.strip() or probe.stderr.strip()}")
    print(f"mismatch 总数: {len(mismatches)}")
    print()

    tested = 0
    skipped = 0
    still_diff = 0
    now_match = 0
    errors = 0

    for index, item in enumerate(mismatches, start=1):
        groups = resolve_retest_groups(item, replied_messages)

        if len(groups) < 2:
            # 单组 / 空组：跳过（评分模式，无法对比胜率）
            if not args.retest_only_diff:
                print(
                    f"[{index}/{len(mismatches)}] {item.message.date} {item.message.username} "
                    f"id={item.message.row_id}"
                )
                print(f"  跳过: 只有 {len(groups)} 组（需 2 组才能跑胜率）")
                if groups:
                    print(f"  组内容: {groups[0][:120]}")
                print()
            skipped += 1
            continue

        group1, group2 = groups[0], groups[1]
        rounds = resolve_rounds_for_retest(item, args.retest_rounds)

        if not args.retest_only_diff:
            print(
                f"[{index}/{len(mismatches)}] {item.message.date} {item.message.username} "
                f"id={item.message.row_id}"
            )
            print(f"  旧: bun={item.bun_2dp}%  tswn(old)={item.tswn}%  diff={item.diff:+.2f}")
            print(f"  队1: {group1[:120]}")
            print(f"  队2: {group2[:120]}")

        buckets_step = args.retest_buckets_step if args.retest_buckets_step > 0 else None
        result = run_tswn_win_rate(
            tswn_bin, group1, group2, rounds, args.retest_timeout, buckets_step
        )
        tested += 1

        if result is None:
            errors += 1
            if not args.retest_only_diff:
                print("  新: [错误] 无法获取 tswn 胜率")
                print()
            continue

        new_diff = result.rate - item.bun_2dp
        if new_diff == Decimal("0"):
            now_match += 1
            if not args.retest_only_diff:
                print(f"  新: tswn(now)={result.rate}%  diff(now)={new_diff:+.2f}  ✓ 已修复")
                _print_exact_summary_comparison(item.bun_2dp, result)
                if result.buckets:
                    _print_bucket_comparison(item.bun_buckets, result.buckets)
                    _print_exact_bucket_comparison(item.bun_buckets, result.buckets)
                print()
        else:
            still_diff += 1
            diff_symbol = "↓" if new_diff < item.diff else ("↑" if new_diff > item.diff else "→")
            # retest_only_diff 模式下也输出这一条
            if args.retest_only_diff:
                print(
                    f"[{index}/{len(mismatches)}] {item.message.date} {item.message.username} "
                    f"id={item.message.row_id}"
                )
                print(f"  旧: bun={item.bun_2dp}%  tswn(old)={item.tswn}%  diff={item.diff:+.2f}")
                print(f"  队1: {group1[:120]}")
                print(f"  队2: {group2[:120]}")
            print(
                f"  新: tswn(now)={result.rate}%  diff(now)={new_diff:+.2f}  {diff_symbol} 仍不一致"
            )
            _print_exact_summary_comparison(item.bun_2dp, result)
            if result.buckets:
                _print_bucket_comparison(item.bun_buckets, result.buckets)
                _print_exact_bucket_comparison(item.bun_buckets, result.buckets)
            print()

    # 汇总
    print("=" * 50)
    print(f"测试: {tested}  跳过: {skipped}  仍不一致: {still_diff}  已修复: {now_match}  错误: {errors}")
    if tested > 0:
        fix_rate = (now_match / tested) * 100
        print(f"修复率: {now_match}/{tested} = {fix_rate:.1f}%")


def append_text_report_reply_lines(lines: list[str], record: dict[str, Any]) -> None:
    if record["resolved_replies"]:
        lines.append("  replied_messages:")
        for reply in record["resolved_replies"]:
            lines.append(
                f"    - {reply['date']} {reply['time']} {reply['username']} id={reply['_id']}"
            )
            lines.append(indent_block(reply["content"], "      "))
        return

    if record["inline_reply_contents"]:
        lines.append("  replied_messages:")
        for inline_content in record["inline_reply_contents"]:
            lines.append(indent_block(inline_content, "    "))
        return

    if record["reply_ids"]:
        lines.append(f"  reply_ids={record['reply_ids']}")
        lines.append("  replied_messages: <未在 messages 中回查到原消息>")
        return

    lines.append("  replied_messages: <空>")


def append_text_report_record(lines: list[str], index: int, record: dict[str, Any]) -> None:
    lines.append(
        f"\n[{index}] {record['date']} {record['time']} {record['username']} id={record['_id']}"
    )
    lines.append(
        f"  bun_raw={record['bun_raw']} bun_2dp={record['bun_2dp']} "
        f"tswn={record['tswn']} diff={record['diff']}"
    )
    if record["bun_buckets"]:
        bucket_text = ", ".join(
            f"{bucket['rounds']}={bucket['rate']}" for bucket in record["bun_buckets"]
        )
        lines.append(f"  bun_buckets={bucket_text}")
    lines.append("  current_content:")
    lines.append(indent_block(record["content"], "    "))
    append_text_report_reply_lines(lines, record)


def render_text_report(records: list[dict[str, Any]]) -> str:
    lines: list[str] = [f"mismatch_count={len(records)}"]
    for index, record in enumerate(records, start=1):
        append_text_report_record(lines, index, record)
    return "\n".join(lines) + "\n"


def print_text_report(records: list[dict[str, Any]]) -> None:
    sys.stdout.write(render_text_report(records))


def main(argv: list[str]) -> int:
    ensure_utf8_stdio()
    args = parse_args(argv)
    if not args.dsn:
        print("缺少 DSN，请通过 --dsn 或环境变量 TSWN_PG_DSN 提供。", file=sys.stderr)
        return 1

    schema_name = validate_identifier(args.schema, "schema")
    table_name = validate_identifier(args.table, "table")

    candidate_messages = fetch_candidate_messages(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        sender_id=args.sender_id,
        content_like=args.content_like,
    )
    mismatches = find_mismatches(candidate_messages)
    reply_ids = sorted({reply_id for item in mismatches for reply_id in item.reply_ids})
    replied_messages = fetch_messages_by_ids(
        dsn=args.dsn,
        schema_name=schema_name,
        table_name=table_name,
        message_ids=reply_ids,
    )
    mismatches = dedup_mismatches(mismatches, replied_messages)
    records = build_output(mismatches, replied_messages)

    # --retest 模式：重跑 tswn-cli 并输出对比
    if args.retest:
        run_retest(mismatches, replied_messages, args)
        return 0

    # 正常文本 / JSON 输出
    if args.json:
        payload = json.dumps(records, ensure_ascii=False, indent=2)
    else:
        payload = render_text_report(records)

    if args.output:
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
