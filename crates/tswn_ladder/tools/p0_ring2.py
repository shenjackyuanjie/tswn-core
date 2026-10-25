"""环状拓扑三方案对比（2 个 seed），把「消除浪费」和「扩大候选池」拆开看。

wrap：现状，i±1..r，溢出绕到另一端（榜顶的「上探」绕去榜底，必然被门拒）。
skip：只去掉绕行，候选预算两侧各保持 r —— 有效候选池不变，纯粹少做无效探测。
fold：你的方案，溢出预算折半转到另一侧（榜顶 r=40 → 下探 60）—— 有效池会变大。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params  # noqa: E402

GROUPS = [("1a", 106), ("3a", 37)]
ROUNDS = 700
SEEDS = (20261023, 20261024)


def spearman(a, b):
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    ra -= ra.mean()
    rb -= rb.mean()
    return float(ra @ rb / np.sqrt((ra @ ra) * (rb @ rb)))


def run(mode: str) -> dict:
    vols, accs, probes, matches, hit_top, hit_bot = [], [], [], [], [], []
    for seed in SEEDS:
        for name, n in GROUPS:
            p = Params(n_rounds=ROUNDS, seed=seed, k_shrink_matches=200.0, k_min_ratio=0.15,
                       decay_mode="uniform", ring_mode=mode, probe_stats=True)
            g = GroupSim.build(name, n, p)
            for _ in range(ROUNDS):
                g.step()
            warm = ROUNDS // 5
            R = np.array(g.rank_hist)[warm:]
            vols.append(float(np.abs(R[1:] - R[:-1]).mean(axis=1).mean()))
            accs.append(spearman(g.true_strength, g.score.astype(float)))
            pr = np.array(g.probe_probes, dtype=float)
            hi = np.array(g.probe_hits, dtype=float)
            probes.append(pr.sum() / ROUNDS)
            hit_top.append(hi[0] / pr[0] * 100)
            hit_bot.append(hi[2] / pr[2] * 100)
            matches.append(float(np.mean(g.match_counts[warm:])))
    return {
        "vol": float(np.mean(vols)),
        "acc": float(np.mean(accs)),
        "probes": float(np.mean(probes)),
        "matches": float(np.mean(matches)),
        "hit_top": float(np.mean(hit_top)),
        "hit_bot": float(np.mean(hit_bot)),
    }


def main() -> None:
    print(f"组={[g[0] for g in GROUPS]} 轮数={ROUNDS} seeds={len(SEEDS)}（每格是 4 次运行的均值）")
    print(f"{'方案':<6} {'探测/轮':>8} {'命中率前20':>10} {'命中率后20':>10} {'人均场次':>8} "
          f"{'|Δ名次|':>8} {'准确度':>7}")
    base = None
    for mode in ("wrap", "wrap_plus", "wrap_plus2"):
        r = run(mode)
        if base is None:
            base = r
        print(f"{mode:<6} {r['probes']:8.0f} {r['hit_top']:9.1f}% {r['hit_bot']:9.1f}% "
              f"{r['matches']:8.1f} {r['vol']:8.3f} {r['acc']:7.3f}   "
              f"探测{(r['probes']/base['probes']-1)*100:+.0f}% 波动{(r['vol']/base['vol']-1)*100:+.0f}%")


if __name__ == "__main__":
    main()
