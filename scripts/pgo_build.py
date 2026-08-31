#!/usr/bin/env python3
"""按 PGO（profile-guided optimization）流程构建 tswn 二进制。

三步走：

1. `-Cprofile-generate` 构建带插桩的二进制；
2. 用固定训练输入跑一遍，产出 `.profraw`；
3. `llvm-profdata merge` 后用 `-Cprofile-use` 重新构建。

训练输入默认使用 `docs/perf/fixed_cases_30`（覆盖 1v1 / 2v2 / ffa / 3v3v3）
加一份评分输入，全部单线程执行：LLVM 的 IR 插桩计数器不是原子的，多线程训练
会丢计数。

用法示例：

```powershell
python scripts/pgo_build.py
python scripts/pgo_build.py --train-runs 8000 --out-dir target/pgo
```
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT_DIR = ROOT / "target" / "pgo"
DEFAULT_TRAIN_DIR = ROOT / "docs" / "perf" / "fixed_cases_30"
SCORE_TRAIN_INPUT = ROOT / "docs" / "perf" / "pgo_training" / "score.txt"


class PgoError(RuntimeError):
    """PGO 流程中的可预期失败。"""


def run(command: list[str], *, env: dict[str, str] | None = None, cwd: Path = ROOT, quiet: bool = False) -> None:
    printable = " ".join(command)
    print(f"[pgo] $ {printable}", flush=True)
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.DEVNULL if quiet else None,
        stderr=None,
    )
    if result.returncode != 0:
        raise PgoError(f"命令失败（exit {result.returncode}）：{printable}")


def rustc_llvm_major() -> int:
    output = subprocess.run(["rustc", "-vV"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    match = re.search(r"^LLVM version: (\d+)", output, re.MULTILINE)
    if match is None:
        raise PgoError("无法从 rustc -vV 读取 LLVM 版本")
    return int(match.group(1))


def profdata_llvm_major(profdata: str) -> int:
    output = subprocess.run([profdata, "--version"], capture_output=True, text=True, check=True).stdout
    match = re.search(r"LLVM version (\d+)", output)
    if match is None:
        raise PgoError(f"无法从 {profdata} --version 读取 LLVM 版本")
    return int(match.group(1))


def resolve_profdata(explicit: str | None) -> str:
    candidates: list[str] = []
    if explicit:
        candidates.append(explicit)
    else:
        found = shutil.which("llvm-profdata")
        if found:
            candidates.append(found)
        # rustup component add llvm-tools-preview 装出来的版本一定和 rustc 对齐。
        sysroot = subprocess.run(
            ["rustc", "--print", "sysroot"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.strip()
        for host_bin in (Path(sysroot) / "lib" / "rustlib").glob("*/bin"):
            for name in ("llvm-profdata.exe", "llvm-profdata"):
                candidate = host_bin / name
                if candidate.exists():
                    candidates.append(str(candidate))
    if not candidates:
        raise PgoError(
            "找不到 llvm-profdata。安装方式二选一：\n"
            "  rustup component add llvm-tools-preview\n"
            "  或安装与 rustc 同 LLVM 大版本的独立 LLVM，并用 --llvm-profdata 指定路径"
        )

    wanted = rustc_llvm_major()
    mismatched: list[str] = []
    for candidate in candidates:
        try:
            major = profdata_llvm_major(candidate)
        except (subprocess.CalledProcessError, OSError, PgoError):
            continue
        if major == wanted:
            print(f"[pgo] llvm-profdata: {candidate} (LLVM {major})")
            return candidate
        mismatched.append(f"{candidate} (LLVM {major})")

    detail = "；".join(mismatched) if mismatched else "无可用候选"
    raise PgoError(f"llvm-profdata 的 LLVM 大版本必须等于 rustc 的 LLVM {wanted}，当前：{detail}")


def cargo_env(extra_rustflags: str, keep_wrapper: bool) -> dict[str, str]:
    env = dict(os.environ)
    existing = env.get("RUSTFLAGS", "").strip()
    env["RUSTFLAGS"] = f"{existing} {extra_rustflags}".strip()
    if not keep_wrapper:
        # profile-generate 的产物不可缓存，且 wrapper 在 RUSTFLAGS 变化时容易超时。
        env.pop("RUSTC_WRAPPER", None)
        env.pop("RUSTC_WORKSPACE_WRAPPER", None)
    return env


def cargo_build(package: str, binary: str, features: str, target_dir: Path, env: dict[str, str]) -> None:
    command = ["cargo", "build", "--release", "-p", package, "--bin", binary, "--target-dir", str(target_dir)]
    if features:
        command += ["--features", features]
    run(command, env=env)


def train_inputs(train_dir: Path) -> list[Path]:
    inputs = sorted(train_dir.glob("*.txt"))
    if not inputs:
        raise PgoError(f"训练输入目录为空：{train_dir}")
    return inputs


def run_training(binary: Path, inputs: list[Path], runs: int, score_input: Path | None) -> None:
    print(f"[pgo] 训练：{len(inputs)} 个胜率输入 x {runs} 场（单线程）", flush=True)
    for path in inputs:
        run(
            [str(binary), "bench", "auto", "-f", str(path), "-n", str(runs), "-s"],
            quiet=True,
        )
    if score_input is not None and score_input.exists():
        print("[pgo] 训练：评分路径", flush=True)
        run([str(binary), "bench", "auto", "-f", str(score_input), "-n", str(runs), "-s"], quiet=True)


def merge_profiles(profdata: str, raw_dir: Path, out_file: Path) -> None:
    raws = list(raw_dir.glob("*.profraw"))
    if not raws:
        raise PgoError(f"训练没有产出 .profraw：{raw_dir}")
    print(f"[pgo] 合并 {len(raws)} 份 profraw -> {out_file}", flush=True)
    run([profdata, "merge", "-o", str(out_file), str(raw_dir)])


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="PGO 构建 tswn 二进制")
    parser.add_argument("--package", default="tswn_core", help="cargo package（默认 tswn_core）")
    parser.add_argument("--bin", dest="binary", default="tswn-cli", help="要构建的 bin（默认 tswn-cli）")
    parser.add_argument(
        "--features",
        default="no_debug",
        help="追加的 cargo features，逗号分隔；传空字符串表示不追加（默认 no_debug）",
    )
    parser.add_argument("--out-dir", default=str(DEFAULT_OUT_DIR), help=f"PGO 工作目录（默认 {DEFAULT_OUT_DIR}）")
    parser.add_argument("--train-dir", default=str(DEFAULT_TRAIN_DIR), help="训练输入目录")
    parser.add_argument("--train-runs", type=int, default=5000, help="每个训练输入跑多少场（默认 5000）")
    parser.add_argument("--llvm-profdata", default=None, help="llvm-profdata 路径；默认自动查找")
    parser.add_argument("--skip-train", action="store_true", help="复用已有 profdata，只做 profile-use 构建")
    parser.add_argument("--keep-rustc-wrapper", action="store_true", help="保留 RUSTC_WRAPPER（默认在 PGO 构建中清掉）")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    out_dir = Path(args.out_dir)
    if not out_dir.is_absolute():
        out_dir = ROOT / out_dir
    raw_dir = out_dir / "profraw"
    profdata_file = out_dir / "merged.profdata"
    gen_target = out_dir / "build-generate"
    use_target = out_dir / "build-use"

    try:
        profdata = resolve_profdata(args.llvm_profdata)

        if not args.skip_train:
            if raw_dir.exists():
                shutil.rmtree(raw_dir)
            raw_dir.mkdir(parents=True, exist_ok=True)

            gen_env = cargo_env(f"-Cprofile-generate={raw_dir}", args.keep_rustc_wrapper)
            cargo_build(args.package, args.binary, args.features, gen_target, gen_env)

            instrumented = gen_target / "release" / (args.binary + (".exe" if os.name == "nt" else ""))
            if not instrumented.exists():
                raise PgoError(f"插桩二进制不存在：{instrumented}")
            score_input = SCORE_TRAIN_INPUT if SCORE_TRAIN_INPUT.exists() else None
            run_training(instrumented, train_inputs(Path(args.train_dir)), args.train_runs, score_input)
            merge_profiles(profdata, raw_dir, profdata_file)
        elif not profdata_file.exists():
            raise PgoError(f"--skip-train 需要已存在的 profdata：{profdata_file}")

        use_env = cargo_env(f"-Cprofile-use={profdata_file}", args.keep_rustc_wrapper)
        cargo_build(args.package, args.binary, args.features, use_target, use_env)
    except PgoError as error:
        print(f"[pgo] 失败：{error}", file=sys.stderr)
        return 1

    optimized = use_target / "release" / (args.binary + (".exe" if os.name == "nt" else ""))
    print()
    print(f"[pgo] profdata : {profdata_file}")
    print(f"[pgo] 优化产物 : {optimized}")
    print("[pgo] 提醒：正式留档请记录被测 commit 与 profdata 的生成参数，不同 profile 的结果不可直接比较。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
