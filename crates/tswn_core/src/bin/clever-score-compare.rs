//! 被测侧 Clever 对原版的配对评分实验；相同 profile 序列分别运行，输出万分制差值。
use clap::Parser;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use tswn_core::runtime::{ScoreTestSession, search::SearchConfig};

#[derive(Parser, Clone)]
struct Args {
    /// 被测名字，可多次传入；完整名字作为一个参数，单挑由程序重复输入。
    #[arg(long, required = true)]
    name: Vec<String>,
    #[arg(long, default_value_t = 10000, value_parser = clap::value_parser!(u32).range(1..=10000))]
    rounds: u32,
    #[arg(long, default_value_t = 1)]
    workers: usize,
    #[arg(long, default_value_t = 16)]
    samples: usize,
    #[arg(long, default_value_t = 128)]
    horizon: usize,
    #[arg(long, default_value_t = 4)]
    shortlist: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let jobs = Arc::new(
        args.name
            .iter()
            .flat_map(|name| {
                [
                    (false, false, "pp"),
                    (false, true, "pd"),
                    (true, false, "qp"),
                    (true, true, "qd"),
                ]
                .map(|(enhanced, duel, mode)| (name.clone(), enhanced, duel, mode))
            })
            .collect::<Vec<_>>(),
    );
    let next = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    for _ in 0..args.workers.max(1).min(jobs.len()) {
        let (jobs, next, tx, args) = (jobs.clone(), next.clone(), tx.clone(), args.clone());
        handles.push(std::thread::spawn(move || {
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some((name, enhanced, duel, mode)) = jobs.get(index) else {
                    break;
                };
                let raw = if *duel { format!("{name}\n{name}") } else { name.clone() };
                let config = SearchConfig {
                    samples: args.samples,
                    horizon: args.horizon,
                    shortlist: args.shortlist,
                    ..SearchConfig::default()
                };
                let result = compare(&raw, *enhanced, args.rounds as usize, config);
                let row = match result {
                    Ok(mut row) => {
                        row["name"] = json!(name);
                        row["mode"] = json!(mode);
                        row
                    }
                    Err(error) => json!({"name":name,"mode":mode,"error":error}),
                };
                if tx.send(row).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);
    let mut failed = false;
    for row in rx {
        failed |= row.get("error").is_some();
        println!("{row}");
    }
    for handle in handles {
        handle.join().map_err(|_| "评分线程异常退出")?;
    }
    if failed {
        return Err("部分评分失败，请检查输出中的 error".into());
    }
    Ok(())
}

fn compare(raw: &str, enhanced: bool, rounds: usize, config: SearchConfig) -> Result<serde_json::Value, String> {
    let started = std::time::Instant::now();
    let mut clever = ScoreTestSession::new_with_mode(raw, Some(config), enhanced).map_err(|e| format!("{e:?}"))?;
    // 实验工具总是显式启用单方对照，网页默认评分仍为双方 Clever。
    clever.set_legacy_opponents(true);
    let mut legacy = ScoreTestSession::new_with_mode(raw, None, enhanced).map_err(|e| format!("{e:?}"))?;
    let (mut gains, mut losses, mut clever_wins, mut legacy_wins) = (0, 0, 0, 0);
    for _ in 0..rounds {
        clever.step().map_err(|e| format!("{e:?}"))?;
        legacy.step().map_err(|e| format!("{e:?}"))?;
        let c = clever.progress().wins;
        let l = legacy.progress().wins;
        match (c > clever_wins, l > legacy_wins) {
            (true, false) => gains += 1,
            (false, true) => losses += 1,
            _ => {}
        }
        (clever_wins, legacy_wins) = (c, l);
    }
    let c = clever.progress();
    let l = legacy.progress();
    let delta = (gains as f64 - losses as f64) / rounds as f64;
    // 配对差值为 -1/0/+1；区间只描述当前名字及 profile 分布，不代表全名单泛化。
    let half_width = if rounds > 1 {
        1.96 * (((gains + losses) as f64 / rounds as f64 - delta * delta).max(0.0) / (rounds - 1) as f64).sqrt()
    } else {
        1.0
    };
    let p = mcnemar_exact(gains, losses);
    Ok(json!({
        "rounds":rounds, "clever_score":c.score, "legacy_score":l.score,
        "delta_score":delta*10000.0,
        "delta_score_ci95":[(delta-half_width).max(-1.0)*10000.0,(delta+half_width).min(1.0)*10000.0],
        "gains":gains,"losses":losses,"mcnemar_p_two_sided":p,
        "positive_significant_unadjusted":gains>losses && p<0.05 && rounds>=30 && c.truncated==0 && l.truncated==0,
        "clever_truncated":c.truncated,"legacy_truncated":l.truncated,
        "samples":config.samples,"horizon":config.horizon,"shortlist":config.shortlist,
        "elapsed_ms":started.elapsed().as_millis()
    }))
}

/// McNemar 精确双侧检验：只比较 Clever 独胜和原版独胜，平局不作为独立证据。
fn mcnemar_exact(gains: usize, losses: usize) -> f64 {
    let n = gains + losses;
    if n == 0 {
        return 1.0;
    }
    let k = gains.min(losses);
    let mut log_mass = -(n as f64) * std::f64::consts::LN_2;
    for i in 1..=k {
        log_mass += ((n - i + 1) as f64).ln() - (i as f64).ln();
    }
    let mut term = log_mass.exp();
    let mut tail = term;
    for j in (1..=k).rev() {
        term *= j as f64 / (n - j + 1) as f64;
        tail += term;
    }
    (2.0 * tail).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_paired_test_handles_ties_and_one_sided_discordance() {
        // 已知二项分布结果，同时检查方向互换不会改变双侧 p 值。
        assert_eq!(mcnemar_exact(0, 0), 1.0);
        assert_eq!(mcnemar_exact(5, 5), 1.0);
        assert!((mcnemar_exact(10, 0) - 0.001953125).abs() < 1e-12);
        assert_eq!(mcnemar_exact(10, 0), mcnemar_exact(0, 10));
        assert!(mcnemar_exact(10000, 0) < 0.05);
    }
}
