#!/usr/bin/env python3
"""
构建 `tswn_capi`，并把可分发内容整理到一个结果目录。

目标目录结构（默认 `crates/tswn_capi/dist/capi/`）：

capi/
  - include/
      - tswn_capi.h
  - lib/
      - tswn_capi.dll / libtswn_capi.so / libtswn_capi.dylib
      - tswn_capi.dll.lib / tswn_capi.lib / *.pdb   （若存在则一并复制）
  - examples/
      - README.md
      - common.h
      - *.c

用法示例：
  python scripts/build_capi.py
  python scripts/build_capi.py --release
  python scripts/build_capi.py --release --clean
  python scripts/build_capi.py -o out/capi
  python scripts/build_capi.py --release --target x86_64-pc-windows-msvc
  python scripts/build_capi.py --release --with-example-build

说明：
- 该脚本负责：
  1) 执行 `cargo build -p tswn_capi`
  2) 收集 capi 头文件
  3) 收集动态库及常见伴生产物（Windows import lib / pdb）
  4) 收集 examples 源码
- `--with-example-build` 会尝试额外编译 examples：
  - Windows: 优先使用 `cl`，其次尝试 `gcc`
  - Linux/macOS: 尝试 `cc`
- examples 默认通过相对路径 `../include/tswn_capi.h` 包含头文件，
  因此打包后会保持 `examples/` 与 `include/` 并列。
"""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Sequence


ROOT = Path(__file__).resolve().parent.parent
CRATE_DIR = ROOT / "crates" / "tswn_capi"
INCLUDE_DIR = CRATE_DIR / "include"
EXAMPLES_DIR = CRATE_DIR / "examples"
DEFAULT_OUTPUT_DIR = CRATE_DIR / "dist" / "capi"


def run(cmd: Sequence[str | Path], cwd: Path = ROOT) -> None:
    print(f"$ {' '.join(str(x) for x in cmd)}", flush=True)
    subprocess.run([str(x) for x in cmd], cwd=str(cwd), check=True)


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="构建 tswn_capi 并生成可分发目录",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "-o",
        "--output-dir",
        default=str(DEFAULT_OUTPUT_DIR),
        metavar="DIR",
        help="输出目录（默认：crates/tswn_capi/dist/capi）",
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
        default=None,
        help="cargo target triple（可选）",
    )
    p.add_argument(
        "--features",
        default=None,
        help="传给 cargo 的 features（逗号分隔），例如 a,b",
    )
    p.add_argument(
        "--no-default-features",
        action="store_true",
        help="传给 cargo 的 --no-default-features",
    )
    p.add_argument(
        "--with-example-build",
        action="store_true",
        help="额外尝试编译 examples 到 output/examples/bin",
    )
    p.add_argument(
        "--cargo",
        nargs=argparse.REMAINDER,
        default=[],
        help="追加 cargo build 参数（放在最后，如：--cargo -vv）",
    )
    return p.parse_args(argv)


def ensure_exists(path: Path, desc: str) -> None:
    if not path.exists():
        raise FileNotFoundError(f"找不到 {desc}: {path}")


def cargo_build_args(
    release: bool,
    target: str | None,
    features: str | None,
    no_default_features: bool,
    extra_cargo: list[str],
) -> list[str]:
    cmd = ["cargo", "build", "-p", "tswn_capi"]
    if release:
        cmd.append("--release")
    if target:
        cmd += ["--target", target]
    if no_default_features:
        cmd.append("--no-default-features")
    if features:
        cmd += ["--features", features]
    if extra_cargo:
        cmd += extra_cargo
    return cmd


def build_output_dir(release: bool, target: str | None) -> Path:
    profile = "release" if release else "debug"
    target_root = ROOT / "target"
    return target_root / target / profile if target else target_root / profile


def dynamic_lib_candidates(crate_name: str) -> list[str]:
    system = platform.system().lower()
    if system == "windows":
        return [f"{crate_name}.dll"]
    if system == "darwin":
        return [f"lib{crate_name}.dylib", f"{crate_name}.dylib", f"lib{crate_name}.so"]
    return [f"lib{crate_name}.so", f"{crate_name}.so"]


