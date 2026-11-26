#!/usr/bin/env python3
"""从累计 PBB 候选池逐层生成 single/duo/trio 的待实战复核名单。

模型输入始终是 `tswn-cli namer-pf` 输出的完整组级特征，因此双人、三人的
TSWN 组队加成已包含在推断中；PBB 只负责廉价预筛和保留每日新候选的机会。
"""
import argparse
import csv
import hashlib
import itertools
import re
import subprocess
from collections import defaultdict
from pathlib import Path

from skill_features import single_skill_features, group_skill_features

SCORES = ("pp", "pd", "qp", "qd", "sum")
FEATURES = ["tswn_" + x for x in SCORES]
PBB_CORE = ("pbb_sum", "pbb_score_qp", "pbb_score_qd", "pbb_score_pp", "pbb_score_pd")


def stable_key(row):
    # 主排序是 PBB 总属性；稳定哈希仅用作无偏的并列/抽样顺序。
    digest = hashlib.sha256(row["candidate_id"].encode()).hexdigest()
    return (-float(row["pbb_sum"]), digest)


def call(args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def group_features(cli, group, cache):
    if group in cache:
        return cache[group]
    out = call([str(cli), "namer-pf", "-r", group, "-n", "16", "--keep-rq"])
    found = {"tswn_" + k: float(v) for k, v in re.findall(r"\b(pp|pd|qp|qd|sum):(\d+(?:\.\d+)?)", out)}
    if set(found) != set(FEATURES):
        raise RuntimeError("无法解析组级 TSWN 特征：" + out)
    cache[group] = found
    return found


def batch_group_features(cli, groups, threads, workdir):
    """一次批量 namer-pf 计算全部组合的组级五项评分（与逐组调用逐字一致，仅更快）。

    groups 为成员元组列表；组串用 + 连接后写入临时文件，批量模式下外层按组并行、
    内层单线程，胜负只由场数决定，线程数不影响结果（已对拍验证）。
    """
    if not groups:
        return {}
    groups = sorted(set(groups))
    path = Path(workdir) / "namer_pf_batch_input.txt"
    path.write_text("\n".join("+".join(members) for members in groups) + "\n", encoding="utf-8")
    out = call([str(cli), "namer-pf", "-f", str(path), "-n", "16", "--keep-rq", "-t", str(threads)])
    feats = {}
    for line in out.splitlines():
        m = re.match(r"^(.*) (pp|pd|qp|qd|sum):(\d+(?:\.\d+)?)$", line)
        if not m:
            raise RuntimeError("批量 namer-pf 输出无法解析：" + line)
        feats.setdefault(m.group(1), {})["tswn_" + m.group(2)] = float(m.group(3))
    for members in groups:
        group = "+".join(members)
        if set(feats.get(group, {})) != set(FEATURES):
            raise RuntimeError("批量 namer-pf 缺少组合：" + group)
    return feats


def predict(model, features, columns):
    return float(model.predict([[features[x] for x in columns]])[0])


def rank_groups(groups, kind, cli, model, cache, columns, pbb, candidate_day, precomputed=None):
    rows = []
    for i, members in enumerate(groups, 1):
        group = "+".join(members)
        if precomputed and group in precomputed:
            feat = dict(precomputed[group])
        else:
            feat = group_features(cli, group, cache)
        for field in PBB_CORE:
            values = [float(pbb[m][field]) for m in members]
            feat.update({f"{field}_sum":sum(values), f"{field}_mean":sum(values)/len(values),
                         f"{field}_min":min(values), f"{field}_max":max(values)})
        skill = (single_skill_features if kind == "single" else group_skill_features)(
            [pbb[m] for m in members])
        feat.update(skill)
        missing=[x for x in columns if x not in feat]
        if missing: raise RuntimeError(f"模型需要但无法生成的特征：{missing}")
        rows.append({"kind": kind, "group": group, "source_days": "+".join(sorted({candidate_day[m] for m in members})),
                     "predicted_win_rate": predict(model, feat, columns), **feat})
        if i % 100 == 0:
            print(f"{kind}: 已计算 {i}/{len(groups)} 个组合", flush=True)
    return sorted(rows, key=lambda r: (-r["predicted_win_rate"], r["group"]))


def write(path, rows, columns):
    path.parent.mkdir(parents=True, exist_ok=True)
    cols = ["kind", "group", "source_days", "predicted_win_rate", "expected_rank_in_format", "expected_percentile_in_format", "expected_website_rank", "expected_website_percentile", *columns]
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=cols, extrasaction="ignore")
        w.writeheader(); w.writerows(rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pool", type=Path, required=True)
    ap.add_argument("--cli", type=Path, required=True)
    ap.add_argument("--models-dir", type=Path, required=True,
                    help="含 single_model/duo_model/trio_model 的训练轮目录")
    ap.add_argument("--reference-training-dir", type=Path,
                    help="网站训练 CSV 所在目录；提供后计算相对网站参照榜单的预计名次")
    ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--screen-size", type=int, default=160,
                    help="PBB 全局前筛数量（每日保留名额在此之外）")
    ap.add_argument("--per-day", type=int, default=24,
                    help="每个 first_seen_day 至少保留的候选数")
    ap.add_argument("--single-beam", type=int, default=48)
    ap.add_argument("--per-day-beam", type=int, default=8, help="每一天至少进入组合 beam 的单人数量")
    ap.add_argument("--duo-seeds", type=int, default=24)
    ap.add_argument("--duo-partners", type=int, default=48)
    ap.add_argument("--duo-beam", type=int, default=48)
    ap.add_argument("--mixed-duo-beam", type=int, default=16, help="至少保留的跨日期 duo 数量")
    ap.add_argument("--trio-seeds", type=int, default=20)
    ap.add_argument("--trio-partners", type=int, default=36)
    ap.add_argument("--trio-beam", type=int, default=24)
    ap.add_argument("--threads", type=int, default=40,
                    help="批量 namer-pf 预计算的线程数（不改变结果，只影响速度）")
    a = ap.parse_args()
    if min(a.screen_size, a.per_day, a.single_beam, a.per_day_beam, a.duo_seeds, a.duo_partners, a.duo_beam, a.mixed_duo_beam, a.trio_seeds, a.trio_partners, a.trio_beam) < 1:
        raise SystemExit("所有数量参数必须为正")
    from catboost import CatBoostRegressor
    models = {}; columns = {}
    for kind in ("single", "duo", "trio"):
        model_dir = a.models_dir / f"{kind}_model"; p = model_dir / "model.cbm"
        if not p.exists():
            raise SystemExit(f"缺少模型：{p}")
        meta = model_dir / "metrics.json"
        if not meta.exists(): raise SystemExit(f"缺少模型特征清单：{meta}")
        import json
        columns[kind] = json.loads(meta.read_text(encoding="utf-8"))["features"]
        m = CatBoostRegressor(); m.load_model(str(p)); models[kind] = m
    with a.pool.open(encoding="utf-8", newline="") as f:
        source = list(csv.DictReader(f))
    if not source:
        raise SystemExit("候选池为空")
    if len({r["candidate_id"] for r in source}) != len(source):
        raise SystemExit("候选池 candidate_id 重复")
    pbb = {r["candidate_id"]:r for r in source}
    candidate_day = {r["candidate_id"]:r["first_seen_day"] for r in source}
    per_day = defaultdict(list)
    for row in source:
        per_day[row["first_seen_day"]].append(row)
    chosen = {r["candidate_id"]: r for r in sorted(source, key=stable_key)[:a.screen_size]}
    for day, rows in per_day.items():
        for row in sorted(rows, key=stable_key)[:a.per_day]:
            chosen[row["candidate_id"]] = row
    candidates = [r["candidate_id"] for r in sorted(chosen.values(), key=stable_key)]
    print(f"pool={len(source)} days={len(per_day)} screened={len(candidates)}", flush=True)
    cache = {}
    a.output_dir.mkdir(parents=True, exist_ok=True)
    singles = rank_groups([(x,) for x in candidates], "single", a.cli, models["single"], cache, columns["single"], pbb, candidate_day,
                          precomputed=batch_group_features(a.cli, [(x,) for x in candidates], a.threads, a.output_dir))
    def annotate(rows, kind):
        total = len(rows)
        reference_predictions = []
        if a.reference_training_dir:
            ref = a.reference_training_dir / f"{kind}_website_training.csv"
            if not ref.exists(): raise SystemExit(f"缺少网站参照数据：{ref}")
            with ref.open(encoding="utf-8", newline="") as f:
                for record in csv.DictReader(f):
                    reference_predictions.append(predict(models[kind], record, columns[kind]))
        for rank, row in enumerate(rows, 1):
            row["expected_rank_in_format"] = rank
            row["expected_percentile_in_format"] = 1.0 if total == 1 else 1 - (rank - 1) / (total - 1)
            if reference_predictions:
                # 1A、1B 等同型训练样本会合并以增加训练量；它们不是一个 197 队的真实榜单。
                # 先保留合并参照中的相对名次，再按百分位映射回实际每榜 100 队的名次。
                merged_rank = 1 + sum(p > row["predicted_win_rate"] for p in reference_predictions)
                row["expected_website_percentile"] = sum(row["predicted_win_rate"] >= p for p in reference_predictions) / len(reference_predictions)
                row["reference_merged_rank"] = merged_rank
                row["expected_website_rank"] = 1 + round((merged_rank - 1) * 99 / max(1, len(reference_predictions) - 1))
        return rows
    annotate(singles, "single")
    # 此名次仅在当前轮实际生成的同赛制组合里有效，不跨赛制比较。
    write(a.output_dir / "single_ranked.csv", singles, columns["single"])
    # 除全局 top 外，每日保留足够强的单人进入后续组合，确保新旧能同池配对。
    selected_single = {r["group"] for r in singles[:a.single_beam]}
    for day in sorted(per_day):
        day_rows = [r for r in singles if r["source_days"] == day]
        selected_single.update(r["group"] for r in day_rows[:a.per_day_beam])
    single_names = [r["group"] for r in singles if r["group"] in selected_single]
    # 种子和伙伴均含每日最优单人，避免 global top 恰好集中在某一天时只生成同日组合。
    seed_set = set(single_names[:a.duo_seeds])
    partner_set = set(single_names[:a.duo_partners])
    for day in sorted(per_day):
        day_names = [r["group"] for r in singles if r["source_days"] == day][:a.per_day_beam]
        seed_set.update(day_names); partner_set.update(day_names)
    duo_seed = [x for x in single_names if x in seed_set]
    duo_partner = [x for x in single_names if x in partner_set]
    duos = [(x, y) for x in duo_seed for y in duo_partner if x < y]
    duos = sorted(set(duos))
    duo_rows = rank_groups(duos, "duo", a.cli, models["duo"], cache, columns["duo"], pbb, candidate_day,
                           precomputed=batch_group_features(a.cli, duos, a.threads, a.output_dir))
    annotate(duo_rows, "duo")
    write(a.output_dir / "duo_ranked.csv", duo_rows, columns["duo"])
    duo_best = duo_rows[:a.duo_beam]
    mixed_duos = [r for r in duo_rows if "+" in r["source_days"]][:a.mixed_duo_beam]
    duo_best = list({r["group"]:r for r in duo_best + mixed_duos}.values())
    trio_partner = single_names[:a.trio_partners]
    trios = set()
    trio_seed_rows = list({r["group"]:r for r in duo_rows[:a.trio_seeds] + mixed_duos}.values())
    for row in trio_seed_rows:
        a1, a2 = row["group"].split("+")
        for a3 in trio_partner:
            if a3 not in (a1, a2):
                trios.add(tuple(sorted((a1, a2, a3))))
    trio_rows = rank_groups(sorted(trios), "trio", a.cli, models["trio"], cache, columns["trio"], pbb, candidate_day,
                            precomputed=batch_group_features(a.cli, sorted(trios), a.threads, a.output_dir))
    annotate(trio_rows, "trio")
    write(a.output_dir / "trio_ranked.csv", trio_rows, columns["trio"])
    finalists = singles[:2] + duo_rows[:2] + trio_rows[:1]
    # 五席跨赛制列不同；以统一的核心 TSWN 字段写出，详细特征分别在三个 ranked 文件。
    write(a.output_dir / "provisional_finalists.csv", finalists, FEATURES)
    (a.output_dir / "README.txt").write_text(
        "这是模型预筛的待实战复核名单，不是比赛站点最终排名。\n"
        "输出中的 expected_rank_in_format 是模型在本轮生成的同赛制候选中的预期名次；"
        "expected_percentile_in_format 是相应百分位，不能跨 single/duo/trio 比较。\n"
        "expected_website_rank 按合并训练参照的百分位换算为单个 100 队网站榜单名次；"
        "reference_merged_rank 是仅供审计的合并参照序号，不能当作网站名次。\n"
        "single 取前2，duo 取前2，trio 取前1；每次新 PBB 日数据入池后可重跑。\n"
        "组级 TSWN 特征由 namer-pf 直接计算，包含运行时的多人组队加成。\n",
        encoding="utf-8")
    print("已写入", a.output_dir, flush=True)


if __name__ == "__main__":
    main()
