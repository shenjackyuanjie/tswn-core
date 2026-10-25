"""顶部三问：
  1. 榜首到底能遇到多少人（是不是封闭小圈子）？
  2. max−median 分差是「拉得过大」还是其实被压小了？对真值比。
  3. 前 10 换人少，是「固化」还是「准确」？
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params, load_real_scores  # noqa: E402

ROUNDS = 2500
TABLE = np.array(load_real_scores()["1a"], dtype=float)
N = len(TABLE)


def spearman(a, b):
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    ra -= ra.mean()
    rb -= rb.mean()
    return float(ra @ rb / np.sqrt((ra @ ra) * (rb @ rb)))


def truth_gaps():
    s = np.sort(TABLE)[::-1]
    med = np.median(TABLE)
    return {
        "max_minus_median": float(s[0] - med),
        "top10_mean_minus_median": float(s[:10].mean() - med),
        "max_minus_rank10": float(s[0] - s[9]),
    }


def run(mode: str) -> dict:
    p = Params(n_rounds=ROUNDS, seed=20261023, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", ring_mode=mode, track_pairs=True)
    g = GroupSim.build("1a", N, p, TABLE)
    for _ in range(ROUNDS):
        g.step()
    score = g.score.astype(float)
    order = np.argsort(-score)
    hist = np.array([s[1] - s[2] for s in g.score_snapshots], dtype=float)  # max−median 轨迹
    return {
        "gap_traj": hist,
        "max_minus_median": float(score.max() - np.median(score)),
        "top10_mean_minus_median": float(np.sort(score)[::-1][:10].mean() - np.median(score)),
        "max_minus_rank10": float(np.sort(score)[::-1][0] - np.sort(score)[::-1][9]),
        "sd": float(score.std()),
        "acc": spearman(TABLE, score),
        "top10_churn": float(np.mean([
            len(set(np.argsort(-np.array(R))[:10]) ^ set(np.argsort(-np.array(Rp))[:10]))
            for Rp, R in zip(g.rank_hist[-400:-1], g.rank_hist[-399:])
        ])),
        "order10": order[:10],
    }


def main() -> None:
    t = truth_gaps()
    print(f"真值（线上 1a 当前分数分布）: max−median={t['max_minus_median']:.0f}  "
          f"前10均值−median={t['top10_mean_minus_median']:.0f}  max−第10名={t['max_minus_rank10']:.0f}")
    print(f"N={N} 轮数={ROUNDS}")
    for mode in ("wrap", "wrap_plus"):
        r = run(mode)
        print(f"\n== {mode} ==")
        print(f"  max−median={r['max_minus_median']:.0f}（真值 {t['max_minus_median']:.0f}，"
              f"比例 {r['max_minus_median']/t['max_minus_median']*100:.0f}%）")
        print(f"  前10均值−median={r['top10_mean_minus_median']:.0f}"
              f"（真值 {t['top10_mean_minus_median']:.0f}，"
              f"比例 {r['top10_mean_minus_median']/t['top10_mean_minus_median']*100:.0f}%）")
        print(f"  max−第10名={r['max_minus_rank10']:.0f}（真值 {t['max_minus_rank10']:.0f}）")
        print(f"  榜sd={r['sd']:.0f}  对真值 Spearman={r['acc']:.4f}  前10每轮换人={r['top10_churn']:.2f}")
        traj = r["gap_traj"]
        print("  max−median 轨迹（每 300 轮）: " +
              " ".join(f"{v:.0f}" for v in traj[::3]))


if __name__ == "__main__":
    main()
