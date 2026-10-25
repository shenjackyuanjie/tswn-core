"""P1 扩展：找出真正能控制「天梯 vs 真实环境榜」差距的旋钮。

三条轴：
  A 克制补偿强度（= handicap 学习增益 K_HANDICAP，决定抵消掉多少真实对位优势）
  B 配对半径（名次距离，决定「跨层能不能碰上」）
  C 算分时是否扣掉克制（决定天梯到底在测「自身强度」还是「环境胜率」）
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from p1_sim import K_FULL, N, build_population, env_win_rate, partial_corr, spearman  # noqa: E402
import ladder_sim as S  # noqa: E402
from ladder_sim import GroupSim, Params  # noqa: E402

ROUNDS = 600
SEED = 20261023


def run(label: str, k_handicap: float = 6.0, weight: float = 1.0, radius: int = 0,
        update_h: bool = False, seed_off: int = 0) -> dict:
    types, own, bias = build_population(SEED + seed_off)
    env = env_win_rate(own, bias)
    p = Params(
        n_rounds=ROUNDS,
        seed=SEED + seed_off,
        k_shrink_matches=200.0,
        k_min_ratio=0.15,
        decay_mode="uniform",
        k_handicap=k_handicap,
        handicap_weight=weight,
        search_radius=radius,
        update_with_handicap=update_h,
    )
    g = GroupSim.build(label, N, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()

    score = g.score.astype(float)
    live = g.h_has_row & (np.abs(g.h_basic) > 1e-9)
    mean_abs_h = float(np.mean(np.abs(g.h_basic[live]))) if live.any() else 0.0
    R = np.array(g.rank_hist)[ROUNDS // 5:]
    delta = np.abs(R[1:] - R[:-1]).mean(axis=1)
    reg = np.polyfit(env, score, 1)
    return {
        "label": label,
        "cancel": k_handicap / K_FULL,
        "h": mean_abs_h,
        "A": spearman(score, env),
        "own": partial_corr(score, own, env),
        "env": partial_corr(score, env, own),
        "pts5": float(reg[0] * 0.05),
        "vol": float(delta.mean()),
        "matches": float(np.mean(g.match_counts[ROUNDS // 5:])),
        "sd": float(score.std()),
    }


SCHEMES = [
    ("S0 现状", dict()),
    ("A1 补偿 0%", dict(k_handicap=0.0, weight=0.0)),
    ("A2 补偿 50%", dict(k_handicap=K_FULL * 0.5)),
    ("A3 补偿 100%", dict(k_handicap=K_FULL)),
    ("A4 补偿 150%", dict(k_handicap=K_FULL * 1.5)),
    ("B1 半径 5", dict(radius=5)),
    ("B2 半径 10", dict(radius=10)),
    ("B3 半径 20", dict(radius=20)),
    ("C1 克制进算分", dict(update_h=True)),
    ("C2 克制进算分+补偿100%", dict(k_handicap=K_FULL, update_h=True)),
]

HEADER = (
    f"{'方案':<22} {'补偿':>5} {'|h|':>5} {'A(天梯vs环境)':>13} {'偏自身':>7} {'偏环境':>7} "
    f"{'换号收益/5pp':>12} {'|Δ名次|':>7} {'场次':>5} {'分数sd':>6}"
)


def main() -> None:
    print(f"N={N} 全抵消 K_HANDICAP={K_FULL:.1f} 轮数={ROUNDS}（单种子，相对比较用）")
    print(HEADER)
    for label, kw in SCHEMES:
        r = run(label, **kw)
        print(
            f"{r['label']:<22} {r['cancel']*100:4.0f}% {r['h']:5.1f} {r['A']:+13.3f} "
            f"{r['own']:+7.3f} {r['env']:+7.3f} {r['pts5']:+12.0f} {r['vol']:7.2f} "
            f"{r['matches']:5.0f} {r['sd']:6.0f}"
        )


if __name__ == "__main__":
    main()
