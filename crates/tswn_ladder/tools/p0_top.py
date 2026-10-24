"""顶部会不会「拉得过大 + 固化」：测顶部选手能遇到谁、分差是否随时间发散。

假设：榜顶的候选池只有「名次往下的 40 个」，绕到榜底的探测基本被资格门拒掉，
所以第 1 名一辈子只能和这 40 个人打 —— 顶部成了封闭小圈子，分数不受全榜约束。
"""
from __future__ import annotations

import os
import sys
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from ladder_sim import GroupSim, Params, load_real_scores  # noqa: E402

ROUNDS = 2500
WINDOW = 200  # 统计最近 200 轮遇到过的对手
TABLE = load_real_scores()["1a"]
N = len(TABLE)


def run(mode: str) -> dict:
    p = Params(n_rounds=ROUNDS, seed=20261023, k_shrink_matches=200.0, k_min_ratio=0.15,
               decay_mode="uniform", ring_mode=mode, track_pairs=True)
    g = GroupSim.build("1a", N, p, TABLE)
    for _ in range(ROUNDS):
        g.step()

    score = g.score.astype(float)
    order = np.argsort(-score)  # 玩家下标，按分数降序
    last_round = ROUNDS

    def window_opponents(pidx: int) -> tuple[int, float]:
        """最近 WINDOW 轮里这个玩家遇到的不同对手数、以及最常遇到的对手占比。"""
        cnt = Counter()
        for (a, b), n in g.pair_counter.items():
            if last_round - g.pair_last[(a, b)] > WINDOW:
                continue
            if a == pidx:
                cnt[b] += n
            elif b == pidx:
                cnt[a] += n
        if not cnt:
            return 0, 0.0
        top = cnt.most_common(1)[0][1]
        return len(cnt), top / sum(cnt.values())

    bands = {
        "第1名": order[:1],
        "前10": order[:10],
        "中位10": order[N // 2 - 5: N // 2 + 5],
        "后10": order[-10:],
    }
    out = {}
    for name, idxs in bands.items():
        ds, shares = zip(*(window_opponents(int(i)) for i in idxs))
        out[name] = (float(np.mean(ds)), float(np.mean(shares)))

    snaps = np.array([[s[0], s[1], s[2], s[3], s[5]] for s in g.score_snapshots], dtype=float)
    gaps = snaps[:, 1] - snaps[:, 2]  # max − median
    top_gap_early = float(gaps[len(gaps) // 4])
    top_gap_late = float(gaps[-1])
    return {
        "bands": out,
        "gap_early": top_gap_early,
        "gap_late": top_gap_late,
        "gap_trend": top_gap_late - top_gap_early,
        "sd": float(score.std()),
        "top1": int(score.max()),
        "median": int(np.median(score)),
        "top10_change": float(np.mean([
            len(set(np.argsort(-np.array(R))[:10]) ^ set(np.argsort(-np.array(Rprev))[:10]))
            for Rprev, R in zip(g.rank_hist[ROUNDS - 400:-1], g.rank_hist[ROUNDS - 399:])
        ])),
    }


def main() -> None:
    print(f"N={N}（线上 1a 的分数分布当真值）轮数={ROUNDS} 统计最近 {WINDOW} 轮的对手")
    for mode in ("wrap", "wrap_plus"):
        r = run(mode)
        print(f"\n== {mode} ==")
        print(f"  {'人群':<6} {'不同对手数':>10} {'最常对手占比':>12}")
        for name, (cnt, share) in r["bands"].items():
            print(f"  {name:<6} {cnt:10.1f} {share*100:11.1f}%")
        print(f"  max−median 分差: 早期 {r['gap_early']:.0f} → 末尾 {r['gap_late']:.0f} "
              f"（{r['gap_trend']:+.0f}）  榜首={r['top1']} 中位={r['median']} 榜sd={r['sd']:.0f}")
        print(f"  前10 每轮换人={r['top10_change']:.2f}")


if __name__ == "__main__":
    main()
