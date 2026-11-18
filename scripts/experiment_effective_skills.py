"""构造 DIY 单抽权重，并用互不重叠的原轮次区间比较十项评分。"""

import argparse
import csv
import json
import os
import subprocess
import time
import tomllib
from pathlib import Path
from statistics import mean


REPO = Path(__file__).resolve().parents[1]
EXE = ".exe" if os.name == "nt" else ""
ACTIVE = """
    fire ice thunder quake absorb poison rapid critical half exchange berserk charm haste slow curse
    heal revive disperse iron charge accumulate assassinate summon clone shadow
""".split()
METRICS = ("qp", "qd", "pp", "pd", "cqd", "fp", "cp", "wcp", "fsp", "pjp")
PAIR_NAMES = {"辅评": "fp", "刺评": "cp", "无刺评": "wcp", "分身评": "fsp", "配件评": "pjp"}
SP1 = dict(exchange=.30, revive=.30, iron=.35, charge=.60, clone=.60, accumulate=.70, summon=.70)
SP2 = dict(zip(ACTIVE, (1, 1, 1, .5, .75, .75, 1, 1, .75, .5, 1, 1, 1, .75, 1, .75,
                       .2, 1, .75, .5, .3, .75, .75, .3, .75), strict=True))
CONTEXT = dict(absorb=.65, exchange=.30, haste=.65, heal=.45, revive=.08, disperse=.25, iron=.55,
               charge=.55, accumulate=.45, assassinate=.80, summon=.35, clone=.55, shadow=.65)
FAMILIES = ("strict", "sp1", "sp2", "context", "context-clone-low", "context-clone-high")
PAIR_METRICS = METRICS[5:]
REDISTRIBUTION = 1.0


def cases(path):
    return [line for line in path.read_text(encoding="utf-8-sig").splitlines() if line.strip()]


def table(path, field):
    return tomllib.loads(path.read_text(encoding="utf-8"))[field]


def spec():
    settings = REPO / "setting"
    targets = {t["id"]: table(settings / t["file"], "targets")
               for t in table(settings / "settings.toml", "targets") if t["id"] in (1, 2)}
    presets = [
        {"metric": PAIR_NAMES[t["name"]], "head": t["head"],
         "teammates": table(settings / t["file"], "targets")}
        for t in table(settings / "settings.toml", "teammate")
    ]
    return {"cqd_targets": targets[1], "pair_targets": targets[2], "pair_presets": presets}


def level(value):
    if isinstance(value, int):
        return value
    if value.startswith("2*"):
        return 2 * int(value[2:])
    base, boost = value.split("+")
    return int(base) + int(boost)


def payload(name):
    ident, overlay = name.split("+ol:", 1)
    return ident, json.loads(overlay)


def encode_effective(ident, overlay):
    overlay = dict(overlay, effective_skill_redistribution=REDISTRIBUTION)
    return ident + "+ol:" + json.dumps(overlay, ensure_ascii=False, separators=(",", ":"))


def reverse_effective(raw):
    ident, overlay = payload(raw)
    for key in ("skills", "effective_skills"):
        overlay[key] = dict(reversed(overlay[key].items()))
    return encode_effective(ident, overlay)


def effective(diy, family):
    ident, overlay = payload(diy)
    a = {"strict": {}, "sp1": SP1, "sp2": SP2}.get(family, CONTEXT)
    left = 1.0
    weights = {}
    for name, value in overlay["skills"].items():
        skill = name.removeprefix("skl")
        if skill in ACTIVE:
            raw = min(level(value), 128)
            current = raw
            if skill == "clone":
                if family == "context-clone-low":
                    current *= .70
                elif family == "context-clone-high":
                    current = min(128, current * 1.30)
            weights[name] = round(left * current, 6)
            left *= 1 - a.get(skill, 1.0) * current / 128
    overlay["effective_skills"] = weights
    return encode_effective(ident, overlay)


