"""P1：handicap（克制补偿）强度对「天梯排名 vs 真实环境强弱」的影响。

用户给的量化口径：
  1. 把所有号两两测胜率 → 每人算一个「环境胜率」= 对全场所有人的平均胜率；
  2. 用它排序得到「真实环境强弱榜」；
  3. 与稳定下来的天梯分数排序比较，看两个榜的差距。

本模型：
  * 每个号有「自身强度」own（N(11000, 2900)）；
  * 每个号有一个「阵容类型」type，4 种、按 45%/25%/20%/10% 分布（模拟有人多的主流阵容）；
  * 类型之间循环克制（A 克 B 克 C 克 D 克 A），优势折算成 ADV 分；
  * 真实胜率 = logistic((own_i − own_j + 克制优势) / 50600)；
  * 天梯完全按线上算法跑（匹配门 + ELO + handicap 学习）。

关键抓手：线上 handicap 的稳态幅度是 h* = K_HANDICAP · bias / (1 − HANDICAP_DECAY)，
而「完全抵消一个 bias 需要的 h」是 993·bias。所以
        补偿比例 = K_HANDICAP / (1 − DECAY) / 993 = K_HANDICAP / 29.8
线上 K_HANDICAP=6 → 只抵消约 20%。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ladder_sim as S  # noqa: E402
from ladder_sim import ELO_SCALE_FACTOR, GroupSim, Params, logistic  # noqa: E402

N = 106
ADV = 2500.0          # 一次循环克制的固有优势（分数尺度）
OWN_SD = 2900.0
TYPE_P = np.array([0.45, 0.25, 0.20, 0.10])
ROUNDS = 700
SEED = 20261023

# 完全抵消所需 K_HANDICAP（因为 h* = K·bias/(1−DECAY)，完全抵消需要 993·bias）
K_FULL = 993.0 * (1.0 - S.HANDICAP_DECAY)


def build_population(seed: int = SEED):
    rng = np.random.default_rng(seed)
    types = rng.choice(len(TYPE_P), size=N, p=TYPE_P)
    own = rng.normal(11_000.0, OWN_SD, size=N)
    bias = np.zeros((N, N))
    for i in range(N):
        for j in range(N):
            if i == j:
                continue
            if (types[i] + 1) % 4 == types[j]:
                bias[i, j] = ADV
            elif (types[j] + 1) % 4 == types[i]:
                bias[i, j] = -ADV
    return types, own, bias


def env_win_rate(own, bias):
    """环境胜率：对全场（含自己之外的每个人）的平均真实胜率。"""
    diff = own[:, None] - own[None, :] + bias
    p = logistic(diff)
    np.fill_diagonal(p, np.nan)
    return np.nanmean(p, axis=1)


def ranks(x):
    return np.argsort(np.argsort(x)).astype(float)


def spearman(a, b):
    ra, rb = ranks(a), ranks(b)
    ra -= ra.mean()
    rb -= rb.mean()
    return float(ra @ rb / np.sqrt((ra @ ra) * (rb @ rb)))


def partial_corr(x, y, z):
    rxy, rxz, ryz = spearman(x, y), spearman(x, z), spearman(y, z)
    den = np.sqrt(max(1e-12, (1 - rxz**2) * (1 - ryz**2)))
    return float((rxy - rxz * ryz) / den)


def run_scheme(label: str, k_handicap: float, handicap_weight: float = 1.0,
               gate: tuple[float, float] = (0.45, 0.55), seed_off: int = 0) -> dict:
    types, own, bias = build_population(SEED + seed_off)
    env = env_win_rate(own, bias)
    p = Params(
        n_rounds=ROUNDS,
        seed=SEED + seed_off,
        k_shrink_matches=200.0,
        k_min_ratio=0.15,
        decay_mode="uniform",
        k_handicap=k_handicap,
        handicap_weight=handicap_weight,
    )
    g = GroupSim.build(label, N, p, own)
    g.pair_bias = bias
    # 资格门改成可配置（默认就是线上的 0.45/0.55）
    for _ in range(ROUNDS):
        g.step()

    score = g.score.astype(float)
    live = g.h_has_row & (np.abs(g.h_basic) > 1e-9)
    mean_abs_h = float(np.mean(np.abs(g.h_basic[live]))) if live.any() else 0.0

    R = np.array(g.rank_hist)
    warm = ROUNDS // 5
    R = R[warm:]
    delta = np.abs(R[1:] - R[:-1]).mean(axis=1)

    reg = np.polyfit(env, score, 1)
    return {
        "label": label,
        "k_handicap": k_handicap,
        "cancel": k_handicap / K_FULL,
        "mean_abs_h": mean_abs_h,
        "A_env": spearman(score, env),
        "r_own": spearman(score, own),
        "p_own": partial_corr(score, own, env),
        "p_env": partial_corr(score, env, own),
        "pts_per_5pp": float(reg[0] * 0.05),
        "vol": float(delta.mean()),
        "moved": float((np.abs(R[1:] - R[:-1]) != 0).mean()),
        "matches": float(np.mean(g.match_counts[warm:])),
        "sd": float(score.std()),
    }


SCHEMES = [
    ("S0 现状 补偿20%", 6.0, 1.0, (0.45, 0.55)),
    ("S1 补偿0%（不用克制配对）", 0.0, 0.0, (0.45, 0.55)),
    ("S2 补偿50%", K_FULL * 0.5, 1.0, (0.45, 0.55)),
    ("S3 补偿100%", K_FULL, 1.0, (0.45, 0.55)),
    ("S4 补偿150%", K_FULL * 1.5, 1.0, (0.45, 0.55)),
    ("S5 现状+放宽门(0.40~0.60)", 6.0, 1.0, (0.40, 0.60)),
    ("S6 补偿50%+放宽门", K_FULL * 0.5, 1.0, (0.40, 0.60)),
]


def main() -> None:
    print(f"N={N} ADV={ADV:.0f} 全抵消所需 K_HANDICAP={K_FULL:.1f} 轮数={ROUNDS}")
    print(f"{'方案':<26} {'K_h':>5} {'补偿':>5} {'|h|':>5} {'A(天梯vs环境)':>13} "
          f"{'偏自身':>7} {'偏环境':>7} {'换号收益/5pp':>12} {'|Δ名次|':>7} {'场次':>5}")
    for label, k, w, gate in SCHEMES:
        r = run_scheme(label, k, w, gate)
        print(
            f"{r['label']:<26} {r['k_handicap']:5.1f} {r['cancel']*100:4.0f}% {r['mean_abs_h']:5.1f} "
            f"{r['A_env']:+13.3f} {r['p_own']:+7.3f} {r['p_env']:+7.3f} "
            f"{r['pts_per_5pp']:+12.0f} {r['vol']:7.2f} {r['matches']:5.0f}"
        )


if __name__ == "__main__":
    main()
