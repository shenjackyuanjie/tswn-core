"""P2 结论数据：补位/退火到底有没有用。

对比三档：
  A 现状（不补位）
  B 旧实现行为（贪心补位、无条件采纳，可能越补越差）
  C 修好的版本（只收 gain>0，且整体 loss 必须真的下降才采纳）
每档看两件事：① 它自己优化的 loss 降了多少 ② 天梯侧指标有没有变化。
"""
from __future__ import annotations

import os
import sys
from dataclasses import replace

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import LIVE_GROUPS, Params  # noqa: E402
from ladder_sim import run as sim_run  # noqa: E402

ROUNDS = 700
GROUPS = [g for g in LIVE_GROUPS if g[0] in ("1a", "3a")]

VARIANTS = {
    "A 不补位（现状）": dict(),
    "B 旧实现：无条件补位": dict(supplement_slots=2, supp_gate=False, supp_only_positive=False),
    "C 修好：仅正向+必须变好": dict(supplement_slots=2, supp_gate=True, supp_only_positive=True),
    "D 修好：只补 1 场": dict(supplement_slots=1, supp_gate=True, supp_only_positive=True),
}


def main() -> None:
    print(f"组={[g[0] for g in GROUPS]}  轮数={ROUNDS}")
    print(f"{'方案':<22} {'loss前':>8} {'loss后':>8} {'loss变化':>9} {'场次/轮':>8} "
          f"{'|Δ名次|':>7} {'变化率':>6} {'sp1':>7} {'准确度':>7}")
    base = None
    for label, kw in VARIANTS.items():
        p = replace(Params(n_rounds=ROUNDS), **kw)
        res = sim_run(p, GROUPS)
        g = res["groups"]
        loss_b = sum(v["supp_loss_base"] for v in g.values()) / len(g)
        loss_a = sum(v["supp_loss_after"] for v in g.values()) / len(g)
        t = res["total"]
        print(
            f"{label:<22} {loss_b:8.1f} {loss_a:8.1f} {(loss_a/loss_b-1)*100:+8.1f}% "
            f"{t['matches_per_round']:8.0f} {t['mean_abs_delta_1']:7.2f} {t['moved_1']*100:5.1f}% "
            f"{t['spearman_1']:7.4f} {t['true_spearman']:7.3f}"
        )


if __name__ == "__main__":
    main()
