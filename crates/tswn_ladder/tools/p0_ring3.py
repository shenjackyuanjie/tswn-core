"""确认 wrap_plus 的效果是否稳（只看 1a，4 个 seed，配对同种子比较）。"""
from __future__ import annotations

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params  # noqa: E402

N = 106
ROUNDS = 700
SEEDS = (20261023, 20261024, 20261025, 20261026)


def vol(mode: str, seed: int) -> float:
    p = Params(n_rounds=ROUNDS, seed=seed, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", ring_mode=mode)
    g = GroupSim.build("1a", N, p)
    for _ in range(ROUNDS):
        g.step()
    R = np.array(g.rank_hist)[ROUNDS // 5:]
    return float(np.abs(R[1:] - R[:-1]).mean(axis=1).mean())


def main() -> None:
    a = [vol("wrap", s) for s in SEEDS]
    b = [vol("wrap_plus", s) for s in SEEDS]
    d = np.array(b) - np.array(a)
    print(f"N={N} 轮数={ROUNDS} seeds={len(SEEDS)}")
    print("wrap      :", " ".join(f"{x:.4f}" for x in a), f" 均值={np.mean(a):.4f}")
    print("wrap_plus :", " ".join(f"{x:.4f}" for x in b), f" 均值={np.mean(b):.4f}")
    print(f"逐 seed 差: {d.mean():+.4f} ± {d.std(ddof=1)/np.sqrt(len(d)):.4f} (SE)  "
          f"相对 {d.mean()/np.mean(a)*100:+.1f}%  正差次数={int((d>0).sum())}/{len(d)}")


if __name__ == "__main__":
    main()
