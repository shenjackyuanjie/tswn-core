"""固定状态重分配比例为 1，逐号最小化十项评分的最大误差。"""

import argparse
import csv
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path

import numpy as np
from scipy.optimize import linprog

import experiment_effective_skills as experiment


TOLERANCE = 2.0
LOG_STEP = .36


@dataclass(frozen=True)
class Rounds:
    start: int
    basic: int
    cqd: int
    pair: int


INITIAL = Rounds(262_144, 16_384, 384, 48)


def read_previous(folder):
    original, incumbent, labels, by_name = {}, {}, {}, {}
    for group in ("cqd", "sqp"):
        source = folder / f"{group}-final.txt"
        if not source.exists():
            source = folder / f"{group}-refined.txt"
        for i, raw in enumerate(experiment.cases(source), 1):
            label = f"{group}-{i:03}"
            name, overlay = experiment.payload(raw)
            key = by_name.setdefault(name, label)
            labels[label] = key
            if key == label:
                original[key] = name
                incumbent[key] = experiment.encode_effective(name, overlay)
    return original, incumbent, labels


def remember_inputs(args, contract, original, incumbent, labels):
    manifest = args.out / "inputs.json"
    contents = {"spec": contract, "original": original, "incumbent": incumbent, "labels": labels}
    if manifest.exists():
        if json.loads(manifest.read_text(encoding="utf-8")) != contents:
            raise ValueError(f"输入已变化，请为本次实验指定新的 --out：{args.out}")
    else:
        manifest.write_text(json.dumps(contents, ensure_ascii=False, indent=2), encoding="utf-8")


def row(name, label, raw, mates=None):
    result = {"id": f"{name}/{label}", "raw": raw}
    if mates is not None:
        result["pair_mates"] = mates
    return result


def vector(scores, heads):
    return np.array([experiment.delta(metric, scores[metric], 0, heads) for metric in experiment.METRICS])


def error(scores, name, heads, label="refined"):
    return vector(scores[f"{name}/{label}"], heads) - vector(scores[f"{name}/original"], heads)


def all_scores(args, contract, jobs, stage, rounds, pair_mode="pair"):
    base = experiment.run_scores(args, contract, jobs, f"{stage}-base", "base", rounds.start, rounds.basic, rounds.cqd)
    pair = experiment.run_scores(args, contract, jobs, f"{stage}-pair", pair_mode, rounds.start, rounds.pair, 1)
    return {key: val | pair[key] for key, val in base.items()}


def verify(args, contract, original, chosen, stage, rounds):
    jobs = experiment.jobs_for({key: original[key] for key in chosen}, "original")
    jobs += experiment.jobs_for(chosen, "refined")
    scores = all_scores(args, contract, jobs, stage, rounds, "pair-detail")
    detail = (json.loads(line) for line in experiment.cases(args.out / f"{stage}-pair.jsonl"))
    return scores, {item["id"]: item["teammate_rates"] for item in detail}


def shortlist(detail, name):
    a, b = (dict(detail[f"{name}/{tag}"]) for tag in ("original", "refined"))
    return {metric: sorted({i for i, _ in a[metric][:8] + b[metric][:8]})
            for metric in experiment.PAIR_METRICS}


def perturbed(raw, keys, change):
    ident, overlay = experiment.payload(raw)
    weights = overlay["effective_skills"]
    for key, offset in zip(keys, change, strict=True):
        weights[key] = round(weights[key] * math.exp(offset), 6)
    return experiment.encode_effective(ident, overlay)


def minimax(error, response, bound, penalty):
    size = response.shape[1]
    # 两组非负变量分别表示增减的对数权重，末项是允许的最大绝对误差。
    sides = np.concatenate((response, -response), axis=1)
    constraints = np.column_stack((np.concatenate((sides, -sides)), -np.ones(2 * len(error))))
    solved = linprog([penalty] * (2 * size) + [1.0], A_ub=constraints, b_ub=np.concatenate((-error, error)),
                     bounds=[(0, bound)] * (2 * size) + [(0, None)], method="highs")
    return solved.x[:size] - solved.x[size:2 * size] if solved.success else np.zeros(size)


