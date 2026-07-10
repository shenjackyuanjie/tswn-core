#!/usr/bin/env python3
"""Run Runtime v2 release gates with rustc's default mutable noalias enabled."""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASE_COMMANDS = [
    ["cargo", "test", "-p", "tswn_core", "runtime_v2", "--lib", "--release"],
    [
        "cargo",
        "test",
        "-p",
        "tswn_core",
        "--features",
        "no_debug",
        "runtime_v2",
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
        "runtime_v2",
        "--release",
    ],
]
CORPUS_COMMAND = [
    "cargo",
    "test",
    "-p",
    "tswn_test",
    "--features",
    "runtime-v2-corpus",
    "--test",
    "runtime_v2",
    "--release",
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="验证 Runtime v2 在 mutable-noalias=yes 下的 release 行为"
    )
    parser.add_argument(
        "--corpus",
        action="store_true",
        help="同时执行当前尚在收敛中的完整 tswn_test v2 corpus",
    )
    args = parser.parse_args()

    env = os.environ.copy()
    env["CARGO_ENCODED_RUSTFLAGS"] = "-Z\x1fmutable-noalias=yes"
    commands = [*BASE_COMMANDS]
    if args.corpus:
        commands.append(CORPUS_COMMAND)
    else:
        commands.append([*CORPUS_COMMAND, "--no-run"])

    for command in commands:
        print(f"[runtime-v2/noalias] {' '.join(command)}", flush=True)
        subprocess.run(command, cwd=ROOT, env=env, check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