def run_scores(args, contract, jobs, stage, kind, start, count, cqd_count):
    output = args.out / f"{stage}.jsonl"
    requested = {job["id"] for job in jobs}
    scores = {}
    if output.exists():
        for line in cases(output):
            row = json.loads(line)
            if row["id"] not in requested or row["id"] in scores:
                raise ValueError(f"unexpected or duplicate id: {row['id']}")
            scores[row["id"]] = dict(row["scores"])
    remaining = [job for job in jobs if job["id"] not in scores]
    if not remaining:
        return scores
    request = output.with_suffix(".request.json")
    request.write_text(json.dumps(dict(contract, rows=remaining), ensure_ascii=False), encoding="utf-8")
    print(f"{stage}: {len(remaining)} rows, {count} rounds, cqd {cqd_count}, offset {start}", flush=True)
    begin = time.monotonic()
    command = [args.runner, kind, str(start), str(count), str(cqd_count), str(args.workers)]
    with request.open("rb") as source, output.open("ab") as sink:
        subprocess.run(command, stdin=source, stdout=sink, cwd=REPO, check=True)
    done = [json.loads(line) for line in cases(output)]
    if len(done) != len(requested) or {row["id"] for row in done} != requested:
        raise RuntimeError(f"{stage}: incomplete results")
    print(f"{stage}: {len(done)} complete in {time.monotonic() - begin:.1f}s; "
          f"{sum(row['guard_exhausted'] for row in done)} guards", flush=True)
    return {row["id"]: dict(row["scores"]) for row in done}


def jobs_for(people, variant):
    return [{"id": f"{name}/{variant}", "raw": raw} for name, raw in people.items()]


def freeze(args, people):
    source, converted = args.out / "source.txt", args.out / "plain-diy.txt"
    source.write_text("\n".join(people.values()) + "\n", encoding="utf-8")
    subprocess.run([args.cli, "to-diy", "-f", str(source), "-o", str(converted), "--no-details"],
                   cwd=REPO, stdout=subprocess.DEVNULL, check=True)
    lines = cases(converted)
    if len(lines) != len(people):
        raise RuntimeError("DIY 导出行数与名字列表不一致")
    for raw, diy in zip(people.values(), lines, strict=True):
        if payload(diy)[0] != raw:
            raise RuntimeError(f"DIY 导出身份与原名不一致: {raw} vs {payload(diy)[0]}")
    return dict(zip(people, lines, strict=True))


def delta(metric, score, baseline, head):
    unit = 100 if metric in METRICS[:4] else head.get(metric, 1)
    return (score - baseline) / unit


def error_key(row, baseline, head):
    errors = [abs(delta(metric, value, baseline[metric], head)) for metric, value in row.items()]
    return max(errors), mean(errors)


def quality(ids, results, variant, metrics, head):
    by_metric = {
        metric: [delta(metric, results[f"{name}/{variant}"][metric], results[f"{name}/original"][metric], head)
                 for name in ids]
        for metric in metrics
    }
    maximum = [max(map(abs, errors)) for errors in zip(*by_metric.values(), strict=True)]
    return {
        "count": len(ids),
        "all_within": {t: sum(x <= t for x in maximum) for t in (1, 2, 3, 5)},
        "median_max": percentile(maximum, .50),
        "p90_max": percentile(maximum, .90),
        "largest_error": max(maximum),
        "metric_mae": {key: mean(map(abs, value)) for key, value in by_metric.items()},
        "metric_bias": {key: mean(value) for key, value in by_metric.items()},
    }


def percentile(numbers, p):
    ordered = sorted(numbers)
    return ordered[round(p * (len(ordered) - 1))]


def report(args, people, family, selected, result, pair, head, order_errors):
    joined = {key: (row | pair.get(key, {})) for key, row in result.items()}
    summary = {"global_family": family, "order_mismatches": order_errors, "datasets": {}}
    for group in ("cqd", "sqp"):
        ids = [name for name in people if name.startswith(group)]
        with_pair = [name for name in ids if f"{name}/fitted" in pair]
        summary["datasets"][group] = {
            "base": {v: quality(ids, joined, v, METRICS[:5], head) for v in ("diy", "global", "fitted")},
            "ten": quality(with_pair, joined, "fitted", METRICS, head) if with_pair else None,
            "clone_count": sum(has_clone(people[name]) for name in ids),
        }
        if with_pair and all(f"{name}/global" in pair for name in with_pair):
            summary["datasets"][group]["ten_global"] = quality(with_pair, joined, "global", METRICS, head)
        if with_pair:
            clone = [name for name in with_pair if has_clone(people[name])]
            if clone:
                summary["datasets"][group]["ten_clone"] = quality(clone, joined, "fitted", METRICS, head)
    with (args.out / "scores.csv").open("w", encoding="utf-8-sig", newline="") as dest:
        writer = csv.writer(dest)
        writer.writerow(["id", "name", "has_clone", "fit_family", "variant", *METRICS])
        for name, diy in people.items():
            for variant in ("original", "diy", "global", "fitted"):
                row = joined[f"{name}/{variant}"]
                writer.writerow([name, payload(diy)[0], has_clone(diy), selected[name], variant,
                                 *(row.get(metric) for metric in METRICS)])
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False), flush=True)