def propose(args, contract, incumbent, errors, mates, step, rounds):
    jobs, keys_by_name, shapes = [], {}, {}
    for name in errors:
        raw = incumbent[name]
        weights = experiment.payload(raw)[1]["effective_skills"]
        keys = sorted((key for key, weight in weights.items() if weight > 0), key=lambda key: -weights[key])
        dim = len(keys)
        if not dim:
            continue
        keys_by_name[name] = keys
        jobs.append(row(name, "incumbent", raw, mates[name]))
        shapes[name] = {}
        for i, axis in enumerate(np.eye(dim)):
            for sign, direction in (("low", -LOG_STEP), ("high", LOG_STEP)):
                label = f"{i}-{sign}"
                candidate = perturbed(raw, keys, axis * direction)
                jobs.append(row(name, label, candidate, mates[name]))
                shapes[name][label] = candidate
    if not jobs:
        return {}
    measured = all_scores(args, contract, jobs, f"{step}-response", rounds)
    proposed = {}
    for name, keys in keys_by_name.items():
        raw = incumbent[name]
        local = {tag: vector(measured[f"{name}/{tag}"], args.heads) for tag in shapes[name]}
        anchor = vector(measured[f"{name}/incumbent"], args.heads)
        response = np.stack([(local[f"{i}-high"] - local[f"{i}-low"]) / (2 * LOG_STEP)
                             for i in range(len(keys))], axis=1)
        directions = sorted(local, key=lambda tag: np.max(np.abs(errors[name] + local[tag] - anchor)))
        options = {raw, *(shapes[name][tag] for tag in directions[:3])}
        for bound, penalty in ((.36, .22), (.70, .12), (1.05, .04)):
            options.add(perturbed(raw, keys, minimax(errors[name], response, bound, penalty)))
        _, overlay = experiment.payload(raw)
        options.update(experiment.effective(raw, family) for family in ("strict", "sp1", "sp2", "context"))
        total = sum(overlay["effective_skills"].values())
        scales = {.52, .72, .87, 1.18, 1.45, 1.85}
        if total > 128:
            scales.update((88 / total, 112 / total))
        options.update(perturbed(raw, keys, np.full(len(keys), math.log(scale))) for scale in scales)
        if step > 1:
            # 对多轮仍超差的名字，扩大扰动范围以跨过状态阈值和 128 权重的边界。
            rng = np.random.default_rng([step, int(name.split("-")[1])])
            moves = rng.uniform(-1.20, 1.20, size=(24, len(keys)))
            options.update(perturbed(raw, keys, movement) for movement in moves)
            for axis in np.eye(len(keys)):
                options.update(perturbed(raw, keys, axis * offset) for offset in (-1.15, -.65, .65, 1.15))
        proposed[name] = sorted(options - {raw})
    return proposed


def select(args, contract, original, incumbent, errors, mates, proposed, step, rounds):
    jobs = []
    for name, options in proposed.items():
        jobs += [row(name, "original", original[name], mates[name]), row(name, "incumbent", incumbent[name], mates[name])]
        jobs += [row(name, f"trial-{i:02}", candidate, mates[name]) for i, candidate in enumerate(options)]
    base = experiment.run_scores(args, contract, jobs, f"{step}-selection-base", "base", rounds.start,
                                 rounds.basic, rounds.cqd)
    by_id = {job["id"]: job for job in jobs}
    finalists, pair_jobs = {}, []
    for name, options in proposed.items():
        origin = base[f"{name}/original"]
        labels = [f"trial-{i:02}" for i in range(len(options))]
        labels.sort(key=lambda label: experiment.error_key(base[f"{name}/{label}"], origin, args.heads))
        # 基础误差明显的候选优先用五项筛选，配队主导的候选保留更多权重组合。
        base_worst = np.max(np.abs(errors[name][:5]))
        pair_worst = np.max(np.abs(errors[name][5:]))
        cutoff = 6 if base_worst >= pair_worst else 14
        finalists[name] = ["incumbent", *labels[:cutoff]]
        pair_jobs.extend(by_id[f"{name}/{label}"] for label in ("original", *finalists[name]))
    pair = experiment.run_scores(args, contract, pair_jobs, f"{step}-selection-pair", "pair", rounds.start,
                                 rounds.pair, 1)
    scores = {key: base[key] | value for key, value in pair.items()}
    result = {name: incumbent[name] for name in errors}
    for name, labels in finalists.items():
        observed_incumbent = error(scores, name, args.heads, "incumbent")

        def objective(label):
            estimate = error(scores, name, args.heads, label) + .30 * (errors[name] - observed_incumbent)
            return np.max(np.abs(estimate)), np.mean(np.abs(estimate))

        best = min(labels, key=objective)
        result[name] = by_id[f"{name}/{best}"]["raw"]
    return result