def support_artifact_candidates(crate_name: str) -> list[str]:
    system = platform.system().lower()
    if system == "windows":
        return [
            f"{crate_name}.dll.lib",
            f"{crate_name}.lib",
            f"{crate_name}.dll.exp",
            f"{crate_name}.exp",
            f"{crate_name}.pdb",
        ]
    return []


def find_primary_artifact(out_dir: Path, crate_name: str) -> Path:
    for name in dynamic_lib_candidates(crate_name):
        candidate = out_dir / name
        if candidate.exists() and candidate.is_file():
            return candidate

    patterns = [
        f"*{crate_name}*.dll",
        f"*{crate_name}*.so",
        f"*{crate_name}*.dylib",
    ]
    for pattern in patterns:
        for candidate in sorted(out_dir.glob(pattern)):
            if candidate.is_file():
                return candidate

    raise FileNotFoundError(
        f"未找到 tswn_capi 动态库，已检查目录：{out_dir}"
    )


def find_support_artifacts(out_dir: Path, crate_name: str) -> list[Path]:
    found: list[Path] = []
    seen: set[Path] = set()

    for name in support_artifact_candidates(crate_name):
        candidate = out_dir / name
        if candidate.exists() and candidate.is_file() and candidate not in seen:
            found.append(candidate)
            seen.add(candidate)

    # 兜底匹配，防止不同工具链命名略有差异
    for pattern in [
        f"*{crate_name}*.lib",
        f"*{crate_name}*.exp",
        f"*{crate_name}*.pdb",
    ]:
        for candidate in sorted(out_dir.glob(pattern)):
            if candidate.is_file() and candidate not in seen:
                found.append(candidate)
                seen.add(candidate)

    return found


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


def remove_tree(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)


def write_manifest(
    output_dir: Path,
    release: bool,
    target: str | None,
    lib_path: Path,
    support_files: list[Path],
    example_sources: list[Path],
    example_bins: list[Path],
) -> None:
    lines = [
        "# tswn_capi package manifest",
        "",
        f"profile={ 'release' if release else 'debug' }",
        f"target={target or ''}",
        f"platform={platform.platform()}",
        f"python={sys.version.split()[0]}",
        "",
        "[library]",
        lib_path.name,
        "",
        "[support]",
    ]
    if support_files:
        lines.extend(p.name for p in support_files)
    else:
        lines.append("(none)")

    lines += [
        "",
        "[examples.sources]",
    ]
    if example_sources:
        lines.extend(str(p.as_posix()) for p in example_sources)
    else:
        lines.append("(none)")

    lines += [
        "",
        "[examples.binaries]",
    ]
    if example_bins:
        lines.extend(str(p.as_posix()) for p in example_bins)
    else:
        lines.append("(none)")

    manifest_path = output_dir / "MANIFEST.txt"
    manifest_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"[write] {manifest_path}")