def has_clone(diy):
    return bool(payload(diy)[1]["skills"].get("sklclone", 0))


def local_candidates(incumbent, global_fit):
    ident, overlay = payload(incumbent)
    weights = overlay["effective_skills"]
    moves = [
        ("mass-down", {key: .78 for key in weights}),
        ("mass-up", {key: 1.26 for key in weights}),
        ("mass-up-more", {key: 1.55 for key in weights}),
    ]
    dominant = [key for key in sorted(weights, key=weights.get, reverse=True) if weights[key] > 0][:2]
    if weights.get("sklclone", 0) and "sklclone" not in dominant:
        dominant.append("sklclone")
    for key in dominant:
        moves.extend(((f"{key}-down", {key: .58}), (f"{key}-up", {key: 1.58})))
    options = {"incumbent": incumbent, "global": global_fit}
    for label, factors in moves:
        new_weights = {key: round(weight * factors.get(key, 1), 6) for key, weight in weights.items()}
        edited = dict(overlay, effective_skills=new_weights)
        options[label] = encode_effective(ident, edited)
    by_raw = {}
    for label, raw in options.items():
        by_raw.setdefault(raw, label)
    return {label: raw for raw, label in by_raw.items()}


def refine(args, contract, original, incumbent, global_fit, base, pair, head):
    difficult = []
    for name in original:
        fitted_id = f"{name}/fitted"
        baseline_id = f"{name}/original"
        if fitted_id in pair:
            candidate = base[fitted_id] | pair[fitted_id]
            baseline = base[baseline_id] | pair[baseline_id]
            worst, _ = error_key(candidate, baseline, head)
            if worst > 2.5:
                difficult.append(name)
    start = max(args.pilot_n, args.pilot_cqd) + max(args.test_n, args.test_cqd, args.pair_n)
    if not difficult:
        for group in ("cqd", "sqp"):
            (args.out / f"{group}-refined.txt").write_text(
                "\n".join(raw for name, raw in incumbent.items() if name.startswith(group)) + "\n", encoding="utf-8")
        return incumbent, {}, set(), start
    variants = {name: local_candidates(incumbent[name], global_fit[name]) for name in difficult}
    origins = jobs_for({name: original[name] for name in difficult}, "original")
    candidates = [{"id": f"{name}/{tag}", "raw": raw}
                  for name, options in variants.items() for tag, raw in options.items()]
    pilot = run_scores(args, contract, origins + candidates, f"refine-base-{start}", "base", start, 1536, 96)
    pair_candidates = []
    for name, options in variants.items():
        baseline = pilot[f"{name}/original"]
        ordered = sorted(options, key=lambda tag: error_key(pilot[f"{name}/{tag}"], baseline, head))
        tags = list(dict.fromkeys(["incumbent", *ordered[:3]]))
        pair_candidates.extend({"id": f"{name}/{tag}", "raw": options[tag]} for tag in tags)
    pair_pilot = run_scores(args, contract, origins + pair_candidates, f"refine-pair-{start}", "pair", start, 12, 1)
    chosen = {}
    for name, options in variants.items():
        baseline = pilot[f"{name}/original"] | pair_pilot[f"{name}/original"]
        scored = (tag for tag in options if f"{name}/{tag}" in pair_pilot)
        chosen[name] = min(scored, key=lambda tag: error_key(
            pilot[f"{name}/{tag}"] | pair_pilot[f"{name}/{tag}"], baseline, head))
    refined = incumbent | {name: variants[name][tag] for name, tag in chosen.items()}
    for group in ("cqd", "sqp"):
        (args.out / f"{group}-refined.txt").write_text(
            "\n".join(raw for name, raw in refined.items() if name.startswith(group)) + "\n", encoding="utf-8")
    before = jobs_for({name: incumbent[name] for name in difficult}, "incumbent")
    after = jobs_for({name: refined[name] for name in difficult if chosen[name] != "incumbent"}, "refined")
    validation = origins + before + after
    check_start = start + 1536
    checked = run_scores(args, contract, validation, f"refine-check-base-{check_start}", "base", check_start, 4096, 192)
    checked_pair = run_scores(args, contract, validation, f"refine-check-pair-{check_start}", "pair", check_start, 24, 1)
    checked = {key: value | checked_pair[key] for key, value in checked.items()}
    for name, tag in chosen.items():
        if tag == "incumbent":
            checked[f"{name}/refined"] = checked[f"{name}/incumbent"]
    report = {group: {
        tag: quality([name for name in difficult if name.startswith(group)], checked, tag, METRICS, head)
        for tag in ("incumbent", "refined")
    } for group in ("cqd", "sqp") if any(name.startswith(group) for name in difficult)}
    output = {"chosen": chosen, "quality": report, "scores": checked}
    (args.out / "refinement.json").write_text(json.dumps(output, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"Refinement: tried {len(difficult)} names, changed {sum(k != 'incumbent' for k in chosen.values())}", flush=True)
    return refined, checked, set(difficult), check_start


def final_check(args, contract, original, refined, checked, difficult, start, head):
    pending = [name for name in original if name not in difficult]
    jobs = jobs_for({name: original[name] for name in pending}, "original")
    jobs += jobs_for({name: refined[name] for name in pending}, "refined")
    if jobs:
        base = run_scores(args, contract, jobs, f"final-base-{start}", "base", start, 4096, 192)
        pair = run_scores(args, contract, jobs, f"final-pair-{start}", "pair", start, 24, 1)
        checked.update({key: row | pair[key] for key, row in base.items()})
    summary = {}
    for group in ("cqd", "sqp"):
        ids = [name for name in original if name.startswith(group)]
        clone = [name for name in ids if has_clone(refined[name])]
        summary[group] = quality(ids, checked, "refined", METRICS, head)
        if clone:
            summary[f"{group}_clone"] = quality(clone, checked, "refined", METRICS, head)
    with (args.out / "final-scores.csv").open("w", encoding="utf-8-sig", newline="") as dest:
        writer = csv.writer(dest)
        writer.writerow(["id", "name", "variant", *METRICS])
        for name, raw in original.items():
            for variant in ("original", "refined"):
                row = checked[f"{name}/{variant}"]
                writer.writerow([name, raw, variant, *(row[metric] for metric in METRICS)])
    (args.out / "final-summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"Final independent check: {len(original)} names, rounds from {start}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sqp", type=Path)
    parser.add_argument("--out", type=Path, default=REPO / "target" / "effective-skills-redistribution")
    parser.add_argument("--cli", type=Path, default=REPO / "target/release-fast" / f"tswn-cli{EXE}")
    parser.add_argument("--runner", type=Path,
                        default=REPO / "target/release-fast/examples" / f"effective_skill_scores{EXE}")
    parser.add_argument("--workers", type=int, default=min(24, os.cpu_count() or 1))
    parser.add_argument("--pilot-n", type=int, default=512)
    parser.add_argument("--pilot-cqd", type=int, default=24)
    parser.add_argument("--test-n", type=int, default=4096)
    parser.add_argument("--test-cqd", type=int, default=192)
    parser.add_argument("--pair-n", type=int, default=24)
    parser.add_argument("--sqp-pair", type=int, help="限制十项复核的 SQP 名字数，默认覆盖全部")
    parser.add_argument("--without-pair", action="store_true")
    parser.add_argument("--refine", action="store_true", help="对十项偏差较大的名字追加独立十项校准")
    parser.add_argument("--global-pair", action="store_true", help="同时复核固定公式的五项配队评分")
    parser.add_argument("--final-check", action="store_true", help="用同一区间复核校准后的完整名单")
    args = parser.parse_args()
    if args.final_check and (not args.refine or args.without_pair):
        parser.error("--final-check 需要 --refine 和完整配队评分")
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    contract = spec()
    cqd = [row["players"][0] for row in contract["cqd_targets"]]
    sqp = list(dict.fromkeys(cases(args.sqp)))
    sqp_pair_limit = len(sqp) if args.sqp_pair is None else args.sqp_pair
    if args.final_check and sqp_pair_limit < len(sqp):
        parser.error("--final-check 需要覆盖全部 SQP 名字的配队评分")
    original = {f"{group}-{i:03}": name for group, names in (("cqd", cqd), ("sqp", sqp))
                for i, name in enumerate(names, start=1)}
    head = {p["metric"]: p["head"] for p in contract["pair_presets"]}
    print(f"Inputs: {len(cqd)} CQD + {len(sqp)} SQP", flush=True)
    diy = freeze(args, original)
    pilot_jobs = jobs_for(original, "original")
    for family in FAMILIES:
        pilot_jobs.extend(jobs_for({key: effective(name, family) for key, name in diy.items()}, family))
    pilot = run_scores(args, contract, pilot_jobs, f"pilot-{args.pilot_n}-{args.pilot_cqd}", "base", 0, args.pilot_n,
                       args.pilot_cqd)
    per_name = lambda name, family: error_key(pilot[f"{name}/{family}"], pilot[f"{name}/original"], head)
    family = min(FAMILIES, key=lambda f: mean(per_name(name, f)[0] for name in original if name.startswith("cqd")))
    selected = {name: min(FAMILIES, key=lambda f: per_name(name, f)) for name in original}
    fitted = {key: effective(name, selected[key]) for key, name in diy.items()}
    global_fit = {key: effective(name, family) for key, name in diy.items()}
    reverse = {key: reverse_effective(raw) for key, raw in fitted.items()}
    for group in ("cqd", "sqp"):
        for variant, variants in (("fitted", fitted), ("global", global_fit), ("reversed", reverse)):
            (args.out / f"{group}-{variant}.txt").write_text(
                "\n".join(raw for key, raw in variants.items() if key.startswith(group)) + "\n", encoding="utf-8")
    # 同一个输入只测一次，随后把对应分数复制给选择了同一方案的变体。
    all_tests = (jobs_for(original, "original") + jobs_for(diy, "diy") + jobs_for(global_fit, "global")
                 + jobs_for(fitted, "fitted"))
    by_raw = {}
    id_to_canonical = {job["id"]: by_raw.setdefault(job["raw"], job["id"]) for job in all_tests}
    test_jobs = [{"id": label, "raw": raw} for raw, label in by_raw.items()]
    start = max(args.pilot_n, args.pilot_cqd)
    result = run_scores(args, contract, test_jobs, f"base-{start}-{args.test_n}-{args.test_cqd}", "base", start,
                        args.test_n, args.test_cqd)
    result = {key: result[value] for key, value in id_to_canonical.items()}
    order_original = {key: fitted[key] for key in original}
    order_jobs = jobs_for(order_original, "fitted") + jobs_for(reverse, "reverse")
    order_all = run_scores(args, contract, order_jobs, "order-base", "base", start, 48, 8)
    order_errors = [name for name in original if order_all[f"{name}/fitted"] != order_all[f"{name}/reverse"]]
    print(f"Order base mismatches: {order_errors}", flush=True)
    report(args, diy, family, selected, result, {}, head, order_errors)
    pair = {}
    if not args.without_pair:
        for group, limit in (("cqd", len(cqd)), ("sqp", sqp_pair_limit)):
            ids = [name for name in original if name.startswith(group)][:limit]
            if not ids:
                continue
            pair_jobs = sum((jobs_for({key: variant[key] for key in ids}, tag)
                             for tag, variant in (("original", original), ("fitted", fitted))), [])
            pair.update(run_scores(args, contract, pair_jobs, f"pair-{group}-{limit}-{args.pair_n}", "pair", start,
                                   args.pair_n, 1))
            if args.global_pair:
                different = [name for name in ids if fitted[name] != global_fit[name]]
                global_jobs = jobs_for({key: global_fit[key] for key in different}, "global")
                if global_jobs:
                    pair.update(run_scores(args, contract, global_jobs, f"pair-global-{group}-{limit}-{args.pair_n}",
                                           "pair", start, args.pair_n, 1))
                pair.update({f"{name}/global": pair[f"{name}/fitted"]
                             for name in ids if fitted[name] == global_fit[name]})
            order_pair_jobs = jobs_for({key: fitted[key] for key in ids}, "fitted")
            order_pair_jobs += jobs_for({key: reverse[key] for key in ids}, "reverse")
            order_pair = run_scores(args, contract, order_pair_jobs, f"order-pair-{group}", "pair", start, 2, 1)
            order_errors.extend(name for name in ids if order_pair[f"{name}/fitted"] != order_pair[f"{name}/reverse"])
            report(args, diy, family, selected, result, pair, head, order_errors)
    if order_errors:
        raise RuntimeError(f"order mismatches: {order_errors}")
    if args.refine:
        calibration = refine(args, contract, original, fitted, global_fit, result, pair, head)
        if args.final_check:
            final_check(args, contract, original, *calibration, head)


if __name__ == "__main__":
    main()
