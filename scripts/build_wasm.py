#!/usr/bin/env python3
"""
构建 `tswn_wasm`，并把浏览器可直接消费的 wasm 包整理到一个结果目录。

目标目录结构（默认 `crates/tswn_wasm/dist/wasm/`）：

wasm/
  - pkg/
      - tswn_wasm.js
      - tswn_wasm_bg.wasm
      - tswn_wasm.d.ts      （若 wasm-bindgen 生成）
  - raw/
      - tswn_wasm.wasm      （cargo 原始 wasm 产物）
  - examples/
      - README.md
      - demo.html
      - demo.js
  - README.txt
  - MANIFEST.txt

用法示例：
  python scripts/build_wasm.py
  python scripts/build_wasm.py --release
  python scripts/build_wasm.py --release --clean
  python scripts/build_wasm.py -o out/wasm

说明：
- 该脚本负责：
  1) 执行 `cargo build -p tswn_wasm --target wasm32-unknown-unknown`
  2) 收集 cargo 生成的原始 `.wasm`
  3) 调用 `wasm-bindgen` 生成浏览器可直接 import 的 JS glue
  4) 收集 examples 源码
- 默认使用 `wasm-bindgen --target web`，方便直接服务静态页面 demo。
"""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Sequence


ROOT = Path(__file__).resolve().parent.parent
CRATE_DIR = ROOT / "crates" / "tswn_wasm"
EXAMPLES_DIR = CRATE_DIR / "examples"
DEFAULT_OUTPUT_DIR = CRATE_DIR / "dist" / "wasm"
CRATE_NAME = "tswn_wasm"
DEFAULT_TARGET = "wasm32-unknown-unknown"


def run(cmd: Sequence[str | Path], cwd: Path = ROOT) -> None:
    print(f"$ {' '.join(str(x) for x in cmd)}", flush=True)
    subprocess.run([str(x) for x in cmd], cwd=str(cwd), check=True)


def ensure_exists(path: Path, desc: str) -> None:
    if not path.exists():
        raise FileNotFoundError(f"找不到 {desc}: {path}")


def remove_tree(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)


def copy_file(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    print(f"[copy] {src} -> {dst}")


def copy_tree_files(src_dir: Path, dst_dir: Path) -> None:
    ensure_exists(src_dir, "目录")
    for path in sorted(src_dir.rglob("*")):
        if path.is_file():
            rel = path.relative_to(src_dir)
            copy_file(path, dst_dir / rel)


def relative_files(root: Path) -> list[Path]:
    if not root.exists():
        return []
    return sorted(path.relative_to(root) for path in root.rglob("*") if path.is_file())


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="构建 tswn_wasm 并生成浏览器可直接消费的分发目录",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "-o",
        "--output-dir",
        default=str(DEFAULT_OUTPUT_DIR),
        metavar="DIR",
        help="输出目录（默认：crates/tswn_wasm/dist/wasm）",
    )
    p.add_argument(
        "--release",
        action="store_true",
        help="使用 release 构建（默认 debug）",
    )
    p.add_argument(
        "--clean",
        action="store_true",
        help="构建前清空输出目录",
    )
    p.add_argument(
        "--target",
        default=DEFAULT_TARGET,
        help="cargo target triple（默认：wasm32-unknown-unknown）",
    )
    p.add_argument(
        "--bindgen-target",
        default="web",
        choices=["web", "bundler", "no-modules"],
        help="wasm-bindgen 生成目标（默认：web）",
    )
    p.add_argument(
        "--out-name",
        default=CRATE_NAME,
        help="wasm-bindgen 输出包名（默认：tswn_wasm）",
    )
    p.add_argument(
        "--features",
        default=None,
        help="传给 cargo 的 features（逗号分隔）",
    )
    p.add_argument(
        "--no-default-features",
        action="store_true",
        help="传给 cargo 的 --no-default-features",
    )
    p.add_argument(
        "--cargo",
        nargs=argparse.REMAINDER,
        default=[],
        help="追加 cargo build 参数（放在最后，如：--cargo -vv）",
    )
    return p.parse_args(argv)


def cargo_profile_dir(release: bool, target: str) -> Path:
    profile = "release" if release else "debug"
    return ROOT / "target" / target / profile


def raw_wasm_artifact(release: bool, target: str) -> Path:
    return cargo_profile_dir(release=release, target=target) / f"{CRATE_NAME}.wasm"


def cargo_build_args(
    release: bool,
    target: str,
    features: str | None,
    no_default_features: bool,
    extra_cargo: list[str],
) -> list[str]:
    cmd = ["cargo", "build", "-p", CRATE_NAME, "--target", target]
    if release:
        cmd.append("--release")
    if no_default_features:
        cmd.append("--no-default-features")
    if features:
        cmd += ["--features", features]
    if extra_cargo:
        cmd += extra_cargo
    return cmd


def wasm_bindgen_path() -> str:
    path = shutil.which("wasm-bindgen")
    if not path:
        raise FileNotFoundError(
            "未找到 wasm-bindgen，可先执行 `cargo install wasm-bindgen-cli` 安装。"
        )
    return path


def cargo_lock_package_version(package_name: str) -> str:
    cargo_lock = ROOT / "Cargo.lock"
    ensure_exists(cargo_lock, "Cargo.lock")
    data = tomllib.loads(cargo_lock.read_text(encoding="utf-8"))
    for package in data.get("package", []):
        if package.get("name") == package_name:
            version = package.get("version")
            if version:
                return str(version)
    raise RuntimeError(f"未能从 {cargo_lock} 解析 {package_name} 版本")


