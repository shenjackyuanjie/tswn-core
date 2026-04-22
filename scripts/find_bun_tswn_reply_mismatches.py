#!/usr/bin/env python3
"""筛出 bun / tswn 胜率不一致的消息，并回查其回复的原始消息内容。"""

from __future__ import annotations

import argparse
import json
import os
import re
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
SAFE_IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
REPLY_ID_KEYS = {"_id", "id", "messageId", "msgId"}
INLINE_CONTENT_KEYS = {"content", "text", "message"}


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
        order by time
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


def build_output(
    mismatches: list[MismatchRow],
    replied_messages: dict[str, MessageRow],
) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for item in mismatches:
        resolved_replies = [
            replied_messages[reply_id]
            for reply_id in item.reply_ids
            if reply_id in replied_messages
        ]
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


def print_text_report(records: list[dict[str, Any]]) -> None:
    print(f"mismatch_count={len(records)}")
    for index, record in enumerate(records, start=1):
        print(f"\n[{index}] {record['date']} {record['time']} {record['username']} id={record['_id']}")
        print(
            f"  bun_raw={record['bun_raw']} bun_2dp={record['bun_2dp']} "
            f"tswn={record['tswn']} diff={record['diff']}"
        )
        print("  current_content:")
        print(indent_block(record["content"], "    "))

        if record["resolved_replies"]:
            print("  replied_messages:")
            for reply in record["resolved_replies"]:
                print(f"    - {reply['date']} {reply['time']} {reply['username']} id={reply['_id']}")
                print(indent_block(reply["content"], "      "))
        elif record["inline_reply_contents"]:
            print("  replied_messages:")
            for inline_content in record["inline_reply_contents"]:
                print(indent_block(inline_content, "    "))
        elif record["reply_ids"]:
            print(f"  reply_ids={record['reply_ids']}")
            print("  replied_messages: <未在 messages 中回查到原消息>")
        else:
            print("  replied_messages: <空>")


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
    records = build_output(mismatches, replied_messages)

    if args.json:
        payload = json.dumps(records, ensure_ascii=False, indent=2)
    else:
        lines: list[str] = [f"mismatch_count={len(records)}"]
        for index, record in enumerate(records, start=1):
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

            if record["resolved_replies"]:
                lines.append("  replied_messages:")
                for reply in record["resolved_replies"]:
                    lines.append(
                        f"    - {reply['date']} {reply['time']} {reply['username']} id={reply['_id']}"
                    )
                    lines.append(indent_block(reply["content"], "      "))
            elif record["inline_reply_contents"]:
                lines.append("  replied_messages:")
                for inline_content in record["inline_reply_contents"]:
                    lines.append(indent_block(inline_content, "    "))
            elif record["reply_ids"]:
                lines.append(f"  reply_ids={record['reply_ids']}")
                lines.append("  replied_messages: <未在 messages 中回查到原消息>")
            else:
                lines.append("  replied_messages: <空>")
        payload = "\n".join(lines) + "\n"

    if args.output:
        output_path = Path(args.output)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
