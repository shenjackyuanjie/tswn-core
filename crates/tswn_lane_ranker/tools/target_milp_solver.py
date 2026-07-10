#!/usr/bin/env python3
"""Weighted MILP target-set solver for TSwn target generation.

Input JSON fields:
- target_total: int
- player_cap: int; for target generation this is passed as 1 to forbid repeated player/member IDs
- weight_min / weight_max: selected target weight bounds
- player_weight_cap: maximum total selected target weight per player
- pool_indices: original Rust candidate indices, len n
- locked_indices: original Rust candidate indices forced selected
- ref_scores: target C-Score for each reference, len m
- rate_matrix: m x n winrate(ref, pool_candidate)
- player_keys: list[list[str]], len n

Output JSON:
- selected_indices: original Rust candidate indices
- selected_weights: weights aligned with selected_indices; sum equals target_total
- status, slope, intercept, max_abs_diff, p95_abs_diff, mean_abs_diff, rmse
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np

try:
    from scipy.optimize import Bounds, LinearConstraint, milp
    from scipy.sparse import lil_matrix
except Exception as exc:
    print(f"scipy.optimize.milp unavailable: {exc}", file=sys.stderr)
    sys.exit(2)


def mean_std(values: np.ndarray) -> tuple[float, float]:
    if values.size == 0:
        return float("nan"), float("nan")
    mean = float(np.mean(values))
    return mean, float(np.sqrt(max(0.0, np.mean((values - mean) ** 2))))


def fixed_slope_chebyshev_fit(avg_values: np.ndarray, score_values: np.ndarray, slope: float) -> tuple[float, float]:
    resid = score_values - slope * avg_values
    mn = float(np.min(resid))
    mx = float(np.max(resid))
    return (mn + mx) / 2.0, abs(mx - mn) / 2.0


def corrcoef(a: np.ndarray, b: np.ndarray) -> float | None:
    if a.size != b.size or a.size < 2:
        return None
    am, asd = mean_std(a)
    bm, bsd = mean_std(b)
    if not math.isfinite(asd) or not math.isfinite(bsd) or asd <= 1e-12 or bsd <= 1e-12:
        return None
    cov = float(np.mean((a - am) * (b - bm)))
    return cov / (asd * bsd)


def affine_chebyshev_fit(avg_values: np.ndarray, score_values: np.ndarray) -> tuple[float, float, np.ndarray]:
    if avg_values.size != score_values.size or avg_values.size < 2:
        return 1.0, 0.0, np.zeros_like(score_values)

    avg_mean, avg_std = mean_std(avg_values)
    score_mean, score_std = mean_std(score_values)
    avg_min, avg_max = float(np.min(avg_values)), float(np.max(avg_values))
    score_min, score_max = float(np.min(score_values)), float(np.max(score_values))

    if (
        not math.isfinite(avg_std)
        or not math.isfinite(score_std)
        or avg_std <= 1e-12
        or score_std <= 1e-12
        or abs(avg_max - avg_min) <= 1e-12
    ):
        intercept = score_mean - avg_mean
        diffs = avg_values + intercept - score_values
        return 1.0, intercept, diffs

    c = corrcoef(avg_values, score_values)
    range_slope = max(abs(score_max - score_min) / abs(avg_max - avg_min), 1e-6)
    std_slope = max(score_std / avg_std, 1e-6)
    corr_slope = max(abs((c if c is not None else 1.0) * score_std / avg_std), 1e-6)
    lo = 0.0
    hi = max(range_slope, std_slope, corr_slope, 1.0) * 4.0
    last_hi = fixed_slope_chebyshev_fit(avg_values, score_values, hi)[1]
    for _ in range(8):
        mid = hi / 2.0
        mid_score = fixed_slope_chebyshev_fit(avg_values, score_values, mid)[1]
        if last_hi + 1e-12 >= mid_score:
            break
        hi *= 2.0
        last_hi = fixed_slope_chebyshev_fit(avg_values, score_values, hi)[1]
    for _ in range(80):
        m1 = lo + (hi - lo) / 3.0
        m2 = hi - (hi - lo) / 3.0
        z1 = fixed_slope_chebyshev_fit(avg_values, score_values, m1)[1]
        z2 = fixed_slope_chebyshev_fit(avg_values, score_values, m2)[1]
        if z1 <= z2:
            hi = m2
        else:
            lo = m1
    slope = max((lo + hi) / 2.0, 0.0)
    intercept, _ = fixed_slope_chebyshev_fit(avg_values, score_values, slope)
    return slope, intercept, slope * avg_values + intercept - score_values


def objective_metrics(
    selected_mask: np.ndarray,
    rate_matrix: np.ndarray,
    scores: np.ndarray,
    weights: np.ndarray | None = None,
) -> dict[str, Any]:
    if weights is None:
        avg = rate_matrix[:, selected_mask].mean(axis=1)
    else:
        w = np.asarray(weights, dtype=float)
        weight_sum = float(np.sum(w[selected_mask]))
        if weight_sum <= 0:
            avg = rate_matrix[:, selected_mask].mean(axis=1)
        else:
            avg = rate_matrix[:, selected_mask] @ w[selected_mask] / weight_sum
    slope, intercept, diffs = affine_chebyshev_fit(avg, scores)
    abs_diff = np.abs(diffs)
    c = corrcoef(avg, scores)
    return {
        "slope": float(slope),
        "intercept": float(intercept),
        "max_abs_diff": float(np.max(abs_diff)),
        "p95_abs_diff": float(np.quantile(abs_diff, 0.95)),
        "mean_abs_diff": float(np.mean(abs_diff)),
        "rmse": float(np.sqrt(np.mean(diffs * diffs))),
        "corr": None if c is None else float(c),
    }


def greedy_feasible_seed(
    pool_indices: list[int],
    locked_set: set[int],
    player_keys: list[list[str]],
    target_total: int,
    player_cap: int,
) -> list[int]:
    selected: list[int] = []
    counts: dict[str, int] = {}
    pool_pos = {idx: pos for pos, idx in enumerate(pool_indices)}

    def can_add(original_idx: int) -> bool:
        pos = pool_pos[original_idx]
        return all(counts.get(k, 0) < player_cap for k in player_keys[pos])

    def add(original_idx: int) -> bool:
        if original_idx in selected or not can_add(original_idx):
            return False
        selected.append(original_idx)
        pos = pool_pos[original_idx]
        for k in player_keys[pos]:
            counts[k] = counts.get(k, 0) + 1
        return True

    for idx in pool_indices:
        if idx in locked_set:
            if not add(idx):
                return []
    for idx in pool_indices:
        if len(selected) >= target_total:
            break
        add(idx)
    return selected if len(selected) == target_total else []


def solve_fixed_slope(
    slope: float,
    rate_matrix: np.ndarray,
    scores: np.ndarray,
    pool_indices: list[int],
    locked_set: set[int],
    player_keys: list[list[str]],
    target_total: int,
    player_cap: int,
    weight_min: float,
    weight_max: float,
    player_weight_cap: float,
    time_limit: float,
) -> tuple[list[int], list[float], dict[str, Any]] | None:
    m, n = rate_matrix.shape
    # variables: x_0..x_{n-1}, w_0..w_{n-1}, b, t
    x0 = 0
    w0 = n
    b_idx = 2 * n
    t_idx = 2 * n + 1
    num_vars = 2 * n + 2

    c = np.zeros(num_vars)
    c[t_idx] = 1.0
    c[x0:x0 + n] = np.linspace(0.0, 1e-7, n)
    c[w0:w0 + n] = np.linspace(0.0, 1e-8, n)

    lb = np.zeros(num_vars)
    ub = np.zeros(num_vars)
    lb[x0:x0 + n] = 0.0
    ub[x0:x0 + n] = 1.0
    lb[w0:w0 + n] = 0.0
    ub[w0:w0 + n] = float(weight_max)
    lb[b_idx], ub[b_idx] = -200.0, 200.0
    lb[t_idx], ub[t_idx] = 0.0, 1000.0

    pool_pos = {idx: pos for pos, idx in enumerate(pool_indices)}
    for idx in locked_set:
        pos = pool_pos.get(idx)
        if pos is None:
            return None
        lb[x0 + pos] = 1.0
        ub[x0 + pos] = 1.0

    rows: list[dict[int, float]] = []
    lows: list[float] = []
    highs: list[float] = []

    rows.append({x0 + j: 1.0 for j in range(n)})
    lows.append(float(target_total))
    highs.append(float(target_total))

    rows.append({w0 + j: 1.0 for j in range(n)})
    lows.append(float(target_total))
    highs.append(float(target_total))

    for j in range(n):
        rows.append({w0 + j: 1.0, x0 + j: -float(weight_max)})
        lows.append(-np.inf)
        highs.append(0.0)

        rows.append({w0 + j: 1.0, x0 + j: -float(weight_min)})
        lows.append(0.0)
        highs.append(np.inf)

    player_to_cols: dict[str, list[int]] = {}
    for j, keys in enumerate(player_keys):
        for key in keys:
            player_to_cols.setdefault(key, []).append(j)
    for cols in player_to_cols.values():
        rows.append({x0 + j: 1.0 for j in cols})
        lows.append(-np.inf)
        highs.append(float(player_cap))

        rows.append({w0 + j: 1.0 for j in cols})
        lows.append(-np.inf)
        highs.append(float(player_weight_cap))

    coeff = float(slope) / float(target_total)
    for i in range(m):
        row = {w0 + j: coeff * float(rate_matrix[i, j]) for j in range(n)}
        row[b_idx] = 1.0
        row[t_idx] = -1.0
        rows.append(row)
        lows.append(-np.inf)
        highs.append(float(scores[i]))

        row = {w0 + j: -coeff * float(rate_matrix[i, j]) for j in range(n)}
        row[b_idx] = -1.0
        row[t_idx] = -1.0
        rows.append(row)
        lows.append(-np.inf)
        highs.append(-float(scores[i]))

    A = lil_matrix((len(rows), num_vars), dtype=float)
    for r, row in enumerate(rows):
        for col, val in row.items():
            A[r, col] = val
    constraints = LinearConstraint(A.tocsr(), np.array(lows), np.array(highs))
    integrality = np.zeros(num_vars, dtype=int)
    integrality[x0:x0 + n] = 1

    res = milp(
        c=c,
        integrality=integrality,
        bounds=Bounds(lb, ub),
        constraints=constraints,
        options={
            "time_limit": float(time_limit),
            "mip_rel_gap": 1e-4,
            "disp": False,
        },
    )
    if res.x is None:
        return None

    x = np.asarray(res.x[x0:x0 + n])
    w = np.asarray(res.x[w0:w0 + n])
    chosen = np.flatnonzero(x >= 0.5).tolist()
    if len(chosen) != target_total:
        return None

    chosen = sorted(chosen)
    selected_weights = [float(w[j]) for j in chosen]
    if any((not math.isfinite(v)) or v < weight_min - 1e-6 or v > weight_max + 1e-6 for v in selected_weights):
        return None
    if abs(sum(selected_weights) - target_total) > 1e-5:
        if abs(sum(selected_weights) - target_total) > 1e-3:
            return None
        scale = target_total / sum(selected_weights)
        selected_weights = [v * scale for v in selected_weights]

    mask = np.zeros(n, dtype=bool)
    full_w = np.zeros(n, dtype=float)
    for j, weight in zip(chosen, selected_weights):
        mask[j] = True
        full_w[j] = weight

    metrics = objective_metrics(mask, rate_matrix, scores, full_w)
    metrics["fixed_slope"] = float(slope)
    metrics["solver_fun"] = None if res.fun is None else float(res.fun)
    metrics["solver_status"] = int(res.status)
    metrics["solver_message"] = str(res.message)
    metrics["weight_min"] = float(min(selected_weights))
    metrics["weight_max"] = float(max(selected_weights))
    metrics["weight_sum"] = float(sum(selected_weights))
    return [pool_indices[j] for j in chosen], selected_weights, metrics


def better_metrics(a: dict[str, Any], b: dict[str, Any] | None) -> bool:
    if b is None:
        return True
    keys = ["max_abs_diff", "p95_abs_diff", "mean_abs_diff", "rmse"]
    for key in keys:
        av = float(a.get(key, float("inf")))
        bv = float(b.get(key, float("inf")))
        if av < bv - 1e-12:
            return True
        if av > bv + 1e-12:
            return False
    ac = a.get("corr")
    bc = b.get("corr")
    acv = -2.0 if ac is None or not math.isfinite(float(ac)) else float(ac)
    bcv = -2.0 if bc is None or not math.isfinite(float(bc)) else float(bc)
    return acv > bcv


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    payload = json.loads(Path(args.input).read_text(encoding="utf-8"))
    target_total = int(payload["target_total"])
    player_cap = int(payload["player_cap"])
    weight_min = float(payload.get("weight_min", 0.25))
    weight_max = float(payload.get("weight_max", 2.5))
    player_weight_cap = float(payload.get("player_weight_cap", float(player_cap)))
    pool_indices = [int(x) for x in payload["pool_indices"]]
    locked_set = {int(x) for x in payload["locked_indices"]}
    scores = np.asarray(payload["ref_scores"], dtype=float)
    rate_matrix = np.asarray(payload["rate_matrix"], dtype=float)
    player_keys = [[str(k) for k in keys] for keys in payload["player_keys"]]

    if rate_matrix.ndim != 2 or rate_matrix.shape[0] != scores.size or rate_matrix.shape[1] != len(pool_indices):
        raise SystemExit("invalid rate matrix dimensions")

    seed = greedy_feasible_seed(pool_indices, locked_set, player_keys, target_total, player_cap)
    if not seed:
        raise SystemExit("locked/player constraints are infeasible")

    seed_pos = {idx: pos for pos, idx in enumerate(pool_indices)}
    seed_mask = np.zeros(len(pool_indices), dtype=bool)
    for idx in seed:
        seed_mask[seed_pos[idx]] = True
    seed_weights = np.zeros(len(pool_indices), dtype=float)
    seed_weights[seed_mask] = 1.0
    seed_metrics = objective_metrics(seed_mask, rate_matrix, scores, seed_weights)
    base_slope = max(float(seed_metrics["slope"]), 1e-6)

    coarse_factors = [0.40, 0.50, 0.60, 0.70, 0.80, 0.90, 1.00, 1.10, 1.20, 1.35, 1.50, 1.70, 2.00]
    slopes = sorted({max(base_slope * f, 1e-6) for f in coarse_factors} | {base_slope, 1.0})

    best_selected = seed
    best_weights = [1.0 for _ in seed]
    best_metrics = seed_metrics | {"fixed_slope": base_slope, "solver_status": -1, "solver_message": "equal-weight greedy seed"}

    for slope in slopes:
        sol = solve_fixed_slope(
            slope,
            rate_matrix,
            scores,
            pool_indices,
            locked_set,
            player_keys,
            target_total,
            player_cap,
            weight_min,
            weight_max,
            player_weight_cap,
            time_limit=4.0,
        )
        if sol is None:
            continue
        selected, weights, metrics = sol
        if better_metrics(metrics, best_metrics):
            best_selected, best_weights, best_metrics = selected, weights, metrics

    best_fixed = max(float(best_metrics.get("fixed_slope", base_slope)), 1e-6)
    fine_slopes = sorted({max(best_fixed * f, 1e-6) for f in [0.82, 0.88, 0.94, 1.00, 1.06, 1.12, 1.18]})
    for slope in fine_slopes:
        sol = solve_fixed_slope(
            slope,
            rate_matrix,
            scores,
            pool_indices,
            locked_set,
            player_keys,
            target_total,
            player_cap,
            weight_min,
            weight_max,
            player_weight_cap,
            time_limit=6.0,
        )
        if sol is None:
            continue
        selected, weights, metrics = sol
        if better_metrics(metrics, best_metrics):
            best_selected, best_weights, best_metrics = selected, weights, metrics

    out = {
        "selected_indices": best_selected,
        "selected_weights": best_weights,
        "status": "ok",
        **best_metrics,
    }
    Path(args.output).write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
