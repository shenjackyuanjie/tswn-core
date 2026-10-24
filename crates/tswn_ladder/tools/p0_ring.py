"""环状拓扑优化：把榜顶/榜底绕到另一端的「无用探测」折半转到有用的一侧。

现状：位置 i 探 i±1..r，溢出部分用 % n 绕到另一端。
  * 榜顶（i 很小）的「上探」会绕到榜底 —— 分数差几千，几乎必然被 0.45~0.55 的门拒掉；
  * 榜底反过来一样。
方案 fold：上探 up = min(r, i)，把没用掉的预算一半折到下探：
  down = min(r + (r − up)//2, n−1−i)。例如 r=20、第 1 名：up=0，down=30。

指标：分带（前20%/中/后20%）的探测命中率、人均场次、波动、准确度。
"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params  # noqa: E402

GROUPS = [("1a", 106), ("3a", 37)]
ROUNDS = 700


def spearman(a, b):
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    ra -= ra.mean()
    rb -= rb.mean()
    return float(ra @ rb / np.sqrt((ra @ ra) * (rb @ rb)))


def run(mode: str) -> dict:
    out = []
    for name, n in GROUPS:
        p = Params(n_rounds=ROUNDS, seed=20261023, k_shrink_matches=200.0, k_min_ratio=0.15,
                   decay_mode="uniform", ring_mode=mode, probe_stats=True)
        g = GroupSim.build(name, n, p)
        for _ in range(ROUNDS):
            g.step()
        warm = ROUNDS // 5
        R = np.array(g.rank_hist)[warm:]
        vol = float(np.abs(R[1:] - R[:-1]).mean(axis=1).mean())
        probes = np.array(g.probe_probes, dtype=float)
        hits = np.array(g.probe_hits, dtype=float)
        band = np.array(g.band_matches, dtype=float) / max(1, ROUNDS)
        band_size = np.array([n // 5, n - 2 * (n // 5), n // 5], dtype=float)
        out.append({
            "name": name,
            "matches": float(np.mean(g.match_counts[warm:])),
            "vol": vol,
            "acc": spearman(g.true_strength, g.score.astype(float)),
            "hit_top": hits[0] / probes[0] * 100,
            "hit_mid": hits[1] / probes[1] * 100,
            "hit_bot": hits[2] / probes[2] * 100,
            "probes": probes.sum() / ROUNDS,
            "per_top": band[0] / band_size[0],
            "per_mid": band[1] / band_size[1],
            "per_bot": band[2] / band_size[2],
        })
    return {
        "groups": out,
        "tot": {
            "matches": sum(o["matches"] for o in out),
            "vol": float(np.mean([o["vol"] for o in out])),
            "acc": float(np.mean([o["acc"] for o in out])),
            "probes": sum(o["probes"] for o in out),
        },
    }


def main() -> None:
    print(f"组={[g[0] for g in GROUPS]} 轮数={ROUNDS}")
    results = {}
    for mode in ("wrap", "fold"):
        r = run(mode)
        results[mode] = r
        print(f"\n== {mode} ==")
        print(f"{'组':<4} {'场次/轮':>8} {'|Δ名次|':>7} {'准确度':>7} {'探测/轮':>8} "
              f"{'命中率前20':>10} {'中':>6} {'后20':>6} {'人均场次 前/中/后':>20}")
        for o in r["groups"]:
            print(f"{o['name']:<4} {o['matches']:8.0f} {o['vol']:7.3f} {o['acc']:7.3f} "
                  f"{o['probes']:8.0f} {o['hit_top']:9.1f}% {o['hit_mid']:5.1f}% {o['hit_bot']:5.1f}% "
                  f"{o['per_top']:7.1f}/{o['per_mid']:.1f}/{o['per_bot']:.1f}")
        t = r["tot"]
        print(f"合计 场次/轮={t['matches']:.0f} |Δ名次|={t['vol']:.3f} 准确度={t['acc']:.3f} "
              f"探测/轮={t['probes']:.0f}")

    a, b = results["wrap"]["tot"], results["fold"]["tot"]
    print("\n== fold 相对 wrap ==")
    print(f"场次 {a['matches']:.0f} -> {b['matches']:.0f} ({(b['matches']/a['matches']-1)*100:+.1f}%)  "
          f"波动 {a['vol']:.3f} -> {b['vol']:.3f} ({(b['vol']/a['vol']-1)*100:+.1f}%)  "
          f"准确度 {a['acc']:.3f} -> {b['acc']:.3f}  "
          f"探测 {a['probes']:.0f} -> {b['probes']:.0f} ({(b['probes']/a['probes']-1)*100:+.1f}%)")


if __name__ == "__main__":
    main()
