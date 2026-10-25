"""P1 补充：真实对局公平度（实际打的对局里真实胜率离 50/50 有多远）随补偿强度的变化。

这一项才是 handicap 真正买到的东西：它不进算分，所以不影响排名，但影响「每场是不是势均力敌」。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params  # noqa: E402
from p1_sim import K_FULL, N, build_population  # noqa: E402

ROUNDS = 500


def run(k_handicap: float, weight: float = 1.0, radius: int = 0):
    types, own, bias = build_population()
    p = Params(n_rounds=ROUNDS, seed=20261023, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", k_handicap=k_handicap, handicap_weight=weight,
               search_radius=radius)
    g = GroupSim.build("f", N, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()
    warm = ROUNDS // 2
    live = g.h_has_row & (np.abs(g.h_basic) > 1e-9)
    return {
        "fair": float(np.mean(g.fairness[warm:])) * 100,
        "h": float(np.mean(np.abs(g.h_basic[live]))) if live.any() else 0.0,
        "matches": float(np.mean(g.match_counts[warm:])),
    }


def main() -> None:
    print("真实对局公平度 = 实际对上的组合里 |真实胜率 − 50%| 的平均值（百分点，越小越公平）")
    print(f"{'补偿':>6} {'K_h':>5} {'|h|':>5} {'真实偏差':>8} {'场次':>5}")
    for label, k, w in [
        ("0%", 0.0, 0.0),
        ("20%（现状）", 6.0, 1.0),
        ("50%", K_FULL * 0.5, 1.0),
        ("100%", K_FULL, 1.0),
        ("150%", K_FULL * 1.5, 1.0),
    ]:
        r = run(k, w)
        print(f"{label:>6} {k:5.1f} {r['h']:5.1f} {r['fair']:7.2f}% {r['matches']:5.0f}")


if __name__ == "__main__":
    main()
