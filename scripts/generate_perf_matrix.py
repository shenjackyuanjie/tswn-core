#!/usr/bin/env python3
"""生成固定性能评估矩阵与独立 PGO 训练矩阵，不根据测量结果筛选。

用法：python scripts/generate_perf_matrix.py --repo . --out /tmp/tswn-matrix
输出目录必须不存在；不会覆盖或删除现有数据。评估目录包含 37 个压力归档
原始输入和 48 个固定生成输入；训练目录另含 48 个不同 roster。
"""

import argparse
import hashlib
import json
from pathlib import Path


LAYOUTS = [
    ("ffa4", [1] * 4),
    ("ffa6", [1] * 6),
    ("ffa8", [1] * 8),
    ("ffa12", [1] * 12),
    ("ffa16", [1] * 16),
    ("2v2", [2, 2]),
    ("3v3v3", [3, 3, 3]),
    ("4v4", [4, 4]),
]
EXPANDED_SALT = "tswn-expanded-20261027-v1"
TRAINING_SALT = "tswn-pgo-training-only-20261027-v1"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def roster_signature(data: bytes) -> tuple[str, ...]:
    """忽略队伍和选手顺序，保守检查是否重用了同一整套名字。"""
    return tuple(sorted(
        line.strip() for line in data.decode("utf-8").splitlines()
        if line.strip() and not line.startswith("seed:")
    ))


def raw_layout(data: bytes) -> list[int]:
    lines = data.decode("utf-8").replace("\r", "").rstrip("\n").split("\n")
    groups = [group.splitlines() for group in "\n".join(lines).split("\n\n")]
    if len(groups) == 1:
        groups = [[line] for line in lines]
    return [sum(not line.startswith("seed:") for line in group) for group in groups]


def generated_inputs(pool: list[str], salt: str, prefix: str) -> list[dict]:
    result = []
    for label, layout in LAYOUTS:
        for repetition in range(6):
            key = f"{salt}/{label}/{repetition}"
            names = sorted(pool, key=lambda name: sha256((key + "\0" + name).encode("utf-8")))[:sum(layout)]
            groups = []
            start = 0
            for size in layout:
                groups.append(names[start:start + size])
                start += size
            data = ("\n\n".join("\n".join(group) for group in groups) + "\n").encode("utf-8")
            result.append({
                "file": f"{prefix}__{label}__{repetition + 1:02}.txt",
                "kind": "generated",
                "selection_key": key,
                "sha256": sha256(data),
                "layout": layout,
                "names": names,
                "data": data,
            })
    return result


def write_suite(directory: Path, records: list[dict], metadata: dict) -> None:
    directory.mkdir()
    inputs = []
    for record in records:
        with (directory / record["file"]).open("xb") as output:
            output.write(record["data"])
        inputs.append({key: value for key, value in record.items() if key != "data"})
    manifest = {**metadata, "inputs": inputs}
    with (directory / "manifest.json").open("x", encoding="utf-8") as output:
        json.dump(manifest, output, ensure_ascii=False, indent=2)
        output.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True, help="公开 tswn-core 仓库根目录")
    parser.add_argument("--out", type=Path, required=True, help="尚不存在的输出目录")
    args = parser.parse_args()
    if args.out.exists():
        parser.error("输出目录已经存在；请选择新目录，不覆盖已有数据")

    stress_root = args.repo / "crates/tswn_test/cases/runtime_stress"
    fixed_root = args.repo / "docs/perf/fixed_cases_30"
    stress_files = sorted(stress_root.glob("*.txt"))
    fixed_files = sorted(fixed_root.glob("*.txt"))
    if len(stress_files) != 37 or len(fixed_files) != 30:
        parser.error("本版本矩阵要求 37 个 runtime_stress 输入与 30 个 fixed_cases 输入")

    pool = set()
    expanded = []
    for path in stress_files:
        data = path.read_bytes()
        expanded.append({
            "file": "stress__" + path.name,
            "kind": "existing_stress",
            "source": path.relative_to(args.repo).as_posix(),
            "sha256": sha256(data),
            "layout": raw_layout(data),
            "data": data,
        })
        pool.update(line for line in data.decode("utf-8").replace("\r", "").split("\n")
                    if line and not line.startswith("seed:") and "\\x" not in line)
    names = sorted(pool)
    if len(names) != 226:
        parser.error("公开输入名字池已变化；本版本矩阵要求 226 个唯一名字")
    expanded.extend(generated_inputs(names, EXPANDED_SALT, "generated"))
    training = generated_inputs(names, TRAINING_SALT, "train")
    heldout = [
        {"file": "expanded/" + record["file"], "sha256": record["sha256"], "data": record["data"]}
        for record in expanded
    ] + [
        {"file": path.relative_to(args.repo).as_posix(), "sha256": sha256(path.read_bytes()), "data": path.read_bytes()}
        for path in fixed_files
    ]
    heldout_signatures = {roster_signature(record["data"]) for record in heldout}
    heldout_hashes = {record["sha256"] for record in heldout}
    seen = set()
    for record in training:
        signature = roster_signature(record["data"])
        if signature in heldout_signatures or record["sha256"] in heldout_hashes or signature in seen:
            parser.error("训练 roster 重复或与留出评估重叠；不自动换样本，以免隐含筛选")
        seen.add(signature)

    common = {
        "version": 1,
        "selection_rule": "固定八种布局各六例；按 SHA256(salt/layout/repetition + NUL + name) 排序选取，独立于运行结果",
        "pool_names": names,
        "pool_sha256": sha256(("\n".join(names) + "\n").encode("utf-8")),
        "evaluation_round_range": {"start_inclusive": 0, "end_exclusive": "n（要求 n <= 1000000）"},
        "training_round_range": {"start_inclusive": 1_000_000, "end_exclusive": 1_010_000},
    }
    # 所有校验通过后才创建输出；若创建时发生并发冲突，直接报错，不写入旧目录。
    args.out.mkdir(parents=True, exist_ok=False)
    write_suite(args.out / "expanded", expanded, {
        **common, "existing_count": 37, "generated_count": 48,
        "notes": "压力输入逐字节保留 seed 行与字面量转义，使用 perf_runtime 标准解析；不是 strict-diff 特殊解码。",
    })
    write_suite(args.out / "training", training, {
        **common, "purpose": "仅供 PGO 训练，全部 115 个评估输入保持留出",
        "heldout_overlap_check": "无相同文件字节、无相同无序名字多重集；单个名字允许跨 roster 出现",
        "heldout_files": [{key: value for key, value in record.items() if key != "data"} for record in heldout],
    })
    print("已生成 85 个扩展评估输入和 48 个独立训练输入；115 个评估 roster 均未进入训练。")


if __name__ == "__main__":
    main()
