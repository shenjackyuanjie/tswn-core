use anyhow::Context;

use crate::db::Db;
use crate::model::StoredGroup;

pub fn compute_rate_without_db(a: &StoredGroup, b: &StoredGroup, samples: usize, inner_workers: u32) -> anyhow::Result<f64> {
    let groups = vec![a.members.clone(), b.members.clone()];
    let summary =
        tswn_core::win_rate::groups_win_rate(&groups, samples, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ, inner_workers)
            .with_context(|| format!("compute win rate: {} vs {}", a.canonical, b.canonical))?;

    Ok(summary.win_rate_percent())
}

#[allow(dead_code)]
pub fn get_or_compute_rate(db: &Db, a: &StoredGroup, b: &StoredGroup, samples: usize) -> anyhow::Result<f64> {
    if a.id == b.id {
        return Ok(50.0);
    }

    if let Some(rate) = db.get_rate(a.id, b.id)? {
        return Ok(rate);
    }

    let rate = compute_rate_without_db(a, b, samples, 0)?;
    db.save_rate_pair(a.id, b.id, rate, samples)?;
    Ok(rate)
}
