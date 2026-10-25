"""天梯算法离线模拟器 v2（纯内存，不接触数据库）。

忠实复刻 `crates/tswn_ladder` 现行实现（matching.rs / battle.rs / updator.rs / storage.rs），
只把「真实战斗」换成合成胜负模型（玩家有隐真实力 true_strength，胜负概率 =
logistic((S_i - S_j)/ELO_SCALE)），因此一分钟可以模拟几十万场对局。

度量口径与 `rank_volatility.py`（分析线上 rank_*.txt）完全一致，方便对照：
  * 每轮 |Δ名次|、名次变化率
  * lag=k 的 Spearman（排名自相关）
  * 前 10 名每轮换人数
  * 对真值 Spearman（排名准确度，仅模拟可得）
  * 数据库写入量：每轮 UPSERT 行数、存活行数（→ 折算字节/天）

标定：strength_sd=4000 时模拟出的 1a（106 人）lag1 |Δ名次|=1.33、Spearman=0.9978，
与线上 rank_1a.txt 实测 1.38 / 0.9978 吻合；3a（37 人）0.44 vs 实测 0.41。
"""
from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass, field

import numpy as np

# ── 现行常量（与 crates/tswn_ladder/src/main.rs 一致） ────────────────────────
ELO_SCALE_FACTOR = 50_600.0
HANDICAP_SCALE = 88.3
BASE_K_FACTOR = 80.0
K_HANDICAP = 6.0
HANDICAP_DECAY = 0.97
MATCHCOUNT_DECAY = 0.9999
PERTURBATION_RANGE = 40.0
DROPRATE = 0.9996
HANDICAP_PRECISION = 2
HANDICAP_ZERO_EPSILON = 1e-9

MAX_ACTIVE_MATCHES = 7
MAX_PASSIVE_MATCHES = 8
MAX_MATCHES = 11
SUPPLEMENT_RESERVED_SLOTS = 2
PRE_SA_MAX_MATCHES = MAX_MATCHES - SUPPLEMENT_RESERVED_SLOTS
SECOND_ACTIVE_MATCHES = 5
SECOND_PASSIVE_MATCHES = 9
GLOBAL_EXPLORATION_RATIO = 0.2
GLOBAL_EXPLORATION_MIN = 2

# 线上真实组别（人数取自 member 表：1a=106 1b=99 2a=93 2b=82 3a=37）
LIVE_GROUPS = [("1a", 106), ("1b", 99), ("2a", 93), ("2b", 82), ("3a", 37)]

# handicap 行在 PostgreSQL 里的估算宽度（见报告：tuple 头 24 + 两个 int 8 +
# varchar("1a") 4 + 两个 real 8 + item pointer 4 ≈ 48 → 按 56 估算含对齐/页开销）
HANDICAP_ROW_BYTES = 56


def logistic(x):
    return 1.0 / (1.0 + np.power(10.0, -np.asarray(x) / ELO_SCALE_FACTOR))


def round_handicap(rng, values):
    scale = 10.0**HANDICAP_PRECISION
    v = values * scale
    floor = np.floor(v)
    return np.where(rng.random(values.shape) < v - floor, floor + 1.0, floor) / scale


def prob_round(rng, values):
    floor = np.floor(values)
    return np.where(rng.random(values.shape) < values - floor, floor + 1.0, floor).astype(np.int64)


