"""最终方案扫描：用线上真实分数分布当真值（模拟器已与线上实测对齐到 ~12% 以内）。

打印相对 baseline 的变化率，直接对应「波动下降多少」。
"""
from __future__ import annotations

import argparse
import os
import sys
from dataclasses import replace

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import Params, load_real_scores  # noqa: E402
from ladder_sim import run as sim_run  # noqa: E402

TABLE = load_real_scores()
GROUPS = [(g, len(v)) for g, v in sorted(TABLE.items())]

CONFIGS: dict[str, dict] = {
    "A baseline(现行)": {},
    "B persist 只写变化 Q=1": {"persist_mode": "changed_only", "persist_min_delta": 1.0},
    "C persist Q=2": {"persist_mode": "changed_only", "persist_min_delta": 2.0},
    "D K=40": {"k_base": 40.0},
    "E K=24": {"k_base": 24.0},
    "F K 自适应 n0=200 floor15%": {"k_shrink_matches": 200.0, "k_min_ratio": 0.15},
    "G F + persist Q=1": {"k_shrink_matches": 200.0, "k_min_ratio": 0.15, "persist_mode": "changed_only", "persist_min_delta": 1.0},
    "H 配额减半+persist": {"max_active": 4, "max_passive": 5, "pre_sa_max": 6, "second_active": 3, "second_passive": 6,
                       "persist_mode": "changed_only", "persist_min_delta": 1.0},
    "I G+配额减半": {"k_shrink_matches": 200.0, "k_min_ratio": 0.15, "persist_mode": "changed_only",
                  "persist_min_delta": 1.0, "max_active": 4, "max_passive": 5, "pre_sa_max": 6,
                  "second_active": 3, "second_passive": 6},
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rounds", type=int, default=900)
    args = ap.parse_args()

    base = None
    print(f"{'方案':<30} {'场次/轮':>7} {'写行/轮':>8} {'|Δ名次|':>7} {'变化率':>6} {'sp1':>7} "
          f"{'前10换人':>8} {'准确度':>7} | {'|Δ|相对':>7} {'写行相对':>8}")
    for label, kw in CONFIGS.items():
        p = replace(Params(n_rounds=args.rounds), **kw)
        t = sim_run(p, GROUPS, TABLE)["total"]
        if base is None:
            base = t
        print(
            f"{label:<30} {t['matches_per_round']:7.0f} {t['rows_per_round']:8.0f} {t['mean_abs_delta_1']:7.2f} "
            f"{t['moved_1']*100:5.1f}% {t['spearman_1']:7.4f} {t['top10_change']:8.2f} {t['true_spearman']:7.3f} | "
            f"{(t['mean_abs_delta_1']/base['mean_abs_delta_1']-1)*100:+6.1f}% "
            f"{(t['rows_per_round']/base['rows_per_round']-1)*100:+7.1f}%"
        )


if __name__ == "__main__":
    main()
