use anyhow::Context;

use crate::model::StoredGroup;

pub fn compute_rate_without_db(a: &StoredGroup, b: &StoredGroup, samples: usize, inner_workers: u32) -> anyhow::Result<f64> {
    let groups = vec![a.members.clone(), b.members.clone()];
    let summary = tswn_core::win_rate::groups_win_rate(
        &groups,
        samples,
        tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ,
        inner_workers,
    )
    .with_context(|| format!("compute win rate: {} vs {}", a.canonical, b.canonical))?;

    Ok(summary.win_rate_percent())
}
