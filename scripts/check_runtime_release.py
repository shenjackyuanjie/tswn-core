#!/usr/bin/env python3
"""Run the main Runtime release gates."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASE_COMMANDS = [
    ["cargo", "test", "-p", "tswn_core", "runtime", "--lib", "--release"],
    [
        "cargo",
        "test",
        "-p",
        "tswn_core",
        "--features",
        "no_debug",
        "runtime",
        "--lib",
        "--release",
    ],
    [
        "cargo",
        "test",
        "-p",
        "tswn_core",
        "--bin",
        "tswn-cli",
        "runtime",
        "--release",
    ],
]
CORPUS_COMMAND = [
    "cargo",
    "test",
    "-p",
    "tswn_test",
    "--features",
    "runtime-corpus",
    "--test",
    "runtime",
    "--release",
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="验证主 Runtime 的 release 行为"
    )
    parser.add_argument(
        "--corpus",
        action="store_true",
        help="同时执行完整 tswn_test runtime corpus",
    )
    args = parser.parse_args()

    commands = [*BASE_COMMANDS]
    if args.corpus:
        commands.append(CORPUS_COMMAND)
    else:
        commands.append([*CORPUS_COMMAND, "--no-run"])

    for command in commands:
        print(f"[runtime/release] {' '.join(command)}", flush=True)
        subprocess.run(command, cwd=ROOT, check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
