"""P1 定稿数据：跑到收敛（1500 轮 ≈ 25 小时线上时间）后的关键方案对比。

前面 600 轮的跑法还没收敛（自适应 K=12 的时间常数约 800 轮），只适合看「哪个旋钮有效」；
这里给绝对数值用。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ladder_sim as S  # noqa: E402
from ladder_sim import GroupSim, Params  # noqa: E402
from p1_sim import ADV, K_FULL, N, OWN_SD, TYPE_P, env_win_rate, partial_corr, spearman  # noqa: E402

ROUNDS = 1500
SEED = 20261023


def build(n, seed):
    rng = np.random.default_rng(seed)
    types = rng.choice(len(TYPE_P), size=n, p=TYPE_P)
    own = rng.normal(11_000.0, OWN_SD, size=n)
    bias = np.zeros((n, n))
    for i in range(n):
        for j in range(n):
            if i == j:
                continue
            if (types[i] + 1) % 4 == types[j]:
                bias[i, j] = ADV
            elif (types[j] + 1) % 4 == types[i]:
                bias[i, j] = -ADV
    return types, own, bias


def run(n, k_handicap=6.0, weight=1.0, radius=0, update_h=False):
    types, own, bias = build(n, SEED)
    env = env_win_rate(own, bias)
    p = Params(n_rounds=ROUNDS, seed=SEED, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", k_handicap=k_handicap, handicap_weight=weight,
               search_radius=radius, update_with_handicap=update_h)
    g = GroupSim.build("x", n, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()
    score = g.score.astype(float)
    R = np.array(g.rank_hist)[ROUNDS // 5:]
    vol = float(np.abs(R[1:] - R[:-1]).mean(axis=1).mean())
    reg = np.polyfit(env, score, 1)
    order_env = np.argsort(-env)
    top, bot = order_env[: n // 5], order_env[-n // 5:]
    live = g.h_has_row & (np.abs(g.h_basic) > 1e-9)
    return {
        "A": spearman(score, env),
        "own": partial_corr(score, own, env),
        "env": partial_corr(score, env, own),
        "pts5": float(reg[0] * 0.05),
        "vol": vol,
        "matches": float(np.mean(g.match_counts[ROUNDS // 5:])),
        "sd": float(score.std()),
        "gap": float(score[top].mean() - score[bot].mean()),
        "h": float(np.mean(np.abs(g.h_basic[live]))) if live.any() else 0.0,
    }


SCHEMES = [
    ("S0 现状（补偿20%/半径40）", dict()),
    ("A1 补偿 0%", dict(k_handicap=0.0, weight=0.0)),
    ("A3 补偿 100%", dict(k_handicap=K_FULL)),
    ("B1 半径 10", dict(radius=10)),
    ("C1 克制进算分", dict(update_h=True)),
]


def main() -> None:
    print(f"N={N} ADV={ADV:.0f} true own sd={OWN_SD:.0f} 轮数={ROUNDS}")
    print(f"{'方案':<26} {'|h|':>5} {'A(天梯vs环境)':>13} {'偏自身':>7} {'偏环境':>7} "
          f"{'换号收益/5pp':>12} {'|Δ名次|':>7} {'分数sd':>6} {'头尾分差':>8} {'场次':>5}")
    for label, kw in SCHEMES:
        r = run(N, **kw)
        print(f"{label:<26} {r['h']:5.1f} {r['A']:+13.3f} {r['own']:+7.3f} "
              f"{r['env']:+7.3f} {r['pts5']:+12.0f} {r['vol']:7.2f} {r['sd']:6.0f} {r['gap']:8.0f} {r['matches']:5.0f}")


if __name__ == "__main__":
    main()
