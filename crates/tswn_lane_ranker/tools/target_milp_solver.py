#!/usr/bin/env python3
"""Weighted target solver for TSwn target generation.

`mode=compress_inherited_big_target_to_top50` starts from the actual browser
Top50. Replacement freezes that support's C-Score Top30 once and searches the
lower 20 low-C-Score-first. A second variable-count stage independently freezes
the completed Top50 C-Score Top30 once. Every deletion is absorbed jointly by
all remaining matchup columns;
the final exported weights sum to the surviving target count n and are scored
as sum(rate*weight)/n. Golden=1 rows receive a continuous cohesion prior, not
an equality constraint; Golden!=1 rows remain forward-fit variables.
Without that mode, the earlier fixed-support projection and legacy weighted
target-set MILP remain available.  In inherited compression, the fixed front
keeps only a secondary C-Score preference among Golden!=1 rows; it does not
override the Golden=1 complete-weight balance or force final-weight monotonicity.

The unlocked seed slots are selected by direct flattened C-Score replay rather
than by big-target profile coverage.  After the deletion path supplies a
starting count, a final 40--50 support beam explores additions, removals and
swaps while protecting only the fixed deletion C-Score Top30. Only final
Pareto improvements in Avg and Max Diff can be exported.

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
import os
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import Any

import numpy as np

try:
    from scipy.optimize import Bounds, LinearConstraint, lsq_linear, milp
    from scipy.sparse import lil_matrix
except Exception as exc:
    print(f"scipy.optimize.milp unavailable: {exc}", file=sys.stderr)
    sys.exit(2)


_TOP40_WORKER_CONTEXT: dict[str, Any] | None = None


def _init_top40_worker(context: dict[str, Any]) -> None:
    global _TOP40_WORKER_CONTEXT
    _TOP40_WORKER_CONTEXT = context


def _top40_worker_slope(slope: float) -> dict[str, Any]:
    """Solve one top40+dynamic10 MILP in an isolated worker process."""
    context = _TOP40_WORKER_CONTEXT
    if context is None:
        raise RuntimeError("top40 worker context was not initialized")
    active = context["active"]
    local_big = context["local_big"]
    locked_count = context["locked_count"]
    dynamic_count = context["dynamic_count"]
    scale_lo = context["scale_lo"]
    scale_hi = context["scale_hi"]
    delta_limit = context["delta_limit"]
    accounts = context["accounts"]
    owners = context["owners"]
    owner_cap = context["owner_cap"]
    single_name_mode = context["single_name_mode"]
    rates = context["rates"]
    scores = context["scores"]
    m = len(active)
    z0, q0, d0 = 0, m, 2 * m
    scale_idx, intercept_idx, error_idx = 3 * m, 3 * m + 1, 3 * m + 2
    count = 3 * m + 3
    c = np.zeros(count); c[error_idx] = 1.0
    integrality = np.zeros(count, dtype=int); integrality[z0:z0+m] = 1
    lb = np.full(count, -np.inf); ub = np.full(count, np.inf)
    lb[z0:z0+m] = 0.0; ub[z0:z0+m] = 1.0
    lb[z0:z0+locked_count] = 1.0; ub[z0:z0+locked_count] = 1.0
    lb[q0:q0+m] = 0.0; ub[q0:q0+m] = scale_hi
    lb[d0:d0+m] = -delta_limit; ub[d0:d0+m] = delta_limit
    lb[scale_idx] = scale_lo; ub[scale_idx] = scale_hi
    lb[intercept_idx] = -100.0; ub[intercept_idx] = 100.0
    lb[error_idx] = 0.0
    rows, lows, highs = [], [], []
    rows.append({z0+i: 1.0 for i in range(locked_count, m)})
    lows.append(dynamic_count); highs.append(dynamic_count)
    for i in range(m):
        rows.append({q0+i: 1.0, z0+i: -scale_lo}); lows.append(0.0); highs.append(np.inf)
        rows.append({q0+i: 1.0, z0+i: -scale_hi}); lows.append(-np.inf); highs.append(0.0)
        rows.append({q0+i: 1.0, scale_idx: -1.0, z0+i: -scale_hi}); lows.append(-scale_hi); highs.append(np.inf)
        rows.append({q0+i: 1.0, scale_idx: -1.0, z0+i: -scale_lo}); lows.append(-np.inf); highs.append(-scale_lo)
        rows.append({d0+i: 1.0, z0+i: -delta_limit}); lows.append(-np.inf); highs.append(0.0)
        rows.append({d0+i: -1.0, z0+i: -delta_limit}); lows.append(-np.inf); highs.append(0.0)
        rows.append({q0+i: local_big[i], d0+i: 1.0}); lows.append(0.0); highs.append(np.inf)
    rows.append({q0+i: local_big[i] for i in range(m)}); lows.append(50.0); highs.append(50.0)
    rows.append({d0+i: 1.0 for i in range(m)}); lows.append(0.0); highs.append(0.0)
    account_cols: dict[str, list[int]] = {}
    owner_cols: dict[str, list[int]] = {}
    for i, original in enumerate(active):
        for key in accounts[original]: account_cols.setdefault(key, []).append(i)
        owner_cols.setdefault(owners[original], []).append(i)
    if not single_name_mode:
        for cols in account_cols.values():
            if len(cols) > 1: rows.append({z0+i: 1.0 for i in cols}); lows.append(-np.inf); highs.append(1.0)
    for cols in owner_cols.values():
        if len(cols) > owner_cap: rows.append({z0+i: 1.0 for i in cols}); lows.append(-np.inf); highs.append(float(owner_cap))
    for row in range(rates.shape[0]):
        pos = {intercept_idx: 1.0, error_idx: -1.0}
        neg = {intercept_idx: -1.0, error_idx: -1.0}
        for i, original in enumerate(active):
            coefficient = slope * rates[row, original] / 50.0
            pos[q0+i] = coefficient * local_big[i]; pos[d0+i] = coefficient
            neg[q0+i] = -coefficient * local_big[i]; neg[d0+i] = -coefficient
        rows.extend([pos, neg]); lows.extend([-np.inf, -np.inf]); highs.extend([float(scores[row]), float(-scores[row])])
    A = lil_matrix((len(rows), count), dtype=float)
    for r, values in enumerate(rows):
        for col, value in values.items(): A[r, col] = value
    result = milp(
        c=c, integrality=integrality, bounds=Bounds(lb, ub),
        constraints=LinearConstraint(A.tocsr(), np.asarray(lows), np.asarray(highs)),
        options={"time_limit": float(context["time_limit"]), "mip_rel_gap": 1e-8, "presolve": True},
    )
    if result.x is None:
        raise RuntimeError(result.message)
    z = result.x[z0:z0+m]; q = result.x[q0:q0+m]; delta = result.x[d0:d0+m]
    positions = np.flatnonzero(z > 0.5)
    support = [active[int(i)] for i in positions]
    weights = local_big[positions] * q[positions] + delta[positions]
    return {
        "support": support, "weights": weights,
        "base_export": local_big[positions] * q[positions],
        "delta": delta[positions], "slope": slope,
        "intercept": float(result.x[intercept_idx]),
        "max": float(result.x[error_idx]),
        "gap": float(getattr(result, "mip_gap", 0.0) or 0.0),
        "scale": float(result.x[scale_idx]),
    }


def compression_metrics(
    rate_matrix: np.ndarray,
    base_weights: np.ndarray,
    tail_signal: np.ndarray,
    additions: np.ndarray,
    score_denominator: float,
) -> dict[str, Any]:
    full_response = (rate_matrix @ base_weights + tail_signal) / score_denominator
    compressed_response = rate_matrix @ (base_weights + additions) / score_denominator
    diff = compressed_response - full_response
    abs_diff = np.abs(diff)
    return {
        "mean_diff": float(np.mean(diff)),
        "mean_abs_diff": float(np.mean(abs_diff)),
        "max_abs_diff": float(np.max(abs_diff)),
        "p95_abs_diff": float(np.quantile(abs_diff, 0.95)),
        "rmse": float(np.sqrt(np.mean(diff * diff))),
        "corr": corrcoef(compressed_response, full_response),
    }


def _sum_constrained_ridge(
    rates: np.ndarray,
    target_signal: np.ndarray,
    anchor: np.ndarray,
    alpha: float,
) -> np.ndarray:
    """Fit a response while preserving the anchor's signed total mass."""
    row_count, column_count = rates.shape
    gram = (rates.T @ rates) / max(1, row_count)
    rhs = rates.T @ (target_signal - rates @ anchor) / max(1, row_count)
    system = np.zeros((column_count + 1, column_count + 1), dtype=float)
    system[:column_count, :column_count] = gram
    if alpha > 0.0:
        system[:column_count, :column_count] += alpha * np.eye(column_count)
    system[:column_count, column_count] = 1.0
    system[column_count, :column_count] = 1.0
    target = np.r_[rhs, 0.0]
    try:
        solution = np.linalg.solve(system, target)
    except np.linalg.LinAlgError:
        solution = np.linalg.lstsq(system, target, rcond=1e-12)[0]
    fitted = anchor + solution[:column_count]
    # Remove the last few ulps of equality-constraint drift.
    fitted -= (float(np.sum(fitted)) - float(np.sum(anchor))) / column_count
    return fitted


def _guard_final_weight_dispersion(
    additions: np.ndarray,
    final_base: np.ndarray | None,
    deviation_l2_limit: float | None,
    max_deviation_limit: float | None,
) -> np.ndarray:
    """Radially shrink a fitted final-weight vector into the stable envelope."""
    if final_base is None:
        return additions
    final_weights = final_base + additions
    center = float(np.mean(final_weights))
    deviation = final_weights - center
    deviation_l2 = float(np.linalg.norm(deviation))
    max_deviation = float(np.max(np.abs(deviation)))
    scale = 1.0
    if deviation_l2_limit is not None and deviation_l2 > 1e-15:
        scale = min(scale, deviation_l2_limit / deviation_l2)
    if max_deviation_limit is not None and max_deviation > 1e-15:
        scale = min(scale, max_deviation_limit / max_deviation)
    if scale >= 1.0:
        return additions
    guarded_final = center + max(0.0, scale) * deviation
    guarded = guarded_final - final_base
    guarded -= (float(np.sum(guarded)) - float(np.sum(additions))) / guarded.size
    return guarded


def _cross_validated_stable_projection(
    rates: np.ndarray,
    target_signal: np.ndarray,
    anchor: np.ndarray,
    movement_limit: float | None = None,
    final_base: np.ndarray | None = None,
    final_deviation_l2_limit: float | None = None,
    final_max_deviation_limit: float | None = None,
) -> tuple[np.ndarray, dict[str, Any]]:
    """Select ridge strength by held-out forward replay (one-SE rule).

    Score rows, rather than coefficient coordinates, are cross-validated.
    Consequently a large positive/negative null-space cancellation has no
    advantage. Among statistically indistinguishable fits the one-standard-
    error rule deliberately selects the most strongly regularized solution.
    """
    row_count, column_count = rates.shape
    gram = (rates.T @ rates) / max(1, row_count)
    positive_eigenvalues = np.linalg.eigvalsh(gram)
    positive_eigenvalues = positive_eigenvalues[positive_eigenvalues > 1e-12]
    scale = (
        float(np.median(positive_eigenvalues))
        if positive_eigenvalues.size
        else max(float(np.trace(gram)) / max(1, column_count), 1.0)
    )
    alphas = np.r_[0.0, scale * np.logspace(-10.0, 6.0, 65)]
    fold_count = min(5, row_count)
    rng = np.random.default_rng(20260725)
    fold_id = np.empty(row_count, dtype=int)
    fold_id[rng.permutation(row_count)] = np.arange(row_count) % fold_count

    candidates: list[dict[str, Any]] = []
    for alpha in alphas:
        fold_rmse = []
        for fold in range(fold_count):
            test = fold_id == fold
            train = ~test
            if not np.any(train) or not np.any(test):
                continue
            fitted = _sum_constrained_ridge(
                rates[train], target_signal[train], anchor, float(alpha),
            )
            fitted = _guard_final_weight_dispersion(
                fitted,
                final_base,
                final_deviation_l2_limit,
                final_max_deviation_limit,
            )
            residual = rates[test] @ fitted - target_signal[test]
            fold_rmse.append(float(np.sqrt(np.mean(residual * residual))))
        fitted_all = _sum_constrained_ridge(rates, target_signal, anchor, float(alpha))
        fitted_all = _guard_final_weight_dispersion(
            fitted_all,
            final_base,
            final_deviation_l2_limit,
            final_max_deviation_limit,
        )
        all_residual = rates @ fitted_all - target_signal
        movement = float(np.linalg.norm(fitted_all - anchor))
        final_weights = (
            fitted_all if final_base is None else final_base + fitted_all
        )
        final_center = float(np.mean(final_weights))
        candidates.append({
            "alpha": float(alpha),
            "fold_rmse": fold_rmse,
            "cv_mean_rmse": float(np.mean(fold_rmse)),
            "cv_se_rmse": (
                float(np.std(fold_rmse, ddof=1) / np.sqrt(len(fold_rmse)))
                if len(fold_rmse) > 1 else 0.0
            ),
            "all_rmse": float(np.sqrt(np.mean(all_residual * all_residual))),
            "all_max_abs": float(np.max(np.abs(all_residual))),
            "movement_l2": movement,
            "final_deviation_l2": float(np.linalg.norm(final_weights - final_center)),
            "final_max_deviation": float(np.max(np.abs(final_weights - final_center))),
            "fitted": fitted_all,
        })

    # The unmodified anchor is the limiting alpha=+infinity model.
    anchor_residual = rates @ anchor - target_signal
    anchor_fold_rmse = [
        float(np.sqrt(np.mean(anchor_residual[fold_id == fold] ** 2)))
        for fold in range(fold_count)
        if np.any(fold_id == fold)
    ]
    anchor_final_weights = anchor if final_base is None else final_base + anchor
    anchor_final_center = float(np.mean(anchor_final_weights))
    candidates.append({
        "alpha": float("inf"),
        "fold_rmse": anchor_fold_rmse,
        "cv_mean_rmse": float(np.mean(anchor_fold_rmse)),
        "cv_se_rmse": (
            float(np.std(anchor_fold_rmse, ddof=1) / np.sqrt(len(anchor_fold_rmse)))
            if len(anchor_fold_rmse) > 1 else 0.0
        ),
        "all_rmse": float(np.sqrt(np.mean(anchor_residual * anchor_residual))),
        "all_max_abs": float(np.max(np.abs(anchor_residual))),
        "movement_l2": 0.0,
        "final_deviation_l2": float(
            np.linalg.norm(anchor_final_weights - anchor_final_center)
        ),
        "final_max_deviation": float(
            np.max(np.abs(anchor_final_weights - anchor_final_center))
        ),
        "fitted": anchor.copy(),
    })

    stable = [
        item for item in candidates
        if movement_limit is None or item["movement_l2"] <= movement_limit + 1e-9
        if (
            final_deviation_l2_limit is None
            or item["final_deviation_l2"] <= final_deviation_l2_limit + 1e-9
        )
        if (
            final_max_deviation_limit is None
            or item["final_max_deviation"] <= final_max_deviation_limit + 1e-9
        )
    ]
    best = min(stable, key=lambda item: item["cv_mean_rmse"])
    one_se_limit = best["cv_mean_rmse"] + best["cv_se_rmse"]
    eligible = [
        item for item in stable
        if item["cv_mean_rmse"] <= one_se_limit + 1e-12
    ]
    selected = max(eligible, key=lambda item: item["alpha"])
    fitted = np.asarray(selected.pop("fitted"), dtype=float)
    for item in candidates:
        item.pop("fitted", None)
        item.pop("fold_rmse", None)
    diagnostics = {
        "selection_rule": "five_fold_forward_replay_one_standard_error",
        "selected_alpha": (
            None if not math.isfinite(selected["alpha"]) else selected["alpha"]
        ),
        "best_cv_mean_rmse_weighted_sum": best["cv_mean_rmse"],
        "selected_cv_mean_rmse_weighted_sum": selected["cv_mean_rmse"],
        "selected_cv_se_rmse_weighted_sum": selected["cv_se_rmse"],
        "movement_limit_l2": movement_limit,
        "selected_movement_l2": selected["movement_l2"],
        "final_deviation_l2_limit": final_deviation_l2_limit,
        "final_max_deviation_limit": final_max_deviation_limit,
        "selected_final_deviation_l2": selected["final_deviation_l2"],
        "selected_final_max_deviation": selected["final_max_deviation"],
        "ridge_path_count": len(candidates),
    }
    return fitted, diagnostics


def solve_merged_tail_compression(payload: dict[str, Any]) -> dict[str, Any]:
    """Project a desired response onto fixed support without null-space spikes.

    The first support weights are kept as a base. A signed addition vector is
    fitted, and its sum is exactly the removed tail's signed sum. The old
    minimax-first solve could exploit nearly-null rate directions and produce
    huge alternating additions. Here proportional redistribution is the
    anchor; ridge strength is selected on held-out score rows. The final
    weights may not become more dispersed than the mass-rescaled proportional
    anchor, either in L2 deviation or in single-weight maximum deviation.
    """
    base = np.asarray(payload["base_weights"], dtype=float)
    rates = np.asarray(payload["rate_matrix"], dtype=float)
    tail_signal = np.asarray(payload["tail_signal"], dtype=float)
    tail_weight = float(payload["tail_weight"])
    tail_l1_weight = float(payload.get("tail_l1_weight", abs(tail_weight)))
    total_weight = float(payload["total_weight"])
    score_denominator = float(payload.get("score_denominator", 50.0))
    if base.ndim != 1 or base.size == 0:
        raise ValueError("base_weights must be a non-empty vector")
    if rates.ndim != 2 or rates.shape[1] != base.size:
        raise ValueError("rate_matrix must have one column per base weight")
    if tail_signal.ndim != 1 or tail_signal.size != rates.shape[0] or tail_signal.size == 0:
        raise ValueError("tail_signal must have one value per reference row")
    if (
        not np.all(np.isfinite(base))
        or not np.all(np.isfinite(rates))
        or not np.all(np.isfinite(tail_signal))
        or not math.isfinite(tail_weight)
        or not math.isfinite(tail_l1_weight)
        or tail_l1_weight < abs(tail_weight) - 1e-8
        or not math.isfinite(total_weight)
        or not math.isfinite(score_denominator)
        or score_denominator <= 0.0
    ):
        raise ValueError("signed compression inputs must be finite and internally consistent")
    if abs(float(np.sum(base)) + tail_weight - total_weight) > 1e-6:
        raise ValueError("base weight plus tail weight must equal total weight")

    k = base.size
    if tail_l1_weight <= 1e-12:
        additions = np.zeros(k, dtype=float)
        metrics = compression_metrics(rates, base, tail_signal, additions, score_denominator)
        center = total_weight / k
        base_std = float(np.std(base, ddof=0))
        base_max_deviation = float(np.max(np.abs(base - center)))
        return {
            "status": "ok_no_tail",
            "selected_weights": base.tolist(),
            "fitted_additions": additions.tolist(),
            "tail_weight": tail_weight,
            "tail_l1_weight": tail_l1_weight,
            "total_weight": total_weight,
            "score_denominator": score_denominator,
            "response_mse_optimum_before_weight_regularization": 0.0,
            "response_mse_cap_for_weight_regularization": 0.0,
            "response_mse_after_weight_regularization": 0.0,
            "weight_regularization_applied": True,
            "weight_regularization_status": "no tail",
            "baseline_addition_l2_norm": 0.0,
            "optimized_addition_l2_norm": 0.0,
            "optimized_addition_max_abs": 0.0,
            "optimized_distance_from_proportional_l2": 0.0,
            "stability_final_weight_mean": center,
            "stability_final_weight_std_limit": base_std,
            "stability_final_weight_max_deviation_limit": base_max_deviation,
            "optimized_final_weight_std": base_std,
            "optimized_final_weight_max_deviation": base_max_deviation,
            **{f"baseline_{name}": value for name, value in metrics.items()},
            **{f"optimized_{name}": value for name, value in metrics.items()},
        }

    base_l1_sum = float(np.sum(np.abs(base)))
    if base_l1_sum > 0.0:
        baseline_additions = tail_weight * np.abs(base) / base_l1_sum
    else:
        baseline_additions = np.full(k, tail_weight / k, dtype=float)
    baseline = compression_metrics(
        rates, base, tail_signal, baseline_additions, score_denominator,
    )

    baseline_final_weights = base + baseline_additions
    final_weight_center = total_weight / k
    baseline_final_deviation_l2 = float(
        np.linalg.norm(baseline_final_weights - final_weight_center)
    )
    baseline_final_max_deviation = float(
        np.max(np.abs(baseline_final_weights - final_weight_center))
    )
    additions, regularization = _cross_validated_stable_projection(
        rates,
        tail_signal,
        baseline_additions,
        final_base=base,
        final_deviation_l2_limit=baseline_final_deviation_l2,
        final_max_deviation_limit=baseline_final_max_deviation,
    )
    # A selected ridge point should obey the physical signed-tail box as well.
    # If numerical/path selection violates it, the proportional anchor is the
    # deterministic safe fallback.
    if float(np.max(np.abs(additions))) > tail_l1_weight + 1e-8:
        additions = baseline_additions.copy()
        status = "ok_proportional_box_fallback"
    else:
        status = "ok_cv_regularized"

    optimized = compression_metrics(rates, base, tail_signal, additions, score_denominator)
    if optimized["max_abs_diff"] > baseline["max_abs_diff"] + 1e-7:
        # Do not accept a stable fit that makes the worst current-row replay
        # worse than the proportional baseline.
        additions = baseline_additions
        optimized = baseline.copy()
        status = "ok_proportional_fallback"

    selected_weights = base + additions
    addition_l2_norm = float(np.linalg.norm(additions))
    addition_max_abs = float(np.max(np.abs(additions)))
    distance_from_proportional_l2 = float(
        np.linalg.norm(additions - baseline_additions)
    )
    return {
        "status": status,
        "selected_weights": selected_weights.tolist(),
        "fitted_additions": additions.tolist(),
        "tail_weight": tail_weight,
        "tail_l1_weight": tail_l1_weight,
        "total_weight": total_weight,
        "score_denominator": score_denominator,
        "response_mse_optimum_before_weight_regularization": (
            optimized["rmse"] * score_denominator
        ) ** 2,
        "response_mse_cap_for_weight_regularization": None,
        "response_mse_after_weight_regularization": (
            optimized["rmse"] * score_denominator
        ) ** 2,
        "weight_regularization_applied": True,
        "weight_regularization_status": regularization["selection_rule"],
        "regularization_selected_alpha": regularization["selected_alpha"],
        "regularization_best_cv_rmse_weighted_sum": (
            regularization["best_cv_mean_rmse_weighted_sum"]
        ),
        "regularization_selected_cv_rmse_weighted_sum": (
            regularization["selected_cv_mean_rmse_weighted_sum"]
        ),
        "regularization_movement_limit_l2": 0.0,
        "stability_final_weight_mean": final_weight_center,
        "stability_final_weight_std_limit": (
            baseline_final_deviation_l2 / math.sqrt(k)
        ),
        "stability_final_weight_max_deviation_limit": baseline_final_max_deviation,
        "optimized_final_weight_std": float(np.std(selected_weights, ddof=0)),
        "optimized_final_weight_max_deviation": float(
            np.max(np.abs(selected_weights - final_weight_center))
        ),
        "baseline_addition_l2_norm": float(np.linalg.norm(baseline_additions)),
        "optimized_addition_l2_norm": addition_l2_norm,
        "optimized_addition_max_abs": addition_max_abs,
        "optimized_distance_from_proportional_l2": distance_from_proportional_l2,
        "solver_status": 0,
        "solver_message": regularization["selection_rule"],
        **{f"baseline_{name}": value for name, value in baseline.items()},
        **{f"optimized_{name}": value for name, value in optimized.items()},
    }


def _candidate_order_key(
    idx: int,
    pair_ranks: list[int | None],
    correct_scores: np.ndarray,
    raw_ranks: list[int],
    group_ids: list[int],
) -> tuple[float, float, int, int]:
    pair_rank = pair_ranks[idx]
    return (
        float("inf") if pair_rank is None else int(pair_rank),
        -float(correct_scores[idx]),
        int(raw_ranks[idx]),
        int(group_ids[idx]),
    )


