"""补充：验证「名次差变大」不是因为整个榜单被压扁，而是因为 spiky 玩家自身的分数分布更宽。

同时给出 A=0 与 A=20000 的：榜单整体 sd、双胞胎之间 |Δscore|、各自 sd。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params  # noqa: E402
from p1_variance import N, ROUNDS, SEED, build, ranks  # noqa: E402


def run(a: float) -> None:
    own, bias, spiky = build(a, SEED)
    p = Params(n_rounds=ROUNDS, seed=SEED, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform")
    g = GroupSim.build("v", N, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()
    score = g.score.astype(float)
    rk = ranks(score)
    flat_i, spiky_i = np.where(~spiky)[0], np.where(spiky)[0]
    d = score[spiky_i] - score[flat_i]
    # 时间上的波动：每 50 轮取一次分数的标准差（围绕自身均值）
    R = np.array(g.rank_hist)
    print(
        f"A={a:6.0f}  榜单sd={score.std():5.0f}  |Δscore|双胞胎={np.abs(d).mean():5.0f}  "
        f"sd(flat)={score[flat_i].std(ddof=1):5.0f}  sd(spiky)={score[spiky_i].std(ddof=1):5.0f}  "
        f"名次差={np.abs(rk[spiky_i] - rk[flat_i]).mean():4.1f}  "
        f"离榜心|s-均值|: flat={np.abs(score[flat_i]-score.mean()).mean():5.0f} "
        f"spiky={np.abs(score[spiky_i]-score.mean()).mean():5.0f}"
    )


def main() -> None:
    for a in (0.0, 10_000.0, 20_000.0):
        run(a)


if __name__ == "__main__":
    main()
