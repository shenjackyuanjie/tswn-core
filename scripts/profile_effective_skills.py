"""用传统选技的扫描数据估计 SP 折减，并在独立名字、种子上比较评分。"""

import argparse
import csv
import json
import random
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from statistics import mean, median, quantiles

from experiment_effective_skills import (
    ACTIVE, EXE, METRICS, REPO, SP2, cases, delta, encode_effective, freeze, level, payload, run_scores, spec,
)


LEGACY_DYNAMIC = ("revive", "iron", "assassinate", "clone", "shadow")
LEGACY_COEFFICIENTS = dict(SP2, accumulate=.65)
PARTITIONS = {
    "shared": (METRICS,),
    "user": (METRICS[:4], METRICS[4:]),
    "format": (("qd", "pd", "cqd"), ("qp", "pp", *METRICS[5:])),
    "three": (("qd", "pd"), ("cqd",), ("qp", "pp", *METRICS[5:])),
    "four": (("qd", "pd"), ("cqd",), ("qp", "pp"), METRICS[5:]),
    "per-metric": tuple((metric,) for metric in METRICS),
}


def choose_names(calibration, holdout, train_size, test_size):
    names = list(dict.fromkeys(holdout))
    training_pool = sorted(set(calibration).difference(names))
    rng = random.Random(1729)
    if train_size > len(training_pool) or test_size > len(names):
        raise ValueError("独立训练或验证名单不足")
    return tuple(
        {f"{prefix}-{index:03}": name for index, name in enumerate(group, 1)}
        for prefix, group in (
            ("train", rng.sample(training_pool, train_size)),
            ("test", rng.sample(names, test_size)),
        )
    )


def configure(diy, coefficients, dynamic, cut_slots=10):
    ident, overlay = payload(diy)
    remaining, weights, cuts = 1.0, {}, {}
    for name, value in overlay["skills"].items():
        current = level(value)
        if current <= 0:
            continue
        skill = name.removeprefix("skl")
        if skill in dynamic:
            cuts[name] = 1.0 - remaining
            weights[name] = current
        elif skill in ACTIVE:
            weights[name] = remaining * min(current, 128)
            remaining *= 1.0 - coefficients[skill] * min(current, 128) / 128.0
        else:
            weights[name] = current
    overlay["effective_skills"] = weights
    overlay["effective_skill_adjustment"] = {
        name: (used / (1.0 - remaining) if remaining < 1.0 else 0.0) * (1.0 - cut_slots * .000001) + index * .000001
        for index, (name, used) in enumerate(cuts.items())
    }
    return encode_effective(ident, overlay)


def estimate(counts):
    if not counts["hit"] or not counts["nominal_128"]:
        return 0.0
    return counts["eligible_128"] / counts["nominal_128"] * counts["committed"] / counts["hit"]


def accumulate(rows):
    stats = defaultdict(lambda: defaultdict(int))
    for row in rows:
        for metric, observations in row["skill_stats"]:
            for observation in observations:
                skill = observation["skill"].removeprefix("core.skill.")
                if skill not in ACTIVE:
                    continue
                key = row["id"], metric, skill, observation["descendant"], observation["smart"]
                for field, value in observation.items():
                    if field not in ("skill", "descendant", "smart"):
                        stats[key][field] += value
    return stats


def collect_stats(args, contract, training, prefix, start, dense=False):
    rng = random.Random(596)
    pair_mates = {preset["metric"]: rng.sample(range(len(preset["teammates"])), preset["head"])
                  for preset in contract["pair_presets"]}
    jobs = [{"id": ident, "raw": raw, "pair_mates": pair_mates} for ident, raw in training.items()]
    base_n, cqd_n, pair_n = (65_536, 2048, 384) if dense else (args.base_n, args.cqd_n, args.pair_n)
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [
            executor.submit(run_scores, args, contract, jobs, f"{prefix}-{kind}", mode, start, n, cqd_n)
            for kind, mode, n in (("base", "stats-base", base_n), ("pair", "stats-pair", pair_n))
        ]
        for future in futures:
            future.result()
    return [json.loads(line) for kind in ("base", "pair") for line in cases(args.out / f"{prefix}-{kind}.jsonl")]


def coefficients_by_metric(stats):
    counts = defaultdict(lambda: defaultdict(int))
    # 每个指标先归一到一个同等权重的环境，避免多靶子指标单凭局数主导共享系数。
    for (_, metric, skill, _, _), row in stats.items():
        for key, value in row.items():
            counts[metric, skill][key] += value
    return {metric: {skill: estimate(counts[metric, skill]) for skill in ACTIVE} for metric in METRICS}