def _browser_main_parent_order(
    pair_ranks: list[int | None],
    correct_scores: np.ndarray,
    raw_ranks: list[int],
    group_ids: list[int],
    selection_status: list[str],
    blocked: np.ndarray,
    raw_members: list[list[str]],
) -> list[int]:
    """Reproduce constrainedPresentationRows + buildFoldedResultGroups.

    The browser folds on literal members as displayed in `canonical`.  Account
    uniqueness after team merging is a separate selection constraint and is
    intentionally not used by this folding routine.
    """
    order = [
        idx
        for idx in range(len(group_ids))
        if selection_status[idx] != "below_threshold"
        and pair_ranks[idx] is not None
        and math.isfinite(float(correct_scores[idx]))
    ]
    order.sort(
        key=lambda idx: _candidate_order_key(
            idx, pair_ranks, correct_scores, raw_ranks, group_ids,
        )
    )
    consumed = [False] * len(order)
    pending: dict[int, list[int]] = {}
    member_sets = [set(raw_members[idx]) for idx in order]
    parents: list[int] = []

    def overlaps(a: set[str], b: set[str]) -> bool:
        return bool(a and b and not a.isdisjoint(b))

    def lower_unblocked_overlap(start: int, members: set[str]) -> int | None:
        for pos in range(start + 1, len(order)):
            if consumed[pos] or bool(blocked[order[pos]]):
                continue
            if overlaps(members, member_sets[pos]):
                return pos
        return None

    for pos, idx in enumerate(order):
        if consumed[pos]:
            continue
        if bool(blocked[idx]):
            target = lower_unblocked_overlap(pos, member_sets[pos])
            if target is not None:
                pending.setdefault(target, []).append(pos)
                consumed[pos] = True
                continue

        consumed[pos] = True
        pending.pop(pos, None)
        for child_pos in range(pos + 1, len(order)):
            if consumed[child_pos] or not overlaps(
                member_sets[pos], member_sets[child_pos],
            ):
                continue
            child_idx = order[child_pos]
            if bool(blocked[child_idx]):
                target = lower_unblocked_overlap(child_pos, member_sets[child_pos])
                if target is not None:
                    pending.setdefault(target, []).append(child_pos)
                    consumed[child_pos] = True
                    continue
            consumed[child_pos] = True
            pending.pop(child_pos, None)
        parents.append(idx)
    return parents


def _support_feasible(
    support: list[int] | tuple[int, ...],
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
    target_total: int | None = None,
) -> bool:
    if target_total is not None and len(support) != target_total:
        return False
    if len(set(support)) != len(support):
        return False
    used_accounts: set[str] = set()
    owner_counts: dict[str, int] = {}
    for idx in support:
        keys = account_keys[idx]
        if any(key in used_accounts for key in keys):
            return False
        used_accounts.update(keys)
        owner = owner_keys[idx]
        owner_counts[owner] = owner_counts.get(owner, 0) + 1
        if owner_counts[owner] > owner_cap:
            return False
    return True


def _initial_support_and_locked(
    parent_order: list[int],
    eligible: list[int],
    player_top3_universe: list[int],
    pair_ranks: list[int | None],
    correct_scores: np.ndarray,
    raw_ranks: list[int],
    group_ids: list[int],
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
    target_total: int,
) -> tuple[list[int], list[int]]:
    eligible_set = set(eligible)
    selected: list[int] = []
    for idx in parent_order:
        if idx not in eligible_set:
            continue
        proposed = selected + [idx]
        if not _support_feasible(
            proposed, account_keys, owner_keys, owner_cap,
        ):
            continue
        selected.append(idx)
        if len(selected) == target_total:
            break
    if len(selected) != target_total:
        raise ValueError(
            "browser C-Score main board cannot provide the requested target "
            "count under account-uniqueness and merged-team cap constraints"
        )

    # "Initial target AND player's Top3": compute each merged team's global
    # C-Score Top3 once, intersect it with S0 once, and never recompute it after
    # replacements.
    by_owner: dict[str, list[int]] = {}
    for idx in player_top3_universe:
        by_owner.setdefault(owner_keys[idx], []).append(idx)
    owner_top3: set[int] = set()
    for indices in by_owner.values():
        indices.sort(
            key=lambda idx: _candidate_order_key(
                idx, pair_ranks, correct_scores, raw_ranks, group_ids,
            )
        )
        owner_top3.update(indices[:3])
    locked = [idx for idx in selected if idx in owner_top3]
    return selected, locked