@dataclass
class Params:
    """可调参数。默认值 = 线上现状（baseline）。"""

    n_rounds: int = 600
    seed: int = 20261023
    strength_sd: float = 4000.0
    strength_df: float = 0.0
    # 真值漂移（每人每轮）：真实天梯里玩家会改阵容/换配置，评分必须持续追
    drift_sd: float = 0.0

    # 评分
    k_base: float = BASE_K_FACTOR
    k_handicap: float = K_HANDICAP
    droprate: float = DROPRATE
    # 衰减方式：proportional = 现有 `score * DROPRATE`（按分数比例，会压缩高分端、
    # 从而把「分数尺度」和「K」绑在一起）；uniform = 每人扣掉同一个量（组内均值的
    # 那一份），均值同样停在 11000 左右，但不压缩分差。
    decay_mode: str = "proportional"
    k_shrink_matches: float = 0.0  # >0：K_i = k_base / (1 + n_i / k_shrink_matches)
    k_min_ratio: float = 0.0  # K 下限（相对 k_base 的比例）

    # 匹配配额
    max_active: int = MAX_ACTIVE_MATCHES
    max_passive: int = MAX_PASSIVE_MATCHES
    pre_sa_max: int = PRE_SA_MAX_MATCHES
    second_active: int = SECOND_ACTIVE_MATCHES
    second_passive: int = SECOND_PASSIVE_MATCHES

    # handicap 持久化
    persist_mode: str = "every_round"  # every_round | changed_only | every_n
    persist_every_n: int = 1
    persist_min_delta: float = 0.05
    count_prune_below: float = 0.0  # >0：count 低于该值的行不再保留（旧逻辑永不淘汰）
    count_decay: float = MATCHCOUNT_DECAY
    handicap_decay: float = HANDICAP_DECAY

    # 竞赛评分（可选）：把 handicap 只当作匹配参考、不参与 ELO 期望
    update_k_scale: float = 1.0

    # 是否每轮都写 member 分数/名次（只有变化才写）
    member_write_changed_only: bool = True

    # ── 补位（旧实现的模拟退火入口）─────────────────────────────────────────
    # >0 时在 base+second 之后按「降低 handicap 诱导偏置」再补每人最多这么多场
    # （旧实现是 2 场：MAX_MATCHES=11 vs PRE_SA_MAX_MATCHES=9）。
    supplement_slots: int = 0
    supp_lo: float = 0.30
    supp_hi: float = 0.70
    # True = 修好的版本：只收 gain>0 的对，且整体 loss 必须真的下降才采纳
    # False = 复刻旧实现：贪心无条件补位（连让 loss 变大的也收）
    supp_gate: bool = True
    supp_only_positive: bool = True

    # ── 资格门里 handicap 的权重（1.0 = 现状，0 = 完全不用 handicap 配对）──────
    handicap_weight: float = 1.0
    # 资格门的胜率区间：第一轮 / 第二轮
    gate_base: tuple = (0.45, 0.55)
    gate_second: tuple = (0.40, 0.60)
    # 候选搜索半径（名次距离）：0 = 沿用旧规则（n<30 取 25，否则 40）
    search_radius: int = 0
    # 算分时是否也扣掉克制项：False = 现状（只用分数差算期望）
    update_with_handicap: bool = False