def write_result(args, original, final, labels, scores, windows):
    summary = {}
    expanded = {f"{label}/{tag}": scores[f"{name}/{tag}"] for label, name in labels.items()
                for tag in ("original", "refined")}
    for group in ("cqd", "sqp"):
        keys = [label for label in labels if label.startswith(group)]
        summary[group] = experiment.quality(keys, expanded, "refined", experiment.METRICS, args.heads)
        (args.out / f"{group}-final.txt").write_text(
            "\n".join(final[labels[label]] for label in keys) + "\n", encoding="utf-8")
    with (args.out / "final-scores.csv").open("w", encoding="utf-8-sig", newline="") as dest:
        writer = csv.writer(dest)
        writer.writerow(["id", "name", "round_start", "base_rounds", "cqd_rounds", "pair_rounds", "variant",
                         *experiment.METRICS])
        for label, name in labels.items():
            rounds = windows[name]
            for tag in ("original", "refined"):
                row_scores = scores[f"{name}/{tag}"]
                writer.writerow([label, original[name], rounds.start, rounds.basic, rounds.cqd, rounds.pair, tag,
                                 *(row_scores[metric] for metric in experiment.METRICS)])
    over_two = []
    for label, name in labels.items():
        errors = error(scores, name, args.heads)
        index = int(np.argmax(np.abs(errors)))
        if abs(errors[index]) > TOLERANCE:
            over_two.append((label, original[name], experiment.METRICS[index], float(errors[index]), windows[name].start))
    with (args.out / "final-over-2.csv").open("w", encoding="utf-8-sig", newline="") as dest:
        writer = csv.writer(dest)
        writer.writerow(["id", "name", "metric", "signed_error_pp", "round_start"])
        writer.writerows(sorted(over_two, key=lambda item: abs(item[3]), reverse=True))
    (args.out / "final-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    return len(over_two)


def audit_order(args, contract, final):
    jobs = experiment.jobs_for(final, "final")
    jobs += experiment.jobs_for({key: experiment.reverse_effective(raw) for key, raw in final.items()}, "reverse")
    scores = all_scores(args, contract, jobs, "order", Rounds(1_000_000_000, 64, 8, 2))
    mismatches = [name for name in final if scores[f"{name}/final"] != scores[f"{name}/reverse"]]
    (args.out / "order-check.json").write_text(json.dumps({"count": len(final), "mismatches": mismatches}, indent=2),
                                              encoding="utf-8")
    if mismatches:
        raise RuntimeError(f"技能重排后评分变化：{mismatches}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    base = experiment.REPO / "target"
    parser.add_argument("--previous", type=Path, default=base / "effective-skills-redistribution",
                        help="初始权重名单，支持 *-refined.txt 或 *-final.txt")
    parser.add_argument("--out", type=Path, default=base / "effective-skills-unified")
    parser.add_argument("--runner", type=Path,
                        default=base / "release-fast/examples" / f"effective_skill_scores{experiment.EXE}")
    parser.add_argument("--workers", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--passes", type=int, default=4, help="超差样本的最大校准轮数")
    args = parser.parse_args()
    if args.passes < 0:
        parser.error("--passes 必须非负")
    args.out.mkdir(parents=True, exist_ok=True)
    original, final, labels = read_previous(args.previous)
    contract = experiment.spec()
    args.heads = {preset["metric"]: preset["head"] for preset in contract["pair_presets"]}
    remember_inputs(args, contract, original, final, labels)
    scores, detail = verify(args, contract, original, final, "initial", INITIAL)
    windows = dict.fromkeys(original, INITIAL)
    for step in range(1, args.passes + 1):
        errors = {name: error(scores, name, args.heads) for name in original}
        hard = {name: values for name, values in errors.items() if np.max(np.abs(values)) > TOLERANCE}
        if not hard:
            break
        worst = max(np.max(np.abs(values)) for values in hard.values())
        print(f"第 {step} 轮，校准 {len(hard)} 个名字，当前最大误差 {worst:.3f}", flush=True)
        mates = {name: shortlist(detail, name) for name in hard}
        start = INITIAL.start + INITIAL.basic + (step - 1) * 131_072
        proposed = propose(args, contract, final, hard, mates, step, Rounds(start, 8_192, 256, 24))
        if not proposed:
            break
        chosen = select(args, contract, original, final, hard, mates, proposed, step,
                        Rounds(start + 8_192, 16_384, 512, 40))
        validation = Rounds(start + 24_576, 32_768, 768, 64)
        checked, rates = verify(args, contract, original, chosen, f"{step}-check", validation)
        final.update(chosen)
        scores.update(checked)
        detail.update(rates)
        windows.update(dict.fromkeys(chosen, validation))
    remaining = write_result(args, original, final, labels, scores, windows)
    audit_order(args, contract, final)
    print(f"{len(labels) - remaining}/{len(labels)} 行通过十项 2 点误差复核", flush=True)
    if remaining:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
