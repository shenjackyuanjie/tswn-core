#!/usr/bin/env python3
"""HP-only 胜率基线：只读数据集，用「队伍血量比例」做无参数基线。

这是 [battle-analyze.md](../docs/design/battle-analyze.md)「先用已决训练集建立 HP 等简单基线」
的第一版：不训练任何参数，只把每支输入队伍的存活实体血量比例作为分数，softmax 成概率，
再按 split 与 progress 分桶报告 Log Loss／Brier／Top-1，用来确认数据与标签可用、
以及后续复杂模型需要超过多少。

计费口径与数据集契约保持一致：只读 `samples.parquet`，标签取 `winner_team_index`，
默认排除空标签；`progress` 只用于分桶分析，不参与打分。

用法（从仓库根目录）：

    python scripts/winprob_hp_baseline.py --dataset target/winprob-100k
    python scripts/winprob_hp_baseline.py --dataset target/winprob-demo --json-out target/hp-baseline.json
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import numpy as np
import pyarrow.compute as pc
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]
PROGRESS_BUCKETS = [(0.2, "0-20%"), (0.4, "20-40%"), (0.6, "40-60%"), (0.8, "60-80%"), (1.01, "80-100%")]


def shard_count(dataset: Path) -> int:
    config = json.loads((dataset / "manifest.json").read_text(encoding="utf-8"))
    total = len(config["cases"]) * config["games_per_matchup"]
    per_shard = config["battles_per_shard"]
    return math.ceil(total / per_shard)


def load_samples(dataset: Path, limit: int) -> dict:
    """读所有已提交分片，返回 HP-only 基线需要的扁平数组。"""
    columns = ["split", "progress", "winner_team_index", "state"]
    hps, max_hps, teams, parents, alive = [], [], [], [], []
    splits, progress, labels, team_counts, row_offset = [], [], [], [], 0
    for index in range(shard_count(dataset)):
        path = dataset / f"shard-{index:06}" / "samples.parquet"
        table = pq.read_table(path, columns=columns)
        if limit and row_offset + table.num_rows > limit:
            table = table.slice(0, limit - row_offset)
        rows = table.num_rows
        if rows == 0:
            continue
        state = table.column("state").combine_chunks()
        entities = pc.list_flatten(state.field("entities"))
        parent = pc.list_parent_indices(state.field("entities")).to_numpy().astype(np.int64)
        runtime = entities.field("runtime")
        hps.append(runtime.field("hp").to_numpy(zero_copy_only=False).astype(np.float64))
        alive.append(runtime.field("alive").to_numpy(zero_copy_only=False).astype(bool))
        teams.append(entities.field("input_team_index").to_numpy(zero_copy_only=False).astype(np.int64))
        max_hps.append(entities.field("template").field("max_hp").to_numpy(zero_copy_only=False).astype(np.float64))
        parents.append(parent + row_offset)
        team_counts.append(pc.list_value_length(state.field("input_teams")).to_numpy().astype(np.int64))
        splits.append(table.column("split").to_pylist())
        progress.append(table.column("progress").to_numpy(zero_copy_only=False).astype(np.float64))
        labels.append(table.column("winner_team_index").to_pylist())
        row_offset += rows
    return {
        "hp": np.concatenate(hps) if hps else np.zeros(0),
        "max_hp": np.concatenate(max_hps) if max_hps else np.zeros(0),
        "team": np.concatenate(teams) if teams else np.zeros(0, dtype=np.int64),
        "parent": np.concatenate(parents) if parents else np.zeros(0, dtype=np.int64),
        "alive": np.concatenate(alive) if alive else np.zeros(0, dtype=bool),
        "split": [value for chunk in splits for value in chunk],
        "progress": np.concatenate(progress) if progress else np.zeros(0),
        "label": [value for chunk in labels for value in chunk],
        "team_count": np.concatenate(team_counts) if team_counts else np.zeros(0, dtype=np.int64),
        "rows": row_offset,
    }


def score_teams(samples: dict) -> np.ndarray:
    """每支输入队伍的血量比例：存活成员当前 HP / 该队全部成员的初始 HP 上限。

    分母必须包含已阵亡成员，否则「死掉一半人」不会让比例下降，基线就会退化成随机。
    未使用的队伍槽为 -inf。
    """
    rows = samples["rows"]
    team_max = int(samples["team_count"].max()) if rows else 0
    shape = (rows, team_max)
    flat = samples["parent"] * team_max + samples["team"]
    alive_hp = np.where(samples["alive"], samples["hp"], 0.0)
    hp_sum = np.bincount(flat, weights=alive_hp, minlength=rows * team_max).reshape(shape)
    max_sum = np.bincount(flat, weights=samples["max_hp"], minlength=rows * team_max).reshape(shape)
    ratio = hp_sum / np.maximum(max_sum, 1.0)
    valid = np.arange(team_max)[None, :] < samples["team_count"][:, None]
    return np.where(valid, ratio, -np.inf)


def softmax(scores: np.ndarray) -> np.ndarray:
    finite = np.isfinite(scores)
    shifted = np.where(finite, scores, -1e30)
    shifted = shifted - shifted.max(axis=1, keepdims=True)
    exponent = np.where(finite, np.exp(shifted), 0.0)
    total = exponent.sum(axis=1, keepdims=True)
    return np.divide(exponent, np.maximum(total, 1e-30), where=total > 0, out=np.zeros_like(exponent))


def bucket_of(progress: float) -> str:
    for upper, name in PROGRESS_BUCKETS:
        if progress < upper:
            return name
    return PROGRESS_BUCKETS[-1][1]


def metrics(probabilities: np.ndarray, labels: np.ndarray) -> dict:
    rows = len(labels)
    picked = probabilities[np.arange(rows), labels]
    one_hot = np.zeros_like(probabilities)
    one_hot[np.arange(rows), labels] = 1.0
    return {
        "samples": rows,
        "log_loss": float(-np.mean(np.log(np.maximum(picked, 1e-12)))),
        "brier": float(np.mean(np.sum((probabilities - one_hot) ** 2, axis=1))),
        "accuracy": float(np.mean(np.argmax(probabilities, axis=1) == labels)),
    }


def evaluate(samples: dict) -> dict:
    scores = score_teams(samples)
    probabilities = softmax(scores)
    splits_all = samples["split"]
    labels_all = samples["label"]
    report: dict = {}
    for split in sorted(set(splits_all)):
        index = np.array(
            [i for i, (value, label) in enumerate(zip(splits_all, labels_all)) if value == split and label is not None],
            dtype=np.int64,
        )
        if index.size == 0:
            continue
        labels = np.array([labels_all[i] for i in index], dtype=np.int64)
        probabilities_split = probabilities[index]
        entry = metrics(probabilities_split, labels)
        buckets: dict = {}
        for name in [name for _, name in PROGRESS_BUCKETS]:
            bucket_index = np.array(
                [position for position, i in enumerate(index) if bucket_of(samples["progress"][i]) == name],
                dtype=np.int64,
            )
            if bucket_index.size:
                buckets[name] = metrics(probabilities_split[bucket_index], labels[bucket_index])
        entry["by_progress"] = buckets
        entry["mean_winner_probability"] = float(np.mean(probabilities_split[np.arange(len(labels)), labels]))
        report[split] = entry
    report["_meta"] = {
        "rows": samples["rows"],
        "labeled_rows": sum(1 for value in labels_all if value is not None),
        "unlabeled_rows": sum(1 for value in labels_all if value is None),
        "scorer": "sum(alive hp) / sum(team max_hp, incl. dead members) per input team, softmax over used teams",
    }
    return report


def print_report(report: dict) -> None:
    print("# HP-only 基线（无参数）")
    print()
    for split, entry in report.items():
        if split.startswith("_"):
            continue
        print(
            f"- {split}: n={entry['samples']} log_loss={entry['log_loss']:.4f} "
            f"brier={entry['brier']:.4f} accuracy={entry['accuracy']:.4f}"
        )
        for name, bucket in entry["by_progress"].items():
            print(
                f"    {name}: n={bucket['samples']} log_loss={bucket['log_loss']:.4f} "
                f"brier={bucket['brier']:.4f} accuracy={bucket['accuracy']:.4f}"
            )
    meta = report["_meta"]
    print()
    print(f"- 行数 {meta['rows']}，已决 {meta['labeled_rows']}，空标签 {meta['unlabeled_rows']}")
    print(f"- 打分口径：{meta['scorer']}")


def main() -> None:
    parser = argparse.ArgumentParser(description="HP-only 胜率基线（只读数据集）。")
    parser.add_argument("--dataset", default=str(ROOT / "target" / "winprob-100k"), help="数据集目录")
    parser.add_argument("--limit", type=int, default=0, help="只统计前 N 个样本；0 表示全部")
    parser.add_argument("--json-out", default="", help="把完整结果写入该 JSON")
    args = parser.parse_args()

    samples = load_samples(Path(args.dataset), args.limit)
    report = evaluate(samples)
    print_report(report)
    if args.json_out:
        out = Path(args.json_out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"- 已写入 {out}")


if __name__ == "__main__":
    main()
