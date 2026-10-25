"""同一环境胜率下，对位方差对天梯排名的影响 —— 多 seed + 配对差分（DiD）。

对每个 seed、每对双胞胎，算 delta(A) = score(spiky) − score(flat)，
再与 A=0 的对照组相减：DiD = delta(A) − delta(0)。
这样把「这对双胞胎本身的强弱」这个固定效应消掉，只剩下方差带来的效应。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params, logistic  # noqa: E402
from p1_variance import N, OWN_SD, PAIRS, build, env_win_rate, ranks  # noqa: E402

ROUNDS = 1200
SEEDS = (20261023, 20261024, 20261025)
A_VALUES = (0.0, 10_000.0, 20_000.0)


def run(a: float, seed: int) -> dict:
    own, bias, spiky = build(a, seed)
    env = env_win_rate(own, bias)
    p = Params(n_rounds=ROUNDS, seed=seed, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform")
    g = GroupSim.build("v", N, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()

    score = g.score.astype(float)
    rk = ranks(score)
    flat_i, spiky_i = np.where(~spiky)[0], np.where(spiky)[0]
    return {
        "delta": score[spiky_i] - score[flat_i],
        "d_env": env[spiky_i] - env[flat_i],
        "rank_gap": np.abs(rk[spiky_i] - rk[flat_i]),
        "spiky_higher": (rk[spiky_i] < rk[flat_i]).astype(float),
        "sd_flat": score[flat_i].std(ddof=1),
        "sd_spiky": score[spiky_i].std(ddof=1),
        "env_pp": np.abs(env[spiky_i] - env[flat_i]).mean() * 100,
    }


def main() -> None:
    print(f"N={N}（{PAIRS} 对双胞胎）每 seed {ROUNDS} 轮，seeds={len(SEEDS)}")
    ref: dict[int, np.ndarray] = {}
    rows = {}
    for a in A_VALUES:
        deltas, rgaps, higher, sd_f, sd_s, envs = [], [], [], [], [], []
        for s in SEEDS:
            r = run(a, s)
            deltas.append(r["delta"])
            rgaps.append(r["rank_gap"].mean())
            higher.append(r["spiky_higher"].mean())
            sd_f.append(r["sd_flat"])
            sd_s.append(r["sd_spiky"])
            envs.append(r["env_pp"])
            if a == 0.0:
                ref[s] = r["delta"]
        d = np.concatenate(deltas)
        diid = np.concatenate([deltas[k] - ref[s] for k, s in enumerate(SEEDS)])
        rows[a] = {
            "d": d.mean(),
            "t": d.mean() / (d.std(ddof=1) / np.sqrt(len(d))),
            "did": diid.mean(),
            "did_t": diid.mean() / (diid.std(ddof=1) / np.sqrt(len(diid))),
            "did_half": 1.96 * diid.std(ddof=1) / np.sqrt(len(diid)),
            "rank_gap": float(np.mean(rgaps)),
            "higher": float(np.mean(higher)),
            "sd_flat": float(np.mean(sd_f)),
            "sd_spiky": float(np.mean(sd_s)),
            "env_pp": float(np.mean(envs)),
        }

    print(f"{'A':>6} {'环境胜率差':>9} {'分差':>7} {'t':>5} {'DiD分差':>8} {'t':>5} "
          f"{'95%CI':>9} {'名次差':>7} {'spiky更高':>9} {'sd(flat)':>9} {'sd(spiky)':>10}")
    for a in A_VALUES:
        r = rows[a]
        print(
            f"{a:6.0f} {r['env_pp']:8.2f}% {r['d']:+7.0f} {r['t']:+5.1f} {r['did']:+8.0f} "
            f"{r['did_t']:+5.1f} {r['did_half']:9.0f} {r['rank_gap']:7.1f} "
            f"{r['higher']*100:8.1f}% {r['sd_flat']:9.0f} {r['sd_spiky']:10.0f}"
        )


if __name__ == "__main__":
    main()