def write_usage_readme(
    output_dir: Path,
    lib_path: Path,
    support_files: list[Path],
    example_bins: list[Path],
) -> None:
    lines = [
        "# tswn_capi package",
        "",
        "本目录由 `scripts/build_capi.py` 生成。",
        "",
        "## 内容",
        "",
        "- `include/`: C 头文件",
        "- `lib/`: 动态库及伴生产物",
        "- `examples/`: 示例源码",
    ]

    if example_bins:
        lines.append("- `examples/bin/`: 已编译的示例程序")

    lines += [
        "",
        "## 主要文件",
        "",
        f"- 动态库：`lib/{lib_path.name}`",
    ]

    for item in support_files:
        lines.append(f"- 伴生产物：`lib/{item.name}`")

    lines += [
        "- 头文件：`include/tswn_capi.h`",
        "",
        "## 示例编译方式",
        "",
        "下面假设你当前就在本目录根下，并希望编译 `examples/version_and_error.c`：",
        "",
        "- 若你希望参考基于 `PreparedRunner` 的批量胜率统计，可查看 `examples/prepared_win_rate.c`。",
        "",
        "### MSVC (`cl`)",
        "",
        "- 典型命令：",
        "  `cl /nologo /Iinclude examples\\version_and_error.c /link /OUT:examples\\version_and_error.exe lib\\tswn_capi.dll.lib`",
        "- 说明：",
        "  - 若当前 shell 中没有 `cl`，可先执行 `start-vs-pwsh.ps1` 进入 Visual Studio Developer PowerShell 环境",
        "  - 头文件搜索路径使用 `/Iinclude`",
        "  - 链接时使用 `lib\\tswn_capi.dll.lib`",
        "  - 该写法已在当前仓库打包结果上实测可编译通过",
        "  - 生成的可执行文件示例为 `examples\\version_and_error.exe`",
        "",
        "### clang（Windows）",
        "",
        "- 典型命令：",
        "  `clang -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`",
        "- 说明：",
        "  - `-Iinclude` 指向头文件目录",
        "  - 直接显式传入 `lib/tswn_capi.dll.lib` 参与链接",
        "  - 这种写法适合当前产物命名为 `tswn_capi.dll.lib` 的 Windows 包",
        "",
        "### gcc（Windows / MinGW 风格）",
        "",
        "- 典型命令：",
        "  `gcc -Iinclude examples/version_and_error.c lib/tswn_capi.dll.lib -o examples/version_and_error.exe`",
        "",
        "### clang / gcc（Linux/macOS 风格）",
        "",
        "- 若目录中是 `libtswn_capi.so` / `libtswn_capi.dylib`，可使用：",
        "  `cc -Iinclude examples/version_and_error.c -Llib -ltswn_capi -o examples/version_and_error`",
        "",
        "## 说明",
        "",
        "- examples 中的源码默认通过相对路径 `../include/tswn_capi.h` 引用头文件。",
        "- `examples/prepared_runner.c` 与 `examples/prepared_win_rate.c` 中，prepared 路径的 seed 应传完整 `seed:...` 行，而不是裸 seed 值。",
        "- Windows 下如使用 MSVC 链接，通常需要 `.lib` 文件。",
        "- 运行 examples 时，请确保动态库可被找到；最简单的做法通常是把可执行文件与动态库放在同一目录，或把 `lib/` 加入系统库搜索路径。",
        "- 若运行时提示找不到 `tswn_capi.dll`，可先把 `lib/tswn_capi.dll` 复制到生成的 `.exe` 同目录，再重新执行。",
        "- 若你想编译别的示例，只需把上述命令里的 `examples/version_and_error.c` 替换成对应源文件即可。",
        "",
    ]

    readme_path = output_dir / "README.txt"
    readme_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {readme_path}")


def which(exe: str) -> str | None:
    return shutil.which(exe)


