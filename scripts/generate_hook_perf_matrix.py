#!/usr/bin/env python3
"""生成恢复轮独立普通/Boss留出32集；不包含武器，不根据测量筛选。"""
import argparse
import hashlib
import json
from pathlib import Path

from generate_perf_matrix import roster_signature

SALT = "tswn-stable-heldout-recovery-v2-20261028"
BOSSES = ("mario", "sonic", "mosquito", "yuri", "slime", "ikaruga", "conan", "aokiji", "lazy", "covid", "saitama", "testsubject")
LAYOUTS = (("ffa4", [1] * 4), ("ffa6", [1] * 6), ("ffa8", [1] * 8), ("ffa12", [1] * 12), ("ffa16", [1] * 16), ("2v2", [2, 2]), ("4v4", [4, 4]), ("3v3v3", [3, 3, 3]))
CLANS = ("Rinick", "云剑", "RailGun", "geometrydash")


def build_cases():
    cases = []
    for family in ("plain", "boss"):
        for layout_index, (label, sizes) in enumerate(LAYOUTS):
            for repetition in range(2):
                key = f"{SALT}/{family}/{label}/{repetition}"
                names = [hashlib.sha256(f"{key}/{i}".encode()).hexdigest()[:14] + "@" + CLANS[i % len(CLANS)] for i in range(sum(sizes))]
                boss = None
                if family == "boss":
                    boss = BOSSES[(layout_index * 2 + repetition) % len(BOSSES)]
                    names[0] = boss + "@!"
                groups = []
                offset = 0
                for size in sizes:
                    groups.append(names[offset:offset + size])
                    offset += size
                data = ("\n\n".join("\n".join(group) for group in groups) + "\n").encode()
                cases.append({"file": f"{family}__{label}__{repetition + 1:02}.txt", "family": family, "group_sizes": sizes, "boss": boss, "selection_key": key, "sha256": hashlib.sha256(data).hexdigest(), "data": data})
    return cases


def generate(repo, prior_matrix, out):
    if out.exists():
        raise ValueError("输出目录已存在，不覆盖")
    paths = sorted((repo / "docs/perf/fixed_cases_30").glob("*.txt"))
    for directory in ("expanded", "training"):
        folder = prior_matrix / directory
        if not folder.is_dir():
            raise ValueError(f"缺少先前矩阵目录: {directory}")
        paths.extend(sorted(folder.glob("*.txt")))
    if len(paths) != 163:
        raise ValueError("要求既有fixed30+expanded85+training48共163例")
    prior = {roster_signature(path.read_bytes()) for path in paths}
    cases = build_cases()
    signatures = [roster_signature(case["data"]) for case in cases]
    if len(set(signatures)) != 32 or prior.intersection(signatures):
        raise ValueError("新roster重复或与先前矩阵重叠")
    out.mkdir(parents=True)
    (out / "inputs").mkdir()
    for case in cases:
        (out / "inputs" / case["file"]).write_bytes(case["data"])
    manifest = {"schema": "stable-heldout-recovery-v2", "salt": SALT, "round_start": 2_000_000, "cases": 32, "prior_cases_checked": len(paths), "excluded_families": ["s11", "deathnote", "xuanyue", "mixed_weapon"], "exclusion_reason": "用户要求武器本地开发期间不纳入；不采用dummy", "source_justification": "crates/tswn_core/src/namerena/input.rs: classify将12个保留名字配合@!识别为Boss；无+后缀", "inputs": [{key: value for key, value in case.items() if key != "data"} for case in cases]}
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--prior-matrix", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    generate(args.repo, args.prior_matrix, args.out)


if __name__ == "__main__":
    main()