def lag_metrics(rank_hist: np.ndarray, lag: int) -> dict:
    """rank_hist: (T, n) 每个人的名次；返回与 rank_volatility.py 同口径的指标。"""
    T = rank_hist.shape[0]
    if T <= lag:
        return {}
    A, B = rank_hist[:-lag], rank_hist[lag:]
    D = np.abs(B - A)
    moved = float(np.mean(D != 0))
    per_round_mean = D.mean(axis=1)
    sps = []
    for t in range(0, T - lag, max(1, (T - lag) // 200)):
        ra, rb = A[t].astype(float), B[t].astype(float)
        ra -= ra.mean()
        rb -= rb.mean()
        den = math.sqrt(float(ra @ ra) * float(rb @ rb))
        sps.append(float(ra @ rb / den) if den > 0 else 0.0)
    return {
        "lag": lag,
        "moved": moved,
        "mean_abs": float(np.mean(per_round_mean)),
        "p95": float(np.percentile(D, 95)),
        "spearman": float(np.mean(sps)),
    }


@dataclass
class GroupSim:
    name: str
    n: int
    rng: np.random.Generator
    params: Params
    true_strength: np.ndarray
    score: np.ndarray
    h_basic: np.ndarray
    h_count: np.ndarray
    h_has_row: np.ndarray
    lifetime_matches: np.ndarray
    last_persist_basic: np.ndarray | None = None
    last_persist_count: np.ndarray | None = None
    rank_hist: list = field(default_factory=list)
    rows_written: list = field(default_factory=list)
    match_counts: list = field(default_factory=list)
    score_drift: list = field(default_factory=list)
    supp_loss_base: list = field(default_factory=list)
    supp_loss_after: list = field(default_factory=list)
    fairness: list = field(default_factory=list)
    # 每对玩家的固有对位优势（分数尺度，反对称）。None = 没有对位效应。
    pair_bias: np.ndarray | None = None
    _rows_this_round: int = 0

    @classmethod
    def build(cls, name, n, params: Params, strength_override=None) -> "GroupSim":
        rng = np.random.default_rng(params.seed)
        w = params.strength_sd
        if strength_override is not None:
            strength = np.array(strength_override, dtype=float)
            assert len(strength) == n
            strength = strength + rng.normal(0.0, 1e-3, n)
        elif params.strength_df > 0:
            raw = rng.standard_t(params.strength_df, size=n)
            strength = 11_000.0 + raw / raw.std() * w
        else:
            strength = rng.normal(11_000.0, w, size=n)
        return cls(
            name=name,
            n=n,
            rng=rng,
            params=params,
            true_strength=strength,
            score=np.full(n, 11_000, dtype=np.int64),
            h_basic=np.zeros((n, n)),
            h_count=np.zeros((n, n)),
            h_has_row=np.zeros((n, n), dtype=bool),
            lifetime_matches=np.zeros(n),
        )

    # ── handicap 视图 ────────────────────────────────────────────────────────
    def _neighbors(self):
        """按 Rust 的 i±1..radius 顺序预生成邻居位置表（缓存）。"""
        if getattr(self, "_neigh", None) is None:
            n = self.n
            radius = self.params.search_radius or (min(25, n - 1) if n < 30 else 40)
            radius = min(radius, n - 1)
            cols = []
            for offset in range(1, radius + 1):
                cols.append((np.arange(n) + offset) % n)
                cols.append((np.arange(n) + n - offset) % n)
            self._neigh = np.stack(cols, axis=1).astype(np.int32)
        return self._neigh

    def disturbed(self):
        noise = self.rng.random((self.n, self.n)) * 2.0 * PERTURBATION_RANGE - PERTURBATION_RANGE
        base = np.where(self.h_has_row, self.h_basic, 0.0)
        shrink = np.clip((100.0 - self.h_count) / 100.0, 0.0, 1.0)
        d = np.where(self.h_has_row, base + noise * shrink, noise)
        np.fill_diagonal(d, 0.0)
        return d

    def win_probs(self, order, disturbed):
        sc = self.score[order].astype(float)
        diff = sc[:, None] - sc[None, :]
        h = disturbed[np.ix_(order, order)] * self.params.handicap_weight
        sign = np.where(order[:, None] < order[None, :], 1.0, -1.0)
        return logistic(diff + h * HANDICAP_SCALE * sign)

    # ── 补位：旧实现被关掉的那一段 ──────────────────────────────────────────
    def _supplement(self, matched, probs, order, total_cnt, k_eff):
        """按「降低 Σ(handicap 诱导的期望改分)²」给每人补 1~2 场。

        返回 (新的 matched, 补位前 loss, 补位后 loss)。`probs` 是含 handicap 的预期胜率
        （按名次位置索引），`order[a]` 是位置 a 对应的玩家下标。
        """
        p = self.params
        n = self.n
        pairs = sorted((key // n, key % n) for key in matched)
        sc = self.score[order].astype(float)
        p_base = logistic(sc[:, None] - sc[None, :])
        k_pos = k_eff[order]  # 按名次位置取每人的 K

        delta = np.zeros(n)
        for a, b in pairs:
            dc = k_pos[a] * (probs[a, b] - p_base[a, b])
            delta[a] += dc
            delta[b] -= dc
        base_loss = float(np.sum(delta * delta))

        if p.supplement_slots <= 0:
            return matched, base_loss, base_loss

        limit = p.pre_sa_max + p.supplement_slots
        cnt = total_cnt.copy()
        used = set(pairs)
        new_pairs = []

        for a in np.argsort(-np.abs(delta)):
            a = int(a)
            if cnt[a] >= limit:
                continue
            best = None
            for b in range(n):
                if b == a or cnt[b] >= limit:
                    continue
                key = (a, b) if a < b else (b, a)
                if key in used:
                    continue
                w = probs[a, b]
                if not (p.supp_lo <= w <= p.supp_hi):
                    continue
                dc = k_pos[a] * (probs[a, b] - p_base[a, b])
                gain = (delta[a] ** 2 + delta[b] ** 2) - ((delta[a] + dc) ** 2 + (delta[b] - dc) ** 2)
                if best is None or gain > best[0]:
                    best = (gain, b, dc)
            if best is None:
                continue
            gain, b, dc = best
            if p.supp_only_positive and gain <= 0:
                continue
            used.add((a, b) if a < b else (b, a))
            new_pairs.append((a, b) if a < b else (b, a))
            delta[a] += dc
            delta[b] -= dc
            cnt[a] += 1
            cnt[b] += 1

        after_loss = float(np.sum(delta * delta))
        if p.supp_gate and after_loss >= base_loss:
            # 修好的版本：整体没变好就不采纳（旧实现缺这一层，可能越补越差）
            return matched, base_loss, base_loss
        if not new_pairs:
            return matched, base_loss, base_loss
        extra = {a * n + b for a, b in new_pairs}
        return matched | extra, base_loss, after_loss

    # ── 单轮 ────────────────────────────────────────────────────────────────
    def step(self):
        p = self.params
        rng = self.rng
        n = self.n

        if p.decay_mode == "uniform":
            # 每人扣掉「组内平均分按 DROPRATE 会掉的那一份」，均值不变、分差不被压缩
            step = (1.0 - p.droprate) * float(np.mean(self.score))
            score0 = prob_round(rng, self.score.astype(float) - step)
        else:
            score0 = prob_round(rng, self.score.astype(float) * p.droprate)
        order = np.lexsort((np.arange(n), -score0))
        prev_rank = np.empty(n, dtype=np.int64)
        prev_rank[order] = np.arange(n)

        disturbed = self.disturbed()
        probs = self.win_probs(order, disturbed)
        pl = probs  # (n, n) ndarray，按位置索引
        neigh = self._neighbors()

        def candidates(i, lo, hi, matched):
            js = neigh[i]
            row = pl[i]
            sel = js[(row[js] >= lo) & (row[js] <= hi)].tolist()
            out, seen = [], set()
            for j in sel:
                if j in seen:
                    continue
                key = (i * n + j) if i < j else (j * n + i)
                if key in matched:
                    continue
                out.append(j)
                seen.add(j)
            quota = max(GLOBAL_EXPLORATION_MIN, math.ceil(len(out) * GLOBAL_EXPLORATION_RATIO))
            quota = min(quota, max(0, n - 1 - len(seen)))
            rand_out = []
            if quota > 0:
                cand = rng.integers(0, n, size=max(20, quota * 8))
                rvals = row[cand]
                ok = (rvals >= lo) & (rvals <= hi)
                for j in cand[ok].tolist():
                    if len(rand_out) >= quota:
                        break
                    if j == i or j in seen:
                        continue
                    key = (i * n + j) if i < j else (j * n + i)
                    if key in matched:
                        continue
                    rand_out.append(j)
                    seen.add(j)
            merged, li, ri = [], 0, 0
            n_out, n_rand = len(out), len(rand_out)
            while li < n_out or ri < n_rand:
                use_rand = ri < n_rand and (li >= n_out or rng.random() < GLOBAL_EXPLORATION_RATIO)
                if use_rand:
                    merged.append(rand_out[ri])
                    ri += 1
                elif li < n_out:
                    merged.append(out[li])
                    li += 1
            if len(merged) > 1:
                arr = np.array(merged, dtype=np.int32)
                rng.shuffle(arr)
                return arr.tolist()
            return merged

        base_cnt = np.zeros(n, dtype=np.int64)
        second_cnt = np.zeros(n, dtype=np.int64)
        total_cnt = np.zeros(n, dtype=np.int64)
        matched: set[int] = set()
        shuffle_order = np.arange(n)
        rng.shuffle(shuffle_order)

        for i in shuffle_order:
            i = int(i)
            for j in candidates(i, p.gate_base[0], p.gate_base[1], matched):
                if base_cnt[i] >= p.max_active or total_cnt[i] >= p.pre_sa_max:
                    break
                if base_cnt[j] >= p.max_passive or total_cnt[j] >= p.pre_sa_max:
                    continue
                key = i * n + j if i < j else j * n + i
                if key in matched:
                    continue
                matched.add(key)
                base_cnt[i] += 1
                base_cnt[j] += 1
                total_cnt[i] += 1
                total_cnt[j] += 1

        for i in shuffle_order:
            i = int(i)
            for j in candidates(i, p.gate_second[0], p.gate_second[1], matched):
                if second_cnt[i] >= p.second_active or total_cnt[i] >= p.pre_sa_max:
                    break
                if second_cnt[j] >= p.second_passive or total_cnt[j] >= p.pre_sa_max:
                    continue
                key = i * n + j if i < j else j * n + i
                if key in matched:
                    continue
                matched.add(key)
                second_cnt[i] += 1
                second_cnt[j] += 1
                total_cnt[i] += 1
                total_cnt[j] += 1
                base_cnt[i] += 1
                base_cnt[j] += 1

        # 自适应 K
        if p.k_shrink_matches > 0:
            k_eff = p.update_k_scale * p.k_base / (1.0 + self.lifetime_matches / p.k_shrink_matches)
            k_eff = np.maximum(k_eff, p.k_base * p.k_min_ratio)
        else:
            k_eff = np.full(n, p.update_k_scale * p.k_base)

        # 补位（旧实现的模拟退火入口）：以「降低 handicap 诱导偏置」为目标给每人多配 1~2 场
        matched, loss_base, loss_after = self._supplement(matched, probs, order, total_cnt, k_eff)
        self.supp_loss_base.append(loss_base)
        self.supp_loss_after.append(loss_after)

        pairs = sorted((k // n, k % n) for k in matched)
        del matched

        # 真实公平度：本局实际对上的组合，真实胜率离 50/50 有多远
        if self.pair_bias is not None and pairs:
            ii = np.fromiter((order[a] for a, _ in pairs), dtype=np.int64, count=len(pairs))
            jj = np.fromiter((order[b] for _, b in pairs), dtype=np.int64, count=len(pairs))
            p_true = logistic(self.true_strength[ii] - self.true_strength[jj] + self.pair_bias[ii, jj])
            self.fairness.append(float(np.mean(np.abs(p_true - 0.5))))
        else:
            self.fairness.append(0.0)

        new_score = score0.tolist()
        strength = self.true_strength.tolist()
        bias = self.pair_bias.tolist() if self.pair_bias is not None else None
        dmat = disturbed.tolist()
        hb = self.h_basic
        hc = self.h_count
        hr = self.h_has_row
        rnd = rng.random
        k_eff_l = k_eff.tolist()
        scale = ELO_SCALE_FACTOR

        def prob_round_scalar(v: float) -> int:
            floor = math.floor(v)
            return int(floor + 1) if rnd() < v - floor else int(floor)

        for a, b in pairs:
            i = order[a]
            j = order[b]
            direction = 1.0 if i < j else -1.0
            hs = dmat[i][j] * HANDICAP_SCALE * direction
            diff = new_score[i] - new_score[j]
            exp_h = 1.0 / (1.0 + 10.0 ** (-(diff + hs) / scale))
            exp_b = 1.0 / (1.0 + 10.0 ** (-diff / scale))
            if self.params.update_with_handicap:
                exp_b = exp_h
            p_win = 1.0 / (1.0 + 10.0 ** (-(strength[i] - strength[j] + (bias[i][j] if bias else 0.0)) / scale))
            actual = 1.0 if rnd() < p_win else 0.0

            d = k_eff_l[i] * (actual - exp_b)
            di = k_eff_l[j] * (actual - exp_b)
            new_score[i] = prob_round_scalar(new_score[i] + d)
            new_score[j] = prob_round_scalar(new_score[j] - di)
            if actual > 0:
                new_score[i] += 1
            else:
                new_score[j] += 1

            lo, hi = (i, j) if i < j else (j, i)
            if hr[lo, hi]:
                hb[lo, hi] += p.k_handicap * (actual - exp_h) * direction
                hc[lo, hi] = 100.0 - (100.0 - hc[lo, hi]) * 0.1
            elif abs(actual - exp_h) > 0.2:
                hb[lo, hi] = p.k_handicap * (actual - exp_h) * direction
                hc[lo, hi] = 1.0
                hr[lo, hi] = True

        self.score = np.array(new_score, dtype=np.int64)
        if p.drift_sd > 0:
            self.true_strength += rng.normal(0.0, p.drift_sd, n)
        self.lifetime_matches += total_cnt
        self._persist([(int(order[a]), int(order[b])) for a, b in pairs])

        rank_new = np.empty(n, dtype=np.int64)
        rank_new[np.lexsort((np.arange(n), -self.score))] = np.arange(n)
        self.rank_hist.append(rank_new)
        self.rows_written.append(self._rows_this_round)
        self.match_counts.append(len(pairs))
        self.score_drift.append(float(np.mean(np.abs(self.score - score0))))

    # ── 持久化模型 ──────────────────────────────────────────────────────────
    def _persist(self, pairs):
        p = self.params
        rng = self.rng
        n = self.n

        changed = np.zeros((n, n), dtype=bool)
        for i, j in pairs:
            changed[min(i, j), max(i, j)] = True

        count = np.minimum(round_handicap(rng, self.h_count * p.count_decay), 100.0)
        raw = np.where(changed, self.h_basic * p.handicap_decay, self.h_basic)
        has_row = self.h_has_row.copy()

        if p.count_prune_below > 0:
            dead = has_row & (count < p.count_prune_below)
            if dead.any():
                has_row &= ~dead
                raw = np.where(dead, 0.0, raw)
                count = np.where(dead, 0.0, count)
                if self.last_persist_basic is not None:
                    self.last_persist_basic = np.where(dead, 0.0, self.last_persist_basic)
                    self.last_persist_count = np.where(dead, 0.0, self.last_persist_count)

        eligible = has_row & ((np.abs(raw) > HANDICAP_ZERO_EPSILON) | (count > 0.0))
        if p.persist_mode == "every_round":
            write = eligible
        elif p.persist_mode == "changed_only":
            if self.last_persist_basic is None:
                write = eligible
            else:
                q = max(p.persist_min_delta, 0.01)
                write = eligible & (
                    (np.abs(raw - self.last_persist_basic) >= q) | (np.abs(count - self.last_persist_count) >= q)
                )
        elif p.persist_mode == "every_n":
            write = eligible if (len(self.rows_written) + 1) % max(1, p.persist_every_n) == 0 else np.zeros_like(eligible)
        else:
            raise ValueError(p.persist_mode)

        self._rows_this_round = int(write.sum())
        persisted = round_handicap(rng, raw)
        self.h_basic = np.where(write, persisted, raw)
        self.h_count = count
        self.h_has_row = has_row
        if self.last_persist_basic is None:
            self.last_persist_basic = np.where(write, persisted, 0.0)
            self.last_persist_count = np.where(write, count, 0.0)
        else:
            self.last_persist_basic = np.where(write, persisted, self.last_persist_basic)
            self.last_persist_count = np.where(write, count, self.last_persist_count)

    # ── 汇总 ────────────────────────────────────────────────────────────────
    def report(self) -> dict:
        warm = max(1, self.params.n_rounds // 5)
        R_full = np.array(self.rank_hist)
        R = R_full[warm:]  # 丢掉冷启动（全员同分）阶段，否则会严重高估波动
        out = {
            "n": self.n,
            "matches_per_round": float(np.mean(self.match_counts[warm:])),
            "rows_per_round": float(np.mean(self.rows_written[warm:])),
            "live_rows": int(self.h_has_row.sum()),
            "score_sd": float(np.std(self.score)),
            "score_mean": float(np.mean(self.score)),
            "score_drift": float(np.mean(self.score_drift[warm:])),
            "fairness": float(np.mean(self.fairness[warm:])) if self.fairness else 0.0,
            "supp_loss_base": float(np.mean(self.supp_loss_base[warm:])) if self.supp_loss_base else 0.0,
            "supp_loss_after": float(np.mean(self.supp_loss_after[warm:])) if self.supp_loss_after else 0.0,
        }
        for lag in (1, 5, 15, 60):
            m = lag_metrics(R, lag)
            if not m:
                continue
            out[f"mean_abs_delta_{lag}"] = m["mean_abs"]
            out[f"moved_{lag}"] = m["moved"]
            out[f"spearman_{lag}"] = m["spearman"]
            out[f"p95_{lag}"] = m["p95"]
        # 前 10 换人率
        chg = [len(set(np.argsort(R[t - 1], kind="stable")[:10]) ^ set(np.argsort(R[t], kind="stable")[:10])) for t in range(1, len(R))]
        out["top10_change"] = float(np.mean(chg[warm:]))
        # 对真值准确度（末 20% 平均）
        last = max(1, len(R) // 5)
        accs = []
        for t in range(len(R) - last, len(R)):
            ra = np.argsort(np.argsort(self.true_strength)).astype(float)
            rb = np.argsort(np.argsort(self.score)).astype(float)
            ra -= ra.mean()
            rb -= rb.mean()
            accs.append(float(ra @ rb / math.sqrt((ra @ ra) * (rb @ rb))))
        out["true_spearman"] = float(np.mean(accs))
        return out


def load_real_scores(path="scores.csv") -> dict:
    """读取线上导出的分数分布（group -> score 列表），作为真值分布使用。"""
    import os

    here = os.path.dirname(os.path.abspath(__file__))
    path = path if os.path.isabs(path) else os.path.join(here, path)
    table: dict[str, list] = {}
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            g, v = line.split(",")
            table.setdefault(g, []).append(float(v))
    return table


def run(params: Params, groups=None, table: dict | None = None) -> dict:
    groups = groups or LIVE_GROUPS
    sims = [
        GroupSim.build(name, n, params, (table or {}).get(name))
        for name, n in groups
    ]
    for _ in range(params.n_rounds):
        for g in sims:
            g.step()
    per_group = {g.name: g.report() for g in sims}
    tot = {
        "rows_per_round": sum(v["rows_per_round"] for v in per_group.values()),
        "live_rows": sum(v["live_rows"] for v in per_group.values()),
        "matches_per_round": sum(v["matches_per_round"] for v in per_group.values()),
        "mean_abs_delta_1": float(np.mean([v.get("mean_abs_delta_1", 0.0) for v in per_group.values()])),
        "spearman_1": float(np.mean([v.get("spearman_1", 0.0) for v in per_group.values()])),
        "mean_abs_delta_60": float(np.mean([v.get("mean_abs_delta_60", 0.0) for v in per_group.values()])),
        "spearman_60": float(np.mean([v.get("spearman_60", 0.0) for v in per_group.values()])),
        "moved_1": float(np.mean([v.get("moved_1", 0.0) for v in per_group.values()])),
        "p95_1": float(np.mean([v["p95_1"] for v in per_group.values()])),
        "score_sd": float(np.mean([v["score_sd"] for v in per_group.values()])),        "top10_change": float(np.mean([v.get("top10_change", 0.0) for v in per_group.values()])),
        "true_spearman": float(np.mean([v.get("true_spearman", 0.0) for v in per_group.values()])),
        "score_drift": float(np.mean([v["score_drift"] for v in per_group.values()])),
    }
    return {"groups": per_group, "total": tot}


def fmt(res: dict, label: str) -> str:
    t = res["total"]
    rows_per_day = t["rows_per_round"] * 1440
    mb_per_day = rows_per_day * HANDICAP_ROW_BYTES / 1e6
    return (
        f"{label:<26} 场次/轮={t['matches_per_round']:7.0f} 写行/轮={t['rows_per_round']:8.0f} "
        f"存活行={t['live_rows']:6.0f} ({mb_per_day:8.1f} MB/天) |Δ名次|={t['mean_abs_delta_1']:.2f} "
        f"变化率={t['moved_1']*100:5.1f}% sp1={t['spearman_1']:.4f} sp60={t['spearman_60']:.3f} "
        f"前10换人={t['top10_change']:.2f} 准确度={t['true_spearman']:.3f} 分数抖动={t['score_drift']:.1f}"
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rounds", type=int, default=600)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    res = run(Params(n_rounds=args.rounds))
    if args.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        print(fmt(res, "baseline(现行)"))
        for name, v in res["groups"].items():
            print(f"  [{name}] n={v['n']:3d} 场次/轮={v['matches_per_round']:6.1f} |Δ|={v['mean_abs_delta_1']:.2f} "
                  f"sp1={v['spearman_1']:.4f} 准确度={v['true_spearman']:.3f} 分数sd={v['score_sd']:.0f}")


if __name__ == "__main__":
    main()
