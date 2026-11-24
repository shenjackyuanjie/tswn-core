#!/usr/bin/env python3
"""PBB 16 槽技能的数值特征（train_website_rank_models.py 与
select_competition_candidates.py 共用同一口径，禁止单边修改）。

技能槽口径：pbb_skill_0..15 为槽内技能 id（>=35 表示非技能槽）、pbb_freq_0..15
为槽的最终熟练度（含 LastBoost/SlotBoost）；freq>0 且 id<35 的槽才是有效技能，
同一技能可占多个槽（熟练度按槽相加，如「隐匿 16+16」计 32）。
"""
import csv

SINGLE_SKILL_COLS = ("pbb_freq_sum", "pbb_freq_max", "pbb_slot_count",
                     "pbb_skill_count", "pbb_top_skill", "pbb_top_freq")
GROUP_SKILL_COLS = ("pbb_freq_sum_sum", "pbb_freq_sum_max", "pbb_skill_union",
                    "pbb_skill_overlap", "pbb_top_match", "pbb_top_freq_max")


def member_profile(row):
    """成员 PBB 特征行的技能画像：(技能 id 列表, 对应熟练度列表)。"""
    ids, freqs = [], []
    for k in range(16):
        sk = (row.get("pbb_skill_%d" % k) or "").strip()
        fq = (row.get("pbb_freq_%d" % k) or "").strip()
        if not sk or not fq:
            continue
        try:
            si, fi = int(sk), float(fq)
        except ValueError:
            continue
        if si < 35 and fi > 0:
            ids.append(si)
            freqs.append(fi)
    return ids, freqs


def _top(ids, freqs):
    """最高熟练度技能的下标；并列取技能 id 小者（稳定口径）。"""
    return max(range(len(ids)), key=lambda i: (freqs[i], -ids[i]))


def single_skill_features(rows):
    """单人技能特征（6 列）；rows 为单成员行列表。"""
    ids, freqs = member_profile(rows[0])
    if not ids:
        return {"pbb_freq_sum": 0.0, "pbb_freq_max": 0.0, "pbb_slot_count": 0.0,
                "pbb_skill_count": 0.0, "pbb_top_skill": -1.0, "pbb_top_freq": 0.0}
    top = _top(ids, freqs)
    return {"pbb_freq_sum": sum(freqs), "pbb_freq_max": max(freqs),
            "pbb_slot_count": float(len(ids)), "pbb_skill_count": float(len(set(ids))),
            "pbb_top_skill": float(ids[top]), "pbb_top_freq": freqs[top]}


def group_skill_features(rows):
    """组级技能特征（6 列）：总熟练度的和/极值、技能并集/交集（重叠/互补）、
    top 技能是否全相同、组内最高熟练度。rows 为各成员行列表（duo/trio）。"""
    profiles = [member_profile(r) for r in rows]
    id_sets = [set(p[0]) for p in profiles]
    freqs_sum = [sum(p[1]) for p in profiles]
    union = set().union(*id_sets)
    overlap = set.intersection(*id_sets)
    top_ids = [p[0][_top(p[0], p[1])] for p in profiles if p[0]]
    return {"pbb_freq_sum_sum": sum(freqs_sum), "pbb_freq_sum_max": max(freqs_sum),
            "pbb_skill_union": float(len(union)),
            "pbb_skill_overlap": float(len(overlap)),
            "pbb_top_match": float(len(set(top_ids)) == 1) if len(top_ids) == len(rows) else 0.0,
            "pbb_top_freq_max": max((max(p[1]) for p in profiles if p[1]), default=0.0)}


def skill_cols_for(kind):
    return SINGLE_SKILL_COLS if kind == "single" else GROUP_SKILL_COLS
