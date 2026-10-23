"""P1 补充：组规模变大时，固定的「名次 ±40」半径会不会真的造成分层。

线上 1a 只有 106 人，半径 40 能覆盖 76% 的对手，所以配对图几乎是全连接、分层很难发生。
这里把 N 放大到 300 看同样的半径会怎样。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ladder_sim as S  # noqa: E402
from ladder_sim import GroupSim, Params, logistic  # noqa: E402
from p1_sim import ADV, OWN_SD, TYPE_P, env_win_rate, partial_corr, spearman  # noqa: E402

SEED = 20261023


def build(n: int, seed: int):
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


def run(n: int, radius: int, rounds: int = 260) -> dict:
    types, own, bias = build(n, SEED)
    env = env_win_rate(own, bias)
    p = Params(n_rounds=rounds, seed=SEED, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", search_radius=radius)
    g = GroupSim.build("scale", n, p, own)
    g.pair_bias = bias
    for _ in range(rounds):
        g.step()
    score = g.score.astype(float)
    R = np.array(g.rank_hist)[rounds // 5:]
    vol = float(np.abs(R[1:] - R[:-1]).mean(axis=1).mean())
    reg = np.polyfit(env, score, 1)
    # 分层探针：环境榜前 20% 与后 20% 之间，天梯分差 vs 他们的真实环境胜率差
    order_env = np.argsort(-env)
    top = order_env[: n // 5]
    bot = order_env[-n // 5:]
    return {
        "n": n,
        "radius": radius,
        "coverage": f"{min(2 * radius, n - 1) / (n - 1) * 100:.0f}%",
        "A": spearman(score, env),
        "own": partial_corr(score, own, env),
        "pts5": float(reg[0] * 0.05),
        "vol": vol,
        "matches": float(np.mean(g.match_counts[rounds // 5:])),
        "sd": float(score.std()),
        "env_gap_pp": float((env[top].mean() - env[bot].mean()) * 100),
        "score_gap": float(score[top].mean() - score[bot].mean()),
    }


def main() -> None:
    print(f"ADV={ADV:.0f}  每轮场次≈9/人")
    print(f"{'N':>4} {'半径':>4} {'覆盖':>6} {'A(天梯vs环境)':>13} {'偏自身':>7} {'换号收益/5pp':>12} "
          f"{'|Δ名次|':>7} {'场次':>5} {'分数sd':>6} {'头尾分差':>8}")
    for n in (106, 300):
        for radius in (40, 100):
            if radius >= n:
                continue
            r = run(n, radius)
            print(f"{r['n']:4d} {r['radius']:4d} {r['coverage']:>6} {r['A']:+13.3f} {r['own']:+7.3f} "
                  f"{r['pts5']:+12.0f} {r['vol']:7.2f} {r['matches']:5.0f} {r['sd']:6.0f} {r['score_gap']:8.0f}")


if __name__ == "__main__":
    main()