def _profile_geometry(rate_matrix: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    means = np.mean(rate_matrix, axis=0)
    centered = rate_matrix - means
    rms = np.sqrt(np.mean(centered * centered, axis=0))
    rms = np.maximum(rms, 1e-6)
    normalized = centered / rms
    return normalized, means


def _profile_distance_block(
    normalized_profiles: np.ndarray,
    profile_means: np.ndarray,
    rows: np.ndarray,
    cols: np.ndarray,
) -> np.ndarray:
    shape_diff = normalized_profiles[:, rows, None] - normalized_profiles[:, None, cols]
    shape_rms = np.sqrt(np.mean(shape_diff * shape_diff, axis=0))
    mean_diff = np.abs(profile_means[rows, None] - profile_means[None, cols]) / 50.0
    return shape_rms + 0.20 * mean_diff


def _type_balanced_transport_anchor(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
    priority_indices: set[int] | None = None,
) -> tuple[np.ndarray, np.ndarray, dict[str, Any]]:
    """Transport each omitted weight across the complete support.

    Every support row receives an equal-share floor.  When
    ``priority_indices`` is supplied, only that fixed front prefix receives
    the profile-sensitive remainder.  Supplemental/tail rows therefore keep
    their own complete-big-target base scale instead of being pulled toward
    the global support mean.  The fixed prefix additions are then projected
    so they are non-increasing by C-Score; final weights themselves are not
    projected.
    """
    n = big_weights.size
    support_array = np.asarray(support, dtype=int)
    support_set = set(support)
    tail = np.asarray(
        [idx for idx in range(n) if idx not in support_set and abs(big_weights[idx]) > 1e-15],
        dtype=int,
    )
    additions = np.zeros(len(support), dtype=float)
    transfers = np.zeros((tail.size, len(support)), dtype=float)
    if tail.size == 0:
        return additions, transfers, {
            "transport_tail_count": 0,
            "transport_effective_receiver_count": 0.0,
            "transport_mean_profile_distance": 0.0,
        }

    normalized, means = _profile_geometry(rate_matrix)
    distances = _profile_distance_block(
        normalized, means, tail, support_array,
    )
    priority_set = set(priority_indices or ())
    profile_receiver_positions = [
        pos for pos, idx in enumerate(support)
        if not priority_set or idx in priority_set
    ]
    if not profile_receiver_positions:
        profile_receiver_positions = list(range(len(support)))
    profile_receiver_array = np.asarray(
        profile_receiver_positions, dtype=int,
    )
    receiver_scores = correct_scores[
        support_array[profile_receiver_array]
    ]
    score_scale = max(float(np.std(receiver_scores, ddof=0)), 1e-6)
    receiver_score_z = np.clip(
        (receiver_scores - np.mean(receiver_scores)) / score_scale,
        -2.5,
        2.5,
    )
    effective_counts = []
    weighted_distances = []
    uniform_share_fraction = 0.45
    for tail_pos, tail_idx in enumerate(tail):
        local_dist = distances[tail_pos]
        receiver_dist = local_dist[profile_receiver_array]
        positive_dist = receiver_dist[receiver_dist > 1e-12]
        temperature = max(
            float(np.median(positive_dist)) if positive_dist.size else 1.0,
            0.08,
        )
        logits = -(receiver_dist / temperature) ** 2
        # This is deliberately mild: type similarity remains the primary
        # allocation rule, while higher C-Score wins near-ties.
        logits += 0.18 * receiver_score_z
        logits -= float(np.max(logits))
        receiver_profile_shares = np.exp(logits)
        receiver_profile_shares /= float(
            np.sum(receiver_profile_shares)
        )
        profile_shares = np.zeros(len(support), dtype=float)
        profile_shares[profile_receiver_array] = receiver_profile_shares
        shares = (
            uniform_share_fraction / len(support)
            + (1.0 - uniform_share_fraction) * profile_shares
        )
        transfers[tail_pos] = big_weights[tail_idx] * shares
        additions += transfers[tail_pos]
        effective_counts.append(1.0 / float(np.sum(shares * shares)))
        weighted_distances.append(float(np.dot(shares, local_dist)))

    priority_projection: dict[str, Any] = {
        "applied": False,
        "priority_count": 0,
        "projection_l2": 0.0,
    }
    if priority_indices:
        priority_positions = [
            pos for pos, idx in enumerate(support)
            if idx in priority_indices
        ]
        if priority_positions:
            priority_support = [support[pos] for pos in priority_positions]
            before = additions[np.asarray(priority_positions, dtype=int)]
            projected, priority_projection = (
                _front_priority_c_score_projection(
                    before,
                    priority_support,
                    correct_scores,
                    len(priority_positions),
                )
            )
            additions[np.asarray(priority_positions, dtype=int)] = projected
            priority_projection = {
                **priority_projection,
                "method": (
                    "fixed_non_deletable_prefix_absorption_addition_projection"
                ),
            }
    return additions, transfers, {
        "transport_tail_count": int(tail.size),
        "transport_effective_receiver_count": float(np.mean(effective_counts)),
        "transport_mean_profile_distance": float(np.mean(weighted_distances)),
        "transport_receiver_count": len(support),
        "transport_profile_receiver_count": len(
            profile_receiver_positions
        ),
        "transport_profile_receiver_rule": (
            "fixed_front_only"
            if priority_set else "all_support"
        ),
        "transport_uniform_share_fraction": uniform_share_fraction,
        "priority_absorption_projection": priority_projection,
    }


GOLDEN_UNIT_ATOL = 1e-8


def _golden_unit_indices(
    golden_weights: np.ndarray,
) -> set[int]:
    """Return candidates whose Raw Golden mass is numerically one."""
    return set(np.flatnonzero(np.isclose(
        np.asarray(golden_weights, dtype=float),
        1.0,
        rtol=0.0,
        atol=GOLDEN_UNIT_ATOL,
    )).tolist())


def _golden_unit_balanced_transport_anchor(
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
    priority_indices: set[int] | None = None,
) -> tuple[np.ndarray, np.ndarray, dict[str, Any]]:
    """Equalize Golden=1 rows and leave every other row free to fit.

    Selected rows retain their own complete Correct big-target weight as the
    lineage base.  The omitted mass and any within-unit balancing shift are
    placed on the selected Golden=1 block so that its complete anchor weights
    share one common level.  Golden!=1 rows keep their own base in the anchor;
    the forward solver, rather than a Golden-proportional rule, decides their
    eventual movement.
    """
    n = big_weights.size
    support_array = np.asarray(support, dtype=int)
    support_set = set(support)
    tail = np.asarray(
        [
            idx for idx in range(n)
            if idx not in support_set and abs(big_weights[idx]) > 1e-15
        ],
        dtype=int,
    )
    base = np.asarray(big_weights[support_array], dtype=float)
    selected_golden = np.asarray(golden_weights[support_array], dtype=float)
    unit_mask = np.isclose(
        selected_golden, 1.0, rtol=0.0, atol=GOLDEN_UNIT_ATOL,
    )
    unit_positions = np.flatnonzero(unit_mask)
    free_positions = np.flatnonzero(~unit_mask)
    omitted_mass = float(np.sum(big_weights[tail]))
    anchor = base.copy()
    if unit_positions.size:
        common_weight = (
            float(np.sum(base[unit_positions])) + omitted_mass
        ) / unit_positions.size
        anchor[unit_positions] = common_weight
        shares = np.zeros(len(support), dtype=float)
        shares[unit_positions] = 1.0 / unit_positions.size
        share_rule = "golden_equals_one_complete_weight_equalization"
    else:
        common_weight = None
        shares = np.full(len(support), 1.0 / len(support), dtype=float)
        anchor += omitted_mass * shares
        share_rule = "uniform_seed_when_no_selected_golden_equals_one"
    transfers = (
        big_weights[tail, None] * shares[None, :]
        if tail.size else np.zeros((0, len(support)), dtype=float)
    )
    additions = anchor - base

    priority_projection: dict[str, Any] = {
        "applied": False,
        "priority_count": 0,
        "projection_l2": 0.0,
        "method": "deferred_to_free_forward_fit",
    }
    unit_spread = (
        float(np.ptp(anchor[unit_positions]))
        if unit_positions.size else 0.0
    )
    return additions, transfers, {
        "transport_tail_count": int(tail.size),
        "transport_receiver_count": len(support),
        "transport_receiver_rule": share_rule,
        "transport_selected_golden_sum": float(np.sum(selected_golden)),
        "transport_golden_share_min": float(np.min(shares)),
        "transport_golden_share_max": float(np.max(shares)),
        "golden_unit_tolerance": GOLDEN_UNIT_ATOL,
        "golden_unit_count": int(unit_positions.size),
        "golden_nonunit_free_count": int(free_positions.size),
        "golden_unit_common_anchor_weight": common_weight,
        "golden_unit_anchor_spread": unit_spread,
        "golden_nonunit_anchor_rule": (
            "own_big_target_base_then_free_forward_fit"
        ),
        "golden_unit_internal_equalization_l1": float(
            np.sum(np.abs(additions - np.sum(transfers, axis=0)))
        ),
        "transport_matrix_l1": float(np.sum(np.abs(transfers))),
        "priority_absorption_projection": priority_projection,
    }


def _project_golden_unit_complete_weights(
    weights: np.ndarray,
    anchor: np.ndarray,
    support: list[int],
    golden_unit_indices: set[int] | None,
) -> tuple[np.ndarray, dict[str, Any]]:
    """Keep Golden=1 complete weights equal; leave all non-unit rows free."""
    out = np.asarray(weights, dtype=float).copy()
    anchor_array = np.asarray(anchor, dtype=float)
    if out.shape != anchor_array.shape or out.size != len(support):
        raise ValueError("Golden=1 complete-weight projection shape mismatch")
    unit_set = set(golden_unit_indices or ())
    unit_positions = [
        pos for pos, idx in enumerate(support) if idx in unit_set
    ]
    if not unit_positions:
        return out, {
            "applied": False,
            "unit_count": 0,
            "free_count": len(support),
            "before_spread": 0.0,
            "after_spread": 0.0,
            "projection_l2": 0.0,
        }
    unit_array = np.asarray(unit_positions, dtype=int)
    free_positions = [
        pos for pos, idx in enumerate(support) if idx not in unit_set
    ]
    original = out.copy()
    total_mass = float(np.sum(out))
    before_spread = float(np.ptp(out[unit_array]))
    if free_positions:
        common_weight = float(np.mean(anchor_array[unit_array]))
        out[unit_array] = common_weight
        free_array = np.asarray(free_positions, dtype=int)
        out[free_array] += (
            total_mass - float(np.sum(out))
        ) / free_array.size
    else:
        common_weight = total_mass / unit_array.size
        out[unit_array] = common_weight
    out += (total_mass - float(np.sum(out))) / out.size
    after_spread = float(np.ptp(out[unit_array]))
    return out, {
        "applied": True,
        "unit_count": len(unit_positions),
        "free_count": len(free_positions),
        "common_weight": common_weight,
        "before_spread": before_spread,
        "after_spread": after_spread,
        "projection_l2": float(np.linalg.norm(out - original)),
        "rule": (
            "golden_equals_one_complete_weights_share_one_level;"
            "golden_nonunit_weights_remain_forward_fit_variables"
        ),
    }


def _project_priority_absorption_additions(
    weights: np.ndarray,
    base_weights: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
    priority_indices: set[int] | None,
) -> tuple[np.ndarray, dict[str, Any]]:
    """Order only the absorption additions of the fixed front prefix."""
    out = np.asarray(weights, dtype=float).copy()
    base = np.asarray(base_weights, dtype=float)
    if out.shape != base.shape or out.size != len(support):
        raise ValueError("priority absorption projection shape mismatch")
    if not priority_indices:
        return out, {
            "applied": False,
            "priority_count": 0,
            "projection_l2": 0.0,
        }
    priority_positions = [
        pos for pos, idx in enumerate(support)
        if idx in priority_indices
    ]
    if not priority_positions:
        return out, {
            "applied": False,
            "priority_count": 0,
            "projection_l2": 0.0,
        }
    pos_array = np.asarray(priority_positions, dtype=int)
    additions = out[pos_array] - base[pos_array]
    priority_support = [support[pos] for pos in priority_positions]
    projected, audit = _front_priority_c_score_projection(
        additions,
        priority_support,
        correct_scores,
        len(priority_positions),
    )
    out[pos_array] = base[pos_array] + projected
    return out, {
        **audit,
        "method": "fixed_non_deletable_prefix_absorption_addition_projection",
    }


def _local_profile_pairs(
    rate_matrix: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
    tail_l1_weight: float,
) -> list[tuple[int, int, float, float]]:
    support_array = np.asarray(support, dtype=int)
    normalized, means = _profile_geometry(rate_matrix)
    distances = _profile_distance_block(
        normalized, means, support_array, support_array,
    )
    score_std = max(float(np.std(correct_scores[support_array], ddof=0)), 1e-6)
    # The desired gap is deliberately small; the comparatively strong local
    # pair penalty below is meant to establish direction, not manufacture a
    # new rank-based weight scale.
    desired_scale = 0.08 * tail_l1_weight / max(1, len(support))
    pairs: dict[tuple[int, int], tuple[float, float]] = {}
    neighbor_count = min(4, max(0, len(support) - 1))
    for i in range(len(support)):
        order = [
            j for j in np.argsort(distances[i], kind="stable")
            if j != i
        ][:neighbor_count]
        for j in order:
            lo, hi = sorted((i, int(j)))
            distance = float(distances[lo, hi])
            similarity = math.exp(-distance * distance / (2.0 * 0.45 * 0.45))
            if similarity < 0.05:
                continue
            score_diff = (
                float(correct_scores[support[lo]])
                - float(correct_scores[support[hi]])
            )
            desired = desired_scale * math.tanh(score_diff / score_std)
            previous = pairs.get((lo, hi))
            if previous is None or similarity > previous[0]:
                pairs[(lo, hi)] = (similarity, desired)
    return [
        (i, j, similarity, desired)
        for (i, j), (similarity, desired) in sorted(pairs.items())
    ]


def _sum_constrained_quadratic_fit(
    support_rates: np.ndarray,
    tail_signal: np.ndarray,
    anchor: np.ndarray,
    tail_mass: float,
    correct_scores: np.ndarray,
    support: list[int],
    tail_l1_weight: float,
    regularization_factor: float,
) -> np.ndarray:
    a = support_rates / 50.0
    b = tail_signal / 50.0
    row_count, column_count = a.shape
    gram = (a.T @ a) / max(1, row_count)
    rhs = a.T @ b / max(1, row_count)
    eigenvalues = np.linalg.eigvalsh(gram)
    positive = eigenvalues[eigenvalues > 1e-12]
    scale = (
        float(np.median(positive))
        if positive.size
        else max(float(np.trace(gram)) / max(1, column_count), 1e-6)
    )
    alpha = scale * max(0.0, float(regularization_factor))
    system = gram.copy()
    if alpha > 0.0:
        system += alpha * np.eye(column_count)
        rhs += alpha * anchor
        for i, j, similarity, desired in _local_profile_pairs(
            support_rates, list(range(column_count)),
            correct_scores[np.asarray(support, dtype=int)], tail_l1_weight,
        ):
            # Within one support matrix the local indices are 0..k-1.
            graph_alpha = 6.0 * alpha * similarity
            system[i, i] += graph_alpha
            system[j, j] += graph_alpha
            system[i, j] -= graph_alpha
            system[j, i] -= graph_alpha
            rhs[i] += graph_alpha * desired
            rhs[j] -= graph_alpha * desired

    kkt = np.zeros((column_count + 1, column_count + 1), dtype=float)
    kkt[:column_count, :column_count] = system
    kkt[:column_count, column_count] = 1.0
    kkt[column_count, :column_count] = 1.0
    target = np.r_[rhs, tail_mass]
    try:
        solution = np.linalg.solve(kkt, target)
    except np.linalg.LinAlgError:
        solution = np.linalg.lstsq(kkt, target, rcond=1e-12)[0]
    additions = np.asarray(solution[:column_count], dtype=float)
    additions -= (float(np.sum(additions)) - tail_mass) / column_count
    return additions


def _positive_affine_fit(
    raw_scores: np.ndarray,
    correct_scores: np.ndarray,
) -> tuple[float, float]:
    raw_mean = float(np.mean(raw_scores))
    correct_mean = float(np.mean(correct_scores))
    centered_raw = raw_scores - raw_mean
    denominator = float(np.dot(centered_raw, centered_raw))
    if denominator <= 1e-14:
        return 1e-8, correct_mean - 1e-8 * raw_mean
    slope = float(np.dot(centered_raw, correct_scores - correct_mean)) / denominator
    slope = max(slope, 1e-8)
    intercept = correct_mean - slope * raw_mean
    return slope, intercept


def _rankdata_average(values: np.ndarray) -> np.ndarray:
    order = np.argsort(values, kind="stable")
    ranks = np.empty(values.size, dtype=float)
    pos = 0
    while pos < values.size:
        end = pos + 1
        while end < values.size and values[order[end]] == values[order[pos]]:
            end += 1
        average_rank = 0.5 * (pos + end - 1)
        ranks[order[pos:end]] = average_rank
        pos = end
    return ranks


def _spearman(a: np.ndarray, b: np.ndarray) -> float | None:
    return corrcoef(_rankdata_average(a), _rankdata_average(b))


def _solve_affine_soft_weights(
    support_rates: np.ndarray,
    reference_scores: np.ndarray,
    structural_prior: np.ndarray,
    total_weight: float,
    regularization_factor: float | None,
    equal_positions: list[int] | None = None,
) -> tuple[np.ndarray, float, float]:
    """Fit direct target weights after positive-affine score alignment.

    C-Score order appears only through the smooth structural prior.  It is not
    a feasibility constraint: matchup structure may legitimately make a
    lower-C-Score target heavier than a higher-C-Score target.
    """
    k = structural_prior.size
    if k <= 1 or regularization_factor is None:
        raw = support_rates @ structural_prior / 50.0
        slope, intercept = _positive_affine_fit(raw, reference_scores)
        return structural_prior.copy(), slope, intercept

    rates = support_rates / 50.0
    identity = np.eye(k, dtype=float)
    first_difference = np.zeros((max(0, k - 1), k), dtype=float)
    for pos in range(k - 1):
        first_difference[pos, pos] = 1.0
        first_difference[pos, pos + 1] = -1.0
    second_difference = np.zeros((max(0, k - 2), k), dtype=float)
    for pos in range(k - 2):
        second_difference[pos, pos] = 1.0
        second_difference[pos, pos + 1] = -2.0
        second_difference[pos, pos + 2] = 1.0

    data_scale = max(
        float(np.trace(rates.T @ rates))
        / max(1, rates.shape[0] * rates.shape[1]),
        1e-9,
    )
    alpha = max(0.0, float(regularization_factor)) * data_scale
    weights = structural_prior.copy()
    slope, intercept = _positive_affine_fit(
        rates @ weights, reference_scores,
    )
    for _ in range(8):
        rows = [slope * rates / math.sqrt(max(1, rates.shape[0]))]
        targets = [
            (
                reference_scores
                - intercept
            ) / math.sqrt(max(1, rates.shape[0]))
        ]
        if alpha > 0.0:
            rows.append(math.sqrt(alpha / max(1, k)) * identity)
            targets.append(
                math.sqrt(alpha / max(1, k)) * structural_prior
            )
            if first_difference.shape[0]:
                rows.append(
                    math.sqrt(0.55 * alpha / first_difference.shape[0])
                    * first_difference
                )
                targets.append(
                    math.sqrt(0.55 * alpha / first_difference.shape[0])
                    * (first_difference @ structural_prior)
                )
            if second_difference.shape[0]:
                rows.append(
                    math.sqrt(0.20 * alpha / second_difference.shape[0])
                    * second_difference
                )
                targets.append(np.zeros(second_difference.shape[0], dtype=float))
        design = np.vstack(rows)
        target = np.concatenate(targets)
        gram = design.T @ design
        rhs = design.T @ target
        # The support is ordered by descending C-Score.  On the unlocked tail
        # use two continuous priors: stay near the inherited anchor and prefer
        # a small positive adjacent C-Score slope.  Neither is a hard order;
        # matchup evidence may retain local inversions.
        tail_start = min(30, k)
        tail_count = k - tail_start
        tail_regularization_scale = (
            1.0 if regularization_factor is None else min(
                1.0, max(0.0, float(regularization_factor)) / 0.01,
            )
        )
        if tail_count > 0:
            tail_selector = np.zeros((tail_count, k), dtype=float)
            for row_pos, pos in enumerate(range(tail_start, k)):
                tail_selector[row_pos, pos] = 1.0
            tail_anchor_beta = (
                0.02 * tail_regularization_scale * data_scale
            )
            gram += (
                tail_anchor_beta / tail_count
                * (tail_selector.T @ tail_selector)
            )
            rhs += (
                tail_anchor_beta / tail_count
                * tail_selector.T
                @ structural_prior[tail_start:]
            )
        if tail_count > 1:
            tail_difference = np.zeros((tail_count - 1, k), dtype=float)
            for row_pos, pos in enumerate(range(tail_start, k - 1)):
                tail_difference[row_pos, pos] = 1.0
                tail_difference[row_pos, pos + 1] = -1.0
            desired_gap = 0.025 * max(
                abs(float(np.mean(structural_prior[tail_start:]))), 1e-6,
            )
            tail_order_beta = (
                0.03 * tail_regularization_scale * data_scale
            )
            gram += (
                tail_order_beta / (tail_count - 1)
                * (tail_difference.T @ tail_difference)
            )
            rhs += (
                tail_order_beta / (tail_count - 1)
                * tail_difference.T
                @ np.full(tail_count - 1, desired_gap, dtype=float)
            )
        # Golden=1 is a soft cohesion prior, not an exact equality.  Penalize
        # deviations from the block mean while retaining only mass as a hard
        # constraint, so matchup evidence may move individual unit rows.
        positions = list(equal_positions or ())
        if len(positions) > 1:
            cohesion = np.zeros((len(positions), k), dtype=float)
            for row_pos, pos in enumerate(positions):
                cohesion[row_pos, pos] = 1.0
                cohesion[row_pos, positions] -= 1.0 / len(positions)
            beta = max(0.35 * alpha, 0.08 * data_scale)
            gram += beta / len(positions) * (cohesion.T @ cohesion)
        equality_rows = [np.ones(k, dtype=float)]
        equality_targets = [float(total_weight)]
        constraints = np.vstack(equality_rows)
        constraint_targets = np.asarray(equality_targets, dtype=float)
        constraint_count = constraints.shape[0]
        kkt = np.zeros(
            (k + constraint_count, k + constraint_count), dtype=float,
        )
        kkt[:k, :k] = gram
        kkt[:k, k:] = constraints.T
        kkt[k:, :k] = constraints
        kkt_target = np.r_[rhs, constraint_targets]
        try:
            solution = np.linalg.solve(kkt, kkt_target)
        except np.linalg.LinAlgError:
            solution = np.linalg.lstsq(
                kkt, kkt_target, rcond=1e-12,
            )[0]
        next_weights = np.asarray(solution[:k], dtype=float)
        next_weights -= (
            float(np.sum(next_weights)) - total_weight
        ) / max(1, k)
        next_slope, next_intercept = _positive_affine_fit(
            rates @ next_weights, reference_scores,
        )
        change = float(np.linalg.norm(next_weights - weights))
        weights = next_weights
        slope = next_slope
        intercept = next_intercept
        if change <= 1e-10 * max(1.0, float(np.linalg.norm(weights))):
            break
    if abs(float(np.sum(weights)) - total_weight) > 1e-7:
        raise RuntimeError("affine soft-order fit changed total target mass")
    return weights, slope, intercept


def _affine_soft_structural_prior(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
) -> tuple[np.ndarray, np.ndarray, dict[str, float]]:
    additions, transfers, transport = _type_balanced_transport_anchor(
        rate_matrix, big_weights, support, correct_scores,
    )
    support_array = np.asarray(support, dtype=int)
    inherited_and_transported = big_weights[support_array] + additions
    order = np.asarray(
        sorted(
            range(len(support)),
            key=lambda pos: (
                -float(correct_scores[support[pos]]),
                int(support[pos]),
            ),
        ),
        dtype=int,
    )
    ordered_scores = correct_scores[support_array[order]]
    total_weight = float(np.sum(big_weights))
    ordered_anchor = inherited_and_transported[order]
    mean_weight = total_weight / max(1, len(support))
    score_range = max(
        float(ordered_scores[0] - ordered_scores[-1]), 1e-9,
    )
    score_curve = (
        ordered_scores - float(np.mean(ordered_scores))
    ) / score_range
    # C-Score is a soft prior only.  Across the full board its contribution is
    # deliberately mild; the retained matchup-type signal may locally reverse
    # the order when the response columns justify it.
    strength_preference_range = 0.15 * max(abs(mean_weight), 1e-6)
    strength_prior = mean_weight + strength_preference_range * score_curve
    type_signal_retention = 0.35
    type_residual = ordered_anchor - mean_weight
    structural_prior = (
        strength_prior + type_signal_retention * type_residual
    )
    structural_prior += (
        total_weight - float(np.sum(structural_prior))
    ) / len(structural_prior)
    transport = {
        **transport,
        "pre_soft_order_weight_std": float(np.std(ordered_anchor, ddof=0)),
        "soft_order_prior_weight_std": float(np.std(structural_prior, ddof=0)),
        "soft_order_prior_weight_min": float(np.min(structural_prior)),
        "soft_order_prior_weight_max": float(np.max(structural_prior)),
        "strength_preference_range": strength_preference_range,
        "type_signal_retention": type_signal_retention,
        "soft_order_projection_l2": float(
            np.linalg.norm(structural_prior - ordered_anchor)
        ),
        "transport_matrix_l1": float(np.sum(np.abs(transfers))),
    }
    return order, structural_prior, transport


def _mass_constrained_ridge_fit(
    design: np.ndarray,
    target: np.ndarray,
    anchor: np.ndarray,
    total_mass: float,
    regularization_factor: float | None,
    equal_positions: list[int] | None = None,
) -> np.ndarray:
    """Fit one weight block while preserving its exact signed mass."""
    k = anchor.size
    if k == 0:
        return np.zeros(0, dtype=float)
    if regularization_factor is None:
        out = anchor.copy()
        out += (total_mass - float(np.sum(out))) / k
        return out
    normalized_design = design / math.sqrt(max(1, design.shape[0]))
    normalized_target = target / math.sqrt(max(1, design.shape[0]))
    gram = normalized_design.T @ normalized_design
    rhs = normalized_design.T @ normalized_target
    scale = max(
        float(np.trace(gram)) / max(1, k),
        1e-9,
    )
    alpha = max(0.0, float(regularization_factor)) * scale
    if alpha > 0.0:
        gram += alpha / max(1, k) * np.eye(k)
        rhs += alpha / max(1, k) * anchor
        if k > 1:
            first = np.zeros((k - 1, k), dtype=float)
            for pos in range(k - 1):
                first[pos, pos] = 1.0
                first[pos, pos + 1] = -1.0
            graph = first.T @ first
            gram += 0.40 * alpha / (k - 1) * graph
            rhs += 0.40 * alpha / (k - 1) * graph @ anchor
        if k > 2:
            second = np.zeros((k - 2, k), dtype=float)
            for pos in range(k - 2):
                second[pos, pos] = 1.0
                second[pos, pos + 1] = -2.0
                second[pos, pos + 2] = 1.0
            gram += 0.12 * alpha / (k - 2) * (second.T @ second)

    positions = list(equal_positions or ())
    if len(positions) > 1:
        cohesion = np.zeros((len(positions), k), dtype=float)
        for row_pos, pos in enumerate(positions):
            cohesion[row_pos, pos] = 1.0
            cohesion[row_pos, positions] -= 1.0 / len(positions)
        beta = max(0.35 * alpha, 0.08 * scale)
        gram += beta / len(positions) * (cohesion.T @ cohesion)
    equality_rows = [np.ones(k, dtype=float)]
    equality_targets = [float(total_mass)]
    constraints = np.vstack(equality_rows)
    targets = np.asarray(equality_targets, dtype=float)
    count = constraints.shape[0]
    kkt = np.zeros((k + count, k + count), dtype=float)
    kkt[:k, :k] = gram
    kkt[:k, k:] = constraints.T
    kkt[k:, :k] = constraints
    rhs_kkt = np.r_[rhs, targets]
    try:
        solution = np.linalg.solve(kkt, rhs_kkt)
    except np.linalg.LinAlgError:
        solution = np.linalg.lstsq(kkt, rhs_kkt, rcond=1e-12)[0]
    weights = np.asarray(solution[:k], dtype=float)
    weights += (total_mass - float(np.sum(weights))) / k
    return weights


def _locked_big_target_structural_prior(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    support: list[int],
    locked: set[int],
    correct_scores: np.ndarray,
    target_total: int | None = None,
    use_selected_base_proportion: bool = True,
    absorption_priority_indices: set[int] | None = None,
) -> tuple[np.ndarray, np.ndarray, dict[str, Any]]:
    """Build initial weights from own Correct base plus Golden=1 balancing.

    The production path equalizes the complete weights of selected Golden=1
    rows.  Golden!=1 rows keep their own base in the anchor and remain free in
    the forward fit; the fixed-front C-Score addition preference is applied
    only to that free block.
    During greedy partial-support evaluation the legacy branch may still be
    requested explicitly, keeping support selection independent from the final
    absorption baseline.
    """
    support_array = np.asarray(support, dtype=int)
    locked_positions = [
        pos for pos, idx in enumerate(support) if idx in locked
    ]
    unlocked_positions = [
        pos for pos, idx in enumerate(support) if idx not in locked
    ]
    locked_indices = np.asarray(
        [support[pos] for pos in locked_positions], dtype=int,
    )
    unlocked_indices = np.asarray(
        [support[pos] for pos in unlocked_positions], dtype=int,
    )
    total_weight = float(np.sum(big_weights))
    locked_original_weight_sum = float(np.sum(big_weights[locked_indices]))
    unlocked_original_weight_sum = float(
        np.sum(big_weights[unlocked_indices])
    )
    selected_original_weight_sum = (
        locked_original_weight_sum + unlocked_original_weight_sum
    )
    target_slot_count = (
        len(support) if target_total is None else int(target_total)
    )
    locked_slot_fraction = len(locked_positions) / max(1, target_slot_count)
    if absorption_priority_indices is not None:
        additions, transfers, transport = (
            _golden_unit_balanced_transport_anchor(
                big_weights,
                golden_weights,
                support,
                correct_scores,
                priority_indices=absorption_priority_indices,
            )
        )
        prior = big_weights[support_array] + additions
        if abs(float(np.sum(prior)) - total_weight) > 1e-8:
            raise RuntimeError(
                "collective support absorption changed big-target mass"
            )
        order = np.asarray(
            sorted(
                range(len(support)),
                key=lambda pos: (
                    -float(correct_scores[support[pos]]),
                    int(support[pos]),
                ),
            ),
            dtype=int,
        )
        full_response = rate_matrix @ big_weights / 50.0
        seed_response = rate_matrix[:, support_array] @ prior / 50.0
        seed_diff = seed_response - full_response
        locked_seed = prior[np.asarray(locked_positions, dtype=int)]
        locked_base = big_weights[locked_indices]
        return order, prior[order], {
            **transport,
            "locked_anchor_count": len(locked_positions),
            "unlocked_anchor_count": len(unlocked_positions),
            "locked_anchor_original_weight_sum": locked_original_weight_sum,
            "locked_anchor_weight_sum": float(np.sum(locked_seed)),
            "locked_anchor_weight_scale": 1.0,
            "locked_slot_fraction": locked_slot_fraction,
            "locked_anchor_max_abs_diff": (
                float(np.max(np.abs(locked_seed - locked_base)))
                if locked_positions else 0.0
            ),
            "selected_original_weight_sum": selected_original_weight_sum,
            "unlocked_original_weight_sum": unlocked_original_weight_sum,
            "seed_mass_partition_rule": (
                "selected_big_target_base_plus_golden_unit_equalization_with_nonunit_free_fit"
            ),
            "remaining_mass_fitted_to_unlocked": float(np.sum(additions)),
            "initial_weight_fit_factor": None,
            "initial_weight_big_replay_rmse": float(
                np.sqrt(np.mean(seed_diff * seed_diff))
            ),
            "initial_weight_big_replay_max_abs_diff": float(
                np.max(np.abs(seed_diff))
            ),
            "initial_weight_std": float(np.std(prior, ddof=0)),
            "initial_weight_min": float(np.min(prior)),
            "initial_weight_max": float(np.max(prior)),
            "initial_weight_c_score_spearman": _spearman(
                prior, correct_scores[support_array],
            ),
            "initial_weight_std_limit": None,
            "initial_weight_max_deviation_limit": None,
            "strength_preference_range": 0.0,
            "type_signal_retention": 1.0,
            "transport_matrix_l1": float(np.sum(np.abs(transfers))),
        }
    if (
        use_selected_base_proportion
        and abs(selected_original_weight_sum) > 1e-12
    ):
        locked_target_weight_sum = (
            total_weight
            * locked_original_weight_sum
            / selected_original_weight_sum
        )
        seed_mass_partition_rule = (
            "selected_big_target_base_weights_rescaled_proportionally"
        )
    else:
        locked_target_weight_sum = total_weight * locked_slot_fraction
        seed_mass_partition_rule = (
            "locked_total_equals_big_total_times_locked_count_over_target_count"
        )
    if locked_positions and abs(locked_original_weight_sum) > 1e-12:
        locked_weight_scale = (
            locked_target_weight_sum / locked_original_weight_sum
        )
        locked_seed_weights = (
            big_weights[locked_indices] * locked_weight_scale
        )
    elif locked_positions:
        locked_weight_scale = 0.0
        locked_seed_weights = np.full(
            len(locked_positions),
            locked_target_weight_sum / len(locked_positions),
            dtype=float,
        )
    else:
        locked_weight_scale = 1.0
        locked_seed_weights = np.zeros(0, dtype=float)
    remaining_mass = total_weight - locked_target_weight_sum
    prior = np.zeros(len(support), dtype=float)
    if locked_positions:
        prior[np.asarray(locked_positions, dtype=int)] = locked_seed_weights
    if not unlocked_positions:
        if abs(remaining_mass) > 1e-8:
            raise ValueError(
                "locked rows fill the support but do not carry all big-target mass"
            )
        order = np.asarray(
            sorted(
                range(len(support)),
                key=lambda pos: (
                    -float(correct_scores[support[pos]]),
                    int(support[pos]),
                ),
            ),
            dtype=int,
        )
        return order, prior[order], {
            "locked_anchor_count": len(locked_positions),
            "unlocked_anchor_count": 0,
            "locked_anchor_original_weight_sum": locked_original_weight_sum,
            "locked_anchor_weight_sum": locked_target_weight_sum,
            "locked_anchor_weight_scale": locked_weight_scale,
            "locked_slot_fraction": locked_slot_fraction,
            "locked_anchor_max_abs_diff": 0.0,
            "selected_original_weight_sum": selected_original_weight_sum,
            "unlocked_original_weight_sum": unlocked_original_weight_sum,
            "seed_mass_partition_rule": seed_mass_partition_rule,
        }

    # Remove locked mass from the transport universe: it is already represented
    # exactly and must not be redistributed onto the new rows.
    transport_weights = big_weights.copy()
    if locked_indices.size:
        transport_weights[locked_indices] = 0.0
    transported, transfers, transport = _type_balanced_transport_anchor(
        rate_matrix,
        transport_weights,
        unlocked_indices.tolist(),
        correct_scores,
    )
    transported_unlocked = (
        transport_weights[unlocked_indices] + transported
    )
    transported_sum = float(np.sum(transported_unlocked))
    if abs(transported_sum) > 1e-12:
        transported_unlocked *= remaining_mass / transported_sum
    else:
        transported_unlocked.fill(
            remaining_mass / len(unlocked_indices)
        )

    unlocked_order = np.asarray(
        sorted(
            range(len(unlocked_indices)),
            key=lambda pos: (
                -float(correct_scores[unlocked_indices[pos]]),
                int(unlocked_indices[pos]),
            ),
        ),
        dtype=int,
    )
    ordered_unlocked = unlocked_indices[unlocked_order]
    ordered_scores = correct_scores[ordered_unlocked]
    ordered_transport = transported_unlocked[unlocked_order]
    mean_unlocked = remaining_mass / max(1, len(unlocked_indices))
    score_range = max(
        float(ordered_scores[0] - ordered_scores[-1]), 1e-9,
    )
    score_curve = (
        ordered_scores - float(np.mean(ordered_scores))
    ) / score_range
    strength_preference_range = 0.15 * max(abs(mean_unlocked), 1e-6)
    type_signal_retention = 0.55
    ordered_anchor = (
        mean_unlocked
        + strength_preference_range * score_curve
        + type_signal_retention * (ordered_transport - mean_unlocked)
    )
    ordered_anchor += (
        remaining_mass - float(np.sum(ordered_anchor))
    ) / len(ordered_anchor)

    full_response = rate_matrix @ big_weights / 50.0
    locked_response = (
        rate_matrix[:, locked_indices] @ locked_seed_weights / 50.0
        if locked_indices.size
        else np.zeros(rate_matrix.shape[0], dtype=float)
    )
    residual_target = full_response - locked_response
    unlocked_design = rate_matrix[:, ordered_unlocked] / 50.0
    candidates: list[tuple[np.ndarray, dict[str, float | None]]] = []
    for factor in [0.3, 1.0, 3.0, 10.0, 30.0, None]:
        weights = _mass_constrained_ridge_fit(
            unlocked_design,
            residual_target,
            ordered_anchor,
            remaining_mass,
            factor,
        )
        full_weights = prior.copy()
        mapped = np.empty_like(weights)
        mapped[unlocked_order] = weights
        full_weights[np.asarray(unlocked_positions, dtype=int)] = mapped
        replay = (
            rate_matrix[:, support_array] @ full_weights / 50.0
            - full_response
        )
        candidates.append((weights, {
            "factor": None if factor is None else float(factor),
            "rmse": float(np.sqrt(np.mean(replay * replay))),
            "max_abs_diff": float(np.max(np.abs(replay))),
            "std": float(np.std(full_weights, ddof=0)),
            "max_deviation": float(
                np.max(np.abs(full_weights - float(np.mean(full_weights))))
            ),
        }))
    prior_metrics = candidates[-1][1]
    std_limit = max(
        1.35 * float(prior_metrics["std"]),
        0.18 * max(abs(total_weight / len(support)), 1e-6),
    )
    deviation_limit = max(
        1.35 * float(prior_metrics["max_deviation"]),
        0.40 * max(abs(total_weight / len(support)), 1e-6),
    )
    stable = [
        item for item in candidates
        if float(item[1]["std"]) <= std_limit + 1e-10
        and float(item[1]["max_deviation"]) <= deviation_limit + 1e-10
    ]
    if not stable:
        stable = [candidates[-1]]
    selected_ordered_weights, selected_metrics = min(
        stable,
        key=lambda item: (
            float(item[1]["rmse"]) + 0.20 * float(item[1]["max_abs_diff"]),
            float(item[1]["std"]),
        ),
    )
    selected_unlocked = np.empty_like(selected_ordered_weights)
    selected_unlocked[unlocked_order] = selected_ordered_weights
    prior[np.asarray(unlocked_positions, dtype=int)] = selected_unlocked
    prior += (total_weight - float(np.sum(prior))) / len(unlocked_positions) * np.asarray(
        [0.0 if idx in locked else 1.0 for idx in support],
        dtype=float,
    )
    locked_diff = (
        np.max(np.abs(
            prior[np.asarray(locked_positions, dtype=int)]
            - locked_seed_weights
        ))
        if locked_positions else 0.0
    )
    if locked_diff > 1e-10:
        raise RuntimeError(
            "locked seed weights no longer equal scaled big-target weights"
        )
    order = np.asarray(
        sorted(
            range(len(support)),
            key=lambda pos: (
                -float(correct_scores[support[pos]]),
                int(support[pos]),
            ),
        ),
        dtype=int,
    )
    return order, prior[order], {
        **transport,
        "locked_anchor_count": len(locked_positions),
        "unlocked_anchor_count": len(unlocked_positions),
        "locked_anchor_original_weight_sum": locked_original_weight_sum,
        "locked_anchor_weight_sum": locked_target_weight_sum,
        "locked_anchor_weight_scale": locked_weight_scale,
        "locked_slot_fraction": locked_slot_fraction,
        "locked_anchor_max_abs_diff": float(locked_diff),
        "selected_original_weight_sum": selected_original_weight_sum,
        "unlocked_original_weight_sum": unlocked_original_weight_sum,
        "seed_mass_partition_rule": seed_mass_partition_rule,
        "remaining_mass_fitted_to_unlocked": remaining_mass,
        "initial_weight_fit_factor": selected_metrics["factor"],
        "initial_weight_big_replay_rmse": selected_metrics["rmse"],
        "initial_weight_big_replay_max_abs_diff": selected_metrics[
            "max_abs_diff"
        ],
        "initial_weight_std": float(np.std(prior, ddof=0)),
        "initial_weight_min": float(np.min(prior)),
        "initial_weight_max": float(np.max(prior)),
        "initial_weight_c_score_spearman": _spearman(
            prior, correct_scores[support_array],
        ),
        "initial_weight_std_limit": std_limit,
        "initial_weight_max_deviation_limit": deviation_limit,
        "strength_preference_range": strength_preference_range,
        "type_signal_retention": type_signal_retention,
        "transport_matrix_l1": float(np.sum(np.abs(transfers))),
    }


def _select_big_target_support_from_locked(
    locked_list: list[int],
    eligible: list[int],
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
    target_total: int,
    group_ids: list[int],
) -> tuple[list[int], list[dict[str, Any]]]:
    """Fill unlocked slots by direct flattened C-Score forward replay.

    A small beam is retained because the best next column can depend on which
    other supplemental columns are added later.  Every proposed partial
    support receives its production Golden=1 equalized anchor before its
    weighted rates are aligned to C-Score.  Profile coverage and support
    strength are only deterministic tie-breakers.
    """
    selected = list(locked_list)
    if not _support_feasible(
        selected, account_keys, owner_keys, owner_cap,
    ):
        raise ValueError("locked Top3 intersection is not a feasible support")
    big_eligible = [
        idx for idx in eligible
        if abs(float(big_weights[idx])) > 1e-15
        and idx not in set(selected)
    ]
    if len(selected) > target_total:
        raise ValueError("locked Top3 intersection exceeds target count")
    normalized, means = _profile_geometry(rate_matrix)
    demand = np.asarray(
        [
            idx for idx in range(big_weights.size)
            if abs(float(big_weights[idx])) > 1e-15
        ],
        dtype=int,
    )
    demand_mass = np.abs(big_weights[demand])
    demand_mass /= max(float(np.sum(demand_mass)), 1e-12)
    candidate_array = np.asarray(big_eligible, dtype=int)
    all_distances = _profile_distance_block(
        normalized, means, demand, candidate_array,
    )
    candidate_position = {
        int(idx): pos for pos, idx in enumerate(candidate_array.tolist())
    }
    if selected:
        initial_nearest_distance = np.min(
            _profile_distance_block(
                normalized,
                means,
                demand,
                np.asarray(selected, dtype=int),
            ),
            axis=1,
        )
    else:
        initial_nearest_distance = np.full(demand.size, 4.0, dtype=float)

    def provisional_metrics(
        support: list[int],
        project_priority: bool = True,
    ) -> dict[str, float]:
        support_array = np.asarray(support, dtype=int)
        priority = set(sorted(
            support,
            key=lambda idx: (
                -float(correct_scores[idx]),
                int(group_ids[idx]),
            ),
        )[:min(30, len(support))])
        additions, _transfers, _transport = (
            _golden_unit_balanced_transport_anchor(
                big_weights,
                golden_weights,
                support,
                correct_scores,
                priority_indices=priority if project_priority else None,
            )
        )
        anchor = big_weights[support_array] + additions
        direct_scores = rate_matrix[:, support_array] @ anchor / 50.0
        slope, intercept, diffs = affine_chebyshev_fit(
            direct_scores, reference_scores,
        )
        abs_diff = np.abs(diffs)
        mean_abs_diff = float(np.mean(abs_diff))
        max_abs_diff = float(np.max(abs_diff))
        rmse = float(np.sqrt(np.mean(diffs * diffs)))
        return {
            "aligned_mean_abs_diff": mean_abs_diff,
            "aligned_max_abs_diff": max_abs_diff,
            "aligned_rmse": rmse,
            "affine_slope": float(slope),
            "affine_intercept": float(intercept),
            "error_score": (
                0.70 * max_abs_diff
                + 0.20 * rmse
                + 0.10 * mean_abs_diff
            ),
        }

    beam: list[dict[str, Any]] = [{
        "support": list(selected),
        "nearest_distance": initial_nearest_distance,
        "added": [],
        "metrics": (
            provisional_metrics(selected)
            if selected else {
                "aligned_mean_abs_diff": float("inf"),
                "aligned_max_abs_diff": float("inf"),
                "aligned_rmse": float("inf"),
                "affine_slope": 1.0,
                "affine_intercept": 0.0,
                "error_score": float("inf"),
            }
        ),
    }]
    beam_width = 8
    while len(beam[0]["support"]) < target_total:
        expanded_by_signature: dict[tuple[int, ...], dict[str, Any]] = {}
        for state in beam:
            current_support = list(state["support"])
            current_set = set(current_support)
            feasible = [
                idx for idx in big_eligible
                if idx not in current_set
                and _support_feasible(
                    current_support + [idx],
                    account_keys,
                    owner_keys,
                    owner_cap,
                )
            ]
            for idx in feasible:
                pos = candidate_position[idx]
                next_distance = np.minimum(
                    state["nearest_distance"],
                    all_distances[:, pos],
                )
                support = current_support + [idx]
                # Cheaply screen every legal branch first.  The front-prefix
                # projection is then evaluated on a bounded shortlist below;
                # invoking its constrained least-squares solve on thousands
                # of obviously inferior branches would waste calibration CPU.
                metrics = provisional_metrics(
                    support, project_priority=False,
                )
                row = {
                    "support": support,
                    "nearest_distance": next_distance,
                    "added": list(state["added"]) + [idx],
                    "metrics": metrics,
                    "remaining_weighted_profile_distance": float(np.dot(
                        demand_mass, next_distance,
                    )),
                    "support_c_score_sum": float(np.sum(
                        correct_scores[np.asarray(support, dtype=int)]
                    )),
                }
                signature = tuple(sorted(support))
                previous = expanded_by_signature.get(signature)
                if previous is None or (
                    float(row["metrics"]["error_score"]),
                    float(row["metrics"]["aligned_max_abs_diff"]),
                    float(row["remaining_weighted_profile_distance"]),
                ) < (
                    float(previous["metrics"]["error_score"]),
                    float(previous["metrics"]["aligned_max_abs_diff"]),
                    float(previous["remaining_weighted_profile_distance"]),
                ):
                    expanded_by_signature[signature] = row
        if not expanded_by_signature:
            raise ValueError(
                "C-Score-aware support search cannot fill Top50 under account "
                "uniqueness and merged-team cap constraints"
            )
        fast_shortlist = sorted(
            expanded_by_signature.values(),
            key=lambda row: (
                float(row["metrics"]["error_score"]),
                float(row["metrics"]["aligned_max_abs_diff"]),
                float(row["metrics"]["aligned_mean_abs_diff"]),
                float(row["remaining_weighted_profile_distance"]),
                -float(row["support_c_score_sum"]),
                tuple(sorted(row["support"])),
            ),
        )[:32]
        for row in fast_shortlist:
            row["metrics"] = provisional_metrics(
                list(row["support"]), project_priority=True,
            )
        beam = sorted(
            fast_shortlist,
            key=lambda row: (
                float(row["metrics"]["error_score"]),
                float(row["metrics"]["aligned_max_abs_diff"]),
                float(row["metrics"]["aligned_mean_abs_diff"]),
                float(row["remaining_weighted_profile_distance"]),
                -float(row["support_c_score_sum"]),
                tuple(sorted(row["support"])),
            ),
        )[:beam_width]

    chosen_state = min(
        beam,
        key=lambda row: (
            float(row["metrics"]["error_score"]),
            float(row["metrics"]["aligned_max_abs_diff"]),
            float(row["metrics"]["aligned_mean_abs_diff"]),
            float(row["remaining_weighted_profile_distance"]),
            -float(row["support_c_score_sum"]),
        ),
    )
    selected = list(chosen_state["support"])
    steps: list[dict[str, Any]] = []
    replay_selected = list(locked_list)
    replay_nearest = initial_nearest_distance.copy()
    for chosen_idx in chosen_state["added"]:
        previous_distance = replay_nearest
        replay_selected.append(int(chosen_idx))
        replay_nearest = np.minimum(
            replay_nearest,
            all_distances[:, candidate_position[int(chosen_idx)]],
        )
        metrics = provisional_metrics(replay_selected)
        steps.append({
            "step": len(steps) + 1,
            "added_index": int(chosen_idx),
            "added_group_id": int(group_ids[int(chosen_idx)]),
            "coverage_gain": float(np.dot(
                demand_mass, previous_distance - replay_nearest,
            )),
            "correct_score": float(correct_scores[int(chosen_idx)]),
            "big_target_weight": float(big_weights[chosen_idx]),
            "selection_score": float(metrics["error_score"]),
            "provisional_aligned_mean_abs_diff": float(
                metrics["aligned_mean_abs_diff"]
            ),
            "provisional_aligned_max_abs_diff": float(
                metrics["aligned_max_abs_diff"]
            ),
            "provisional_aligned_rmse": float(
                metrics["aligned_rmse"]
            ),
            "remaining_weighted_profile_distance": float(np.dot(
                demand_mass, replay_nearest,
            )),
        })
    if not _support_feasible(
        selected,
        account_keys,
        owner_keys,
        owner_cap,
        target_total,
    ):
        raise RuntimeError("C-Score-aware seed returned an infeasible support")
    return selected, steps


def _inherited_compression_metrics(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    reference_scores: np.ndarray,
    support: list[int],
    final_weights: np.ndarray,
    structural_prior: np.ndarray,
    correct_scores: np.ndarray,
    affine_slope: float,
    affine_intercept: float,
    score_denominator: float = 50.0,
    full_response_denominator: float = 50.0,
) -> dict[str, float | int | None]:
    support_array = np.asarray(support, dtype=int)
    base = big_weights[support_array]
    additions = final_weights - base
    full_response = rate_matrix @ big_weights / full_response_denominator
    compressed_response = (
        rate_matrix[:, support_array] @ final_weights / score_denominator
    )
    aligned_response = affine_slope * compressed_response + affine_intercept
    flattened_correct = (reference_scores - affine_intercept) / affine_slope
    big_diff = compressed_response - full_response
    correct_diff = compressed_response - reference_scores
    aligned_diff = aligned_response - reference_scores
    flat_diff = compressed_response - flattened_correct
    tail_indices = np.asarray(
        [idx for idx in range(big_weights.size) if idx not in set(support)],
        dtype=int,
    )
    pairs = _local_profile_pairs(
        rate_matrix[:, support_array],
        list(range(len(support))),
        correct_scores[support_array],
        float(np.sum(np.abs(big_weights[tail_indices]))),
    )
    pair_diffs = []
    addition_inversions = []
    for i, j, similarity, _desired in pairs:
        pair_diffs.append(similarity * (additions[i] - additions[j]) ** 2)
        score_diff = float(correct_scores[support[i]] - correct_scores[support[j]])
        if abs(score_diff) <= 1e-12:
            continue
        signed_addition_diff = (
            (additions[i] - additions[j]) * math.copysign(1.0, score_diff)
        )
        addition_inversions.append(max(0.0, -signed_addition_diff))
    score_order = np.asarray(
        sorted(
            range(len(support)),
            key=lambda pos: (-float(correct_scores[support[pos]]), int(support[pos])),
        ),
        dtype=int,
    )
    ordered_weights = final_weights[score_order]
    monotonic_violations = np.maximum(
        ordered_weights[1:] - ordered_weights[:-1], 0.0,
    )
    adjacent_gaps = ordered_weights[:-1] - ordered_weights[1:]
    second_gap = np.diff(adjacent_gaps) if adjacent_gaps.size > 1 else np.zeros(0)
    tail_ordered_weights = ordered_weights[min(30, ordered_weights.size):]
    tail_violations = np.maximum(
        tail_ordered_weights[1:] - tail_ordered_weights[:-1], 0.0,
    )
    tail_slot_penalty = float(np.sum(np.maximum(
        0.75 - tail_ordered_weights, 0.0,
    ) ** 2))
    addition_l1 = float(np.sum(np.abs(additions)))
    addition_signed = abs(float(np.sum(additions)))
    weight_l1 = float(np.sum(np.abs(final_weights)))
    weight_signed = abs(float(np.sum(final_weights)))
    abs_big = np.abs(big_diff)
    abs_correct = np.abs(correct_diff)
    abs_aligned = np.abs(aligned_diff)
    abs_flat = np.abs(flat_diff)
    return {
        "big_mean_abs_diff": float(np.mean(abs_big)),
        "big_max_abs_diff": float(np.max(abs_big)),
        "big_p95_abs_diff": float(np.quantile(abs_big, 0.95)),
        "big_rmse": float(np.sqrt(np.mean(big_diff * big_diff))),
        "correct_mean_abs_diff": float(np.mean(abs_correct)),
        "correct_max_abs_diff": float(np.max(abs_correct)),
        "correct_p95_abs_diff": float(np.quantile(abs_correct, 0.95)),
        "correct_rmse": float(np.sqrt(np.mean(correct_diff * correct_diff))),
        "aligned_mean_abs_diff": float(np.mean(abs_aligned)),
        "aligned_max_abs_diff": float(np.max(abs_aligned)),
        "aligned_p95_abs_diff": float(np.quantile(abs_aligned, 0.95)),
        "aligned_rmse": float(np.sqrt(np.mean(aligned_diff * aligned_diff))),
        "flat_raw_mean_abs_diff": float(np.mean(abs_flat)),
        "flat_raw_max_abs_diff": float(np.max(abs_flat)),
        "flat_raw_rmse": float(np.sqrt(np.mean(flat_diff * flat_diff))),
        "affine_slope": float(affine_slope),
        "affine_intercept": float(affine_intercept),
        "score_spearman": _spearman(compressed_response, reference_scores),
        "anchor_distance_l2": float(np.linalg.norm(final_weights - structural_prior)),
        "addition_l2": float(np.linalg.norm(additions)),
        "addition_max_abs": float(np.max(np.abs(additions))),
        "similar_addition_rms": (
            float(np.sqrt(np.mean(pair_diffs))) if pair_diffs else 0.0
        ),
        "local_order_inversion_rms": (
            float(np.sqrt(np.mean(np.square(addition_inversions))))
            if addition_inversions else 0.0
        ),
        "local_order_inversion_rate": (
            float(np.mean(np.asarray(addition_inversions) > 1e-12))
            if addition_inversions else 0.0
        ),
        "final_weight_monotonic_violation_count": int(
            np.sum(monotonic_violations > 1e-10)
        ),
        "final_weight_monotonic_violation_max": float(
            np.max(monotonic_violations) if monotonic_violations.size else 0.0
        ),
        "final_weight_c_score_spearman": _spearman(
            final_weights, correct_scores[support_array],
        ),
        "adjacent_weight_gap_rms": float(
            np.sqrt(np.mean(adjacent_gaps * adjacent_gaps))
            if adjacent_gaps.size else 0.0
        ),
        "adjacent_weight_gap_max": float(
            np.max(np.abs(adjacent_gaps)) if adjacent_gaps.size else 0.0
        ),
        "weight_gap_second_difference_rms": float(
            np.sqrt(np.mean(second_gap * second_gap))
            if second_gap.size else 0.0
        ),
        "tail_weight_monotonic_violation_count": int(
            np.sum(tail_violations > 1e-10)
        ),
        "tail_weight_monotonic_violation_rms": float(
            np.sqrt(np.mean(tail_violations * tail_violations))
            if tail_violations.size else 0.0
        ),
        "tail_low_effective_slot_penalty": tail_slot_penalty,
        "addition_cancellation_ratio": addition_l1 / max(addition_signed, 1e-9),
        "weight_cancellation_ratio": weight_l1 / max(weight_signed, 1e-9),
        "final_weight_std": float(np.std(final_weights, ddof=0)),
        "final_weight_min": float(np.min(final_weights)),
        "final_weight_max": float(np.max(final_weights)),
        "final_weight_sum": float(np.sum(final_weights)),
        "compressed_corr_to_big": corrcoef(compressed_response, full_response),
        "compressed_corr_to_correct": corrcoef(compressed_response, reference_scores),
    }


def _support_structure_score(
    metrics: dict[str, Any],
    structural_prior: np.ndarray,
) -> float:
    scale = max(float(np.linalg.norm(structural_prior)), 1e-6)
    mean_abs = max(float(np.mean(np.abs(structural_prior))), 1e-6)
    return (
        float(metrics["anchor_distance_l2"]) / scale
        + 0.35 * float(metrics["similar_addition_rms"]) / mean_abs
        + 0.35 * float(metrics["adjacent_weight_gap_rms"]) / mean_abs
        + 0.25 * float(metrics["weight_gap_second_difference_rms"]) / mean_abs
        + 0.20 * float(metrics["final_weight_monotonic_violation_max"]) / mean_abs
        + 0.02 * float(metrics["final_weight_monotonic_violation_count"])
        / max(1, structural_prior.size)
        + 0.10 * max(0.0, float(metrics["addition_cancellation_ratio"]) - 1.0)
        + 0.10 * max(0.0, float(metrics["weight_cancellation_ratio"]) - 1.0)
        + 0.25 * float(metrics["tail_weight_monotonic_violation_rms"])
        / mean_abs
        + 0.05 * float(metrics["tail_low_effective_slot_penalty"])
        / max(1, structural_prior.size - min(30, structural_prior.size))
    )


def _evaluate_inherited_support(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    support: list[int],
    locked: set[int] | None = None,
    regularization_factors: list[float | None] | None = None,
    absorption_priority_indices: set[int] | None = None,
) -> dict[str, Any]:
    support_array = np.asarray(support, dtype=int)
    base = big_weights[support_array]
    if locked is None:
        order, structural_prior_ordered, transport = (
            _affine_soft_structural_prior(
                rate_matrix, big_weights, support, correct_scores,
            )
        )
        seed_anchor_ordered = structural_prior_ordered.copy()
    else:
        order, seed_anchor_ordered, transport = (
            _locked_big_target_structural_prior(
                rate_matrix,
                big_weights,
                golden_weights,
                support,
                locked,
                correct_scores,
                absorption_priority_indices=absorption_priority_indices,
            )
        )
        ordered_support_for_prior = support_array[order]
        ordered_base_for_prior = big_weights[ordered_support_for_prior]
        ordered_additions_for_prior = (
            seed_anchor_ordered - ordered_base_for_prior
        )
        unlocked_positions = np.asarray([
            pos for pos, idx in enumerate(ordered_support_for_prior)
            if int(idx) not in locked
        ], dtype=int)
        locked_positions = np.asarray([
            pos for pos, idx in enumerate(ordered_support_for_prior)
            if int(idx) in locked
        ], dtype=int)
        # Do not flatten complete weights toward total_mass / support_count.
        # That obsolete centering lifted a 0.5--0.6 supplemental big-target
        # base toward 1 before deletion. Golden=1 rows share one complete
        # weight level, while every Golden!=1 row stays a forward-fit variable.
        # Preserve this audited anchor without global-mean centering.
        structural_prior_ordered = seed_anchor_ordered.copy()
        prior_std = float(np.std(structural_prior_ordered, ddof=0))
        unlocked_additions = (
            ordered_additions_for_prior[unlocked_positions]
            if unlocked_positions.size else np.zeros(0, dtype=float)
        )
        locked_additions = (
            ordered_additions_for_prior[locked_positions]
            if locked_positions.size else np.zeros(0, dtype=float)
        )
        transport = {
            **transport,
            "structural_prior_rule": (
                "golden_equals_one_complete_weight_equalization_"
                "with_golden_nonunit_free_forward_fit"
            ),
            "global_mean_weight_centering_applied": False,
            "seed_shape_retention_for_final_flattening": 1.0,
            "seed_shape_retention_maximum": 1.0,
            # Retain the legacy field for export compatibility.  It is an
            # audit value, not a global-mean dispersion cap.
            "flattened_structural_prior_std_target": prior_std,
            "flattened_structural_prior_std": prior_std,
            "flattened_structural_prior_min": float(
                np.min(structural_prior_ordered)
            ),
            "flattened_structural_prior_max": float(
                np.max(structural_prior_ordered)
            ),
            "supplement_structural_prior_count": int(
                unlocked_positions.size
            ),
            "supplement_absorption_addition_mean": (
                float(np.mean(unlocked_additions))
                if unlocked_additions.size else 0.0
            ),
            "supplement_absorption_addition_max": (
                float(np.max(unlocked_additions))
                if unlocked_additions.size else 0.0
            ),
            "locked_absorption_addition_mean": (
                float(np.mean(locked_additions))
                if locked_additions.size else 0.0
            ),
        }
    ordered_support = support_array[order]
    structural_prior = np.empty_like(structural_prior_ordered)
    structural_prior[order] = structural_prior_ordered
    seed_anchor = np.empty_like(seed_anchor_ordered)
    seed_anchor[order] = seed_anchor_ordered
    golden_units = _golden_unit_indices(golden_weights)
    free_priority_indices = (
        set(absorption_priority_indices or ()) - golden_units
    )
    factors = regularization_factors or [
        0.0,
        3e-4, 5e-4, 7e-4,
        1e-3, 1.3e-3, 1.7e-3, 2.1e-3, 2.5e-3, 2.8e-3, 3e-3,
        4e-3, 6e-3, 1e-2, 3e-2, 1e-1,
        0.3, 1.0, 3.0, 10.0, 30.0,
    ]
    factor_options: list[float | None] = list(factors)
    if regularization_factors is None:
        factor_options.append(None)
    path = []
    ordered_unit_positions = [
        pos for pos, idx in enumerate(ordered_support)
        if int(idx) in golden_units
    ]
    for factor in factor_options:
        ordered_weights, affine_slope, affine_intercept = (
            _solve_affine_soft_weights(
                rate_matrix[:, ordered_support],
                reference_scores,
                structural_prior_ordered,
                float(np.sum(big_weights)),
                factor,
                equal_positions=ordered_unit_positions,
            )
        )
        final_weights = np.empty_like(ordered_weights)
        final_weights[order] = ordered_weights
        unit_values = final_weights[np.asarray([
            pos for pos, idx in enumerate(support) if idx in golden_units
        ], dtype=int)]
        golden_unit_projection = {
            "unit_count": int(unit_values.size),
            "free_count": int(len(support) - unit_values.size),
            "after_spread": float(np.ptp(unit_values)) if unit_values.size else 0.0,
            "projection_l2": 0.0,
            "rule": "embedded_in_affine_kkt_feasible_space",
        }
        absorption_projection = {
            "projection_l2": 0.0,
            "before_front_inversion_count": 0,
            "after_front_inversion_count": 0,
            "method": "retired_hard_projection;continuous_soft_prior_only",
        }
        supplement_pin = {
            "applied": False,
            "pinned_count": 0,
            "released_mass": 0.0,
            "max_abs_before": 0.0,
            "max_abs_after": 0.0,
            "rule": "retired;golden_nonunit_rows_are_free",
        }
        ordered_weights = final_weights[order]
        direct_scores = (
            rate_matrix[:, ordered_support] @ ordered_weights / 50.0
        )
        affine_slope, affine_intercept, _chebyshev_diffs = (
            affine_chebyshev_fit(direct_scores, reference_scores)
        )
        affine_slope = max(float(affine_slope), 1e-8)
        affine_intercept, _chebyshev_max = fixed_slope_chebyshev_fit(
            direct_scores, reference_scores, affine_slope,
        )
        metrics = _inherited_compression_metrics(
            rate_matrix,
            big_weights,
            reference_scores,
            support,
            final_weights,
            structural_prior,
            correct_scores,
            affine_slope,
            affine_intercept,
        )
        metrics["regularization_factor"] = (
            -1.0 if factor is None else float(factor)
        )
        metrics["pure_structural_prior"] = factor is None
        metrics["supplement_tail_pin_applied"] = bool(
            supplement_pin["applied"]
        )
        metrics["supplement_tail_pin_count"] = int(
            supplement_pin["pinned_count"]
        )
        metrics["supplement_tail_pin_released_mass"] = float(
            supplement_pin["released_mass"]
        )
        metrics["supplement_tail_pin_max_abs_before"] = float(
            supplement_pin["max_abs_before"]
        )
        metrics["structure_score"] = _support_structure_score(
            metrics, structural_prior,
        )
        metrics["replay_score"] = (
            float(metrics["aligned_rmse"])
            + 0.20 * float(metrics["aligned_max_abs_diff"])
        )
        metrics["max_diff_target"] = 0.20
        metrics["max_diff_target_met"] = (
            float(metrics["aligned_max_abs_diff"]) <= 0.20 + 1e-9
        )
        metrics["max_diff_target_margin"] = (
            0.20 - float(metrics["aligned_max_abs_diff"])
        )
        metrics["priority_absorption_projection_l2"] = float(
            absorption_projection["projection_l2"]
        )
        metrics["priority_absorption_inversion_count_before"] = int(
            absorption_projection.get("before_front_inversion_count", 0)
        )
        metrics["priority_absorption_inversion_count_after"] = int(
            absorption_projection.get("after_front_inversion_count", 0)
        )
        metrics["golden_unit_equalization_count"] = int(
            golden_unit_projection["unit_count"]
        )
        metrics["golden_unit_final_spread"] = float(
            golden_unit_projection["after_spread"]
        )
        metrics["golden_unit_projection_l2"] = float(
            golden_unit_projection["projection_l2"]
        )
        metrics["golden_nonunit_free_count"] = int(
            golden_unit_projection["free_count"]
        )
        path.append((final_weights, metrics))

    replay_values = np.asarray(
        [item[1]["replay_score"] for item in path], dtype=float,
    )
    structure_values = np.asarray(
        [item[1]["structure_score"] for item in path], dtype=float,
    )
    def normalized(values: np.ndarray) -> np.ndarray:
        lo = float(np.min(values))
        span = float(np.max(values)) - lo
        if span <= 1e-12:
            return np.zeros(values.size, dtype=float)
        return (values - lo) / span

    max_error_values = np.asarray(
        [item[1]["aligned_max_abs_diff"] for item in path], dtype=float,
    )
    rmse_values = np.asarray(
        [item[1]["aligned_rmse"] for item in path], dtype=float,
    )
    mean_error_values = np.asarray(
        [item[1]["aligned_mean_abs_diff"] for item in path], dtype=float,
    )
    normalized_error = (
        0.70 * normalized(max_error_values)
        + 0.20 * normalized(rmse_values)
        + 0.10 * normalized(mean_error_values)
    )
    normalized_structure = normalized(structure_values)
    # 0.20 is a reporting goal, not a feasibility switch. Path selection is a
    # continuous error/structure tradeoff, with replay quality deliberately
    # dominant and the structural term breaking near-ties toward smoother
    # weights.
    selection_scores = (
        0.75 * normalized_error + 0.25 * normalized_structure
    )
    for pos, (_weights, item_metrics) in enumerate(path):
        item_metrics["selection_error_cost_normalized"] = float(
            normalized_error[pos]
        )
        item_metrics["selection_structure_cost_normalized"] = float(
            normalized_structure[pos]
        )
        item_metrics["selection_score"] = float(selection_scores[pos])
        item_metrics["selection_rule"] = (
            "continuous_75pct_error_25pct_structure_pareto_score"
        )
    selected_pos = min(
        range(len(path)),
        key=lambda pos: (
            float(selection_scores[pos]),
            float(max_error_values[pos]),
            float(structure_values[pos]),
            -float(path[pos][1]["regularization_factor"]),
        ),
    )
    selected_weights, metrics = path[selected_pos]
    additions = selected_weights - base
    return {
        "support": list(support),
        "base_weights": base,
        "additions": additions,
        "selected_weights": selected_weights,
        "anchor": structural_prior,
        "seed_anchor": seed_anchor,
        "metrics": metrics,
        "transport": transport,
        "path": [
            {
                "regularization_factor": item_metrics["regularization_factor"],
                "pure_structural_prior": item_metrics["pure_structural_prior"],
                "big_mean_abs_diff": item_metrics["big_mean_abs_diff"],
                "big_max_abs_diff": item_metrics["big_max_abs_diff"],
                "big_rmse": item_metrics["big_rmse"],
                "aligned_mean_abs_diff": item_metrics["aligned_mean_abs_diff"],
                "aligned_max_abs_diff": item_metrics["aligned_max_abs_diff"],
                "aligned_rmse": item_metrics["aligned_rmse"],
                "affine_slope": item_metrics["affine_slope"],
                "affine_intercept": item_metrics["affine_intercept"],
                "score_spearman": item_metrics["score_spearman"],
                "final_weight_c_score_spearman": item_metrics[
                    "final_weight_c_score_spearman"
                ],
                "final_weight_monotonic_violation_count": item_metrics[
                    "final_weight_monotonic_violation_count"
                ],
                "final_weight_std": item_metrics["final_weight_std"],
                "structure_score": item_metrics["structure_score"],
                "anchor_distance_l2": item_metrics["anchor_distance_l2"],
                "similar_addition_rms": item_metrics["similar_addition_rms"],
                "adjacent_weight_gap_rms": item_metrics["adjacent_weight_gap_rms"],
                "max_diff_target": item_metrics["max_diff_target"],
                "max_diff_target_met": item_metrics["max_diff_target_met"],
                "max_diff_target_margin": item_metrics[
                    "max_diff_target_margin"
                ],
                "selection_error_cost_normalized": item_metrics[
                    "selection_error_cost_normalized"
                ],
                "selection_structure_cost_normalized": item_metrics[
                    "selection_structure_cost_normalized"
                ],
                "selection_score": item_metrics["selection_score"],
                "selection_rule": item_metrics["selection_rule"],
            }
            for _item_weights, item_metrics in path
        ],
    }


def _replacement_shortlists(
    current: dict[str, Any],
    eligible: list[int],
    locked: set[int],
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
) -> tuple[list[int], list[int]]:
    support = list(current["support"])
    support_set = set(support)
    removable = [idx for idx in support if idx not in locked]
    support_array = np.asarray(support, dtype=int)
    normalized, means = _profile_geometry(rate_matrix)
    support_dist = _profile_distance_block(
        normalized, means, support_array, support_array,
    )
    np.fill_diagonal(support_dist, np.inf)
    redundancy = {
        idx: float(np.min(support_dist[pos]))
        for pos, idx in enumerate(support)
    }
    final_weights = np.asarray(current["selected_weights"], dtype=float)
    weight_by_idx: dict[int, float] = {}
    for pos, idx in enumerate(support):
        weight_by_idx[idx] = float(final_weights[pos])
    low_effective_rows = sorted(
        removable,
        key=lambda idx: (
            weight_by_idx.get(idx, 0.0),
            redundancy[idx],
            idx,
        ),
    )[:8]
    low_score = sorted(removable, key=lambda idx: (correct_scores[idx], idx))[:6]
    redundant = sorted(removable, key=lambda idx: (redundancy[idx], idx))[:6]
    low_base = sorted(removable, key=lambda idx: (abs(big_weights[idx]), idx))[:4]
    removal_list = list(dict.fromkeys(
        low_score + low_effective_rows + low_base + redundant
    ))[:16]

    compressed = rate_matrix[:, support_array] @ final_weights / 50.0
    affine_slope = max(float(current["metrics"]["affine_slope"]), 1e-8)
    affine_intercept = float(current["metrics"]["affine_intercept"])
    flattened_correct = (
        reference_scores - affine_intercept
    ) / affine_slope
    # A replacement is useful when its matchup column explains what the
    # current support is missing after C-Score has been flattened onto the
    # direct weighted-target scale.  Matching the uncompressed big target here
    # would reintroduce the obsolete "raw replay must be identical" objective.
    residual = flattened_correct - compressed
    residual_centered = residual - float(np.mean(residual))
    residual_norm = max(float(np.linalg.norm(residual_centered)), 1e-12)
    outside = [idx for idx in eligible if idx not in support_set]
    candidate_scores = []
    for idx in outside:
        column = rate_matrix[:, idx]
        centered = column - float(np.mean(column))
        correlation = abs(float(np.dot(centered, residual_centered))) / (
            max(float(np.linalg.norm(centered)), 1e-12) * residual_norm
        )
        coverage = float(np.min(_profile_distance_block(
            normalized, means, np.asarray([idx]), support_array,
        )))
        candidate_scores.append((idx, correlation, coverage))
    by_residual = [
        idx for idx, _corr, _coverage in sorted(
            candidate_scores, key=lambda item: (-item[1], item[0]),
        )[:7]
    ]
    by_coverage = [
        idx for idx, _corr, _coverage in sorted(
            candidate_scores, key=lambda item: (-item[2], item[0]),
        )[:5]
    ]
    by_score = sorted(outside, key=lambda idx: (-correct_scores[idx], idx))[:5]
    by_tail_mass = sorted(
        outside, key=lambda idx: (-abs(big_weights[idx]), idx),
    )[:4]
    addition_list = list(dict.fromkeys(
        by_residual + by_coverage + by_score + by_tail_mass
    ))[:24]

    # Keep only candidates that can participate in at least one legal swap.
    feasible_additions = []
    for added in addition_list:
        if any(
            _support_feasible(
                [idx for idx in support if idx != removed] + [added],
                account_keys,
                owner_keys,
                owner_cap,
                len(support),
            )
            for removed in removal_list
        ):
            feasible_additions.append(added)
    return removal_list, feasible_additions


def _collective_absorption_anchor(
    rate_matrix: np.ndarray,
    support: list[int],
    lineage_weights: np.ndarray,
    removed_pos: int,
) -> tuple[list[int], np.ndarray, np.ndarray, np.ndarray]:
    """Build an all-receiver anchor for one deletion.

    Equal sharing supplies the mass-conserving baseline.  A mild zero-sum
    matchup-profile contrast gives closer columns a larger share and farther
    columns a smaller share, while the subsequent forward fit is free to move
    every remaining row jointly.
    """
    removed_idx = support[removed_pos]
    remaining = [
        idx for pos, idx in enumerate(support) if pos != removed_pos
    ]
    remaining_weights = np.delete(lineage_weights, removed_pos)
    removed_weight = float(lineage_weights[removed_pos])
    count = len(remaining)
    if count == 0:
        raise ValueError("cannot absorb the final target")

    normalized, means = _profile_geometry(rate_matrix)
    distances = _profile_distance_block(
        normalized,
        means,
        np.asarray([removed_idx], dtype=int),
        np.asarray(remaining, dtype=int),
    )[0]
    positive = distances[distances > 1e-12]
    bandwidth = (
        float(np.median(positive)) if positive.size else 1.0
    )
    bandwidth = max(bandwidth, 1e-6)
    similarities = np.exp(-np.square(distances / bandwidth))
    centered = similarities - float(np.mean(similarities))

    equal_transfer = np.full(count, removed_weight / count, dtype=float)
    centered_l1 = float(np.sum(np.abs(centered)))
    if centered_l1 <= 1e-12 or abs(removed_weight) <= 1e-15:
        profile_transfer = equal_transfer
    else:
        # This contrast is deliberately bounded.  It expresses the requested
        # "nearer receives more, farther receives less" direction without
        # recreating the old nearest-neighbour concentration.
        contrast = (
            math.copysign(0.75 * abs(removed_weight), removed_weight)
            * centered
            / centered_l1
        )
        profile_transfer = equal_transfer + contrast
        profile_transfer += (
            removed_weight - float(np.sum(profile_transfer))
        ) / count
    anchor = remaining_weights + profile_transfer
    anchor += (
        float(np.sum(lineage_weights)) - float(np.sum(anchor))
    ) / count
    return remaining, anchor, similarities, remaining_weights


def _normalized_cost(values: np.ndarray) -> np.ndarray:
    lo = float(np.min(values))
    span = float(np.max(values)) - lo
    if span <= 1e-12:
        return np.zeros(values.size, dtype=float)
    return (values - lo) / span


def _front_priority_c_score_projection(
    weights: np.ndarray,
    support: list[int],
    correct_scores: np.ndarray,
    priority_count: int,
) -> tuple[np.ndarray, dict[str, Any]]:
    """Project weights onto a front-priority C-Score shape.

    The protected C-Score prefix is monotonically non-increasing.  Its
    adjacent drop is capped by the actual C-Score gap, so near-tied rows receive
    near-tied weights.  Tail rows are only capped above by the last protected
    row: genuinely low tail weights remain low and can drive deletion.
    """
    original = np.asarray(weights, dtype=float)
    if original.ndim != 1 or original.size != len(support):
        raise ValueError("front-priority projection shape mismatch")
    count = original.size
    if count <= 1:
        return original.copy(), {
            "applied": False,
            "priority_count": count,
            "projection_l2": 0.0,
        }
    priority_count = min(max(1, int(priority_count)), count)
    support_array = np.asarray(support, dtype=int)
    order = np.asarray(
        sorted(
            range(count),
            key=lambda pos: (
                -float(correct_scores[support[pos]]),
                int(support[pos]),
            ),
        ),
        dtype=int,
    )
    ordered_scores = correct_scores[support_array[order]]
    ordered_original = original[order]
    mean_abs_weight = max(
        abs(float(np.sum(original)) / count), 1e-6,
    )

    prefix_scores = ordered_scores[:priority_count]
    if priority_count > 1:
        score_gaps = np.maximum(
            prefix_scores[:-1] - prefix_scores[1:], 0.0,
        )
        score_span = max(
            float(prefix_scores[0] - prefix_scores[-1]), 1e-9,
        )
        # A zero/near-zero score gap permits only a 0.3%-of-mean weight gap.
        # Across the complete protected score span, another 38%-of-mean is
        # available.  This keeps close C-Scores visually close without forcing
        # the whole front prefix to one constant.
        adjacent_drop_caps = (
            0.003 * mean_abs_weight
            + 0.38 * mean_abs_weight * score_gaps / score_span
        )
    else:
        adjacent_drop_caps = np.zeros(0, dtype=float)

    total_weight = float(np.sum(ordered_original))

    # Moving an early row is progressively more expensive.  Tail rows have
    # unit fidelity: low tail rows are not raised merely to make the board
    # prettier, while excess tail mass can move into the protected prefix.
    fidelity = np.ones(count, dtype=float)
    if priority_count > 1:
        fidelity[:priority_count] = np.linspace(
            4.0, 1.0, priority_count,
        )
    else:
        fidelity[0] = 4.0

    # Parameterize every feasible vector by bounded front drops d_i and
    # non-negative tail slacks s_j:
    #
    #   w_i = boundary + Σ_{k=i}^{p-2} d_k
    #   w_j = boundary - s_j, j >= p
    #
    # Eliminating `boundary` with Σw=total converts the projection into a
    # bounded linear least-squares problem.  This is a convex, deterministic
    # solve and is substantially more reliable than a generic constrained
    # optimizer along the thousands of deletion-path candidates.
    front_drop_count = max(0, priority_count - 1)
    tail_count = count - priority_count
    variable_count = front_drop_count + tail_count
    base_level = total_weight / count
    transform = np.zeros((count, variable_count), dtype=float)
    for drop_pos in range(front_drop_count):
        contribution_count = drop_pos + 1
        transform[:, drop_pos] -= contribution_count / count
        transform[:contribution_count, drop_pos] += 1.0
    for tail_offset in range(tail_count):
        column = front_drop_count + tail_offset
        transform[:, column] += 1.0 / count
        transform[priority_count + tail_offset, column] -= 1.0
    lower_bounds = np.zeros(variable_count, dtype=float)
    upper_bounds = np.r_[
        adjacent_drop_caps,
        np.full(tail_count, np.inf, dtype=float),
    ]
    sqrt_fidelity = np.sqrt(fidelity)
    result = lsq_linear(
        sqrt_fidelity[:, None] * transform,
        sqrt_fidelity * (ordered_original - base_level),
        bounds=(lower_bounds, upper_bounds),
        method="trf",
        tol=1e-12,
        lsmr_tol="auto",
        max_iter=2000,
    )
    ordered_projected = (
        np.full(count, base_level, dtype=float)
        + transform @ np.asarray(result.x, dtype=float)
    )
    prefix_drops = (
        ordered_projected[:priority_count - 1]
        - ordered_projected[1:priority_count]
    )
    tail_max = (
        float(np.max(ordered_projected[priority_count:]))
        if priority_count < count
        else float(ordered_projected[priority_count - 1])
    )
    constraint_tolerance = 2e-7
    valid = (
        np.all(np.isfinite(ordered_projected))
        and abs(float(np.sum(ordered_projected)) - total_weight) <= 2e-7
        and (
            prefix_drops.size == 0
            or (
                float(np.min(prefix_drops)) >= -constraint_tolerance
                and float(np.max(
                    prefix_drops - adjacent_drop_caps
                )) <= constraint_tolerance
            )
        )
        and tail_max
        <= float(ordered_projected[priority_count - 1])
        + constraint_tolerance
    )
    if not valid:
        raise RuntimeError(
            "front-priority C-Score projection failed: "
            f"success={result.success} message={result.message}"
        )

    projected = np.empty_like(ordered_projected)
    projected[order] = ordered_projected
    before_prefix = ordered_original[:priority_count]
    after_prefix = ordered_projected[:priority_count]
    before_inversions = (
        np.maximum(before_prefix[1:] - before_prefix[:-1], 0.0)
        if priority_count > 1 else np.zeros(0, dtype=float)
    )
    after_inversions = (
        np.maximum(after_prefix[1:] - after_prefix[:-1], 0.0)
        if priority_count > 1 else np.zeros(0, dtype=float)
    )
    near_weights = (
        np.exp(
            -np.maximum(prefix_scores[:-1] - prefix_scores[1:], 0.0)
            / max(
                float(prefix_scores[0] - prefix_scores[-1])
                / max(1, priority_count - 1),
                1e-9,
            )
        )
        if priority_count > 1 else np.zeros(0, dtype=float)
    )

    def near_gap_rms(values: np.ndarray) -> float:
        if values.size <= 1 or float(np.sum(near_weights)) <= 1e-12:
            return 0.0
        gaps = values[:-1] - values[1:]
        return float(np.sqrt(
            np.sum(near_weights * gaps * gaps)
            / np.sum(near_weights)
        ))

    return projected, {
        "applied": True,
        "method": (
            "weighted_least_change_projection_with_front_monotone_"
            "score_gap_caps_and_tail_ceiling"
        ),
        "optimizer_success": bool(result.success),
        "optimizer_message": str(result.message),
        "priority_count": priority_count,
        "total_weight": total_weight,
        "projection_l2": float(np.linalg.norm(projected - original)),
        "before_weight_std": float(np.std(original, ddof=0)),
        "after_weight_std": float(np.std(projected, ddof=0)),
        "before_weight_min": float(np.min(original)),
        "before_weight_max": float(np.max(original)),
        "after_weight_min": float(np.min(projected)),
        "after_weight_max": float(np.max(projected)),
        "before_front_inversion_count": int(
            np.sum(before_inversions > 1e-10)
        ),
        "after_front_inversion_count": int(
            np.sum(after_inversions > 1e-10)
        ),
        "before_front_near_score_gap_rms": near_gap_rms(before_prefix),
        "after_front_near_score_gap_rms": near_gap_rms(after_prefix),
        "front_first_weight": float(after_prefix[0]),
        "front_last_weight": float(after_prefix[-1]),
        "tail_weight_min": (
            float(np.min(ordered_projected[priority_count:]))
            if priority_count < count else float(after_prefix[-1])
        ),
        "tail_weight_max": tail_max,
        "adjacent_drop_cap_min": (
            float(np.min(adjacent_drop_caps))
            if adjacent_drop_caps.size else 0.0
        ),
        "adjacent_drop_cap_max": (
            float(np.max(adjacent_drop_caps))
            if adjacent_drop_caps.size else 0.0
        ),
        "c_score_weight_spearman_before": _spearman(
            ordered_original, ordered_scores,
        ),
        "c_score_weight_spearman_after": _spearman(
            ordered_projected, ordered_scores,
        ),
    }


def _evaluate_variable_count_stage(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    support: list[int],
    lineage_weights: np.ndarray,
    lineage_anchor: np.ndarray,
    previous_support: list[int] | None = None,
    previous_lineage_weights: np.ndarray | None = None,
    removed_index: int | None = None,
    similarities: np.ndarray | None = None,
    regularization_factors: list[float | None] | None = None,
    c_score_priority_count: int = 30,
    absorption_priority_indices: set[int] | None = None,
) -> dict[str, Any]:
    """Evaluate one n-target support under the exported /n convention."""
    total_mass = float(np.sum(big_weights))
    if abs(total_mass) <= 1e-12:
        raise ValueError("big-target mass is too close to zero")
    target_count = len(support)
    export_scale = target_count / total_mass
    factors = regularization_factors or [
        0.0,
        1e-4, 3e-4, 1e-3, 3e-3, 1e-2,
        3e-2, 1e-1, 3e-1, 1.0, 3.0, 10.0, 30.0,
        None,
    ]
    if previous_support is None:
        factors = [0.0]

    if previous_support is None or previous_lineage_weights is None:
        fit_target = (
            rate_matrix[:, np.asarray(support, dtype=int)]
            @ lineage_weights
            / total_mass
        )
        solve_design = (
            rate_matrix[:, np.asarray(support, dtype=int)] / total_mass
        )
    else:
        fit_target = (
            rate_matrix[:, np.asarray(previous_support, dtype=int)]
            @ previous_lineage_weights
            / total_mass
        )
        solve_design = (
            rate_matrix[:, np.asarray(support, dtype=int)] / total_mass
        )

    path: list[dict[str, Any]] = []
    golden_units = _golden_unit_indices(golden_weights)
    unit_positions = [
        pos for pos, idx in enumerate(support) if idx in golden_units
    ]
    for factor in factors:
        if previous_support is None:
            fitted_lineage = lineage_weights.copy()
        else:
            fitted_lineage = _mass_constrained_ridge_fit(
                solve_design,
                fit_target,
                lineage_anchor,
                total_mass,
                factor,
                equal_positions=unit_positions,
            )
        unit_values = fitted_lineage[np.asarray(unit_positions, dtype=int)]
        golden_unit_projection = {
            "unit_count": len(unit_positions),
            "free_count": len(support) - len(unit_positions),
            "after_spread": float(np.ptp(unit_values)) if unit_values.size else 0.0,
            "projection_l2": 0.0,
        }
        front_projection = {
            "projection_l2": 0.0,
            "method": "retired_hard_projection;continuous_soft_prior_only",
        }
        supplement_pin = {
            "applied": False,
            "pinned_count": 0,
            "released_mass": 0.0,
            "max_abs_before": 0.0,
            "max_abs_after": 0.0,
            "rule": "retired;golden_nonunit_rows_are_free",
        }
        front_projection = {
            **front_projection,
            "priority_count": min(
                max(1, int(c_score_priority_count)),
                target_count,
            ),
        }
        exported = fitted_lineage * export_scale
        direct_scores = (
            rate_matrix[:, np.asarray(support, dtype=int)]
            @ exported
            / target_count
        )
        affine_slope, _affine_intercept, _ = affine_chebyshev_fit(
            direct_scores, reference_scores,
        )
        affine_slope = max(float(affine_slope), 1e-8)
        affine_intercept, _ = fixed_slope_chebyshev_fit(
            direct_scores, reference_scores, affine_slope,
        )
        exported_anchor = lineage_anchor * export_scale
        metrics = _inherited_compression_metrics(
            rate_matrix,
            big_weights,
            reference_scores,
            support,
            exported,
            exported_anchor,
            correct_scores,
            affine_slope,
            affine_intercept,
            score_denominator=float(target_count),
            full_response_denominator=50.0,
        )
        lineage_additions = (
            fitted_lineage
            - big_weights[np.asarray(support, dtype=int)]
        )
        metrics["addition_l2"] = float(np.linalg.norm(lineage_additions))
        metrics["addition_max_abs"] = float(
            np.max(np.abs(lineage_additions))
        )
        metrics["anchor_distance_l2"] = float(
            np.linalg.norm(fitted_lineage - lineage_anchor)
        )
        metrics["final_weight_sum"] = float(np.sum(exported))
        metrics["regularization_factor"] = (
            -1.0 if factor is None else float(factor)
        )
        metrics["pure_structural_prior"] = factor is None
        metrics["supplement_tail_pin_applied"] = bool(
            supplement_pin["applied"]
        )
        metrics["supplement_tail_pin_count"] = int(
            supplement_pin["pinned_count"]
        )
        metrics["supplement_tail_pin_released_mass"] = float(
            supplement_pin["released_mass"]
        )
        metrics["supplement_tail_pin_max_abs_before"] = float(
            supplement_pin["max_abs_before"]
        )
        mean_abs_weight = max(float(np.mean(np.abs(exported))), 1e-9)
        center = float(np.mean(exported))
        max_deviation = float(np.max(np.abs(exported - center)))
        anchor_scale = max(float(np.linalg.norm(lineage_anchor)), 1e-9)
        cancellation = float(np.sum(np.abs(exported))) / max(
            abs(float(np.sum(exported))), 1e-9,
        )
        metrics["absorption_max_weight_deviation"] = max_deviation
        metrics["absorption_cancellation_ratio"] = cancellation
        metrics["absorption_structure_score"] = (
            float(np.std(exported, ddof=0)) / mean_abs_weight
            + 0.35 * max_deviation / mean_abs_weight
            + 0.20 * float(metrics["anchor_distance_l2"]) / anchor_scale
            + 0.10 * max(0.0, cancellation - 1.0)
            + 0.05 * float(metrics["final_weight_monotonic_violation_count"])
            / max(1, target_count)
        )
        metrics["structure_score"] = metrics[
            "absorption_structure_score"
        ]
        metrics["replay_score"] = (
            float(metrics["aligned_max_abs_diff"])
            + 0.35 * float(metrics["aligned_rmse"])
            + 0.10 * float(metrics["aligned_mean_abs_diff"])
        )
        metrics["max_diff_target"] = 0.20
        metrics["max_diff_target_met"] = (
            float(metrics["aligned_max_abs_diff"]) <= 0.20 + 1e-9
        )
        metrics["max_diff_target_margin"] = (
            0.20 - float(metrics["aligned_max_abs_diff"])
        )
        metrics["front_priority_projection_l2"] = float(
            front_projection["projection_l2"]
        )
        metrics["front_priority_inversion_count"] = int(
            metrics["final_weight_monotonic_violation_count"]
        )
        metrics["front_priority_near_score_gap_rms"] = float(
            metrics["adjacent_weight_gap_rms"]
        )
        metrics["golden_unit_equalization_count"] = int(
            golden_unit_projection["unit_count"]
        )
        metrics["golden_unit_final_spread"] = float(
            golden_unit_projection["after_spread"]
        )
        metrics["golden_unit_projection_l2"] = float(
            golden_unit_projection["projection_l2"]
        )
        metrics["golden_nonunit_free_count"] = int(
            golden_unit_projection["free_count"]
        )
        front_projection.update({
            "actual_front_inversion_count": int(
                metrics["final_weight_monotonic_violation_count"]
            ),
            "actual_adjacent_weight_gap_rms": float(
                metrics["adjacent_weight_gap_rms"]
            ),
        })

        receiver_count = 0
        similarity_transfer_corr = None
        transfer_min = 0.0
        transfer_max = 0.0
        if previous_support is not None and previous_lineage_weights is not None:
            removed_pos = previous_support.index(int(removed_index))
            previous_remaining = np.delete(
                previous_lineage_weights, removed_pos,
            )
            transfer = fitted_lineage - previous_remaining
            receiver_count = int(np.sum(np.abs(transfer) > 1e-10))
            transfer_min = float(np.min(transfer))
            transfer_max = float(np.max(transfer))
            if similarities is not None:
                similarity_transfer_corr = corrcoef(
                    np.asarray(similarities, dtype=float),
                    transfer,
                )
        path.append({
            "support": list(support),
            "lineage_weights": fitted_lineage,
            "selected_weights": exported,
            "anchor": lineage_anchor,
            "metrics": metrics,
            "regularization_factor": (
                -1.0 if factor is None else float(factor)
            ),
            "absorption_receiver_count": receiver_count,
            "similarity_transfer_correlation": similarity_transfer_corr,
            "transfer_min": transfer_min,
            "transfer_max": transfer_max,
            "front_priority_projection": front_projection,
        })

    error_values = np.asarray([
        0.70 * float(item["metrics"]["aligned_max_abs_diff"])
        + 0.20 * float(item["metrics"]["aligned_rmse"])
        + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"])
        for item in path
    ])
    structure_values = np.asarray([
        float(item["metrics"]["absorption_structure_score"])
        for item in path
    ])
    selection_scores = (
        0.75 * _normalized_cost(error_values)
        + 0.25 * _normalized_cost(structure_values)
    )
    for pos, item in enumerate(path):
        item["metrics"]["selection_error_cost_normalized"] = float(
            _normalized_cost(error_values)[pos]
        )
        item["metrics"]["selection_structure_cost_normalized"] = float(
            _normalized_cost(structure_values)[pos]
        )
        item["metrics"]["selection_score"] = float(selection_scores[pos])
        item["metrics"]["selection_rule"] = (
            "persistent_predeletion_anchor_75pct_error_25pct_structure"
        )
    selected_pos = min(
        range(len(path)),
        key=lambda pos: (
            float(selection_scores[pos]),
            float(path[pos]["metrics"]["aligned_max_abs_diff"]),
            float(path[pos]["metrics"]["absorption_structure_score"]),
            -float(path[pos]["regularization_factor"]),
        ),
    )
    selected = path[selected_pos]
    selected["path"] = [{
        "regularization_factor": item["regularization_factor"],
        "pure_structural_prior": item["metrics"]["pure_structural_prior"],
        "aligned_mean_abs_diff": item["metrics"]["aligned_mean_abs_diff"],
        "aligned_max_abs_diff": item["metrics"]["aligned_max_abs_diff"],
        "aligned_rmse": item["metrics"]["aligned_rmse"],
        "final_weight_std": item["metrics"]["final_weight_std"],
        "absorption_max_weight_deviation": item["metrics"][
            "absorption_max_weight_deviation"
        ],
        "absorption_structure_score": item["metrics"][
            "absorption_structure_score"
        ],
        "anchor_distance_l2": item["metrics"]["anchor_distance_l2"],
        "front_priority_projection_l2": item["metrics"][
            "front_priority_projection_l2"
        ],
        "front_priority_inversion_count": item["metrics"][
            "front_priority_inversion_count"
        ],
        "front_priority_near_score_gap_rms": item["metrics"][
            "front_priority_near_score_gap_rms"
        ],
        "supplement_tail_pin_count": item["metrics"][
            "supplement_tail_pin_count"
        ],
        "supplement_tail_pin_max_abs_before": item["metrics"][
            "supplement_tail_pin_max_abs_before"
        ],
        "selection_score": item["metrics"]["selection_score"],
        "selection_rule": item["metrics"]["selection_rule"],
    } for item in path]
    return selected


def _deletion_shortlist(
    rate_matrix: np.ndarray,
    support: list[int],
    exported_weights: np.ndarray,
    deletion_locked: set[int],
    correct_scores: np.ndarray,
) -> list[int]:
    removable = [idx for idx in support if idx not in deletion_locked]
    if len(removable) <= 18:
        return removable
    support_array = np.asarray(support, dtype=int)
    normalized, means = _profile_geometry(rate_matrix)
    distances = _profile_distance_block(
        normalized, means, support_array, support_array,
    )
    np.fill_diagonal(distances, np.inf)
    redundancy = {
        idx: float(np.min(distances[pos]))
        for pos, idx in enumerate(support)
    }
    weight_by_idx = {
        idx: float(exported_weights[pos])
        for pos, idx in enumerate(support)
    }
    by_small_weight = sorted(
        removable, key=lambda idx: (abs(weight_by_idx[idx]), idx),
    )[:10]
    by_redundancy = sorted(
        removable, key=lambda idx: (redundancy[idx], idx),
    )[:8]
    by_low_score = sorted(
        removable, key=lambda idx: (correct_scores[idx], idx),
    )[:5]
    return list(dict.fromkeys(
        by_small_weight + by_redundancy + by_low_score
    ))[:18]


def _run_collective_deletion_path(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    current: dict[str, Any],
    deletion_locked: set[int],
    max_deletions: int,
    c_score_priority_count: int,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    """Greedily build and continuously select a 50..(50-d) Pareto path."""
    support = list(current["support"])
    lineage = np.asarray(current["selected_weights"], dtype=float)
    base_stage = _evaluate_variable_count_stage(
        rate_matrix,
        big_weights,
        golden_weights,
        reference_scores,
        correct_scores,
        support,
        lineage,
        np.asarray(current["seed_anchor"], dtype=float),
        c_score_priority_count=c_score_priority_count,
        absorption_priority_indices=deletion_locked,
    )
    base_stage["removed_index"] = None
    stages = [base_stage]
    deletion_steps: list[dict[str, Any]] = []
    persistent_support = list(base_stage["support"])
    persistent_lineage = np.asarray(
        base_stage["lineage_weights"], dtype=float,
    )
    persistent_weight_by_index = {
        idx: float(persistent_lineage[pos])
        for pos, idx in enumerate(persistent_support)
    }
    total_mass = float(np.sum(big_weights))

    for _step in range(max(0, max_deletions)):
        if len(support) <= max(2, len(deletion_locked)):
            break
        shortlist = _deletion_shortlist(
            rate_matrix,
            support,
            np.asarray(stages[-1]["selected_weights"], dtype=float),
            deletion_locked,
            correct_scores,
        )
        candidates = []
        for removed_idx in shortlist:
            removed_pos = support.index(removed_idx)
            remaining = [
                idx for pos, idx in enumerate(support)
                if pos != removed_pos
            ]
            persistent_anchor = np.asarray([
                persistent_weight_by_index[idx] for idx in remaining
            ], dtype=float)
            missing_mass = total_mass - float(
                np.sum(persistent_anchor)
            )
            remaining_golden = np.asarray(
                golden_weights[np.asarray(remaining, dtype=int)],
                dtype=float,
            )
            unit_mask = np.isclose(
                remaining_golden,
                1.0,
                rtol=0.0,
                atol=GOLDEN_UNIT_ATOL,
            )
            anchor = persistent_anchor.copy()
            if np.any(unit_mask):
                unit_count = int(np.sum(unit_mask))
                common_weight = (
                    float(np.sum(persistent_anchor[unit_mask]))
                    + missing_mass
                ) / unit_count
                anchor[unit_mask] = common_weight
            else:
                anchor += missing_mass / len(remaining)
            anchor += (
                total_mass - float(np.sum(anchor))
            ) / len(anchor)
            candidate = _evaluate_variable_count_stage(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                remaining,
                lineage_weights=anchor,
                lineage_anchor=anchor,
                previous_support=support,
                previous_lineage_weights=lineage,
                removed_index=removed_idx,
                similarities=None,
                c_score_priority_count=c_score_priority_count,
                absorption_priority_indices=deletion_locked,
            )
            candidate["removed_index"] = removed_idx
            candidates.append(candidate)
        if not candidates:
            break

        error = np.asarray([
            0.70 * float(item["metrics"]["aligned_max_abs_diff"])
            + 0.20 * float(item["metrics"]["aligned_rmse"])
            + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"])
            for item in candidates
        ])
        structure = np.asarray([
            float(item["metrics"]["absorption_structure_score"])
            for item in candidates
        ])
        choice_score = (
            0.80 * _normalized_cost(error)
            + 0.20 * _normalized_cost(structure)
        )
        chosen_pos = min(
            range(len(candidates)),
            key=lambda pos: (
                float(choice_score[pos]),
                float(candidates[pos]["metrics"]["aligned_max_abs_diff"]),
                float(candidates[pos]["metrics"]["absorption_structure_score"]),
                int(candidates[pos]["removed_index"]),
            ),
        )
        chosen = candidates[chosen_pos]
        removed_idx = int(chosen["removed_index"])
        deletion_steps.append({
            "step": len(deletion_steps) + 1,
            "removed_index": removed_idx,
            "before_target_count": len(support),
            "after_target_count": len(chosen["support"]),
            "removed_lineage_weight": float(
                lineage[support.index(removed_idx)]
            ),
            "absorption_receiver_count": int(
                chosen["absorption_receiver_count"]
            ),
            "similarity_transfer_correlation": (
                chosen["similarity_transfer_correlation"]
            ),
            "transfer_min": float(chosen["transfer_min"]),
            "transfer_max": float(chosen["transfer_max"]),
            "aligned_mean_abs_diff": float(
                chosen["metrics"]["aligned_mean_abs_diff"]
            ),
            "aligned_max_abs_diff": float(
                chosen["metrics"]["aligned_max_abs_diff"]
            ),
            "aligned_rmse": float(chosen["metrics"]["aligned_rmse"]),
            "final_weight_std": float(
                chosen["metrics"]["final_weight_std"]
            ),
            "regularization_factor": float(
                chosen["regularization_factor"]
            ),
            "deletion_anchor_rule": (
                "persistent_predeletion_base_with_golden_equals_one_"
                "complete_weight_equalization_and_nonunit_free_fit"
            ),
            "supplement_tail_pin_count": int(
                chosen["metrics"]["supplement_tail_pin_count"]
            ),
            "supplement_tail_pin_max_abs_before": float(
                chosen["metrics"][
                    "supplement_tail_pin_max_abs_before"
                ]
            ),
        })
        support = list(chosen["support"])
        lineage = np.asarray(chosen["lineage_weights"], dtype=float)
        stages.append(chosen)

    error_values = np.asarray([
        0.70 * float(stage["metrics"]["aligned_max_abs_diff"])
        + 0.20 * float(stage["metrics"]["aligned_rmse"])
        + 0.10 * float(stage["metrics"]["aligned_mean_abs_diff"])
        for stage in stages
    ])
    structure_values = np.asarray([
        float(stage["metrics"]["absorption_structure_score"])
        for stage in stages
    ])
    min_count = min(len(stage["support"]) for stage in stages)
    max_count = max(len(stage["support"]) for stage in stages)
    count_cost = np.asarray([
        (
            (len(stage["support"]) - min_count) / (max_count - min_count)
            if max_count > min_count else 0.0
        )
        for stage in stages
    ])
    global_scores = (
        0.82 * _normalized_cost(error_values)
        + 0.15 * _normalized_cost(structure_values)
        + 0.03 * count_cost
    )
    for pos, stage in enumerate(stages):
        stage["global_selection_score"] = float(global_scores[pos])
        stage["global_selection_error_normalized"] = float(
            _normalized_cost(error_values)[pos]
        )
        stage["global_selection_structure_normalized"] = float(
            _normalized_cost(structure_values)[pos]
        )
    base_avg = float(stages[0]["metrics"]["aligned_mean_abs_diff"])
    base_max = float(stages[0]["metrics"]["aligned_max_abs_diff"])
    incumbent_safe_positions = [0] + [
        pos for pos in range(1, len(stages))
        if float(stages[pos]["metrics"]["aligned_mean_abs_diff"])
            <= base_avg + 1e-12
        and float(stages[pos]["metrics"]["aligned_max_abs_diff"])
            <= base_max + 1e-12
        and (
            float(stages[pos]["metrics"]["aligned_mean_abs_diff"])
                < base_avg - 1e-12
            or float(stages[pos]["metrics"]["aligned_max_abs_diff"])
                < base_max - 1e-12
        )
    ]
    selected_pos = min(
        incumbent_safe_positions,
        key=lambda pos: (
            float(global_scores[pos]),
            float(stages[pos]["metrics"]["aligned_max_abs_diff"]),
            len(stages[pos]["support"]),
        ),
    )
    selected = stages[selected_pos]
    selected["metrics"]["selection_score"] = float(global_scores[selected_pos])
    selected["metrics"]["selection_error_cost_normalized"] = float(
        selected["global_selection_error_normalized"]
    )
    selected["metrics"]["selection_structure_cost_normalized"] = float(
        selected["global_selection_structure_normalized"]
    )
    selected["metrics"]["selection_rule"] = (
        "incumbent_safe_avg_and_max_pareto_then_"
        "variable_count_82pct_error_15pct_structure_3pct_count"
    )
    stage_audit = []
    for pos, stage in enumerate(stages):
        stage_audit.append({
            "target_count": len(stage["support"]),
            "removed_index": stage.get("removed_index"),
            "aligned_mean_abs_diff": stage["metrics"][
                "aligned_mean_abs_diff"
            ],
            "aligned_max_abs_diff": stage["metrics"][
                "aligned_max_abs_diff"
            ],
            "aligned_rmse": stage["metrics"]["aligned_rmse"],
            "final_weight_std": stage["metrics"]["final_weight_std"],
            "final_weight_min": stage["metrics"]["final_weight_min"],
            "final_weight_max": stage["metrics"]["final_weight_max"],
            "global_selection_score": stage["global_selection_score"],
            "selected": pos == selected_pos,
        })
    return selected, deletion_steps[:selected_pos], stage_audit


def _final_swap_addition_shortlist(
    rate_matrix: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    big_weights: np.ndarray,
    support: list[int],
    selected_weights: np.ndarray,
    eligible: list[int],
) -> list[int]:
    """Find outside columns most likely to repair the final /n residual."""
    support_set = set(support)
    outside = [
        idx for idx in eligible
        if idx not in support_set
        and abs(float(big_weights[idx])) > 1e-15
    ]
    if len(outside) <= 24:
        return outside
    support_array = np.asarray(support, dtype=int)
    direct = (
        rate_matrix[:, support_array] @ selected_weights / len(support)
    )
    slope, intercept, _ = affine_chebyshev_fit(direct, reference_scores)
    slope = max(float(slope), 1e-8)
    flattened = (reference_scores - float(intercept)) / slope
    residual = flattened - direct
    residual_centered = residual - float(np.mean(residual))
    residual_norm = max(float(np.linalg.norm(residual_centered)), 1e-12)
    normalized, means = _profile_geometry(rate_matrix)
    rows = []
    for idx in outside:
        column = rate_matrix[:, idx]
        centered = column - float(np.mean(column))
        correlation = abs(float(np.dot(
            centered, residual_centered,
        ))) / (
            max(float(np.linalg.norm(centered)), 1e-12) * residual_norm
        )
        coverage = float(np.min(_profile_distance_block(
            normalized,
            means,
            np.asarray([idx], dtype=int),
            support_array,
        )))
        rows.append((idx, correlation, coverage))
    by_residual = [
        idx for idx, _corr, _coverage in sorted(
            rows, key=lambda item: (-item[1], item[0]),
        )[:14]
    ]
    by_coverage = [
        idx for idx, _corr, _coverage in sorted(
            rows, key=lambda item: (-item[2], item[0]),
        )[:6]
    ]
    by_score = sorted(
        outside, key=lambda idx: (-float(correct_scores[idx]), idx),
    )[:6]
    by_mass = sorted(
        outside, key=lambda idx: (-abs(float(big_weights[idx])), idx),
    )[:4]
    return list(dict.fromkeys(
        by_residual + by_coverage + by_score + by_mass
    ))[:24]


def _final_stage_from_fresh_support(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    support: list[int],
    replacement_locked: set[int],
    deletion_locked: set[int],
    regularization_factors: list[float | None] | None = None,
) -> dict[str, Any]:
    """Fit one final-count support from its Golden=1-balanced anchor."""
    protected = (
        set(deletion_locked)
        | (set(replacement_locked) & set(support))
    )
    inherited = _evaluate_inherited_support(
        rate_matrix,
        big_weights,
        golden_weights,
        reference_scores,
        correct_scores,
        support,
        locked=protected,
        regularization_factors=regularization_factors,
        absorption_priority_indices=set(deletion_locked),
    )
    lineage = np.asarray(inherited["selected_weights"], dtype=float)
    anchor = np.asarray(inherited["seed_anchor"], dtype=float)
    stage = _evaluate_variable_count_stage(
        rate_matrix,
        big_weights,
        golden_weights,
        reference_scores,
        correct_scores,
        support,
        lineage_weights=lineage,
        lineage_anchor=anchor,
        c_score_priority_count=len(deletion_locked),
        absorption_priority_indices=set(deletion_locked),
    )
    stage["source_regularization_factor"] = float(
        inherited["metrics"]["regularization_factor"]
    )
    return stage


def _screen_final_support(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    support: list[int],
) -> dict[str, float]:
    """Cheap Golden=1-balanced replay used before any weight-path solve."""
    support_array = np.asarray(support, dtype=int)
    additions, _transfers, _transport = (
        _golden_unit_balanced_transport_anchor(
            big_weights,
            golden_weights,
            support,
            correct_scores,
            priority_indices=None,
        )
    )
    anchor = big_weights[support_array] + additions
    direct = rate_matrix[:, support_array] @ anchor / 50.0
    _slope, _intercept, diffs = affine_chebyshev_fit(
        direct, reference_scores,
    )
    abs_diff = np.abs(diffs)
    return {
        "aligned_mean_abs_diff": float(np.mean(abs_diff)),
        "aligned_max_abs_diff": float(np.max(abs_diff)),
        "aligned_rmse": float(np.sqrt(np.mean(diffs * diffs))),
    }


def _support_replay_key(item: dict[str, Any]) -> tuple[Any, ...]:
    metrics = item["metrics"]
    return (
        0.70 * float(metrics["aligned_max_abs_diff"])
        + 0.20 * float(metrics["aligned_rmse"])
        + 0.10 * float(metrics["aligned_mean_abs_diff"]),
        float(metrics["aligned_max_abs_diff"]),
        float(metrics["aligned_mean_abs_diff"]),
        tuple(sorted(item["support"])),
    )


def _run_final_support_beam_refinement(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    current: dict[str, Any],
    eligible: list[int],
    replacement_locked: set[int],
    deletion_locked: set[int],
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
    group_ids: list[int],
    max_edits: int,
    min_target_count: int,
    max_target_count: int,
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any]]:
    """Beam-search additions, removals and swaps across final target counts.

    Intermediate supports may be worse than the starting point.  The final
    exported support must still Pareto-improve aligned Avg and Max Diff and
    stay inside the starting support's weight-dispersion envelope.
    """
    baseline = current
    min_target_count = max(
        len(deletion_locked), int(min_target_count),
    )
    max_target_count = max(
        min_target_count, int(max_target_count),
    )
    if not (
        min_target_count
        <= len(current["support"])
        <= max_target_count
    ):
        raise ValueError("final support beam count range excludes its start")

    current["edit_path"] = []
    beam: list[dict[str, Any]] = [current]
    evaluated: dict[tuple[int, ...], dict[str, Any]] = {
        tuple(sorted(current["support"])): current
    }
    beam_width = 8
    screen_width = 28

    for _depth in range(max(0, max_edits)):
        screened_by_signature: dict[tuple[int, ...], dict[str, Any]] = {}
        for state in beam:
            support = list(state["support"])
            support_set = set(support)
            protected = (
                set(deletion_locked)
                | (set(replacement_locked) & support_set)
            )
            removable = [
                idx for idx in support if idx not in protected
            ]
            additions = _final_swap_addition_shortlist(
                rate_matrix,
                reference_scores,
                correct_scores,
                big_weights,
                support,
                np.asarray(state["selected_weights"], dtype=float),
                eligible,
            )
            operations: list[tuple[str, int | None, int | None]] = []
            if len(support) < max_target_count:
                operations.extend(
                    ("add", None, added) for added in additions
                )
            if len(support) > min_target_count:
                operations.extend(
                    ("remove", removed, None) for removed in removable
                )
            operations.extend(
                ("swap", removed, added)
                for removed in removable
                for added in additions
            )

            for operation, removed, added in operations:
                if operation == "add":
                    proposed = support + [int(added)]
                elif operation == "remove":
                    proposed = [
                        idx for idx in support if idx != int(removed)
                    ]
                else:
                    proposed = [
                        int(added) if idx == int(removed) else idx
                        for idx in support
                    ]
                signature = tuple(sorted(proposed))
                if (
                    signature in evaluated
                    or len(proposed) < min_target_count
                    or len(proposed) > max_target_count
                    or not _support_feasible(
                        proposed,
                        account_keys,
                        owner_keys,
                        owner_cap,
                    )
                ):
                    continue
                metrics = _screen_final_support(
                    rate_matrix,
                    big_weights,
                    golden_weights,
                    reference_scores,
                    correct_scores,
                    proposed,
                )
                edit = {
                    "operation": operation,
                    "removed_index": removed,
                    "added_index": added,
                }
                candidate = {
                    "support": proposed,
                    "metrics": metrics,
                    "edit_path": list(state.get("edit_path", [])) + [edit],
                    "support_c_score_sum": float(np.sum(
                        correct_scores[np.asarray(proposed, dtype=int)]
                    )),
                }
                previous = screened_by_signature.get(signature)
                if previous is None or _support_replay_key(
                    candidate
                ) < _support_replay_key(previous):
                    screened_by_signature[signature] = candidate
        if not screened_by_signature:
            break

        screened = list(screened_by_signature.values())
        shortlist: dict[tuple[int, ...], dict[str, Any]] = {}

        def retain(rows: list[dict[str, Any]], count: int) -> None:
            for item in rows[:count]:
                shortlist[tuple(sorted(item["support"]))] = item

        retain(sorted(screened, key=_support_replay_key), 16)
        retain(sorted(
            screened,
            key=lambda item: (
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
            ),
        ), 8)
        retain(sorted(
            screened,
            key=lambda item: (
                float(item["metrics"]["aligned_mean_abs_diff"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
            ),
        ), 8)
        for count in range(min_target_count, max_target_count + 1):
            rows = [
                item for item in screened
                if len(item["support"]) == count
            ]
            if rows:
                retain(sorted(rows, key=_support_replay_key), 1)
        screened_shortlist = sorted(
            shortlist.values(), key=_support_replay_key,
        )[:screen_width]

        fast_candidates: list[dict[str, Any]] = []
        for screened_item in screened_shortlist:
            try:
                candidate = _final_stage_from_fresh_support(
                    rate_matrix,
                    big_weights,
                    golden_weights,
                    reference_scores,
                    correct_scores,
                    list(screened_item["support"]),
                    replacement_locked,
                    deletion_locked,
                    regularization_factors=[
                        0.0, 1e-3, 1e-2, 1e-1,
                    ],
                )
            except (RuntimeError, ValueError, np.linalg.LinAlgError):
                continue
            candidate["edit_path"] = list(
                screened_item["edit_path"]
            )
            candidate["support_c_score_sum"] = float(
                screened_item["support_c_score_sum"]
            )
            signature = tuple(sorted(candidate["support"]))
            evaluated[signature] = candidate
            fast_candidates.append(candidate)
        if not fast_candidates:
            break

        pool = list(beam) + fast_candidates
        next_beam: dict[tuple[int, ...], dict[str, Any]] = {}
        for count in range(min_target_count, max_target_count + 1):
            rows = [
                item for item in pool
                if len(item["support"]) == count
            ]
            if rows:
                best = min(rows, key=_support_replay_key)
                next_beam[tuple(sorted(best["support"]))] = best
        for item in sorted(pool, key=_support_replay_key):
            next_beam.setdefault(tuple(sorted(item["support"])), item)
            if len(next_beam) >= beam_width:
                break
        beam = sorted(
            next_beam.values(), key=_support_replay_key,
        )[:beam_width]

    evaluated_rows = list(evaluated.values())
    final_shortlist: dict[tuple[int, ...], dict[str, Any]] = {}

    def retain_final(
        rows: list[dict[str, Any]], count: int,
    ) -> None:
        for item in rows[:count]:
            final_shortlist[tuple(sorted(item["support"]))] = item

    retain_final(sorted(evaluated_rows, key=_support_replay_key), 10)
    retain_final(sorted(
        evaluated_rows,
        key=lambda item: (
            float(item["metrics"]["aligned_max_abs_diff"]),
            float(item["metrics"]["aligned_mean_abs_diff"]),
        ),
    ), 6)
    retain_final(sorted(
        evaluated_rows,
        key=lambda item: (
            float(item["metrics"]["aligned_mean_abs_diff"]),
            float(item["metrics"]["aligned_max_abs_diff"]),
        ),
    ), 6)

    full_candidates: list[dict[str, Any]] = [baseline]
    baseline_signature = tuple(sorted(baseline["support"]))
    for fast_item in sorted(
        final_shortlist.values(), key=_support_replay_key,
    )[:12]:
        if tuple(sorted(fast_item["support"])) == baseline_signature:
            continue
        try:
            candidate = _final_stage_from_fresh_support(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                list(fast_item["support"]),
                replacement_locked,
                deletion_locked,
            )
        except (RuntimeError, ValueError, np.linalg.LinAlgError):
            continue
        candidate["edit_path"] = list(fast_item.get("edit_path", []))
        candidate["support_c_score_sum"] = float(np.sum(
            correct_scores[np.asarray(candidate["support"], dtype=int)]
        ))
        full_candidates.append(candidate)

    before = baseline["metrics"]
    before_max = float(before["aligned_max_abs_diff"])
    before_avg = float(before["aligned_mean_abs_diff"])
    before_std = float(before["final_weight_std"])
    before_structure = float(before["structure_score"])
    before_deviation = float(
        before.get(
            "absorption_max_weight_deviation",
            np.max(np.abs(
                np.asarray(baseline["selected_weights"], dtype=float)
                - float(np.mean(baseline["selected_weights"]))
            )),
        )
    )
    admissible = [
        item for item in full_candidates
        if float(item["metrics"]["aligned_max_abs_diff"])
        <= before_max + 1e-9
        and float(item["metrics"]["aligned_mean_abs_diff"])
        <= before_avg + 1e-9
        and (
            float(item["metrics"]["aligned_max_abs_diff"])
            < before_max - 1e-7
            or float(item["metrics"]["aligned_mean_abs_diff"])
            < before_avg - 1e-7
        )
        and float(item["metrics"]["final_weight_std"])
        <= max(before_std * 1.10, before_std + 0.02) + 1e-9
        and float(item["metrics"]["structure_score"])
        <= max(before_structure * 1.15, before_structure + 0.03) + 1e-9
        and float(item["metrics"]["absorption_max_weight_deviation"])
        <= max(
            before_deviation * 1.10,
            before_deviation + 0.03,
        ) + 1e-9
    ]
    if admissible:
        best_max = min(
            float(item["metrics"]["aligned_max_abs_diff"])
            for item in admissible
        )
        max_plateau = [
            item for item in admissible
            if float(item["metrics"]["aligned_max_abs_diff"])
            <= best_max + 0.005
        ]
        chosen = min(
            max_plateau,
            key=lambda item: (
                0.65 * float(item["metrics"]["aligned_mean_abs_diff"])
                + 0.35 * float(item["metrics"]["aligned_rmse"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["structure_score"]),
                -float(item.get("support_c_score_sum", 0.0)),
            ),
        )
    else:
        chosen = baseline

    audit: list[dict[str, Any]] = []
    for step, edit in enumerate(chosen.get("edit_path", []), start=1):
        removed = edit.get("removed_index")
        added = edit.get("added_index")
        audit.append({
            "step": step,
            "operation": edit["operation"],
            "removed_index": removed,
            "added_index": added,
            "removed_group_id": (
                None if removed is None else int(group_ids[int(removed)])
            ),
            "added_group_id": (
                None if added is None else int(group_ids[int(added)])
            ),
            "start_target_count": len(baseline["support"]),
            "final_target_count": len(chosen["support"]),
            "before_aligned_mean_abs_diff": before_avg,
            "after_aligned_mean_abs_diff": float(
                chosen["metrics"]["aligned_mean_abs_diff"]
            ),
            "before_aligned_max_abs_diff": before_max,
            "after_aligned_max_abs_diff": float(
                chosen["metrics"]["aligned_max_abs_diff"]
            ),
            "before_aligned_rmse": float(before["aligned_rmse"]),
            "after_aligned_rmse": float(
                chosen["metrics"]["aligned_rmse"]
            ),
            "before_final_weight_std": before_std,
            "after_final_weight_std": float(
                chosen["metrics"]["final_weight_std"]
            ),
            "lock_rule": "fixed_deletion_c_score_top30",
        })
    search_audit = {
        "applied": chosen is not baseline,
        "max_edits": max_edits,
        "beam_width": beam_width,
        "screen_width": screen_width,
        "min_target_count": min_target_count,
        "max_target_count": max_target_count,
        "evaluated_support_count": len(evaluated),
        "full_finalist_count": len(full_candidates),
        "start_target_count": len(baseline["support"]),
        "final_target_count": len(chosen["support"]),
        "start_aligned_mean_abs_diff": before_avg,
        "final_aligned_mean_abs_diff": float(
            chosen["metrics"]["aligned_mean_abs_diff"]
        ),
        "start_aligned_max_abs_diff": before_max,
        "final_aligned_max_abs_diff": float(
            chosen["metrics"]["aligned_max_abs_diff"]
        ),
        "start_aligned_rmse": float(before["aligned_rmse"]),
        "final_aligned_rmse": float(
            chosen["metrics"]["aligned_rmse"]
        ),
        "selection_rule": (
            "minimax_0.005_plateau_then_avg_rmse_with_avg_and_max_"
            "nonworsening_and_weight_dispersion_guards"
        ),
    }
    return chosen, audit, search_audit


def _run_final_tail_swap_refinement(
    rate_matrix: np.ndarray,
    big_weights: np.ndarray,
    golden_weights: np.ndarray,
    reference_scores: np.ndarray,
    correct_scores: np.ndarray,
    current: dict[str, Any],
    eligible: list[int],
    replacement_locked: set[int],
    deletion_locked: set[int],
    account_keys: list[list[str]],
    owner_keys: list[str],
    owner_cap: int,
    group_ids: list[int],
    max_swaps: int,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Pareto-improve the final support without reopening weight dispersion.

    The fixed C-Score Top30 and every still-present initial/player-Top3
    intersection are immutable.  Legal one-for-one swaps are accepted only
    when neither aligned Max Diff nor Avg Diff increases.
    """
    audit: list[dict[str, Any]] = []
    seen = {tuple(sorted(current["support"]))}
    for _step in range(max(0, max_swaps)):
        support = list(current["support"])
        support_set = set(support)
        protected = (
            set(deletion_locked)
            | (set(replacement_locked) & support_set)
        )
        removable = [idx for idx in support if idx not in protected]
        additions = _final_swap_addition_shortlist(
            rate_matrix,
            reference_scores,
            correct_scores,
            big_weights,
            support,
            np.asarray(current["selected_weights"], dtype=float),
            eligible,
        )
        if not removable or not additions:
            break

        screened: list[dict[str, Any]] = []
        for removed in removable:
            for added in additions:
                proposed = [
                    added if idx == removed else idx for idx in support
                ]
                signature = tuple(sorted(proposed))
                if (
                    signature in seen
                    or not _support_feasible(
                        proposed,
                        account_keys,
                        owner_keys,
                        owner_cap,
                        len(support),
                    )
                ):
                    continue
                metrics = _screen_final_support(
                    rate_matrix,
                    big_weights,
                    golden_weights,
                    reference_scores,
                    correct_scores,
                    proposed,
                )
                screened.append({
                    "support": proposed,
                    "metrics": metrics,
                    "removed_index": removed,
                    "added_index": added,
                })
        if not screened:
            break
        screened.sort(
            key=lambda item: (
                0.70 * float(item["metrics"]["aligned_max_abs_diff"])
                + 0.20 * float(item["metrics"]["aligned_rmse"])
                + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
                tuple(sorted(item["support"])),
            )
        )

        fast_candidates: list[dict[str, Any]] = []
        for screened_item in screened[:12]:
            candidate = _final_stage_from_fresh_support(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                list(screened_item["support"]),
                replacement_locked,
                deletion_locked,
                regularization_factors=[
                    0.0, 1e-3, 1e-2, 1e-1,
                ],
            )
            candidate["removed_index"] = screened_item["removed_index"]
            candidate["added_index"] = screened_item["added_index"]
            fast_candidates.append(candidate)
        fast_candidates.sort(
            key=lambda item: (
                0.70 * float(item["metrics"]["aligned_max_abs_diff"])
                + 0.20 * float(item["metrics"]["aligned_rmse"])
                + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
            )
        )

        full_candidates: list[dict[str, Any]] = []
        for fast_item in fast_candidates[:4]:
            candidate = _final_stage_from_fresh_support(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                list(fast_item["support"]),
                replacement_locked,
                deletion_locked,
            )
            candidate["removed_index"] = fast_item["removed_index"]
            candidate["added_index"] = fast_item["added_index"]
            full_candidates.append(candidate)

        before = current["metrics"]
        before_max = float(before["aligned_max_abs_diff"])
        before_avg = float(before["aligned_mean_abs_diff"])
        before_std = float(before["final_weight_std"])
        before_structure = float(before["structure_score"])
        admissible = [
            item for item in full_candidates
            if float(item["metrics"]["aligned_max_abs_diff"])
            <= before_max + 1e-9
            and float(item["metrics"]["aligned_mean_abs_diff"])
            <= before_avg + 1e-9
            and (
                float(item["metrics"]["aligned_max_abs_diff"])
                < before_max - 1e-7
                or float(item["metrics"]["aligned_mean_abs_diff"])
                < before_avg - 1e-7
            )
            and float(item["metrics"]["final_weight_std"])
            <= max(before_std * 1.10, before_std + 0.02) + 1e-9
            and float(item["metrics"]["structure_score"])
            <= max(before_structure * 1.15, before_structure + 0.03) + 1e-9
        ]
        if not admissible:
            break
        chosen = min(
            admissible,
            key=lambda item: (
                0.70 * float(item["metrics"]["aligned_max_abs_diff"])
                + 0.20 * float(item["metrics"]["aligned_rmse"])
                + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
                -float(np.sum(correct_scores[
                    np.asarray(item["support"], dtype=int)
                ])),
            ),
        )
        removed = int(chosen["removed_index"])
        added = int(chosen["added_index"])
        audit.append({
            "step": len(audit) + 1,
            "removed_index": removed,
            "added_index": added,
            "removed_group_id": int(group_ids[removed]),
            "added_group_id": int(group_ids[added]),
            "before_aligned_mean_abs_diff": before_avg,
            "after_aligned_mean_abs_diff": float(
                chosen["metrics"]["aligned_mean_abs_diff"]
            ),
            "before_aligned_max_abs_diff": before_max,
            "after_aligned_max_abs_diff": float(
                chosen["metrics"]["aligned_max_abs_diff"]
            ),
            "before_aligned_rmse": float(before["aligned_rmse"]),
            "after_aligned_rmse": float(
                chosen["metrics"]["aligned_rmse"]
            ),
            "before_final_weight_std": before_std,
            "after_final_weight_std": float(
                chosen["metrics"]["final_weight_std"]
            ),
            "lock_rule": "fixed_deletion_c_score_top30",
        })
        current = chosen
        seen.add(tuple(sorted(current["support"])))
    return current, audit


def _solve_top40_dynamic10_aligned_minimax(payload: dict[str, Any]) -> dict[str, Any]:
    big = np.asarray(payload["big_weights"], dtype=float)
    golden = np.asarray(
        payload.get("golden_weights", np.zeros_like(big)), dtype=float,
    )
    rates = np.asarray(payload["rate_matrix"], dtype=float)
    scores = np.asarray(payload["reference_scores"], dtype=float)
    correct = np.asarray(payload["candidate_correct_scores"], dtype=float)
    if golden.shape != big.shape or not np.all(np.isfinite(golden)):
        golden = np.zeros_like(big)
    status = [str(value) for value in payload["selection_status"]]
    blocked = np.asarray(payload["blocked"], dtype=bool)
    accounts = [[str(key) for key in values] for values in payload["account_keys"]]
    raw_members = [[str(key) for key in values] for values in payload.get("raw_members", [[] for _ in range(big.size)])]
    owners = [str(value) for value in payload["owner_keys"]]
    owner_cap = int(payload.get("owner_cap", 5))
    # Lane-1 candidates are already individual groups.  The account/owner
    # collision limits are pair-lane constraints and must not make a locked
    # single-name front infeasible.
    single_name_mode = all(len(values) <= 1 for values in raw_members)
    locked_count = int(payload.get("locked_big_weight_count", 40))
    dynamic_count = int(payload.get("dynamic_big_weight_count", 10))
    delta_limit = float(payload.get("delta_limit", 0.1))
    total = locked_count + dynamic_count
    mass = float(np.sum(big))

    # Reproduce the browser main-board folding before the weight MILP.  A
    # higher-Correct parent consumes lower-ranked candidates sharing one of
    # its displayed members; their complete BigWeight is transferred to that
    # parent (mass-conserving, never duplicated).  This makes a newly promoted
    # sibling such as ⑨+ⅺ inherit the weight of the displaced ⑨+凯洛斯总督
    # instead of being filtered out merely because its own Golden weight was 0.
    pair_ranks = list(payload.get("pair_ranks", [None] * big.size))
    raw_ranks = [int(value) for value in payload.get("raw_ranks", range(big.size))]
    group_ids = [int(value) for value in payload.get("group_ids", range(big.size))]
    if len(pair_ranks) != big.size or len(raw_ranks) != big.size or len(group_ids) != big.size:
        raise ValueError("invalid browser-order metadata length")
    browser_order = _browser_main_parent_order(
        pair_ranks, correct, raw_ranks, group_ids, status, blocked, raw_members,
    )
    eligible_for_fold = {
        idx for idx in range(big.size)
        if not blocked[idx] and status[idx] not in {"below_threshold", "blocked"}
        and pair_ranks[idx] is not None and math.isfinite(float(correct[idx]))
    }
    inherited_big = big.copy()
    inheritance = []
    consumed_children: set[int] = set()
    parent_set = set(browser_order)
    for parent in browser_order:
        if parent not in eligible_for_fold:
            continue
        parent_members = set(raw_members[parent])
        if not parent_members:
            continue
        child_indices = []
        for child in range(big.size):
            if child == parent or child in consumed_children or child in parent_set:
                continue
            if child not in eligible_for_fold:
                continue
            if parent_members.intersection(raw_members[child]):
                child_indices.append(child)
        # Include candidates that the browser folding routine consumed but
        # which are not returned as parents (the normal parent/child case).
        if child_indices:
            transferred = float(np.sum(big[np.asarray(child_indices, dtype=int)]))
            inherited_big[parent] += transferred
            inherited_big[np.asarray(child_indices, dtype=int)] = 0.0
            consumed_children.update(child_indices)
            inheritance.append({
                "parent_index": int(parent),
                "child_indices": [int(idx) for idx in child_indices],
                "transferred_weight": transferred,
            })
    # Candidates not folded into a higher-ranked parent retain their own
    # weight.  The MILP sees only positive inherited weights, so zeroed child
    # rows cannot re-enter as independent support items.
    big = inherited_big
    eligible = [
        idx for idx in range(big.size)
        if big[idx] > 1e-12 and not blocked[idx]
        and status[idx] not in {"below_threshold", "blocked"}
    ]
    parent_rank = {idx: rank for rank, idx in enumerate(browser_order)}
    ordered = sorted(
        eligible,
        key=lambda idx: (-big[idx], parent_rank.get(idx, len(parent_rank)), idx),
    )
    if len(ordered) < total:
        raise RuntimeError("top40+dynamic10 compression has fewer than 50 positive eligible big targets")
    # Build the locked front by score while respecting the per-team cap.
    # This avoids making the MILP infeasible when the raw top-40 contains
    # more than owner_cap candidates from one team.  The skipped candidates
    # remain available to the dynamic tail.
    locked = []
    locked_owner_counts: dict[str, int] = {}
    deferred = []
    for idx in ordered:
        owner = owners[idx]
        if locked_owner_counts.get(owner, 0) >= owner_cap:
            deferred.append(idx)
            continue
        locked.append(idx)
        locked_owner_counts[owner] = locked_owner_counts.get(owner, 0) + 1
        if len(locked) >= locked_count:
            break
    if len(locked) < locked_count:
        raise RuntimeError("cannot construct owner-cap-feasible locked front")
    locked_set = set(locked)
    pool = [idx for idx in ordered if idx not in locked_set]
    active = locked + pool
    m = len(active)
    local_big = big[np.asarray(active, dtype=int)]
    locked_mass = float(np.sum(local_big[:locked_count]))
    tail_sorted = np.sort(local_big[locked_count:])
    scale_lo = 50.0 / (locked_mass + float(np.sum(tail_sorted[-dynamic_count:])))
    scale_hi = 50.0 / (locked_mass + float(np.sum(tail_sorted[:dynamic_count])))

    def solve_slope(
        slope: float,
        center_bounds: dict[int, float] | None = None,
        fixed_support: set[int] | None = None,
    ) -> dict[str, Any]:
        # z, q=normalization_scale*z, delta, normalization_scale, intercept, max error
        z0, q0, d0 = 0, m, 2 * m
        scale_idx, intercept_idx, error_idx = 3 * m, 3 * m + 1, 3 * m + 2
        count = 3 * m + 3
        c = np.zeros(count); c[error_idx] = 1.0
        integrality = np.zeros(count, dtype=int); integrality[z0:z0+m] = 1
        lb = np.full(count, -np.inf); ub = np.full(count, np.inf)
        lb[z0:z0+m] = 0.0; ub[z0:z0+m] = 1.0
        lb[z0:z0+locked_count] = 1.0; ub[z0:z0+locked_count] = 1.0
        if fixed_support is not None:
            for i, original in enumerate(active):
                value = 1.0 if original in fixed_support else 0.0
                lb[z0+i] = value
                ub[z0+i] = value
        lb[q0:q0+m] = 0.0; ub[q0:q0+m] = scale_hi
        lb[d0:d0+m] = -delta_limit; ub[d0:d0+m] = delta_limit
        lb[scale_idx] = scale_lo; ub[scale_idx] = scale_hi
        lb[intercept_idx] = -100.0; ub[intercept_idx] = 100.0
        lb[error_idx] = 0.0
        rows, lows, highs = [], [], []
        def add(values, lo=-np.inf, hi=np.inf):
            rows.append(values); lows.append(lo); highs.append(hi)
        if fixed_support is None:
            add({z0+i: 1.0 for i in range(locked_count, m)}, dynamic_count, dynamic_count)
        for i in range(m):
            add({q0+i: 1.0, z0+i: -scale_lo}, 0.0, np.inf)
            add({q0+i: 1.0, z0+i: -scale_hi}, -np.inf, 0.0)
            add({q0+i: 1.0, scale_idx: -1.0, z0+i: -scale_hi}, -scale_hi, np.inf)
            add({q0+i: 1.0, scale_idx: -1.0, z0+i: -scale_lo}, -np.inf, -scale_lo)
            add({d0+i: 1.0, z0+i: -delta_limit}, -np.inf, 0.0)
            add({d0+i: -1.0, z0+i: -delta_limit}, -np.inf, 0.0)
            add({q0+i: local_big[i], d0+i: 1.0}, 0.0, np.inf)
            if center_bounds is not None and active[i] in center_bounds:
                centre = float(center_bounds[active[i]])
                add({q0+i: local_big[i], d0+i: 1.0}, centre - 0.1, centre + 0.1)
        add({q0+i: local_big[i] for i in range(m)}, 50.0, 50.0)
        add({d0+i: 1.0 for i in range(m)}, 0.0, 0.0)
        account_cols: dict[str, list[int]] = {}
        owner_cols: dict[str, list[int]] = {}
        for i, original in enumerate(active):
            for key in accounts[original]: account_cols.setdefault(key, []).append(i)
            owner_cols.setdefault(owners[original], []).append(i)
        if not single_name_mode:
            for cols in account_cols.values():
                if len(cols) > 1: add({z0+i: 1.0 for i in cols}, -np.inf, 1.0)
        for cols in owner_cols.values():
            if len(cols) > owner_cap: add({z0+i: 1.0 for i in cols}, -np.inf, float(owner_cap))
        for row in range(rates.shape[0]):
            pos = {intercept_idx: 1.0, error_idx: -1.0}
            neg = {intercept_idx: -1.0, error_idx: -1.0}
            for i, original in enumerate(active):
                coefficient = slope * rates[row, original] / 50.0
                pos[q0+i] = coefficient * local_big[i]; pos[d0+i] = coefficient
                neg[q0+i] = -coefficient * local_big[i]; neg[d0+i] = -coefficient
            add(pos, -np.inf, float(scores[row])); add(neg, -np.inf, float(-scores[row]))
        A = lil_matrix((len(rows), count), dtype=float)
        for r, values in enumerate(rows):
            for col, value in values.items(): A[r, col] = value
        # Configurable per-MILP budget.  Thirty seconds is the formal default;
        # callers may raise it for a deliberately higher-budget audit run.
        try:
            time_limit = float(os.environ.get("TARGET_MILP_TIME_LIMIT", "30"))
        except ValueError:
            time_limit = 30.0
        if not math.isfinite(time_limit) or time_limit <= 0.0:
            time_limit = 30.0
        result = milp(
            c=c, integrality=integrality, bounds=Bounds(lb, ub),
            constraints=LinearConstraint(A.tocsr(), np.asarray(lows), np.asarray(highs)),
            options={"time_limit": time_limit, "mip_rel_gap": 1e-8, "presolve": True},
        )
        if result.x is None: raise RuntimeError(result.message)
        z = result.x[z0:z0+m]; q = result.x[q0:q0+m]; delta = result.x[d0:d0+m]
        positions = np.flatnonzero(z > 0.5)
        support = [active[int(i)] for i in positions]
        weights = local_big[positions] * q[positions] + delta[positions]
        return {
            "support": support, "weights": weights,
            "base_export": local_big[positions] * q[positions],
            "delta": delta[positions], "slope": slope,
            "intercept": float(result.x[intercept_idx]),
            "max": float(result.x[error_idx]),
            "gap": float(getattr(result, "mip_gap", 0.0) or 0.0),
            "scale": float(result.x[scale_idx]),
        }

    coarse = [0.90, 0.95, 1.00, 1.05, 1.10]
    # Independent slope MILPs can run concurrently.  Results are collected in
    # slope-list order so the existing deterministic tie-break is unchanged.
    try:
        solver_workers = int(os.environ.get("TARGET_SOLVER_WORKERS", "8"))
    except ValueError:
        solver_workers = 8
    solver_workers = max(1, min(solver_workers, 20))

    # HiGHS (used by scipy.optimize.milp) is not safe when several solves are
    # launched concurrently from threads in one Python process on Windows.
    # Keep the same bounded parallelism, but isolate every solve in its own
    # worker process.  The immutable model inputs are copied once into each
    # worker through the initializer rather than captured by a local closure
    # (which is not picklable under the Windows ``spawn`` start method).
    try:
        solver_time_limit = float(os.environ.get("TARGET_MILP_TIME_LIMIT", "30"))
    except ValueError:
        solver_time_limit = 30.0
    if not math.isfinite(solver_time_limit) or solver_time_limit <= 0.0:
        solver_time_limit = 30.0
    worker_context = {
        "active": active,
        "local_big": local_big,
        "locked_count": locked_count,
        "dynamic_count": dynamic_count,
        "scale_lo": scale_lo,
        "scale_hi": scale_hi,
        "delta_limit": delta_limit,
        "accounts": accounts,
        "owners": owners,
        "owner_cap": owner_cap,
        "single_name_mode": single_name_mode,
        "rates": rates,
        "scores": scores,
        "time_limit": solver_time_limit,
    }

    def solve_many(slopes):
        if solver_workers <= 1 or len(slopes) <= 1:
            return [solve_slope(value) for value in slopes]
        with ProcessPoolExecutor(
            max_workers=min(solver_workers, len(slopes)),
            initializer=_init_top40_worker,
            initargs=(worker_context,),
        ) as pool:
            # map() preserves slope-list order, retaining deterministic
            # tie-breaking while avoiding same-process HiGHS concurrency.
            return list(pool.map(_top40_worker_slope, slopes))

    def solve_center_refit(slope: float, centers: np.ndarray) -> dict[str, Any]:
        """Refit weights on the first-pass support using only centre intervals.

        This deliberately does not reuse the original q/delta formulation or
        its uniform +/-0.1 delta bound.  The second pass is a fresh continuous
        minimax problem whose only weight constraints are centre_i +/- 0.1 and
        total mass 50.
        """
        if centers.shape != (len(support),):
            raise ValueError("center refit requires one centre per selected candidate")
        support_array2 = np.asarray(support, dtype=int)
        m2 = len(support)
        weight0, intercept2, error2 = 0, m2, m2 + 1
        count2 = m2 + 2
        c2 = np.zeros(count2); c2[error2] = 1.0
        lb2 = np.full(count2, -np.inf); ub2 = np.full(count2, np.inf)
        lb2[weight0:weight0 + m2] = centers - 0.1
        ub2[weight0:weight0 + m2] = centers + 0.1
        lb2[intercept2] = -100.0; ub2[intercept2] = 100.0
        lb2[error2] = 0.0
        rows2, lows2, highs2 = [], [], []
        rows2.append({weight0 + i: 1.0 for i in range(m2)}); lows2.append(50.0); highs2.append(50.0)
        for row in range(rates.shape[0]):
            pos = {intercept2: 1.0, error2: -1.0}
            neg = {intercept2: -1.0, error2: -1.0}
            for i, original in enumerate(support):
                coefficient = slope * rates[row, original] / 50.0
                pos[weight0 + i] = coefficient
                neg[weight0 + i] = -coefficient
            rows2.extend([pos, neg]); lows2.extend([-np.inf, -np.inf]); highs2.extend([float(scores[row]), float(-scores[row])])
        A2 = lil_matrix((len(rows2), count2), dtype=float)
        for r, values in enumerate(rows2):
            for col, value in values.items(): A2[r, col] = value
        try:
            time_limit = float(os.environ.get("TARGET_MILP_TIME_LIMIT", "30"))
        except ValueError:
            time_limit = 30.0
        if not math.isfinite(time_limit) or time_limit <= 0.0:
            time_limit = 30.0
        result = milp(
            c=c2, bounds=Bounds(lb2, ub2),
            constraints=LinearConstraint(A2.tocsr(), np.asarray(lows2), np.asarray(highs2)),
            options={"time_limit": time_limit, "mip_rel_gap": 1e-8, "presolve": True},
        )
        if result.x is None:
            raise RuntimeError(result.message)
        weights2 = result.x[weight0:weight0 + m2]
        return {
            "support": list(support),
            "weights": weights2,
            "base_export": np.asarray(base_export_seed, dtype=float),
            "delta": weights2 - big[support_array2],
            "slope": slope,
            "intercept": float(result.x[intercept2]),
            "max": float(result.x[error2]),
            "gap": float(getattr(result, "mip_gap", 0.0) or 0.0),
            "scale": 1.0,
        }

    solved = solve_many(coarse)
    center = min(solved, key=lambda item: item["max"])["slope"]
    fine = sorted(set(center + step for step in [-0.04, -0.03, -0.02, -0.01, 0.01, 0.02, 0.03, 0.04] if center + step > 0.0))
    solved.extend(solve_many([value for value in fine if value not in coarse]))
    center = min(solved, key=lambda item: item["max"])["slope"]
    micro = sorted(set(center + step for step in [-0.008, -0.006, -0.004, -0.002, 0.002, 0.004, 0.006, 0.008] if center + step > 0.0))
    solved.extend(solve_many([value for value in micro if value not in {x["slope"] for x in solved}]))
    best = min(solved, key=lambda item: item["max"])
    support = best["support"]
    selected = np.asarray(best["weights"], dtype=float)
    base_export = np.asarray(best["base_export"], dtype=float)
    base = big[np.asarray(support, dtype=int)]
    base_export_seed = base_export.copy()

    # Second pass: keep the first-pass support fixed, move each candidate's
    # centre according to Golden/CQD, then solve the same replay MILP again
    # with an individual centre +/- 0.1 interval.  This is preferable to a
    # post-hoc multiplication because the new centres participate in the
    # actual rate-matrix optimization.
    center_refit_info = {
        "applied": False,
        "rule": "post_compression_center_refit_fixed_support_v1",
        "interval": 0.1,
    }
    golden_max = float(np.max(golden)) if golden.size else 0.0
    support_array = np.asarray(support, dtype=int)
    support_golden = golden[support_array] if golden_max > 1e-12 else np.zeros(len(support))
    high_mask = support_golden > 0.99 * golden_max if golden_max > 1e-12 else np.zeros(len(support), dtype=bool)
    low_mask = ~high_mask
    if np.any(high_mask) and np.any(low_mask):
        factors = np.ones(len(support), dtype=float)
        support_scores = correct[support_array]
        high_scores = support_scores[high_mask]
        score_lo = float(np.min(high_scores))
        score_hi = float(np.max(high_scores))
        high_u = (
            np.clip((high_scores - score_lo) / (score_hi - score_lo), 0.0, 1.0)
            if score_hi - score_lo > 1e-12
            else np.zeros_like(high_scores)
        )
        factors[high_mask] = 1.0 + 0.25 * np.power(high_u, 1.35)
        factors[low_mask] = 0.75 + 0.25 * np.clip(support_golden[low_mask] / golden_max, 0.0, 1.0)
        centers = selected * factors
        centers *= float(np.sum(selected)) / max(float(np.sum(centers)), 1e-12)
        try:
            refit = solve_center_refit(best["slope"], centers)
        except Exception as exc:
            refit = None
            center_refit_info["error"] = f"{type(exc).__name__}: {exc}"
        if refit is not None and set(refit["support"]) == set(support):
            selected = np.asarray(refit["weights"], dtype=float)
            base_export = np.asarray(refit["base_export"], dtype=float)
            best = refit
            center_refit_info.update({
                "applied": True,
                "support_count": len(support),
                "high_count": int(np.sum(high_mask)),
                "low_count": int(np.sum(low_mask)),
                "center_min": float(np.min(centers)),
                "center_max": float(np.max(centers)),
                "weight_min": float(np.min(selected)),
                "weight_max": float(np.max(selected)),
            })
        elif refit is not None:
            center_refit_info["error"] = "refit support differs from first-pass support"

    # The MILP is deliberately focused on replay error and therefore often
    # leaves a block of equal weights when several Golden anchors are equal.
    # Apply a deterministic post-compression centre spread to those anchors:
    # strong Golden rows rise with CQD (up to 1.25x), while weaker Golden rows
    # are reduced (down to 0.75x).  The spread is then blended back until the
    # replay max error stays within a small, explicit tolerance of the MILP
    # optimum.  This keeps the optimization/support semantics unchanged.
    spread_info = {
        "applied": False,
        "rule": "post_compression_golden_center_spread_v1",
        "high_golden_threshold": 0.99,
        "high_multiplier_cap": 1.25,
        "low_multiplier_floor": 0.75,
        "blend": 0.0,
        "high_count": 0,
        "low_count": 0,
    }
    golden_max = float(np.max(golden)) if golden.size else 0.0
    support_golden = golden[np.asarray(support, dtype=int)] if golden_max > 1e-12 else np.zeros(len(support))
    high_mask = support_golden > 0.99 * golden_max if golden_max > 1e-12 else np.zeros(len(support), dtype=bool)
    low_mask = ~high_mask
    spread_info["high_count"] = int(np.sum(high_mask))
    spread_info["low_count"] = int(np.sum(low_mask))
    if not center_refit_info["applied"] and np.any(high_mask) and np.any(low_mask):
        factors = np.ones(len(support), dtype=float)
        high_scores = correct[np.asarray(support, dtype=int)][high_mask]
        score_lo = float(np.min(high_scores))
        score_hi = float(np.max(high_scores))
        if score_hi - score_lo > 1e-12:
            high_u = np.clip((high_scores - score_lo) / (score_hi - score_lo), 0.0, 1.0)
        else:
            high_u = np.zeros_like(high_scores)
        # A power > 1 keeps the lower-CQD end visually close to the original
        # centre while reserving the larger lift for genuinely strong rows.
        factors[high_mask] = 1.0 + 0.25 * np.power(high_u, 1.35)
        golden_ratio = np.clip(support_golden[low_mask] / golden_max, 0.0, 1.0)
        factors[low_mask] = 0.75 + 0.25 * golden_ratio
        desired = selected * factors
        desired *= float(np.sum(selected)) / max(float(np.sum(desired)), 1e-12)

        baseline_metrics = _inherited_compression_metrics(
            rates, big, scores, support, selected, base_export, correct,
            best["slope"], best["intercept"],
        )
        desired_metrics = _inherited_compression_metrics(
            rates, big, scores, support, desired, base_export, correct,
            best["slope"], best["intercept"],
        )
        # Permit only a small absolute replay degradation, and never cross the
        # existing 0.20 audit target solely because of the visual spread.
        baseline_max = float(baseline_metrics["aligned_max_abs_diff"])
        allowed_max = min(0.20, baseline_max + 0.02)
        if float(desired_metrics["aligned_max_abs_diff"]) <= allowed_max + 1e-12:
            blend = 1.0
        else:
            lo, hi = 0.0, 1.0
            for _ in range(36):
                mid = (lo + hi) / 2.0
                trial = selected + mid * (desired - selected)
                trial_metrics = _inherited_compression_metrics(
                    rates, big, scores, support, trial, base_export, correct,
                    best["slope"], best["intercept"],
                )
                if float(trial_metrics["aligned_max_abs_diff"]) <= allowed_max + 1e-12:
                    lo = mid
                else:
                    hi = mid
            blend = lo
        if blend > 1e-9:
            selected = selected + blend * (desired - selected)
            selected *= float(np.sum(best["weights"])) / max(float(np.sum(selected)), 1e-12)
            spread_info["applied"] = True
            spread_info["blend"] = float(blend)
            spread_info["factor_min"] = float(np.min(factors))
            spread_info["factor_max"] = float(np.max(factors))

    lineage = selected * mass / 50.0
    seed_lineage = base_export * mass / 50.0
    additions = lineage - base

    def decorate(metrics):
        metrics.update({
            "regularization_factor": 0.0, "pure_structural_prior": False,
            "structure_score": 0.0,
            "replay_score": float(metrics["aligned_rmse"]) + 0.2 * float(metrics["aligned_max_abs_diff"]),
            "max_diff_target": 0.20,
            "max_diff_target_met": float(metrics["aligned_max_abs_diff"]) <= 0.20 + 1e-9,
            "max_diff_target_margin": 0.20 - float(metrics["aligned_max_abs_diff"]),
            "selection_error_cost_normalized": 0.0,
            "selection_structure_cost_normalized": 0.0,
            "selection_score": float(metrics["aligned_max_abs_diff"]),
            "selection_rule": "top40_dynamic10_normalized_delta_0p1_aligned_minimax_post_golden_spread",
            "golden_unit_equalization_count": 0, "golden_unit_final_spread": 0.0,
            "golden_unit_projection_l2": 0.0, "golden_nonunit_free_count": len(support),
        })
        return metrics
    initial_slope, initial_intercept, _ = affine_chebyshev_fit(
        rates[:, support] @ base_export / 50.0, scores,
    )
    initial_metrics = decorate(_inherited_compression_metrics(
        rates, big, scores, support, base_export, base_export, correct,
        initial_slope, initial_intercept,
    ))
    final_metrics = decorate(_inherited_compression_metrics(
        rates, big, scores, support, selected, base_export, correct,
        best["slope"], best["intercept"],
    ))
    selected_set = set(support); locked_selected = [idx for idx in locked if idx in selected_set]
    deletion_locked = locked_selected[:30]
    tail = [idx for idx in range(big.size) if idx not in selected_set]
    transport = {
        "algorithm": "browser_parent_weight_inheritance_top40_dynamic10_minimax",
        "affine_slope": best["slope"], "affine_intercept": best["intercept"],
        "mip_gap": best["gap"], "normalization_scale": best["scale"],
        "browser_parent_weight_inheritance": {
            "applied": bool(inheritance),
            "rule": "higher_correct_browser_parent_inherits_all_lower_overlapping_child_big_weights",
            "transfer_count": len(inheritance),
            "transferred_weight_sum": float(sum(item["transferred_weight"] for item in inheritance)),
            "transfers": inheritance,
        },
        "post_compression_spread": spread_info,
        "post_compression_center_refit": center_refit_info,
    }
    return {
        "status": "ok_top40_dynamic10_aligned_minimax",
        "browser_initial_indices": support, "initial_indices": support,
        "locked_indices": locked_selected, "pre_deletion_selected_indices": support,
        "deletion_locked_indices": deletion_locked,
        "locked_seed_weight_scale": 1.0,
        "locked_seed_original_weight_sum": float(np.sum(big[locked_selected])),
        "locked_seed_target_weight_sum": float(sum(seed_lineage[pos] for pos, idx in enumerate(support) if idx in set(locked_selected))),
        "initial_anchor_weights": seed_lineage.tolist(),
        "final_seed_anchor_weights": seed_lineage.tolist(), "seed_selection_steps": [],
        "selected_indices": support, "base_weights": base.tolist(),
        "effective_big_weights": big.tolist(),
        "fitted_additions": additions.tolist(), "lineage_weights": lineage.tolist(),
        "selected_weights": selected.tolist(), "replacements": [], "deletions": [],
        "deletion_path": [], "final_tail_swaps": [], "final_support_edits": [],
        "final_support_search": transport, "owner_cap": owner_cap, "target_total": total,
        "deletion_lock_count": 30, "output_target_count": total, "score_denominator": total,
        "tail_group_count": int(sum(abs(big[idx]) > 1e-15 for idx in tail)),
        "tail_weight_sum": float(np.sum(big[tail])),
        "tail_l1_weight_sum": float(np.sum(np.abs(big[tail]))),
        "big_target_weight_sum": mass, "initial_metrics": initial_metrics,
        "initial_transport": transport, "pre_deletion_metrics": initial_metrics,
        "pre_deletion_transport": transport, "final_metrics": final_metrics,
        "final_transport": transport, "regularization_path": [{
            "affine_slope": item["slope"], "aligned_max_abs_diff": item["max"],
            "mip_gap": item["gap"],
        } for item in solved],
        "post_compression_spread": spread_info,
        "post_compression_center_refit": center_refit_info,
    }


def solve_inherited_big_target_compression(payload: dict[str, Any]) -> dict[str, Any]:
    if payload.get("compression_algorithm") == "big_weight_top40_dynamic10_normalized_delta_minimax_v1":
        return _solve_top40_dynamic10_aligned_minimax(payload)
    big_weights = np.asarray(payload["big_weights"], dtype=float)
    golden_weights = np.asarray(
        payload.get("golden_weights", np.maximum(big_weights, 0.0)),
        dtype=float,
    )
    rate_matrix = np.asarray(payload["rate_matrix"], dtype=float)
    reference_scores = np.asarray(payload["reference_scores"], dtype=float)
    correct_scores = np.asarray(payload["candidate_correct_scores"], dtype=float)
    pair_ranks = [
        None if value is None else int(value)
        for value in payload["pair_ranks"]
    ]
    raw_ranks = [int(value) for value in payload["raw_ranks"]]
    group_ids = [int(value) for value in payload["group_ids"]]
    selection_status = [str(value) for value in payload["selection_status"]]
    blocked = np.asarray(payload["blocked"], dtype=bool)
    raw_members = [[str(key) for key in keys] for keys in payload["raw_members"]]
    account_keys = [[str(key) for key in keys] for keys in payload["account_keys"]]
    owner_keys = [str(value) for value in payload["owner_keys"]]
    target_total = int(payload.get("target_total", 50))
    owner_cap = int(payload.get("owner_cap", 5))
    max_replacements = int(payload.get("max_replacements", 8))
    max_deletions = int(payload.get("max_deletions", 0))
    max_final_edits = int(payload.get(
        "max_final_edits",
        payload.get("max_final_swaps", 4),
    ))
    deletion_lock_count = int(payload.get("deletion_lock_count", 30))
    n = big_weights.size
    if (
        n == 0
        or rate_matrix.ndim != 2
        or rate_matrix.shape[1] != n
        or golden_weights.shape != (n,)
        or reference_scores.shape != (rate_matrix.shape[0],)
        or correct_scores.shape != (n,)
        or any(len(values) != n for values in [
            pair_ranks, raw_ranks, group_ids, selection_status, blocked,
            raw_members, account_keys, owner_keys,
        ])
        or not np.all(np.isfinite(big_weights))
        or not np.all(np.isfinite(golden_weights))
        or np.any(golden_weights < 0.0)
        or not np.all(np.isfinite(rate_matrix))
        or not np.all(np.isfinite(reference_scores))
        or not np.all(np.isfinite(correct_scores))
    ):
        raise ValueError("invalid inherited big-target compression payload")
    if (
        target_total <= 0
        or target_total > n
        or owner_cap <= 0
        or deletion_lock_count < 0
        or max_final_edits < 0
    ):
        raise ValueError("invalid target_total/owner_cap")

    parent_order = _browser_main_parent_order(
        pair_ranks,
        correct_scores,
        raw_ranks,
        group_ids,
        selection_status,
        blocked,
        raw_members,
    )
    eligible = [
        idx for idx in range(n)
        if not bool(blocked[idx])
        and selection_status[idx] not in {"below_threshold", "blocked"}
        and pair_ranks[idx] is not None
        and math.isfinite(float(correct_scores[idx]))
    ]
    player_top3_universe = [
        idx for idx in range(n)
        if selection_status[idx] != "below_threshold"
        and pair_ranks[idx] is not None
        and math.isfinite(float(correct_scores[idx]))
    ]
    browser_initial_support, _player_top3_locked = _initial_support_and_locked(
        parent_order,
        eligible,
        player_top3_universe,
        pair_ranks,
        correct_scores,
        raw_ranks,
        group_ids,
        account_keys,
        owner_keys,
        owner_cap,
        target_total,
    )
    # Reset replacement from the actual browser Top50.  Freeze its C-Score
    # Top30 once; the lower 20 remain eligible for low-C-Score-first swaps.
    # The lock never replenishes after a replacement.
    replacement_order = sorted(
        browser_initial_support,
        key=lambda idx: _candidate_order_key(
            idx, pair_ranks, correct_scores, raw_ranks, group_ids,
        ),
    )
    locked_list = replacement_order[:min(30, len(replacement_order))]
    locked = set(locked_list)
    initial_support = list(browser_initial_support)
    seed_selection_steps: list[dict[str, Any]] = []

    def absorption_priority_for(support: list[int]) -> set[int]:
        ordered = sorted(
            support,
            key=lambda idx: _candidate_order_key(
                idx,
                pair_ranks,
                correct_scores,
                raw_ranks,
                group_ids,
            ),
        )
        return set(ordered[:min(deletion_lock_count, len(ordered))])

    current = _evaluate_inherited_support(
        rate_matrix,
        big_weights,
        golden_weights,
        reference_scores,
        correct_scores,
        initial_support,
        locked=locked,
        absorption_priority_indices=absorption_priority_for(
            initial_support
        ),
    )
    initial = current
    replacements: list[dict[str, Any]] = []
    seen = {tuple(sorted(initial_support))}

    for _step in range(max_replacements):
        removals, additions = _replacement_shortlists(
            current,
            eligible,
            locked,
            rate_matrix,
            big_weights,
            reference_scores,
            correct_scores,
            account_keys,
            owner_keys,
            owner_cap,
        )
        if not removals or not additions:
            break
        fast_candidates = []
        current_support = list(current["support"])
        for removed in removals:
            for added in additions:
                proposed = [
                    added if idx == removed else idx
                    for idx in current_support
                ]
                signature = tuple(sorted(proposed))
                if signature in seen or not _support_feasible(
                    proposed,
                    account_keys,
                    owner_keys,
                    owner_cap,
                    target_total,
                ):
                    continue
                candidate = _evaluate_inherited_support(
                    rate_matrix,
                    big_weights,
                    golden_weights,
                    reference_scores,
                    correct_scores,
                    proposed,
                    locked=locked,
                    absorption_priority_indices=absorption_priority_for(
                        proposed
                    ),
                    # Screening hundreds of legal swaps does not need a full
                    # regularization path. Keep enough points around the
                    # low-error/high-stability bend to avoid screening swaps
                    # by a smooth but inaccurate pure prior.
                    regularization_factors=[
                        0.0, 1e-3, 2e-3, 3e-3, 1e-2, None,
                    ],
                )
                candidate["removed"] = removed
                candidate["added"] = added
                fast_candidates.append(candidate)
        if not fast_candidates:
            break
        fast_candidates.sort(
            key=lambda item: (
                float(item["metrics"]["aligned_max_abs_diff"])
                + 0.35 * float(item["metrics"]["aligned_rmse"])
                + 0.10 * float(item["metrics"]["aligned_mean_abs_diff"])
                + 0.03 * float(item["metrics"]["structure_score"]),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["structure_score"]),
                -float(np.sum(correct_scores[np.asarray(item["support"], dtype=int)])),
            )
        )
        # Retain both the error frontier and the low-effective-slot frontier.
        # This is derived only from fitted weights, never from text labels.
        finalist_by_signature: dict[tuple[int, ...], dict[str, Any]] = {}
        for item in fast_candidates[:8]:
            finalist_by_signature[tuple(sorted(item["support"]))] = item
        for item in sorted(
            fast_candidates,
            key=lambda item: (
                sum(
                    max(0.0, 0.75 - float(weight)) ** 2
                    for weight in item["selected_weights"]
                ),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
            ),
        )[:8]:
            finalist_by_signature[tuple(sorted(item["support"]))] = item
        finalists = list(finalist_by_signature.values())
        full_candidates = [current]
        for fast in finalists:
            candidate = _evaluate_inherited_support(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                list(fast["support"]),
                locked=locked,
                absorption_priority_indices=absorption_priority_for(
                    list(fast["support"])
                ),
            )
            candidate["removed"] = fast["removed"]
            candidate["added"] = fast["added"]
            full_candidates.append(candidate)

        before_avg = float(current["metrics"]["aligned_mean_abs_diff"])
        before_max = float(current["metrics"]["aligned_max_abs_diff"])
        before_std = float(current["metrics"]["final_weight_std"])
        before_cancel = float(current["metrics"]["weight_cancellation_ratio"])
        admissible = [
            item for item in full_candidates
            if item is not current
            and float(item["metrics"]["aligned_mean_abs_diff"])
            <= before_avg + 1e-9
            and float(item["metrics"]["aligned_max_abs_diff"])
            <= before_max + 1e-9
            and (
                float(item["metrics"]["aligned_mean_abs_diff"])
                < before_avg - 1e-7
                or float(item["metrics"]["aligned_max_abs_diff"])
                < before_max - 1e-7
            )
            and float(item["metrics"]["final_weight_std"])
            <= max(before_std * 1.15, before_std + 0.02) + 1e-9
            and float(item["metrics"]["weight_cancellation_ratio"])
            <= max(before_cancel * 1.05, 1.02) + 1e-9
        ]
        if not admissible:
            break
        # Among incumbent-safe swaps, first remove the lowest-C-Score row;
        # then prefer a smaller type slot-vs-mass mismatch.
        chosen = min(
            admissible,
            key=lambda item: (
                float(correct_scores[int(item["removed"])]),
                sum(
                    max(0.0, 0.75 - float(weight)) ** 2
                    for weight in item["selected_weights"]
                ),
                float(item["metrics"]["aligned_max_abs_diff"]),
                float(item["metrics"]["aligned_mean_abs_diff"]),
            ),
        )
        current_structure = float(current["metrics"]["structure_score"])
        chosen_structure = float(chosen["metrics"]["structure_score"])
        current_slot_inefficiency = float(sum(
            max(0.0, 0.75 - float(weight)) ** 2
            for weight in current["selected_weights"]
        ))
        chosen_slot_inefficiency = float(sum(
            max(0.0, 0.75 - float(weight)) ** 2
            for weight in chosen["selected_weights"]
        ))
        current_score_sum = float(
            np.sum(correct_scores[np.asarray(current["support"], dtype=int)])
        )
        chosen_score_sum = float(
            np.sum(correct_scores[np.asarray(chosen["support"], dtype=int)])
        )
        removed = int(chosen["removed"])
        added = int(chosen["added"])
        replacements.append({
            "step": len(replacements) + 1,
            "removed_index": removed,
            "added_index": added,
            "removed_group_id": group_ids[removed],
            "added_group_id": group_ids[added],
            "before_aligned_rmse": current["metrics"]["aligned_rmse"],
            "after_aligned_rmse": chosen["metrics"]["aligned_rmse"],
            "before_aligned_max_abs_diff": current["metrics"][
                "aligned_max_abs_diff"
            ],
            "after_aligned_max_abs_diff": chosen["metrics"][
                "aligned_max_abs_diff"
            ],
            "before_support_c_score_sum": current_score_sum,
            "after_support_c_score_sum": chosen_score_sum,
            "before_raw_big_rmse_audit": current["metrics"]["big_rmse"],
            "after_raw_big_rmse_audit": chosen["metrics"]["big_rmse"],
            "before_structure_score": current_structure,
            "after_structure_score": chosen_structure,
            "before_low_effective_slot_penalty": current_slot_inefficiency,
            "after_low_effective_slot_penalty": chosen_slot_inefficiency,
        })
        current = chosen
        seen.add(tuple(sorted(current["support"])))

    pre_deletion_support = list(current["support"])
    if not locked.issubset(pre_deletion_support):
        raise RuntimeError("replacement removed an initially locked target")
    if not _support_feasible(
        pre_deletion_support, account_keys, owner_keys, owner_cap, target_total,
    ):
        raise RuntimeError("pre-deletion support violates target constraints")

    # Deletion has a deliberately different lock from replacement. Sort the
    # completed post-replacement Top50 once by the browser C-Score order and
    # freeze only its first N rows. Deleting a row never promotes another row
    # into the protected prefix.
    deletion_order = sorted(
        pre_deletion_support,
        key=lambda idx: _candidate_order_key(
            idx, pair_ranks, correct_scores, raw_ranks, group_ids,
        ),
    )
    effective_deletion_lock_count = min(
        deletion_lock_count, len(deletion_order),
    )
    deletion_locked = set(
        deletion_order[:effective_deletion_lock_count]
    )
    if max_deletions > 0:
        final_stage, deletion_steps, deletion_path = (
            _run_collective_deletion_path(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                current,
                deletion_locked,
                max_deletions,
                effective_deletion_lock_count,
            )
        )
    else:
        final_stage = {
            "support": list(current["support"]),
            "lineage_weights": np.asarray(
                current["selected_weights"], dtype=float,
            ),
            "selected_weights": np.asarray(
                current["selected_weights"], dtype=float,
            ),
            "anchor": np.asarray(current["seed_anchor"], dtype=float),
            "metrics": current["metrics"],
            "path": current["path"],
        }
        deletion_steps = []
        deletion_path = []
    final_beam_min_count = max(
        effective_deletion_lock_count,
        target_total - max_deletions,
    )
    # Reset the final add/remove/swap search from the best pre-deletion
    # incumbent.  It may revisit every requested count, including 50, rather
    # than being trapped in the old greedy 40..44 tail window.
    final_beam_max_count = target_total
    if final_beam_min_count <= len(final_stage["support"]) <= final_beam_max_count:
        final_stage, final_support_edits, final_support_search = (
            _run_final_support_beam_refinement(
                rate_matrix,
                big_weights,
                golden_weights,
                reference_scores,
                correct_scores,
                final_stage,
                eligible,
                set(),
                deletion_locked,
                account_keys,
                owner_keys,
                owner_cap,
                group_ids,
                max_final_edits,
                final_beam_min_count,
                final_beam_max_count,
            )
        )
    else:
        # The deletion path is allowed to retain its incumbent when no
        # acceptable removal exists.  A 40..44-only refinement must not turn
        # that valid 50-target incumbent into a runtime failure.
        final_support_edits = []
        final_support_search = {
            "applied": False,
            "reason": "incumbent_count_outside_final_beam_range",
            "incumbent_target_count": len(final_stage["support"]),
            "min_target_count": final_beam_min_count,
            "max_target_count": final_beam_max_count,
            "incumbent_preserved": True,
        }
    final_tail_swaps = [
        edit for edit in final_support_edits
        if edit.get("operation") == "swap"
    ]
    support = list(final_stage["support"])
    if not deletion_locked.issubset(support):
        raise RuntimeError("deletion removed a fixed C-Score Top-N target")
    if not _support_feasible(
        support, account_keys, owner_keys, owner_cap,
    ):
        raise RuntimeError("final support violates target constraints")

    lineage = np.asarray(final_stage["lineage_weights"], dtype=float)
    selected = np.asarray(final_stage["selected_weights"], dtype=float)
    base = big_weights[np.asarray(support, dtype=int)]
    additions = lineage - base
    final_seed_anchor = np.asarray(final_stage["anchor"], dtype=float)
    tail_set = set(range(n)) - set(support)
    tail_weight = float(sum(big_weights[idx] for idx in tail_set))
    if abs(float(np.sum(additions)) - tail_weight) > 1e-7:
        raise RuntimeError("transported additions do not preserve tail mass")
    if abs(float(np.sum(lineage)) - float(np.sum(big_weights))) > 1e-7:
        raise RuntimeError("compressed lineage weights do not preserve big-target mass")
    expected_export_sum = (
        float(len(support)) if max_deletions > 0
        else float(np.sum(big_weights))
    )
    if abs(float(np.sum(selected)) - expected_export_sum) > 1e-7:
        raise RuntimeError("exported weights do not sum to the final target count")
    if not np.allclose(base, big_weights[np.asarray(support, dtype=int)], atol=1e-12):
        raise RuntimeError("selected base weights no longer inherit big-target weights")

    return {
        "status": "ok_variable_count_final_support_beam_collective_absorption",
        "browser_initial_indices": browser_initial_support,
        "initial_indices": initial_support,
        "locked_indices": locked_list,
        "pre_deletion_selected_indices": pre_deletion_support,
        "deletion_locked_indices": sorted(deletion_locked),
        "locked_seed_weight_scale": float(
            initial["transport"]["locked_anchor_weight_scale"]
        ),
        "locked_seed_original_weight_sum": float(
            initial["transport"]["locked_anchor_original_weight_sum"]
        ),
        "locked_seed_target_weight_sum": float(
            initial["transport"]["locked_anchor_weight_sum"]
        ),
        "initial_anchor_weights": np.asarray(
            initial["seed_anchor"], dtype=float,
        ).tolist(),
        "final_seed_anchor_weights": final_seed_anchor.tolist(),
        "seed_selection_steps": seed_selection_steps,
        "selected_indices": support,
        "base_weights": base.tolist(),
        "fitted_additions": additions.tolist(),
        "lineage_weights": lineage.tolist(),
        "selected_weights": selected.tolist(),
        "replacements": replacements,
        "deletions": deletion_steps,
        "deletion_path": deletion_path,
        "final_tail_swaps": final_tail_swaps,
        "final_support_edits": final_support_edits,
        "final_support_search": final_support_search,
        "owner_cap": owner_cap,
        "target_total": target_total,
        "deletion_lock_count": effective_deletion_lock_count,
        "output_target_count": len(support),
        "score_denominator": len(support),
        "tail_group_count": int(sum(
            1 for idx in tail_set if abs(big_weights[idx]) > 1e-15
        )),
        "tail_weight_sum": tail_weight,
        "tail_l1_weight_sum": float(sum(abs(big_weights[idx]) for idx in tail_set)),
        "big_target_weight_sum": float(np.sum(big_weights)),
        "initial_metrics": initial["metrics"],
        "initial_transport": initial["transport"],
        "pre_deletion_metrics": current["metrics"],
        "pre_deletion_transport": current["transport"],
        "final_metrics": final_stage["metrics"],
        "final_transport": {
            **current["transport"],
            "collective_deletion_applied": len(support) < target_total,
            "pre_deletion_target_count": target_total,
            "output_target_count": len(support),
            "export_scale": len(support) / float(np.sum(big_weights)),
            "score_denominator": len(support),
            "deletion_locked_count": len(deletion_locked),
            "deletion_locked_rule": "fixed_pre_deletion_support_c_score_topn",
            "final_tail_swap_count": len(final_tail_swaps),
            "final_support_edit_count": len(final_support_edits),
            "final_support_search_rule": (
                "beam_add_remove_swap_across_40_to_50_then_avg_and_max_"
                "pareto_acceptance_with_weight_dispersion_guards"
            ),
        },
        "regularization_path": final_stage["path"],
    }


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
    if payload.get("mode") == "compress_inherited_big_target_to_top50":
        out = solve_inherited_big_target_compression(payload)
        Path(args.output).write_text(
            json.dumps(out, ensure_ascii=False, allow_nan=False),
            encoding="utf-8",
        )
        return 0
    if payload.get("mode") == "compress_merged_tail_to_fixed_support":
        out = solve_merged_tail_compression(payload)
        Path(args.output).write_text(
            json.dumps(out, ensure_ascii=False), encoding="utf-8",
        )
        return 0

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
