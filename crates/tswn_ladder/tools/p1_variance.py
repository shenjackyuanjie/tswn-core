"""同一环境胜率下，「对位方差（克制的尖锐程度）」对天梯排名的影响。

实验设计：把人做成 53 对「双胞胎」——
  * 每对两人自身强度完全相同；
  * 每对里的 A（flat）对所有对手都没有对位偏差（p 均匀）；
  * 每对里的 B（spiky）对**一半**对手 +A、对另一半 −A（+A 的集合每人随机，避免对整个环境
    产生系统性的强弱格局），于是 A、B 的对全场平均胜率（环境胜率）几乎一样，
    但 B 的对位方差大得多。

于是「同环境胜率、不同方差」这一对就是干净的对照：如果天梯公正，两人分数应当持平；
任何系统性分差都只能来自方差本身。

扫 A（对位偏差幅度）看趋势，并同时报告：
  * 环境胜率差（验证操作性：两人是否真的同环境胜率）
  * 分数差（spiky − flat）与 t 值
  * spiky 排名更高的比例（应当 ≈50%）
  * 两人名次差 |Δrank|（同环境胜率下"本该"接近 0）
  * 两组各自的分数标准差与每轮 |Δ名次|（方差是否只是让两人都更抖）
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import ELO_SCALE_FACTOR, GroupSim, Params, logistic  # noqa: E402

N = 106
PAIRS = N // 2
ROUNDS = 1200
SEED = 20261023
OWN_SD = 2900.0


def build(a: float, seed: int = SEED):
    rng = np.random.default_rng(seed)
    # 53 个强度，每个强度两个人（双胞胎）
    base = rng.normal(11_000.0, OWN_SD, size=PAIRS)
    own = np.repeat(base, 2)
    is_spiky = np.zeros(N, dtype=bool)
    is_spiky[1::2] = True  # 每对的第二个人是 spiky

    bias = np.zeros((N, N))
    if a > 0:
        for i in range(N):
            if not is_spiky[i]:
                continue
            # 随机挑一半对手给 +A，其余 −A；对整个场无系统性偏向
            order = rng.permutation(N)
            beat = order[: N // 2]
            bias[i, beat] = +a
            mask = np.ones(N, dtype=bool)
            mask[beat] = False
            mask[i] = False
            bias[i, mask] = -a
        # 反对称化（只看 i 对 j 的优势，j 对 i 自动取反）
        bias = (bias - bias.T) / 2.0
    return own, bias, is_spiky


def env_win_rate(own, bias):
    p = logistic(own[:, None] - own[None, :] + bias)
    np.fill_diagonal(p, np.nan)
    return np.nanmean(p, axis=1)


def ranks(x):
    return np.argsort(np.argsort(x)).astype(float)


def run(a: float) -> dict:
    own, bias, spiky = build(a)
    env = env_win_rate(own, bias)
    p = Params(n_rounds=ROUNDS, seed=SEED, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform")
    g = GroupSim.build("v", N, p, own)
    g.pair_bias = bias
    for _ in range(ROUNDS):
        g.step()

    score = g.score.astype(float)
    rk = ranks(score)
    warm = ROUNDS // 5
    R = np.array(g.rank_hist)[warm:]
    vol = np.abs(R[1:] - R[:-1])

    flat_i = np.where(~spiky)[0]
    spiky_i = np.where(spiky)[0]

    d_score = score[spiky_i] - score[flat_i]
    d_env = env[spiky_i] - env[flat_i]
    # 扣掉环境胜率差之后的分差（把方差效应单独摘出来）
    if np.std(d_env) > 1e-12:
        beta = np.polyfit(d_env, d_score, 1)[0]
    else:
        beta = 0.0
    resid = d_score - beta * d_env

    # 对位方差（每人 p 在对手上的标准差）
    pmat = logistic(own[:, None] - own[None, :] + bias)
    np.fill_diagonal(pmat, np.nan)
    p_sd = np.nanstd(pmat, axis=1)

    return {
        "a": a,
        "p_sd_flat": float(p_sd[flat_i].mean()),
        "p_sd_spiky": float(p_sd[spiky_i].mean()),
        "env_diff_pp": float(np.abs(d_env).mean() * 100),
        "d_score": float(d_score.mean()),
        "t": float(d_score.mean() / (d_score.std(ddof=1) / np.sqrt(len(d_score)))),
        "resid": float(resid.mean()),
        "spiky_higher": float((rk[spiky_i] < rk[flat_i]).mean()),
        "rank_gap": float(np.abs(rk[spiky_i] - rk[flat_i]).mean()),
        "sd_flat": float(score[flat_i].std()),
        "sd_spiky": float(score[spiky_i].std()),
        "vol_flat": float(vol[:, flat_i].mean()),
        "vol_spiky": float(vol[:, spiky_i].mean()),
    }


def main() -> None:
    print(f"N={N}（{PAIRS} 对双胞胎）轮数={ROUNDS} own_sd={OWN_SD:.0f}")
    print(f"{'A':>6} {'p_sd(flat)':>10} {'p_sd(spiky)':>11} {'环境胜率差':>9} "
          f"{'分差(spiky-flat)':>16} {'t':>6} {'扣环境后':>9} {'spiky更高占比':>12} {'名次差':>7} "
          f"{'波动flat':>9} {'波动spiky':>9}")
    for a in (0.0, 5_000.0, 10_000.0, 15_000.0, 20_000.0):
        r = run(a)
        print(
            f"{r['a']:6.0f} {r['p_sd_flat']:10.3f} {r['p_sd_spiky']:11.3f} "
            f"{r['env_diff_pp']:8.2f}% {r['d_score']:+16.0f} {r['t']:+6.1f} "
            f"{r['resid']:+9.0f} {r['spiky_higher']*100:11.1f}% {r['rank_gap']:7.1f} "
            f"{r['vol_flat']:9.3f} {r['vol_spiky']:9.3f}"
        )


if __name__ == "__main__":
    main()
