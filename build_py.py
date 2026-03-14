#!/usr/bin/env python3
"""
tswn_py wheel 构建脚本

用法（推荐）：
  uv run python build_py.py [options]

常用示例：
  uv run python build_py.py                        # 标准构建（隔离环境）
  uv run python build_py.py --no-isolation          # 快速构建（复用当前环境）
  uv run python build_py.py --no-isolation --clean  # 清理后快速构建
  uv run python build_py.py --verify                # 构建并验证 import

参数：
  -o, --output-dir DIR   wheel 输出目录（默认：crates/tswn_py/dist）
  --clean                构建前清空输出目录
  --no-isolation         跳过 PEP 517 隔离，直接使用当前 Python 环境（更快）
                         要求当前环境已安装：setuptools, setuptools-rust, wheel
  --verify               构建完成后安装 wheel 并验证 import
"""

from __future__ import annotations

import argparse
import importlib.util
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CRATE_DIR = ROOT / "crates" / "tswn_py"


def run(cmd: list[str | Path], cwd: Path = ROOT) -> None:
    print(f"$ {' '.join(str(x) for x in cmd)}", flush=True)
    subprocess.run(cmd, cwd=str(cwd), check=True)


def ensure_build_frontend() -> None:
    """确保当前环境有 build 包（构建前端）。"""
    if importlib.util.find_spec("build") is not None:
        return
    print("[info] 未找到 `build` 包，正在通过 uv 安装...")
    run(["uv", "pip", "install", "build"])
    # 重新检查
    if importlib.util.find_spec("build") is None:
        print("[error] 安装 `build` 失败，请手动执行：uv pip install build", file=sys.stderr)
        raise SystemExit(1)


def find_latest_wheel(output_dir: Path) -> Path | None:
    wheels = sorted(output_dir.glob("tswn_py-*.whl"))
    return wheels[-1] if wheels else None


def verify_wheel(wheel: Path) -> None:
    print(f"\n[verify] 安装 wheel：{wheel.name}")
    run(["uv", "pip", "install", "--force-reinstall", str(wheel)])
    print("[verify] 验证 import ...")
    run([
        sys.executable, "-c",
        (
            "import tswn_py; "
            "print('  wrapper_version_str():', tswn_py.wrapper_version_str()); "
            "print('  core_version_str():   ', tswn_py.core_version_str())"
        ),
    ])
    print("[verify] OK")


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="构建 tswn_py wheel",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "-o", "--output-dir",
        default=str(CRATE_DIR / "dist"),
        metavar="DIR",
        help="wheel 输出目录（默认：crates/tswn_py/dist）",
    )
    p.add_argument(
        "--clean",
        action="store_true",
        help="构建前清空输出目录",
    )
    p.add_argument(
        "--no-isolation",
        action="store_true",
        help="跳过 PEP 517 隔离，直接使用当前 Python 环境（需已安装 setuptools-rust）",
    )
    p.add_argument(
        "--verify",
        action="store_true",
        help="构建完成后安装 wheel 并验证 import",
    )
    return p.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    output_dir = Path(args.output_dir).resolve()

    print(f"[build] crate    : {CRATE_DIR}")
    print(f"[build] output   : {output_dir}")
    print(f"[build] python   : {sys.executable}  ({sys.version.split()[0]})")
    print(f"[build] isolation: {'off (--no-isolation)' if args.no_isolation else 'on'}")
    print()

    # 1) 清理
    if args.clean and output_dir.exists():
        print(f"[clean] 清空 {output_dir}")
        shutil.rmtree(output_dir)

    output_dir.mkdir(parents=True, exist_ok=True)

    # 2) 确保 build 前端可用
    ensure_build_frontend()

    # 3) 执行 python -m build --wheel
    cmd: list[str | Path] = [
        sys.executable, "-m", "build",
        "--wheel",
        "-o", str(output_dir),
    ]
    if args.no_isolation:
        cmd.append("--no-isolation")

    # build 的最后一个位置参数是 srcdir（即 pyproject.toml 所在目录）
    cmd.append(str(CRATE_DIR))

    run(cmd)

    # 4) 报告产物
    wheel = find_latest_wheel(output_dir)
    if wheel is None:
        print("\n[error] 构建完成但未找到 wheel 文件，请检查上方输出。", file=sys.stderr)
        return 1

    print(f"\n[ok] 产出：{wheel}")

    # 5) 可选验证
    if args.verify:
        verify_wheel(wheel)

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))