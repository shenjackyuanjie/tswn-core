#!/usr/bin/env python3
"""生成 PBB 命中候选的特征分布与无标签排序快照。

这是候选筛选分析，不把 PBB 分数误称为真实 TSWN 胜率标签。
"""
import argparse
import csv
import math
from collections import Counter
from pathlib import Path

ACTIVE = ["pbb_score_qp", "pbb_score_qd", "pbb_score_cqd", "pbb_score_cp",
          "pbb_score_fp", "pbb_score_wc", "pbb_score_fs", "pbb_score_pj"]

def q(values, p):
    xs = sorted(values)
    if not xs: return 0.0
    k = (len(xs) - 1) * p
    lo, hi = int(k), math.ceil(k)
    return xs[lo] if lo == hi else xs[lo] + (xs[hi] - xs[lo]) * (k - lo)

def pearson(a, b):
    ma, mb = sum(a)/len(a), sum(b)/len(b)
    den = math.sqrt(sum((x-ma)**2 for x in a) * sum((y-mb)**2 for y in b))
    return 0.0 if not den else sum((x-ma)*(y-mb) for x, y in zip(a, b)) / den

def percentile_ranks(values):
    order = sorted(range(len(values)), key=values.__getitem__)
    out = [0.0] * len(values)
    n = max(1, len(values)-1)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j+1]] == values[order[i]]: j += 1
        rank = ((i + j) / 2) / n
        for k in range(i, j+1): out[order[k]] = rank
        i = j + 1
    return out

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", type=Path, required=True)
    ap.add_argument("--output-dir", type=Path, required=True)
    ap.add_argument("--top", type=int, default=200)
    a = ap.parse_args()
    with a.input.open(encoding="utf-8", newline="") as f:
        rows = list(csv.DictReader(f))
    if not rows: raise SystemExit("输入特征文件为空")
    ids = [r["candidate_id"] for r in rows]
    if len(ids) != len(set(ids)): raise SystemExit("候选 ID 有重复，拒绝生成可能误导的排序")
    columns = ["pbb_sum", *ACTIVE]
    vals = {c: [float(r[c]) for r in rows] for c in columns}
    ranks = {c: percentile_ranks(vals[c]) for c in columns}
    composite = [sum(ranks[c][i] for c in columns) / len(columns) for i in range(len(rows))]
    a.output_dir.mkdir(parents=True, exist_ok=True)
    top_idx = sorted(range(len(rows)), key=lambda i: (composite[i], vals["pbb_sum"][i]), reverse=True)[:a.top]
    rank_path = a.output_dir / "pbb_composite_top.csv"
    fields = ["rank", "composite_percentile", "candidate_id", "name", "team", "pbb_sum", *ACTIVE,
              *[f"pbb_prop_{i}" for i in range(8)]]
    with rank_path.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields); w.writeheader()
        for rank, i in enumerate(top_idx, 1):
            r = rows[i]
            out = {k: r[k] for k in fields if k in r}
            out.update({"rank": rank, "composite_percentile": f"{composite[i]:.6f}"})
            w.writerow(out)
    report = ["# PBB 命中候选初步分析", "", f"- 输入：`{a.input.name}`", f"- 命中候选：{len(rows):,}",
              f"- 唯一候选：{len(set(ids)):,}",
              "- 标签状态：仅为 PBB 阈值命中；尚未包含 TSWN 实测胜率，不能视作胜率模型训练标签。", "",
              "## 分布", "", "|字段|最小|P25|中位数|P75|P95|最大|非零比例|", "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for c in columns:
        x = vals[c]
        report.append(f"|{c}|{q(x,0):.0f}|{q(x,.25):.0f}|{q(x,.5):.0f}|{q(x,.75):.0f}|{q(x,.95):.0f}|{q(x,1):.0f}|{sum(v != 0 for v in x)/len(x):.2%}|")
    report += ["", "## 与 PBB 八围总和的 Pearson 相关", "", "|评分|相关系数|", "|---|---:|"]
    for c in ACTIVE: report.append(f"|{c}|{pearson(vals['pbb_sum'], vals[c]):.4f}|")
    lengths = Counter(len(r["name"]) for r in rows)
    report += ["", "## 名字长度", "", "|长度|数量|", "|---:|---:|"]
    report += [f"|{k}|{v:,}|" for k, v in sorted(lengths.items())]
    report += ["", "## 排序说明", "", "`pbb_composite_top.csv` 的 composite_percentile 是 PBB 八围总和与默认门控的 8 个评分项各自百分位的等权平均，仅用于筛选待做 TSWN 对局标注的优先队列。"]
    (a.output_dir / "pbb_analysis_report.md").write_text("\n".join(report) + "\n", encoding="utf-8")
    print(f"rows={len(rows)} ranking={rank_path}")

if __name__ == "__main__": main()
