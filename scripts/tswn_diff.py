#!/usr/bin/env python3
"""查询和复核历史 Bun / tswn 差异。"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable

from _tswn_diff import common, pf, rate, round


Command = Callable[[list[str]], int]
COMMANDS: dict[str, Command] = {
    "rate": rate.main,
    "pf": pf.main,
    "round": round.main,
}


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="查询和复核历史 Bun / tswn 差异。",
        epilog=(
            "子命令：rate 检查胜率；pf 检查 /namer-pf 评分；"
            "round 定位逐局胜负分叉。\n\n"
            "示例：python scripts/tswn_diff.py rate --retest"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("command", choices=COMMANDS, help="要执行的差异检查。")
    parser.add_argument("args", nargs=argparse.REMAINDER, help="传递给子命令的参数。")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    common.ensure_utf8_stdio()
    args = parse_args(argv)
    return COMMANDS[args.command](args.args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
