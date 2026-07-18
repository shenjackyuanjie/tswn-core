#!/usr/bin/env python3
"""Run the main Runtime release gates."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REMOVED_PATHS = [
    "crates/tswn_core/src/engine",
    "crates/tswn_core/src/player",
    "crates/tswn_core/src/error.rs",
    "crates/tswn_core/src/runtime/oracle.rs",
]
BANNED_RUST_TOKENS = [
    "LegacyRunner",
    "LegacyPreparedRunner",
    "tswn_core::legacy",
    "tswn_core::engine",
    "tswn_core::player",
    "crate::engine",
    "crate::player",
    "engine::runners",
    "RuntimeEngineArg",
    "RuntimeParity",
    "default_custom_runtime_parity",
]
SOURCE_ROOTS = [
    ROOT / "crates" / "tswn_core" / "src",
    ROOT / "crates" / "tswn_capi" / "src",
    ROOT / "crates" / "tswn_py" / "src",
    ROOT / "crates" / "tswn_wasm" / "src",
    ROOT / "crates" / "tswn_openbox" / "src",
    ROOT / "crates" / "tswn_ladder" / "src",
    ROOT / "crates" / "tswn_lane_ranker" / "src",
    ROOT / "crates" / "tswn_test" / "src",
]
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


def verify_runtime_independence() -> None:
    failures: list[str] = []
    for relative in REMOVED_PATHS:
        path = ROOT / relative
        if path.is_file() or (path.is_dir() and any(path.rglob("*.rs"))):
            failures.append(f"removed path still exists: {relative}")

    for source_root in SOURCE_ROOTS:
        for path in source_root.rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            for token in BANNED_RUST_TOKENS:
                if token in source:
                    failures.append(f"{path.relative_to(ROOT)} contains {token!r}")

    cli_sources = [
        ROOT / "crates/tswn_core/src/bin/tswn_cli/args/cli.rs",
        ROOT / "crates/tswn_core/src/bin/tswn_cli/args/parsed.rs",
        ROOT / "crates/tswn_core/src/bin/tswn_cli/main.rs",
    ]
    for path in cli_sources:
        source = path.read_text(encoding="utf-8")
        for token in ("--runtime", "RuntimeParity", "runtime parity"):
            if token in source:
                failures.append(f"{path.relative_to(ROOT)} still exposes {token!r}")

    if failures:
        raise SystemExit("主 Runtime 独立性检查失败:\n- " + "\n- ".join(failures))


def verify_corpus_inventory() -> None:
    golden = (ROOT / "crates/tswn_test/src/golden.rs").read_text(encoding="utf-8")
    corpus = (ROOT / "crates/tswn_test/runtime_corpus.rs").read_text(encoding="utf-8")

    def constant(name: str) -> int:
        match = re.search(rf"pub const {name}: usize = (\d+);", golden)
        if match is None:
            raise SystemExit(f"未找到 corpus 常量 {name}")
        return int(match.group(1))

    exact_count = constant("EXACT_TRACE_CASE_COUNT")
    stress_count = constant("STRESS_CASE_COUNT")
    compiled_stress_tests = len(re.findall(r"(?m)^#\[test\]\s*$", corpus))
    if (exact_count, stress_count, compiled_stress_tests) != (87, 37, 37):
        raise SystemExit(
            "Runtime corpus 清单异常: "
            f"exact={exact_count}, stress={stress_count}, compiled_stress={compiled_stress_tests}"
        )
    if exact_count + stress_count != 124:
        raise SystemExit(f"Runtime corpus 总数应为 124，实际为 {exact_count + stress_count}")


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

    verify_runtime_independence()
    verify_corpus_inventory()
    print("[runtime/release] independence and 124-case inventory checks passed", flush=True)

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