def dispersion(stats):
    counts = defaultdict(lambda: defaultdict(int))
    for (ident, metric, skill, descendant, smart), row in stats.items():
        groups = [(ident, metric, "all"), (ident, metric, f"{int(descendant)}{int(smart)}")]
        for group in groups:
            for key, value in row.items():
                counts[skill, group][key] += value
    results = {}
    for skill in ACTIVE:
        by_name = [estimate(row) for (entry, (_, _, mode)), row in counts.items()
                   if entry == skill and mode == "all" and row["nominal_128"] >= 64_000]
        differences = []
        for entry, (ident, metric, mode) in counts:
            if entry != skill or mode != "00":
                continue
            pairs = (("00", "01"), ("00", "10"), ("01", "11"))
            for left, right in pairs:
                a, b = counts.get((skill, (ident, metric, left))), counts.get((skill, (ident, metric, right)))
                if a and b and min(a["nominal_128"], b["nominal_128"]) >= 32_000:
                    differences.append(abs(estimate(a) - estimate(b)))
        if by_name:
            deciles = quantiles(by_name, n=10) if len(by_name) > 1 else by_name * 9
            results[skill] = dict(n=len(by_name), p10=deciles[0], p50=median(by_name), p90=deciles[-1],
                                  state_n=len(differences), state_gap=median(differences) if differences else 0)
    return results


def write_counts(path, stats):
    with path.open("w", newline="", encoding="utf-8-sig") as file:
        writer = csv.writer(file)
        fields = "reached nominal_128 eligible_128 gate_failed hit committed gate_to_skill no_target_to_skill".split()
        writer.writerow(["name_id", "metric", "skill", "descendant", "smart", *fields, "a_empirical",
                         "gate_to_basic", "no_target", "no_target_to_basic"])
        for key, counts in sorted(stats.items()):
            writer.writerow([*key, *(counts[field] for field in fields), estimate(counts),
                             counts["gate_failed"] - counts["gate_to_skill"], counts["hit"] - counts["committed"],
                             counts["hit"] - counts["committed"] - counts["no_target_to_skill"]])


def layouts(args, measured):
    dyn = tuple(args.dynamic.split(","))
    reduced = tuple(args.reduced.split(","))
    if any(skill not in ACTIVE for skill in (*dyn, *reduced)) or len(set(dyn)) != len(dyn) or len(dyn) > 10:
        raise ValueError("动态技能须为不重复的普通主动技，最多十项")
    # 发动率原值与现有整场近似模型的共同系数做插值。
    # 让工作流可以在不使用任何验证评分的情况下比较不同的混合程度。
    # 保留这一轮实验开始前的 40 维基线，不依赖运行器内置的默认系数。
    result = {"current": (PARTITIONS["shared"], LEGACY_DYNAMIC, 0.0),
              "shared-fixed": (PARTITIONS["shared"], LEGACY_DYNAMIC, 1.0),
              "shared-reduced": (PARTITIONS["shared"], reduced, 1.0),
              "shared-dynamic": (PARTITIONS["shared"], dyn, 1.0),
              "shared-mixed": (PARTITIONS["shared"], dyn, .5),
              "shared-no-clone": (PARTITIONS["shared"], tuple(skill for skill in dyn if skill != "clone"), 1.0),
              "shared-eight": (PARTITIONS["shared"], tuple(skill for skill in dyn if skill not in ("clone", "iron")), 1.0),
              "shared-accumulate": (PARTITIONS["shared"], tuple(dict.fromkeys(
                  "accumulate" if skill == "clone" else skill for skill in dyn)), 1.0)}
    result.update({name: (groups, dyn, 1.0) for name, groups in PARTITIONS.items() if name != "shared"})
    configured = {}
    for name, (groups, dynamic, mixture) in result.items():
        configured[name] = (dynamic, {
            group: {skill: mixture * mean(measured[metric][skill] for metric in group)
                    + (1.0 - mixture) * LEGACY_COEFFICIENTS[skill] for skill in ACTIVE}
            for group in groups
        })
    return configured


def evaluation_jobs(configured, originals, diy, selected, variants):
    jobs = [{"id": f"{ident}/original", "raw": originals[ident], "pair_mates": selected.get(ident, {})}
            for ident in originals]
    for variant in variants:
        dynamic, groups = configured[variant]
        for ident, raw in originals.items():
            by_metric = {}
            for metrics, coefficients in groups.items():
                value = configure(diy[ident], coefficients, dynamic, cut_slots=5 if variant == "current" else 10)
                by_metric.update(dict.fromkeys(metrics, value))
            jobs.append({"id": f"{ident}/{variant}", "raw": raw, "raw_by_metric": by_metric,
                         "pair_mates": selected.get(ident, {})})
    return jobs