def installed_wasm_bindgen_version(wasm_bindgen: str) -> str:
    result = subprocess.run(
        [wasm_bindgen, "--version"],
        cwd=str(ROOT),
        check=True,
        capture_output=True,
        text=True,
    )
    tokens = result.stdout.strip().split()
    if not tokens:
        raise RuntimeError("无法解析 wasm-bindgen --version 输出")
    return tokens[-1]


def ensure_compatible_wasm_bindgen(wasm_bindgen: str) -> None:
    expected = cargo_lock_package_version("wasm-bindgen")
    installed = installed_wasm_bindgen_version(wasm_bindgen)
    if installed != expected:
        raise RuntimeError(
            "当前 wasm-bindgen-cli 版本与仓库依赖不匹配："
            f"installed={installed}, expected={expected}。"
            f"请执行 `cargo install -f wasm-bindgen-cli --version {expected}` 后重试。"
        )


def write_readme(output_dir: Path, bindgen_target: str, pkg_files: list[Path], example_files: list[Path]) -> None:
    lines = [
        "# tswn_wasm package",
        "",
        "本目录由 `scripts/build_wasm.py` 生成。",
        "",
        "## 内容",
        "",
        "- `pkg/`: `wasm-bindgen` 生成的浏览器可直接消费的 JS glue 与 `.wasm`。",
        "- `raw/tswn_wasm.wasm`: cargo 原始输出，便于调试或继续做其他后处理。",
        "- `examples/`: 静态页面 demo 与使用说明。",
        "",
        "## wasm-bindgen target",
        "",
        f"- 当前打包目标：`{bindgen_target}`",
        "",
        "## 运行 demo",
        "",
        "1. 切到当前目录。",
        "2. 启动静态服务器，例如：`python -m http.server 8000`。",
        "3. 打开 `http://127.0.0.1:8000/examples/demo.html`。",
        "",
        "## 生成的 pkg 文件",
        "",
    ]

    if pkg_files:
        for item in pkg_files:
            lines.append(f"- `pkg/{item.as_posix()}`")
    else:
        lines.append("- (none)")

    lines += [
        "",
        "## 示例文件",
        "",
    ]

    if example_files:
        for item in example_files:
            lines.append(f"- `examples/{item.as_posix()}`")
    else:
        lines.append("- (none)")

    lines.append("")

    path = output_dir / "README.txt"
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {path}")


def write_manifest(
    output_dir: Path,
    release: bool,
    target: str,
    bindgen_target: str,
    raw_wasm_dst: Path,
    pkg_files: list[Path],
    example_files: list[Path],
) -> None:
    lines = [
        "# tswn_wasm package manifest",
        "",
        f"profile={'release' if release else 'debug'}",
        f"cargo_target={target}",
        f"bindgen_target={bindgen_target}",
        f"platform={platform.platform()}",
        f"python={sys.version.split()[0]}",
        "",
        "[raw]",
        raw_wasm_dst.as_posix(),
        "",
        "[pkg]",
    ]

    if pkg_files:
        lines.extend(f"pkg/{item.as_posix()}" for item in pkg_files)
    else:
        lines.append("(none)")

    lines += [
        "",
        "[examples]",
    ]

    if example_files:
        lines.extend(f"examples/{item.as_posix()}" for item in example_files)
    else:
        lines.append("(none)")

    path = output_dir / "MANIFEST.txt"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"[write] {path}")


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    output_dir = Path(args.output_dir).resolve()
    pkg_dir = output_dir / "pkg"
    raw_dir = output_dir / "raw"
    examples_dir = output_dir / "examples"

    print(f"[build] root           : {ROOT}")
    print(f"[build] output         : {output_dir}")
    print(f"[build] cargo target   : {args.target}")
    print(f"[build] bindgen target : {args.bindgen_target}")
    print(f"[build] profile        : {'release' if args.release else 'debug'}")
    print()

    if args.clean and output_dir.exists():
        print(f"[clean] 清空 {output_dir}")
        remove_tree(output_dir)

    output_dir.mkdir(parents=True, exist_ok=True)
    remove_tree(pkg_dir)
    remove_tree(raw_dir)
    remove_tree(examples_dir)
    pkg_dir.mkdir(parents=True, exist_ok=True)
    raw_dir.mkdir(parents=True, exist_ok=True)
    examples_dir.mkdir(parents=True, exist_ok=True)

    run(
        cargo_build_args(
            release=args.release,
            target=args.target,
            features=args.features,
            no_default_features=args.no_default_features,
            extra_cargo=args.cargo,
        ),
        cwd=ROOT,
    )

    raw_wasm_src = raw_wasm_artifact(release=args.release, target=args.target)
    ensure_exists(raw_wasm_src, "cargo 原始 wasm 产物")
    raw_wasm_dst = Path("raw") / raw_wasm_src.name
    copy_file(raw_wasm_src, output_dir / raw_wasm_dst)

    wasm_bindgen = wasm_bindgen_path()
    ensure_compatible_wasm_bindgen(wasm_bindgen)

    run(
        [
            wasm_bindgen,
            raw_wasm_src,
            "--target",
            args.bindgen_target,
            "--out-dir",
            pkg_dir,
            "--out-name",
            args.out_name,
        ],
        cwd=ROOT,
    )

    copy_tree_files(EXAMPLES_DIR, examples_dir)

    pkg_files = relative_files(pkg_dir)
    example_files = relative_files(examples_dir)
    write_readme(output_dir, bindgen_target=args.bindgen_target, pkg_files=pkg_files, example_files=example_files)
    write_manifest(
        output_dir,
        release=args.release,
        target=args.target,
        bindgen_target=args.bindgen_target,
        raw_wasm_dst=raw_wasm_dst,
        pkg_files=pkg_files,
        example_files=example_files,
    )

    print()
    print(f"[ok] wasm package：{output_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))