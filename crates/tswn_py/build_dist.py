#!/usr/bin/env python3
"""
一键构建 tswn_py，并把产物整理成可直接 import 的 dist/tswn_py 包目录。

目标结构：
crates/tswn_py/dist/tswn_py/
  - __init__.py
  - tswn_py.*.(pyd|so|dylib)   (根据平台不同)

使用：
  python build_dist.py
可选参数：
  --release            使用 release 构建（默认 debug）
  --clean-dist         先删除 dist/tswn_py 再生成
  --python PYTHON      指定 python 可执行文件（仅用于 sanity check，可不传）
  --target TRIPLE      指定 cargo target（交叉编译时使用）
  --features "a,b"     传给 cargo 的 features（逗号分隔）
  --no-default-features
  --cargo ARGS...      追加 cargo build 参数（放在最后；例：--cargo -vv）
"""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


def _project_dirs() -> tuple[Path, Path, Path, Path]:
    crate_dir = Path(__file__).resolve().parent
    dist_dir = crate_dir / "dist"
    pkg_dir = dist_dir / "tswn_py"
    src_init = crate_dir / "__init__.py"
    return crate_dir, dist_dir, pkg_dir, src_init


def _run(cmd: list[str], cwd: Path) -> None:
    print(f"+ ({cwd}) {' '.join(cmd)}")
    subprocess.run(cmd, cwd=str(cwd), check=True)


def _ext_suffix_candidates() -> list[str]:
    # 解释：
    # - Windows: cargo cdylib 通常产出 .dll；Python 扩展模块需要 .pyd
    # - Linux: .so
    # - macOS:  .dylib (cdylib) 以及有时会出现 .so（取决于工具链/配置）
    system = platform.system().lower()
    if system == "windows":
        return [".pyd", ".dll"]
    if system == "darwin":
        return [".dylib", ".so"]
    return [".so"]


def _find_built_artifact(crate_dir: Path, release: bool, target: str | None) -> Path:
    # cargo 构建产物目录
    # - 默认：{workspace}/target/{debug|release}
    # - 指定 target：{workspace}/target/{triple}/{debug|release}
    profile = "release" if release else "debug"

    # 正确的 workspace 根目录推导：
    # crate_dir = tswn-core/crates/tswn_py
    # workspace_dir 应为 tswn-core
    workspace_dir = crate_dir.parents[1]
    target_dir = workspace_dir / "target"

    if target:
        out_dir = target_dir / target / profile
    else:
        out_dir = target_dir / profile

    name = "tswn_py"
    suffixes = _ext_suffix_candidates()

    # 优先找精确文件名：tswn_py{suffix}
    for suf in suffixes:
        p = out_dir / f"{name}{suf}"
        if p.exists():
            return p

    # 再退一步：找包含 tswn_py 的动态库文件（避免某些平台前缀 lib）
    # 例如 Linux/macOS 常见：libtswn_py.so / libtswn_py.dylib
    for suf in suffixes:
        candidates = sorted(out_dir.glob(f"*{name}*{suf}"))
        for c in candidates:
            if c.is_file():
                return c

    raise FileNotFoundError(
        f"找不到构建产物：已检查目录 {out_dir}，后缀候选 {suffixes}。"
        f"请确认已成功 cargo build，并且 crate-type=cdylib。"
    )


def _ensure_empty_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def _rmtree_if_exists(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)


def _copy_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


def _cargo_build_args(
    release: bool,
    target: str | None,
    features: str | None,
    no_default_features: bool,
    extra_cargo: list[str],
) -> list[str]:
    cmd = ["cargo", "build"]
    if release:
        cmd.append("--release")
    if target:
        cmd += ["--target", target]
    if no_default_features:
        cmd.append("--no-default-features")
    if features:
        # cargo 接受用逗号分隔或空格分隔；这里保持用户输入（逗号）直接传
        cmd += ["--features", features]
    if extra_cargo:
        cmd += extra_cargo
    return cmd


def _parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(description="构建 tswn_py 并生成 dist/tswn_py 包目录")
    p.add_argument("--release", action="store_true", help="使用 release 构建（默认 debug）")
    p.add_argument("--clean-dist", action="store_true", help="先删除 dist/tswn_py 再生成")
    p.add_argument("--python", default=None, help="指定 python 可执行文件（可选）")
    p.add_argument("--target", default=None, help="cargo target triple（可选）")
    p.add_argument("--features", default=None, help="cargo features（逗号分隔），例如 a,b")
    p.add_argument("--no-default-features", action="store_true", help="传给 cargo 的 --no-default-features")
    p.add_argument(
        "--cargo",
        nargs=argparse.REMAINDER,
        default=[],
        help="追加 cargo build 参数（放在最后，如：--cargo -vv）",
    )
    return p.parse_args(argv)


def _sanity_import(python_exe: str, pkg_dir: Path) -> None:
    # 用 dist 目录作为 import 源
    # python -c "import sys; sys.path.insert(0, 'dist'); import tswn_py; print(tswn_py.get_version_str())"
    dist_dir = pkg_dir.parent
    cmd = [
        python_exe,
        "-c",
        (
            "import sys; "
            f"sys.path.insert(0, r'{dist_dir.as_posix()}'); "
            "import tswn_py; "
            "print('tswn_py imported, version=', tswn_py.get_version_str())"
        ),
    ]
    _run(cmd, cwd=dist_dir)


def _dst_extension_suffix() -> str:
    # dist 中扩展模块的目标后缀：Windows 必须是 .pyd
    return ".pyd" if os.name == "nt" else ".so"


def main(argv: list[str]) -> int:
    args = _parse_args(argv)

    crate_dir, dist_dir, pkg_dir, src_init = _project_dirs()

    if not src_init.exists():
        raise FileNotFoundError(f"找不到 {src_init}，请确认该文件存在。")

    if args.clean_dist:
        _rmtree_if_exists(pkg_dir)

    _ensure_empty_dir(pkg_dir)

    # 1) cargo build
    cmd = _cargo_build_args(
        release=args.release,
        target=args.target,
        features=args.features,
        no_default_features=args.no_default_features,
        extra_cargo=args.cargo,
    )
    _run(cmd, cwd=crate_dir)

    # 2) 把构建产物复制到 dist/tswn_py/
    artifact = _find_built_artifact(crate_dir=crate_dir, release=args.release, target=args.target)

    # Rust 模块名是 #[pyo3(name="tswn_py")]，并且 __init__.py 会 `from .tswn_py import *`
    # 因此 dist 中的扩展模块文件名必须是 tswn_py.{pyd|so}
    dst_artifact = pkg_dir / f"tswn_py{_dst_extension_suffix()}"

    print(f"artifact: {artifact}")
    print(f"-> copy to: {dst_artifact}")
    _copy_file(artifact, dst_artifact)

    # 3) 复制 __init__.py
    _copy_file(src_init, pkg_dir / "__init__.py")

    # 4) 可选：import sanity check
    if args.python:
        _sanity_import(args.python, pkg_dir)

    print(f"OK: 已生成 {pkg_dir}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