def compare(args, contract, configured, names, prefix, start, variants, shortlist=None):
    diy = freeze(args, names)
    jobs = evaluation_jobs(configured, names, diy, shortlist or {}, variants)
    head = {preset["metric"]: preset["head"] for preset in contract["pair_presets"]}
    scores = {}
    for kind, n in (("base", args.eval_base_n), ("pair", args.eval_pair_n)):
        scores_by_kind = run_scores(args, contract, jobs, f"{prefix}-{kind}", kind, start, n, args.eval_cqd_n)
        for ident, values in scores_by_kind.items():
            scores.setdefault(ident, {}).update(values)
    errors = {variant: {ident: {metric: delta(metric, scores[f"{ident}/{variant}"][metric],
                                              scores[f"{ident}/original"][metric], head) for metric in METRICS}
                        for ident in names} for variant in variants}
    summary = {variant: dict(
        mae=mean(abs(value) for record in values.values() for value in record.values()),
        max_abs=max(abs(value) for record in values.values() for value in record.values()),
        per_metric={metric: mean(abs(record[metric]) for record in values.values()) for metric in METRICS},
        mean_name_max=mean(max(map(abs, record.values())) for record in values.values()),
    ) for variant, values in errors.items()}
    (args.out / f"{prefix}-summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    with (args.out / f"{prefix}-errors.csv").open("w", newline="", encoding="utf-8-sig") as file:
        writer = csv.writer(file)
        writer.writerow(["id", "name", "variant", *METRICS])
        for variant, values in errors.items():
            for ident, record in values.items():
                writer.writerow([ident, names[ident], variant, *(record[metric] for metric in METRICS)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("calibration", type=Path)
    parser.add_argument("holdout", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("stage", choices=("observe", "compare", "confirm", "focus"))
    parser.add_argument("--cli", type=Path, default=REPO / "target/release-fast" / f"tswn-cli{EXE}")
    parser.add_argument("--runner", type=Path, default=REPO / "target/release-fast/examples" / f"effective_skill_scores{EXE}")
    for name, default in (("workers", 14), ("train-size", 120), ("test-size", 96), ("base-n", 12288),
                          ("cqd-n", 160), ("pair-n", 80), ("eval-base-n", 16384), ("eval-cqd-n", 320),
                          ("eval-pair-n", 24)):
        parser.add_argument(f"--{name}", type=int, default=default)
    parser.add_argument("--dynamic", default="revive,iron,assassinate,clone,shadow,half,exchange,heal,absorb,charge")
    parser.add_argument("--reduced", default="revive,assassinate,half,exchange,heal")
    parser.add_argument("--variants", default="current,shared-fixed,shared-reduced,shared-dynamic,shared-no-clone,"
                        "shared-eight,shared-accumulate,user,format,three,four,per-metric")
    parser.add_argument("--focus", type=Path)
    args = parser.parse_args()
    if args.stage == "focus" and args.focus is None:
        parser.error("focus 阶段须提供 --focus 名单")
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    contract = spec()
    train, test = choose_names(cases(args.calibration), cases(args.holdout), args.train_size, args.test_size)
    if args.stage == "observe":
        rows = collect_stats(args, contract, train, "native", 65_536)
        stats = accumulate(rows)
        measured = coefficients_by_metric(stats)
        write_counts(args.out / "native-skill-counts.csv", stats)
        (args.out / "coefficients.json").write_text(json.dumps(measured, indent=2), encoding="utf-8")
        (args.out / "dispersion.json").write_text(json.dumps(dispersion(stats), indent=2), encoding="utf-8")
    elif args.stage == "focus":
        focus = {f"focus-{i:02}": name for i, name in enumerate(cases(args.focus), 1)}
        rows = collect_stats(args, contract, focus, "focus", 524_288, dense=True)
        write_counts(args.out / "focus-skill-counts.csv", accumulate(rows))
    else:
        measured = json.loads((args.out / "coefficients.json").read_text(encoding="utf-8"))
        configured = layouts(args, measured)
        variants = args.variants.split(",")
        if args.stage == "compare":
            # 筛选阶段缩小队友集合，用同一候选集比较全部方案。
            rng = random.Random(596)
            pool = {preset["metric"]: rng.sample(range(len(preset["teammates"])), preset["head"] * 2)
                    for preset in contract["pair_presets"]}
            compare(args, contract, configured, dict(list(test.items())[::2]), "screen", 262_144, variants,
                    {ident: pool for ident in test})
        else:
            compare(args, contract, configured, dict(list(test.items())[1::2]), "confirm", 1_048_576, variants)


if __name__ == "__main__":
    main()
