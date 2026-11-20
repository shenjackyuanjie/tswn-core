//! SSH/CPU 批量搜索实验入口，按独立对局并行，不在搜索内递归开线程。
use clap::Parser;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use tswn_core::runtime::{RuntimeRunner, search::SearchConfig};

#[derive(Parser, Clone)]
struct Args {
    /// 运行时队伍编号；省略则双方均搜索。
    #[arg(long)]
    search_team: Option<usize>,
    /// 使用原版策略，便于在同一输入和 seed 集合上建立基线。
    #[arg(long)]
    legacy: bool,
    #[arg(short, long)]
    input: String,
    #[arg(long, default_value_t = 1)]
    matches: usize,
    #[arg(long, default_value_t = 1)]
    workers: usize,
    #[arg(long, default_value_t = 16)]
    samples: usize,
    #[arg(long, default_value_t = 128)]
    horizon: usize,
    #[arg(long, default_value_t = 4)]
    shortlist: usize,
    #[arg(long, default_value_t = 20000)]
    max_rounds: usize,
    #[arg(long, default_value_t = 1)]
    seed: u64,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let raw = Arc::new(std::fs::read_to_string(&args.input)?);
    let next = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    let mut handles = Vec::new();
    for _ in 0..args.workers.max(1).min(args.matches.max(1)) {
        let (raw, next, sender, args) = (raw.clone(), next.clone(), sender.clone(), args.clone());
        handles.push(std::thread::spawn(move || {
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= args.matches {
                    break;
                }
                let seed = args.seed.wrapping_add(index as u64);
                let (groups, _) = RuntimeRunner::split_namerena_into_groups((*raw).clone());
                let result = RuntimeRunner::new_from_groups_with_seed(&groups, &[format!("clever-{seed}")]);
                let row = match result {
                    Err(error) => serde_json::json!({"match":index,"error":error.to_string()}),
                    Ok(mut runner) => {
                        let mut winner = None;
                        let mut decisions = 0;
                        let started = std::time::Instant::now();
                        for round in 0..args.max_rounds {
                            if args.legacy {
                                winner = runner.runtime.run_minimal_round().winner_team;
                                if winner.is_some() {
                                    break;
                                }
                                continue;
                            }
                            let result = runner.runtime.run_search_round(SearchConfig {
                                team: args.search_team,
                                samples: args.samples,
                                horizon: args.horizon,
                                shortlist: args.shortlist,
                                seed: seed.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(round as u64),
                            });
                            decisions += usize::from(!result.estimates.is_empty());
                            winner = result.outcome.winner_team;
                            if winner.is_some() {
                                break;
                            }
                        }
                        serde_json::json!({"match":index,"seed":seed,"winner":winner,
                            "truncated":winner.is_none(),"decisions":decisions,"elapsed_ms":started.elapsed().as_millis()})
                    }
                };
                if sender.send(row).is_err() {
                    break;
                }
            }
        }));
    }
    drop(sender);
    for row in receiver {
        println!("{row}");
    }
    for handle in handles {
        handle.join().map_err(|_| "搜索工作线程异常退出")?;
    }
    Ok(())
}
