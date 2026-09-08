#!/usr/bin/env python3
"""按 FeatureEncoder 规格第 4 节的计费公式统计数据集的每样本容量峰值。

用于冻结 `baseline-32` profile 的 `H_max` / `L_max` / `Q_max` / `V_max` / `X_max`：
逐样本算出模板数、lane 总数、五类 lane list 条目、世界列表、状态及其注册序、保护链、
输入成员、解冰事件、deferred、槽条目和 X 记录数，再报 min/p50/p99/max。

计费口径与 `docs/design/feature-encoder-spec.md` 第 4 节一致：

    V_required = lane_lists + W + 3*s + P + I + ice + deferred + e
    X_required = clone_plan + P + assassinate + slot_ref + raw
    raw        = 8*e + 10*h + 3*s + 2*q + 2

用法（从仓库根目录）：

    python scripts/measure_encoder_capacity.py --dataset target/winprob-100k
    python scripts/measure_encoder_capacity.py --dataset target/winprob-100k --out target/caps-100k.json

只读：不修改数据集，不写除 `--out` 之外的任何文件。
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import time

try:
    import numpy as np
    import pyarrow.compute as pc
    import pyarrow.parquet as pq
except ImportError as error:  # pragma: no cover - 依赖缺失时给出明确提示
    print(f"需要 numpy 与 pyarrow：{error}", file=sys.stderr)
    raise SystemExit(2)

ROOT = pathlib.Path(__file__).resolve().parent.parent

KEYS = (
    "e",
    "h",
    "l",
    "lane_lists",
    "deferred",
    "w",
    "s",
    "p",
    "i",
    "ice",
    "q",
    "v_required",
    "x_required",
    "x_clone",
    "x_slot_ref",
    "x_raw",
)

# 默认注册表里可能持有 PlayerTemplate 的实体槽（见规格第 3.2 节）：幻影／使魔／丧尸蓝图。
TEMPLATE_ENTITY_SLOTS = (0, 1, 2)
# 默认注册表里唯一的实体引用槽：core.entity.summoned_entity。
ENTITY_REF_SLOT = 4


def list_lengths(list_array):
    """list 每行元素数；null 记 0。"""
    filled = pc.fill_null(pc.list_value_length(list_array), 0)
    return filled.to_numpy(zero_copy_only=False).astype(np.int64)


def group_sum(parents, values, rows):
    """按父行下标把 values 求和到 rows 行上。"""
    if len(parents) == 0:
        return np.zeros(rows, dtype=np.int64)
    return np.bincount(parents, weights=values, minlength=rows).astype(np.int64)


def template_lane_counts(template_array, valid_mask):
    """返回 (lane 总数, 五类 lane list 条目数, deferred 条目数)，仅统计有效模板。"""
    skills = template_array.field("skills")
    lanes = list_lengths(skills.field("lanes"))
    lists = lanes.copy()
    for name in ("merge_lane_order", "active_order", "pre_action_order", "post_damage_order"):
        lists += list_lengths(skills.field(name))
    deferred = list_lengths(skills.field("post_action_after_states"))
    if valid_mask is not None:
        lanes = lanes[valid_mask]
        lists = lists[valid_mask]
        deferred = deferred[valid_mask]
    return lanes, lists, deferred


def clone_plan_leaves(template_array, valid_mask):
    """计划存在记 1，每个 Some 的 slot_boosts[i] 记 2；None 不计费。"""
    plan = template_array.field("clone_build").field("score_skill_boost_plan")
    total = pc.is_valid(plan.field("initially_boosted_mask")).to_numpy(zero_copy_only=False).astype(np.int64)
    boosts = plan.field("slot_boosts")
    for index in ("0", "1"):
        total += pc.is_valid(boosts.field(index).field("0")).to_numpy(zero_copy_only=False).astype(np.int64) * 2
    return total[valid_mask] if valid_mask is not None else total


def measure_batch(state, stats):
    rows = len(state)
    entities = pc.list_flatten(state.field("entities"))
    entity_parent = pc.list_parent_indices(state.field("entities")).to_numpy().astype(np.int64)
    e_counts = np.bincount(entity_parent, minlength=rows).astype(np.int64)

    entity_template = entities.field("template")
    e_lanes, e_lists, e_deferred = template_lane_counts(entity_template, None)

    slots = pc.list_flatten(entities.field("slots"))
    slot_parent_entity = pc.list_parent_indices(entities.field("slots")).to_numpy().astype(np.int64)
    slot_parent_row = entity_parent[slot_parent_entity] if len(slot_parent_entity) else np.array([], dtype=np.int64)
    slot_template = slots.field("template")
    slot_valid = pc.is_valid(slot_template.field("skills")).to_numpy(zero_copy_only=False)
    s_lanes, s_lists, s_deferred = template_lane_counts(slot_template, slot_valid)

    global_slots = pc.list_flatten(state.field("template_slots"))
    global_parent = pc.list_parent_indices(state.field("template_slots")).to_numpy().astype(np.int64)
    global_template = global_slots.field("template")
    global_valid = pc.is_valid(global_template.field("skills")).to_numpy(zero_copy_only=False)
    g_lanes, g_lists, g_deferred = template_lane_counts(global_template, global_valid)

    h = e_counts.copy()
    h += group_sum(slot_parent_row[slot_valid], np.ones(int(slot_valid.sum()), dtype=np.int64), rows)
    h += group_sum(global_parent[global_valid], np.ones(int(global_valid.sum()), dtype=np.int64), rows)

    lanes_total = group_sum(entity_parent, e_lanes, rows) + group_sum(slot_parent_row[slot_valid], s_lanes, rows) + group_sum(global_parent[global_valid], g_lanes, rows)
    lane_lists = group_sum(entity_parent, e_lists, rows) + group_sum(slot_parent_row[slot_valid], s_lists, rows) + group_sum(global_parent[global_valid], g_lists, rows)
    deferred = group_sum(entity_parent, e_deferred, rows) + group_sum(slot_parent_row[slot_valid], s_deferred, rows) + group_sum(global_parent[global_valid], g_deferred, rows)

    world = state.field("world")
    world_lists = list_lengths(world.field("round_order")) + list_lengths(world.field("flat_alive"))
    for name in ("team_roster", "team_alive"):
        outer = world.field(name)
        world_lists += group_sum(
            pc.list_parent_indices(outer).to_numpy().astype(np.int64),
            list_lengths(pc.list_flatten(outer)),
            rows,
        )

    states = group_sum(entity_parent, list_lengths(entities.field("states")), rows)
    runtime = entities.field("runtime")
    protect = group_sum(entity_parent, list_lengths(runtime.field("protect_from")), rows)
    assassinate = runtime.field("assassinate")
    assassinate_count = pc.is_valid(assassinate.field("target")).to_numpy(zero_copy_only=False).astype(np.int64)
    assassinate_total = group_sum(entity_parent, assassinate_count, rows)

    slot_count = group_sum(entity_parent, list_lengths(entities.field("slots")), rows)
    slot_count += list_lengths(state.field("template_slots")) + list_lengths(state.field("battle_slots"))

    input_teams = state.field("input_teams")
    input_members = group_sum(
        pc.list_parent_indices(input_teams).to_numpy().astype(np.int64),
        list_lengths(pc.list_flatten(input_teams)),
        rows,
    )
    ice_events = list_lengths(state.field("ice_release_events"))

    x_clone = group_sum(entity_parent, clone_plan_leaves(entity_template, None), rows)
    x_clone += group_sum(slot_parent_row[slot_valid], clone_plan_leaves(slot_template, slot_valid), rows)
    x_clone += group_sum(global_parent[global_valid], clone_plan_leaves(global_template, global_valid), rows)

    slot_ids = slots.field("slot_id").to_numpy(zero_copy_only=False)
    slot_u64_valid = pc.is_valid(slots.field("u64_value")).to_numpy(zero_copy_only=False)
    ref_mask = slot_u64_valid & (slot_ids == ENTITY_REF_SLOT)
    x_slot_ref = group_sum(slot_parent_row[ref_mask], np.ones(int(ref_mask.sum()), dtype=np.int64), rows)

    x_raw = 8 * e_counts + 10 * h + 3 * states + 2 * slot_count + 2

    stats["e"].append(e_counts)
    stats["h"].append(h)
    stats["l"].append(lanes_total)
    stats["lane_lists"].append(lane_lists)
    stats["deferred"].append(deferred)
    stats["w"].append(world_lists)
    stats["s"].append(states)
    stats["p"].append(protect)
    stats["i"].append(input_members)
    stats["ice"].append(ice_events)
    stats["q"].append(slot_count)
    stats["v_required"].append(lane_lists + world_lists + 3 * states + protect + input_members + ice_events + deferred + e_counts)
    stats["x_required"].append(x_clone + protect + assassinate_total + x_slot_ref + x_raw)
    stats["x_clone"].append(x_clone)
    stats["x_slot_ref"].append(x_slot_ref)
    stats["x_raw"].append(x_raw)


def main():
    parser = argparse.ArgumentParser(description="统计 FeatureEncoder 的每样本容量峰值。")
    parser.add_argument("--dataset", default=str(ROOT / "target" / "winprob-100k"), help="数据集输出目录（默认 target/winprob-100k）")
    parser.add_argument("--shards", type=int, default=0, help="只统计前 N 个分片；0 表示全部")
    parser.add_argument("--batch-rows", type=int, default=2048, help="每批读取的样本行数")
    parser.add_argument("--out", default="", help="把汇总结果写成 JSON")
    args = parser.parse_args()

    dataset = pathlib.Path(args.dataset)
    if not dataset.is_dir():
        print(f"数据集目录不存在：{dataset}", file=sys.stderr)
        raise SystemExit(2)
    shards = sorted(dataset.glob("shard-*/samples.parquet"))
    if not shards:
        print(f"{dataset} 下没有 shard-*/samples.parquet", file=sys.stderr)
        raise SystemExit(2)
    if args.shards:
        shards = shards[: args.shards]

    stats = {key: [] for key in KEYS}
    started = time.time()
    rows = 0
    for index, path in enumerate(shards, 1):
        parquet = pq.ParquetFile(path)
        for batch in parquet.iter_batches(batch_size=args.batch_rows, columns=["state"]):
            state = batch.column("state")
            rows += len(state)
            measure_batch(state, stats)
        if index % 10 == 0 or index == len(shards):
            print(f"  {index}/{len(shards)} 分片，{rows} 行，{time.time() - started:.1f}s", flush=True)

    summary = {
        "dataset": str(dataset),
        "shards": len(shards),
        "rows": rows,
        "seconds": round(time.time() - started, 1),
        "metrics": {},
    }
    print(f"\n统计 {rows} 行 / {len(shards)} 分片，用时 {summary['seconds']}s")
    print(f"{'指标':<12}{'min':>7}{'p50':>8}{'p99':>9}{'max':>9}{'mean':>10}")
    for key in KEYS:
        values = np.concatenate(stats[key]) if stats[key] else np.zeros(1, dtype=np.int64)
        entry = {
            "min": int(values.min()),
            "p50": int(np.percentile(values, 50)),
            "p99": int(np.percentile(values, 99)),
            "max": int(values.max()),
            "mean": round(float(values.mean()), 3),
        }
        summary["metrics"][key] = entry
        print(f"{key:<12}{entry['min']:>7}{entry['p50']:>8}{entry['p99']:>9}{entry['max']:>9}{entry['mean']:>10}")
    if args.out:
        out = pathlib.Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print("已写出", out)


if __name__ == "__main__":
    main()
