#!/usr/bin/env python3
"""
聚合打包脚本：一次性整理并压缩 `tswn_capi`、`tswn-cli` 以及现有 `tswn_py` 产物。

设计目标：
1) `capi`：现场构建并打包
2) `cli`：现场构建并打包
3) `py`：不现场构建，只收集当前仓库里已存在的 Python 分发产物
4) 最终输出一个 zip

默认输出结构（示例）：

dist/all/tswn_core_x_y_z_capi_a_b_c_py_m_n_k_bundle/
  README.txt
  MANIFEST.txt
  capi/
    include/
    lib/
    examples/
    README.txt
    MANIFEST.txt
  cli/
    bin/
      tswn-cli_alpha_x_y_z.exe
    README.txt
    MANIFEST.txt
  py/
    dist/
      *.whl
      *.tar.gz
      tswn_py/
        ...
    examples/
      *.py
    changelog/
      CHANGELOG.md
    README.txt
    MANIFEST.txt

用法示例：
  python scripts/build_all.py
  python scripts/build_all.py --release
  python scripts/build_all.py --release --clean
  python scripts/build_all.py -o dist/all
  python scripts/build_all.py --bundle-name tswn_core_x_y_z_capi_a_b_c_py_m_n_k_bundle_win_x64
  python scripts/build_all.py --skip-capi
  python scripts/build_all.py --skip-cli

说明：
- `py` 部分只打包现有产物，不调用 Python wheel 构建流程。
- `capi` 部分优先复用 `scripts/build_capi.py`，以保证目录结构一致。
- `cli` 部分直接执行 cargo build，然后把可执行文件及说明文件整理到结果目录。
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
SCRIPTS_DIR = ROOT / "scripts"

CRATE_CAPI_DIR = ROOT / "crates" / "tswn_capi"
CRATE_PY_DIR = ROOT / "crates" / "tswn_py"
CRATE_CORE_CARGO_TOML = ROOT / "crates" / "tswn_core" / "Cargo.toml"
CRATE_CAPI_CARGO_TOML = CRATE_CAPI_DIR / "Cargo.toml"
CRATE_PY_CARGO_TOML = CRATE_PY_DIR / "Cargo.toml"
CORE_CHANGELOG = ROOT / "crates" / "tswn_core" / "CHANGELOG.md"
CAPI_CHANGELOG = CRATE_CAPI_DIR / "CHANGELOG.md"
PY_CHANGELOG = CRATE_PY_DIR / "CHANGELOG.md"
UPDATE_DOCS_DIR = ROOT / "docs" / "update"

DEFAULT_OUTPUT_DIR = ROOT / "dist" / "all"

CAPI_BUILD_SCRIPT = SCRIPTS_DIR / "build_capi.py"


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


def copy_tree(src: Path, dst: Path) -> None:
    ensure_exists(src, "目录")
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    print(f"[copytree] {src} -> {dst}")


def copy_tree_files(src_dir: Path, dst_dir: Path) -> None:
    ensure_exists(src_dir, "目录")
    for path in sorted(src_dir.rglob("*")):
        if path.is_file():
            rel = path.relative_to(src_dir)
            copy_file(path, dst_dir / rel)


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="构建并聚合打包 capi / cli / 现有 py 产物",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "-o",
        "--output-dir",
        default=str(DEFAULT_OUTPUT_DIR),
        metavar="DIR",
        help="输出目录（默认：dist/all）",
    )
    p.add_argument(
        "--bundle-name",
        default=None,
        help="bundle 目录名与 zip 基名（默认：自动按 core/capi/py 版本生成）",
    )
    p.add_argument(
        "--release",
        action="store_true",
        help="对 capi/cli 使用 release 构建（默认 debug）",
    )
    p.add_argument(
        "--clean",
        action="store_true",
        help="构建前清空 bundle 目录与最终 zip",
    )
    p.add_argument(
        "--target",
        default=None,
        help="cargo target triple（可选）",
    )
    p.add_argument(
        "--skip-capi",
        action="store_true",
        help="跳过 capi 打包",
    )
    p.add_argument(
        "--skip-cli",
        action="store_true",
        help="跳过 cli 打包",
    )
    p.add_argument(
        "--skip-py",
        action="store_true",
        help="跳过 py 打包",
    )
    p.add_argument(
        "--capi-with-example-build",
        action="store_true",
        help="传给 build_capi.py：额外尝试编译 examples",
    )
    p.add_argument(
        "--cli-features",
        default="no_debug",
        help="CLI 构建 features，逗号分隔；传空字符串表示不追加 features（默认：no_debug）",
    )
    p.add_argument(
        "--cargo",
        nargs=argparse.REMAINDER,
        default=[],
        help="追加到 cargo/build_capi 的额外参数（放在最后，如：--cargo -vv）",
    )
    return p.parse_args(argv)


def cargo_profile_dir(release: bool, target: str | None) -> Path:
    profile = "release" if release else "debug"
    target_root = ROOT / "target"
    return target_root / target / profile if target else target_root / profile


def cli_binary_candidates() -> list[str]:
    system = platform.system().lower()
    if system == "windows":
        return ["tswn-cli.exe", "tswn_cli.exe"]
    return ["tswn-cli", "tswn_cli"]


def find_cli_binary(out_dir: Path) -> Path:
    for name in cli_binary_candidates():
        candidate = out_dir / name
        if candidate.exists() and candidate.is_file():
            return candidate

    patterns = ["*tswn-cli*", "*tswn_cli*"]
    for pattern in patterns:
        for candidate in sorted(out_dir.glob(pattern)):
            if candidate.is_file():
                return candidate

    raise FileNotFoundError(f"未找到 tswn-cli 构建产物，已检查目录：{out_dir}")


def cli_support_artifacts(binary: Path) -> list[Path]:
    system = platform.system().lower()
    found: list[Path] = []
    if system == "windows":
        pdb = binary.with_suffix(".pdb")
        if pdb.exists() and pdb.is_file():
            found.append(pdb)
    return found


def _cargo_package_version(cargo_toml: Path) -> str:
    ensure_exists(cargo_toml, "Cargo.toml")
    for line in cargo_toml.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("version = "):
            value = stripped.split("=", 1)[1].strip().strip('"')
            if value:
                return value
    raise RuntimeError(f"未能从 {cargo_toml} 解析版本")


def tswn_core_version() -> str:
    return _cargo_package_version(CRATE_CORE_CARGO_TOML)


def tswn_capi_version() -> str:
    return _cargo_package_version(CRATE_CAPI_CARGO_TOML)


def tswn_py_version() -> str:
    return _cargo_package_version(CRATE_PY_CARGO_TOML)


def _version_token(version: str) -> str:
    return version.replace(".", "_").replace("-", "_")


def default_bundle_name() -> str:
    return (
        f"tswn_core_{_version_token(tswn_core_version())}"
        f"_capi_{_version_token(tswn_capi_version())}"
        f"_py_{_version_token(tswn_py_version())}"
        "_bundle"
    )


def bundled_cli_binary_name(src_binary: Path) -> str:
    version = _version_token(tswn_core_version())
    system = platform.system().lower()
    suffix = ".exe" if system == "windows" else ""
    return f"tswn-cli_alpha_{version}{suffix}"


def package_component_changelog(dst_dir: Path, changelog_src: Path, update_doc_src: Path | None = None) -> list[Path]:
    copied: list[Path] = []
    changelog_dir = dst_dir / "changelog"
    changelog_dir.mkdir(parents=True, exist_ok=True)

    if changelog_src.exists():
        dst = changelog_dir / "CHANGELOG.md"
        copy_file(changelog_src, dst)
        copied.append(dst)

    if update_doc_src is not None and update_doc_src.exists():
        dst = changelog_dir / update_doc_src.name
        copy_file(update_doc_src, dst)
        copied.append(dst)

    return copied


def build_cli(
    dst_dir: Path,
    release: bool,
    target: str | None,
    cli_features: str,
    extra_cargo: list[str],
) -> tuple[Path, list[Path]]:
    out_dir = cargo_profile_dir(release=release, target=target)

    cmd: list[str] = ["cargo", "build", "-p", "tswn_core", "--bin", "tswn-cli"]
    if release:
        cmd.append("--release")
    if target:
        cmd += ["--target", target]
    if cli_features.strip():
        cmd += ["--features", cli_features]
    if extra_cargo:
        cmd += extra_cargo

    run(cmd, cwd=ROOT)

    binary = find_cli_binary(out_dir)
    support = cli_support_artifacts(binary)

    bin_dir = dst_dir / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)

    bundled_binary = bin_dir / bundled_cli_binary_name(binary)
    copy_file(binary, bundled_binary)
    copied_support: list[Path] = []
    for item in support:
        dst = bin_dir / item.name
        copy_file(item, dst)
        copied_support.append(dst)

    return bundled_binary, copied_support


def write_cli_readme(dst_dir: Path, binary_path: Path, support_files: list[Path]) -> None:
    lines = [
        "# tswn-cli package",
        "",
        "本目录由 `scripts/build_all.py` 生成。",
        "",
        "## 内容",
        "",
        f"- 可执行文件：`bin/{binary_path.name}`",
    ]
    for item in support_files:
        lines.append(f"- 伴生产物：`bin/{item.name}`")

    lines += [
        "",
        "## 示例",
        "",
        f"- `bin/{binary_path.name} fight --help`",
        f"- `bin/{binary_path.name} bench auto --help`",
        f"- `bin/{binary_path.name} icon show --help`",
        "",
    ]

    path = dst_dir / "README.txt"
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {path}")


def write_cli_manifest(
    dst_dir: Path,
    release: bool,
    target: str | None,
    binary_path: Path,
    support_files: list[Path],
) -> None:
    lines = [
        "# tswn-cli manifest",
        "",
        f"profile={'release' if release else 'debug'}",
        f"target={target or ''}",
        f"platform={platform.platform()}",
        "",
        "[bin]",
        binary_path.name,
        "",
        "[support]",
    ]
    if support_files:
        lines.extend(p.name for p in support_files)
    else:
        lines.append("(none)")

    path = dst_dir / "MANIFEST.txt"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"[write] {path}")


def build_capi(dst_dir: Path, args: argparse.Namespace) -> None:
    ensure_exists(CAPI_BUILD_SCRIPT, "build_capi.py")

    cmd: list[str | Path] = [
        sys.executable,
        CAPI_BUILD_SCRIPT,
        "--output-dir",
        str(dst_dir),
    ]
    if args.release:
        cmd.append("--release")
    if args.target:
        cmd += ["--target", args.target]
    if args.capi_with_example_build:
        cmd.append("--with-example-build")
    if args.cargo:
        cmd.append("--cargo")
        cmd += args.cargo

    run(cmd, cwd=ROOT)


def collect_py_artifacts(dst_dir: Path) -> tuple[list[Path], list[Path], list[Path]]:
    """
    返回：
    - copied_files: 复制到 bundle 内的普通文件相对路径
    - copied_dirs: 复制到 bundle 内的目录相对路径
    - source_hits: 源目录中实际找到并复制的文件相对路径（用于 manifest）
    """
    dist_src = CRATE_PY_DIR / "dist"
    examples_src = CRATE_PY_DIR / "examples"
    changelog_src = PY_CHANGELOG
    dst_dist = dst_dir / "dist"
    dst_examples = dst_dir / "examples"
    dst_changelog = dst_dir / "changelog"

    copied_files: list[Path] = []
    copied_dirs: list[Path] = []
    source_hits: list[Path] = []

    dst_dist.mkdir(parents=True, exist_ok=True)

    if dist_src.exists():
        for item in sorted(dist_src.iterdir()):
            if item.is_file():
                copy_file(item, dst_dist / item.name)
                copied_files.append(Path("dist") / item.name)
                source_hits.append(Path("dist") / item.name)
            elif item.is_dir():
                copy_tree(item, dst_dist / item.name)
                copied_dirs.append(Path("dist") / item.name)
                for nested in sorted(item.rglob("*")):
                    if nested.is_file():
                        source_hits.append(Path("dist") / nested.relative_to(dist_src).as_posix())

    if examples_src.exists():
        copy_tree(examples_src, dst_examples)
        copied_dirs.append(Path("examples"))
        for nested in sorted(examples_src.rglob("*")):
            if nested.is_file():
                source_hits.append(Path("examples") / nested.relative_to(examples_src).as_posix())

    dst_changelog.mkdir(parents=True, exist_ok=True)
    if changelog_src.exists():
        copy_file(changelog_src, dst_changelog / "CHANGELOG.md")
        copied_files.append(Path("changelog") / "CHANGELOG.md")
        source_hits.append(Path("changelog") / "CHANGELOG.md")

    return copied_files, copied_dirs, source_hits


def write_py_readme(dst_dir: Path, copied_files: list[Path], copied_dirs: list[Path]) -> None:
    lines = [
        "# tswn_py package snapshot",
        "",
        "本目录由 `scripts/build_all.py` 生成。",
        "",
        "注意：这里不会现场构建 Python 产物，只会收集仓库里当前已经存在的内容。",
        "",
        "## 收集结果",
        "",
    ]

    if not copied_files and not copied_dirs:
        lines.append("- 未发现现有 Python 分发产物。")
    else:
        for item in copied_files:
            lines.append(f"- 文件：`{item.as_posix()}`")
        for item in copied_dirs:
            lines.append(f"- 目录：`{item.as_posix()}`")

    lines += [
        "",
        "## 说明",
        "",
        "- 若需要 wheel，请先单独执行 Python 构建流程。",
        "- `examples/` 会一并收集，方便直接参考 Python 用法。",
        "- `changelog/` 会附带 `tswn_py` 的 changelog，方便对外查看 Python 侧版本变化。",
        "- 本目录仅作为现有产物快照，不保证覆盖你需要的全部 Python/ABI 环境。",
        "",
    ]

    path = dst_dir / "README.txt"
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {path}")


def write_py_manifest(dst_dir: Path, source_hits: list[Path]) -> None:
    lines = [
        "# tswn_py manifest",
        "",
        f"platform={platform.platform()}",
        "",
        "[files]",
    ]
    if source_hits:
        lines.extend(p.as_posix() for p in source_hits)
    else:
        lines.append("(none)")

    path = dst_dir / "MANIFEST.txt"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"[write] {path}")


def write_root_readme(bundle_dir: Path, enabled: list[str], skipped: list[str]) -> None:
    lines = [
        "# tswn bundle",
        "",
        "本目录由 `scripts/build_all.py` 生成。",
        "",
        "## 版本概览",
        "",
        f"- `tswn_core`: `{tswn_core_version()}`",
        f"- `tswn_capi`: `{tswn_capi_version()}`",
        f"- `tswn_py`: `{tswn_py_version()}`",
        "",
        "## 包含内容",
        "",
    ]

    if enabled:
        for name in enabled:
            lines.append(f"- `{name}/`")
    else:
        lines.append("- (none)")

    lines += [
        "",
        "## 跳过内容",
        "",
    ]
    if skipped:
        for name in skipped:
            lines.append(f"- `{name}`")
    else:
        lines.append("- (none)")

    lines += [
        "",
        "## 主要产物",
        "",
        f"- CLI: `cli/bin/{bundled_cli_binary_name(Path('tswn-cli'))}` 与 `cli/changelog/`",
        "- C-API: `capi/include/tswn_capi.h`、`capi/lib/` 与 `capi/changelog/`",
        "- Python: `py/dist/*.whl`、`py/examples/` 与 `py/changelog/`",
        "",
        "## 说明",
        "",
        "- `capi/` 与 `cli/` 可以现场构建。",
        "- `py/` 只收集现有产物，不现场构建。",
        "- 最终 zip 为整个 bundle 目录的压缩包。",
        "",
    ]

    path = bundle_dir / "README.txt"
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {path}")


def write_root_manifest(
    bundle_dir: Path,
    args: argparse.Namespace,
    enabled: list[str],
    skipped: list[str],
    zip_path: Path,
) -> None:
    lines = [
        "# tswn bundle manifest",
        "",
        f"bundle={bundle_dir.name}",
        f"profile={'release' if args.release else 'debug'}",
        f"target={args.target or ''}",
        f"platform={platform.platform()}",
        f"python={sys.version.split()[0]}",
        "",
        "[enabled]",
    ]
    if enabled:
        lines.extend(enabled)
    else:
        lines.append("(none)")

    lines += [
        "",
        "[skipped]",
    ]
    if skipped:
        lines.extend(skipped)
    else:
        lines.append("(none)")

    lines += [
        "",
        "[archive]",
        zip_path.name,
        "",
    ]

    path = bundle_dir / "MANIFEST.txt"
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[write] {path}")


def make_zip(bundle_dir: Path, zip_path: Path) -> Path:
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    base_name = zip_path.with_suffix("")
    archive = shutil.make_archive(
        base_name=str(base_name),
        format="zip",
        root_dir=str(bundle_dir.parent),
        base_dir=bundle_dir.name,
    )
    final_path = Path(archive)
    print(f"[zip] {final_path}")
    return final_path


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    output_dir = Path(args.output_dir).resolve()
    bundle_name = args.bundle_name or default_bundle_name()
    bundle_dir = output_dir / bundle_name
    zip_path = output_dir / f"{bundle_name}.zip"

    print(f"[build] root     : {ROOT}")
    print(f"[build] output   : {output_dir}")
    print(f"[build] bundle   : {bundle_dir}")
    print(f"[build] zip      : {zip_path}")
    print(f"[build] profile  : {'release' if args.release else 'debug'}")
    print(f"[build] target   : {args.target or '(default)'}")
    print()

    if args.clean:
        if bundle_dir.exists():
            print(f"[clean] 清空 {bundle_dir}")
            remove_tree(bundle_dir)
        if zip_path.exists():
            print(f"[clean] 删除 {zip_path}")
            zip_path.unlink()

    bundle_dir.mkdir(parents=True, exist_ok=True)

    enabled: list[str] = []
    skipped: list[str] = []

    if args.skip_capi:
        skipped.append("capi")
    else:
        capi_dir = bundle_dir / "capi"
        build_capi(capi_dir, args)
        package_component_changelog(capi_dir, changelog_src=CAPI_CHANGELOG)
        enabled.append("capi")

    if args.skip_cli:
        skipped.append("cli")
    else:
        cli_dir = bundle_dir / "cli"
        binary_path, support_files = build_cli(
            dst_dir=cli_dir,
            release=args.release,
            target=args.target,
            cli_features=args.cli_features,
            extra_cargo=args.cargo,
        )
        package_component_changelog(
            cli_dir,
            changelog_src=CORE_CHANGELOG,
            update_doc_src=UPDATE_DOCS_DIR / f"{tswn_core_version()}.md",
        )
        write_cli_readme(cli_dir, binary_path=binary_path, support_files=support_files)
        write_cli_manifest(
            cli_dir,
            release=args.release,
            target=args.target,
            binary_path=binary_path,
            support_files=support_files,
        )
        enabled.append("cli")

    if args.skip_py:
        skipped.append("py")
    else:
        py_dir = bundle_dir / "py"
        copied_files, copied_dirs, source_hits = collect_py_artifacts(py_dir)
        write_py_readme(py_dir, copied_files=copied_files, copied_dirs=copied_dirs)
        write_py_manifest(py_dir, source_hits=source_hits)
        enabled.append("py")

    write_root_readme(bundle_dir, enabled=enabled, skipped=skipped)
    write_root_manifest(
        bundle_dir,
        args=args,
        enabled=enabled,
        skipped=skipped,
        zip_path=zip_path,
    )

    final_zip = make_zip(bundle_dir, zip_path)

    print()
    print(f"[ok] bundle 目录：{bundle_dir}")
    print(f"[ok] zip 文件   ：{final_zip}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))