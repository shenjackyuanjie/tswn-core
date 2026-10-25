//! core 胜率接口的无存储适配：按输入 a、b 顺序构造两队并返回 a 的百分比胜率。
//!
//! 固定使用 WIN_RATE_EVAL_RQ，采样数和内部并行度来自调用方；自身不访问数据库或缓存。
//! 解析和战斗失败附上双方规范名称后返回；是否缓存反向的 100-rate 由存储层决定。

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
