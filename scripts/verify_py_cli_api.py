#!/usr/bin/env python3
"""
Verify Python APIs aligned with tswn-cli.

The script builds the local tswn_py extension, imports it from a temporary
target directory, then checks:
- summary APIs match the older top-level win-rate APIs;
- score / namer-pf / batch-rate / pair-rate compose consistently;
- to_diy roundtrips through Runner while preserving initial player status;
- icon_info matches the byte/icon helpers at a structural level.
"""

from __future__ import annotations

import argparse
import importlib
import os
import platform
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
CRATE_DIR = ROOT / "crates" / "tswn_py"
TARGET_DIR = ROOT / "target" / "py_cli_api_verify"
IMPORT_ROOT = TARGET_DIR / "import"
PKG_DIR = IMPORT_ROOT / "tswn_py"


def run(cmd: list[str], cwd: Path = ROOT) -> None:
    print(f"$ {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, cwd=str(cwd), check=True)


def extension_suffix() -> str:
    return ".pyd" if os.name == "nt" else ".so"


def artifact_suffixes() -> list[str]:
    system = platform.system().lower()
    if system == "windows":
        return [".pyd", ".dll"]
    if system == "darwin":
        return [".dylib", ".so"]
    return [".so"]


def find_artifact(release: bool) -> Path:
    profile = "release" if release else "debug"
    out_dir = ROOT / "target" / profile
    for suffix in artifact_suffixes():
        path = out_dir / f"tswn_py{suffix}"
        if path.exists():
            return path
    for suffix in artifact_suffixes():
        candidates = sorted(out_dir.glob(f"*tswn_py*{suffix}"))
        if candidates:
            return candidates[0]
    raise FileNotFoundError(f"cannot find tswn_py extension artifact in {out_dir}")


def prepare_import_tree(release: bool) -> None:
    run(["cargo", "build", "-p", "tswn_py"] + (["--release"] if release else []))

    if IMPORT_ROOT.exists():
        shutil.rmtree(IMPORT_ROOT)
    PKG_DIR.mkdir(parents=True, exist_ok=True)

    artifact = find_artifact(release)
    shutil.copy2(artifact, PKG_DIR / f"tswn_py{extension_suffix()}")
    for name in ["__init__.py", "_version.py", "__init__.pyi", "tswn_py.pyi", "py.typed"]:
        src = CRATE_DIR / "tswn_py" / name
        if src.exists():
            shutil.copy2(src, PKG_DIR / name)

    sys.path.insert(0, str(IMPORT_ROOT))


def assert_close(actual: float, expected: float, label: str, eps: float = 1e-9) -> None:
    if abs(actual - expected) > eps:
        raise AssertionError(f"{label}: actual={actual!r}, expected={expected!r}")


def assert_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: actual={actual!r}, expected={expected!r}")


@dataclass(frozen=True)
class PlayerStatus:
    id: int
    hp: int
    max_hp: int
    move_point: int
    attack: int
    defense: int
    speed: int
    agility: int
    magic: int
    mp: int
    resistance: int
    wisdom: int
    all_sum: int
    name_factor: float


def split_raw(raw: str) -> list[list[str]]:
    groups: list[list[str]] = []
    current: list[str] = []
    for line in raw.strip().splitlines():
        line = line.strip()
        if not line:
            if current:
                groups.append(current)
                current = []
        else:
            current.append(line)
    if current:
        groups.append(current)
    return groups


def collect_statuses(tswn_py: Any, raw: str) -> list[PlayerStatus]:
    runner = tswn_py.Runner.new_from_namerena_raw(raw)
    storage = runner.storage
    statuses: list[PlayerStatus] = []
    for pid in runner.all_plrs():
        player = storage.get_player_by_id(pid)
        if player is None:
            raise AssertionError(f"missing player id={pid}")
        statuses.append(
            PlayerStatus(
                id=pid,
                hp=player.hp,
                max_hp=player.max_hp,
                move_point=player.move_point,
                attack=player.attack,
                defense=player.defense,
                speed=player.speed,
                agility=player.agility,
                magic=player.magic,
                mp=player.magic_point,
                resistance=player.resistance,
                wisdom=player.wisdom,
                all_sum=player.all_sum,
                name_factor=player.name_factor,
            )
        )
    return sorted(statuses, key=lambda item: item.id)


def build_diy_input(tswn_py: Any, raw: str, *, minions: bool) -> str:
    out_groups = []
    for group in split_raw(raw):
        out_groups.append("\n".join(tswn_py.to_diy(name, minions=minions) for name in group))
    return "\n\n".join(out_groups)


def verify_win_rate_apis(tswn_py: Any) -> None:
    raw = "mario\n\nluigi"
    old_rate = tswn_py.win_rate(raw, 8, thread=1)
    summary = tswn_py.win_rate_summary(raw, 8, thread=1)
    assert_close(summary.win_rate, old_rate, "win_rate_summary matches win_rate")
    assert_equal(summary.total, 8, "win_rate_summary total")
    assert_equal(summary.wins, round(old_rate * 8 / 100), "win_rate_summary wins")

    team = tswn_py.team_win_rate_summary("mario", "luigi", 8, thread=1)
    assert_equal(team.wins, summary.wins, "team_win_rate_summary wins")
    assert_equal(team.total, summary.total, "team_win_rate_summary total")
    assert_close(team.win_rate, summary.win_rate, "team_win_rate_summary rate")

    group_old = tswn_py.group_win_rate("mario", ["luigi", "peach"], 8, thread=1)
    group_summary = tswn_py.group_win_rate_summary("mario", ["luigi", "peach"], 8, thread=1)
    assert_equal([name for name, _ in group_summary], [name for name, _ in group_old], "group labels")
    for (old_name, old_rate), (new_name, result) in zip(group_old, group_summary, strict=True):
        assert_equal(new_name, old_name, "group result name")
        assert_close(result.win_rate, old_rate, f"group_win_rate_summary {old_name}")


def verify_score_and_namer_pf(tswn_py: Any) -> None:
    raw = "mario+luigi\npeach"
    pp_only = tswn_py.namer_pf(raw, 6, modes=["pp"], thread=1)
    assert_equal(len(pp_only), 2, "namer_pf row count")
    assert_equal(pp_only[0].modes, ["pp"], "namer_pf mode labels")
    assert_equal(pp_only[0].group, ["mario", "luigi"], "namer_pf plus parsing")
    assert_close(pp_only[0].scores[0], pp_only[0].total_score, "single namer_pf total")

    score = tswn_py.score("mario", 6, mode="normal", thread=1)
    one_group = tswn_py.namer_pf("mario", 6, modes=["pp"], thread=1)[0]
    assert_close(one_group.scores[0], score.score, "namer_pf pp matches score")
    assert_equal(score.total, 6, "score total")

    all_modes = tswn_py.namer_pf("mario", 4, thread=1)[0]
    assert_equal(all_modes.modes, ["pp", "pd", "qp", "qd"], "namer_pf default modes")
    assert_close(sum(all_modes.scores), all_modes.total_score, "namer_pf total")
    assert_equal(all_modes.as_line(0).count("|"), 4, "namer_pf line has sum column")


def verify_batch_and_pair(tswn_py: Any) -> None:
    targets = ["luigi", "peach"]
    players = ["mario", "luigi"]
    labels = ["mario-label", "dupe-label"]
    results = tswn_py.batch_rate(targets, players, 6, player_labels=labels, thread=1)
    assert_equal([r.label for r in results], labels, "batch_rate labels")

    mario_vs = [
        tswn_py.team_win_rate_summary("mario", target, 6, thread=1).win_rate
        for target in targets
    ]
    assert_close(results[0].avg_win_rate, sum(mario_vs) / len(mario_vs), "batch avg")
    assert_equal(results[0].valid_matchups, 2, "batch valid count")
    assert_equal(results[0].skipped_matchups, 0, "batch skipped count")

    assert_equal(results[1].valid_matchups, 1, "batch duplicate skip valid count")
    assert_equal(results[1].skipped_matchups, 1, "batch duplicate skip count")

    pair = tswn_py.pair_rate(["peach"], ["mario"], ["luigi", "mario"], head=1, n=4, thread=1)[0]
    assert_equal(pair.label, "mario", "pair label")
    assert_equal(pair.head, 1, "pair head")
    assert_equal(pair.selected, 1, "pair selected")
    assert_equal(len(pair.top_pairs), 1, "pair top pairs")
    assert_equal(pair.valid_matchups + pair.skipped_matchups, 2, "pair matchup accounting")
    assert_close(pair.final_score, pair.top_pairs[0][1], "pair final score for head=1")


def verify_to_diy_roundtrip(tswn_py: Any) -> None:
    raw = (
        "mario@red+fire\n"
        "luigi@red+heal\n\n"
        "peach@blue+shadow\n"
        "bowser@blue+poison"
    )
    diy_raw = build_diy_input(tswn_py, raw, minions=True)
    orig = collect_statuses(tswn_py, raw)
    diy = collect_statuses(tswn_py, diy_raw)
    assert_equal(diy, orig, "to_diy minions roundtrip statuses")

    old = tswn_py.to_diy("mario@red+fire", old=True)
    assert_equal("+diy[" in old, True, "to_diy old format")
    new = tswn_py.to_diy("mario@red+fire")
    assert_equal("+ol:" in new, True, "to_diy default ol format")
    with_minions = tswn_py.to_diy("mario@red+shadow", minions=True)
    assert_equal("+ol:" in with_minions, True, "to_diy minions ol format")

    try:
        tswn_py.to_diy("mario", old=True, minions=True)
    except ValueError:
        pass
    else:
        raise AssertionError("to_diy(old=True, minions=True) should raise ValueError")

    batch = tswn_py.to_diy_batch(["mario", "luigi"], old=True)
    assert_equal(len(batch), 2, "to_diy_batch length")
    assert_equal(all("+diy[" in item for item in batch), True, "to_diy_batch old entries")


def verify_icon_and_parsers(tswn_py: Any) -> None:
    info = tswn_py.icon_info("player@mario")
    same = tswn_py.icon_info("mario")
    assert_equal(info.border_style, same.border_style, "icon uses team name")
    assert_equal(info.shapes, same.shapes, "icon shapes")
    assert_equal(info.bg_color, same.bg_color, "icon bg")
    assert_equal(info.fg_colors, same.fg_colors, "icon fg")

    rgba = tswn_py.name_to_icon_rgba("mario")
    assert_equal(len(rgba), 16 * 16 * 4, "icon rgba length")
    png = tswn_py.name_to_png_bytes("mario")
    assert_equal(bytes(png[:8]), b"\x89PNG\r\n\x1a\n", "png signature")

    parsed = tswn_py.parse_group_lines("mario+luigi\n\npeach", False)
    assert_equal(parsed, ["mario\nluigi", "peach"], "parse_group_lines plus")
    parsed_double = tswn_py.parse_group_lines("mario+diy[1,2,3]++luigi", True)
    assert_equal(parsed_double, ["mario+diy[1,2,3]\nluigi"], "parse_group_lines double plus")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify tswn_py CLI-aligned APIs")
    parser.add_argument("--release", action="store_true", help="build/import release artifact")
    parser.add_argument("--skip-build", action="store_true", help="reuse existing target import tree")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if not args.skip_build:
        prepare_import_tree(args.release)
    else:
        sys.path.insert(0, str(IMPORT_ROOT))

    tswn_py = importlib.import_module("tswn_py")
    print(f"imported tswn_py wrapper={tswn_py.wrapper_version_str()} core={tswn_py.core_version_str()}")

    checks = [
        verify_win_rate_apis,
        verify_score_and_namer_pf,
        verify_batch_and_pair,
        verify_to_diy_roundtrip,
        verify_icon_and_parsers,
    ]
    for check in checks:
        print(f"[check] {check.__name__}", flush=True)
        check(tswn_py)

    print("OK: all Python CLI API verification checks passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