def compile_examples(
    packaged_root: Path,
    crate_name: str,
    built_lib: Path,
) -> list[Path]:
    examples_dst = packaged_root / "examples"
    bin_dir = examples_dst / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)

    lib_dir = packaged_root / "lib"
    include_dir = packaged_root / "include"
    system = platform.system().lower()
    built: list[Path] = []

    c_files = sorted(p for p in examples_dst.glob("*.c"))
    if not c_files:
        print("[info] 未找到需要编译的 examples 源码")
        return built

    if system == "windows":
        cl = which("cl")
        gcc = which("gcc")

        if cl:
            print("[info] 使用 cl 编译 examples")
            for src in c_files:
                exe_name = src.stem + ".exe"
                out_path = bin_dir / exe_name
                cmd = [
                    "cl",
                    "/nologo",
                    f"/I{include_dir}",
                    str(src),
                    "/link",
                    f"/OUT:{out_path}",
                    str(lib_dir / f"{crate_name}.dll.lib") if (lib_dir / f"{crate_name}.dll.lib").exists() else str(built_lib),
                ]
                run(cmd, cwd=examples_dst)
                built.append(out_path)
            return built

        if gcc:
            print("[info] 使用 gcc 编译 examples")
            for src in c_files:
                exe_name = src.stem + ".exe"
                out_path = bin_dir / exe_name
                cmd = [
                    "gcc",
                    "-I",
                    str(include_dir),
                    str(src),
                    "-L",
                    str(lib_dir),
                    "-ltswn_capi",
                    "-o",
                    str(out_path),
                ]
                run(cmd, cwd=examples_dst)
                built.append(out_path)
            return built

        print("[warn] 未找到可用的 C 编译器（cl/gcc），跳过 example 编译")
        return built

    cc = which("cc") or which("gcc") or which("clang")
    if cc is None:
        print("[warn] 未找到可用的 C 编译器（cc/gcc/clang），跳过 example 编译")
        return built

    print(f"[info] 使用 {cc} 编译 examples")
    for src in c_files:
        out_path = bin_dir / src.stem
        cmd = [
            cc,
            "-I",
            str(include_dir),
            str(src),
            "-L",
            str(lib_dir),
            f"-l{crate_name}",
            "-o",
            str(out_path),
        ]
        run(cmd, cwd=examples_dst)
        built.append(out_path)

    return built


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    output_dir = Path(args.output_dir).resolve()

    ensure_exists(CRATE_DIR, "tswn_capi crate 目录")
    ensure_exists(INCLUDE_DIR, "include 目录")
    ensure_exists(EXAMPLES_DIR, "examples 目录")

    print(f"[build] crate    : {CRATE_DIR}")
    print(f"[build] output   : {output_dir}")
    print(f"[build] profile  : {'release' if args.release else 'debug'}")
    print(f"[build] target   : {args.target or '(default)'}")
    print(f"[build] platform : {platform.system()} / {platform.machine()}")
    print()

    if args.clean and output_dir.exists():
        print(f"[clean] 清空 {output_dir}")
        remove_tree(output_dir)

    output_dir.mkdir(parents=True, exist_ok=True)
    include_out = output_dir / "include"
    lib_out = output_dir / "lib"
    examples_out = output_dir / "examples"

    # 1) cargo build
    cmd = cargo_build_args(
        release=args.release,
        target=args.target,
        features=args.features,
        no_default_features=args.no_default_features,
        extra_cargo=args.cargo,
    )
    run(cmd, cwd=ROOT)

    # 2) 收集构建产物
    out_dir = build_output_dir(release=args.release, target=args.target)
    primary_lib = find_primary_artifact(out_dir, "tswn_capi")
    support_files = find_support_artifacts(out_dir, "tswn_capi")

    lib_out.mkdir(parents=True, exist_ok=True)
    copy_file(primary_lib, lib_out / primary_lib.name)
    for support in support_files:
        copy_file(support, lib_out / support.name)

    # 3) 收集 include
    copy_tree_files(INCLUDE_DIR, include_out)

    # 4) 收集 examples
    copy_tree_files(EXAMPLES_DIR, examples_out)
    example_sources = sorted(p.relative_to(output_dir) for p in examples_out.rglob("*") if p.is_file())

    # 5) 可选编译 examples
    example_bins_abs: list[Path] = []
    if args.with_example_build:
        example_bins_abs = compile_examples(
            packaged_root=output_dir,
            crate_name="tswn_capi",
            built_lib=lib_out / primary_lib.name,
        )

    example_bins = [p.relative_to(output_dir) for p in example_bins_abs]

    # 6) 写说明文件
    write_manifest(
        output_dir=output_dir,
        release=args.release,
        target=args.target,
        lib_path=lib_out / primary_lib.name,
        support_files=[lib_out / p.name for p in support_files],
        example_sources=example_sources,
        example_bins=example_bins,
    )
    write_usage_readme(
        output_dir=output_dir,
        lib_path=lib_out / primary_lib.name,
        support_files=[lib_out / p.name for p in support_files],
        example_bins=example_bins,
    )

    print()
    print(f"[ok] 已生成 capi 结果目录：{output_dir}")
    print(f"[ok] 动态库：{lib_out / primary_lib.name}")
    print(f"[ok] 头文件：{include_out / 'tswn_capi.h'}")
    print(f"[ok] 示例目录：{examples_out}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))