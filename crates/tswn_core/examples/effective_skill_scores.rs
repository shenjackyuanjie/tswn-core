//! 批量验证等效熟练度。分片使用原轮次编号，以便拟合和复核使用不同的随机区间。

use std::collections::BTreeMap;
use std::io::{self, BufWriter, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use tswn_core::cli_api;
use tswn_core::namerena::eval_name::{DEFAULT_EVAL_RQ, WIN_RATE_EVAL_RQ};
use tswn_core::namerena::raw_namerena_to_id_name;
use tswn_core::runtime::{RuntimeRunner, prepared_runtime_win_rate_range, runtime_score_range};

#[derive(Deserialize)]
struct Row {
    id: String,
    raw: String,
    #[serde(default)]
    raw_by_metric: BTreeMap<String, String>,
    #[serde(default)]
    pair_mates: BTreeMap<String, Vec<usize>>,
}

impl Row {
    fn raw_for(&self, metric: &str) -> &str { self.raw_by_metric.get(metric).map_or(&self.raw, String::as_str) }
}

#[derive(Deserialize)]
struct Group {
    players: Vec<String>,
    factor: f64,
}

#[derive(Deserialize)]
struct PairPreset {
    metric: String,
    head: usize,
    teammates: Vec<Group>,
}

#[derive(Deserialize)]
struct Experiment {
    rows: Vec<Row>,
    cqd_targets: Vec<Group>,
    pair_targets: Vec<Group>,
    pair_presets: Vec<PairPreset>,
}

#[derive(Serialize)]
struct Scores<'a> {
    id: &'a str,
    scores: Vec<(&'a str, f64)>,
    guard_exhausted: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    teammate_rates: Vec<(&'a str, Vec<(usize, f64)>)>,
    #[cfg(feature = "effective-skill-stats")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skill_stats: Vec<(&'a str, Vec<tswn_core::runtime::effective_skill_stats::SkillObservation>)>,
}

fn freeze(name: &str) -> String {
    if name.contains("+ol:") || name.contains("+diy[") {
        name.to_owned()
    } else {
        cli_api::to_diy(name, false, false).unwrap_or_else(|error| panic!("转换 {name} 失败: {error}"))
    }
}

fn weighted_rate(players: &[String], targets: &[Group], start: usize, count: usize, guard_exhausted: &mut usize) -> f64 {
    let ids = |group: &[String]| {
        let mut ids = group.iter().map(|name| raw_namerena_to_id_name(name)).collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    };
    let player_ids = ids(players);
    let mut rate = 0.0;
    let mut weight = 0.0;
    // 对齐 Openbox 的带权镜像规则和累加顺序。
    for mirror in [true, false] {
        for target in targets {
            if (ids(&target.players) == player_ids) != mirror {
                continue;
            }
            let percent = if mirror {
                50.0
            } else {
                let groups = [players.to_vec(), target.players.clone()];
                let prepared = RuntimeRunner::prepare_groups_with_eval_rq(&groups, DEFAULT_EVAL_RQ)
                    .unwrap_or_else(|error| panic!("{groups:?}: {error}"));
                let summary = prepared_runtime_win_rate_range(&prepared, start, start + count)
                    .unwrap_or_else(|error| panic!("{groups:?}: {error}"));
                assert_eq!(summary.total, count, "{groups:?}");
                *guard_exhausted += summary.guard_exhausted;
                summary.win_rate_percent()
            };
            rate += percent * target.factor;
            weight += target.factor;
        }
    }
    rate / weight
}

fn measure<'a>(row: &'a Row, experiment: &'a Experiment, kind: &str, start: usize, count: usize, cqd_count: usize) -> Scores<'a> {
    let mut scores = Vec::new();
    let mut guard_exhausted = 0;
    let mut teammate_rates = Vec::new();
    #[cfg(feature = "effective-skill-stats")]
    let mut skill_stats = Vec::new();
    macro_rules! measure_metric {
        ($metric:expr, $call:expr) => {{
            #[cfg(feature = "effective-skill-stats")]
            let result = if kind.starts_with("stats-") {
                let (result, stats) = tswn_core::runtime::effective_skill_stats::observe(row.raw_for($metric), $call);
                skill_stats.push(($metric, stats));
                result
            } else {
                ($call)()
            };
            #[cfg(not(feature = "effective-skill-stats"))]
            let result = ($call)();
            result
        }};
    }
    if !kind.ends_with("base") {
        for preset in &experiment.pair_presets {
            let player = freeze(row.raw_for(&preset.metric));
            let selected = row.pair_mates.get(&preset.metric);
            let mut rates = measure_metric!(preset.metric.as_str(), || preset
                .teammates
                .iter()
                .enumerate()
                .filter(|(index, _)| selected.is_none_or(|mates| mates.contains(index)))
                .map(|(index, mate)| {
                    let mut team = vec![player.clone()];
                    team.extend(mate.players.iter().cloned());
                    (
                        index,
                        weighted_rate(&team, &experiment.pair_targets, start, count, &mut guard_exhausted) * mate.factor,
                    )
                })
                .collect::<Vec<_>>());
            rates.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            scores.push((
                preset.metric.as_str(),
                rates.iter().take(preset.head).map(|(_, score)| score).sum(),
            ));
            if kind == "pair-detail" {
                teammate_rates.push((preset.metric.as_str(), rates));
            }
        }
    } else {
        for (metric, modifier, members) in [("qp", "!", 1), ("qd", "!", 2), ("pp", "\u{0002}", 1), ("pd", "\u{0002}", 2)] {
            let team = vec![row.raw_for(metric).to_owned(); members];
            let summary = measure_metric!(metric, || {
                runtime_score_range(&team, modifier, start, start + count, WIN_RATE_EVAL_RQ).unwrap()
            });
            assert_eq!(summary.total, count, "{} {metric}", row.id);
            guard_exhausted += summary.guard_exhausted;
            scores.push((metric, summary.score_10000()));
        }
        let players = [row.raw_for("cqd").to_owned()];
        scores.push((
            "cqd",
            measure_metric!("cqd", || {
                weighted_rate(&players, &experiment.cqd_targets, start, cqd_count, &mut guard_exhausted)
            }),
        ));
    }
    Scores {
        id: &row.id,
        scores,
        guard_exhausted,
        teammate_rates,
        #[cfg(feature = "effective-skill-stats")]
        skill_stats,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let [kind, start, count, cqd_count, workers]: [String; 5] = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .try_into()
        .expect("effective_skill_scores <base|pair|pair-detail|stats-base|stats-pair> <start> <count> <cqd_count> <workers>");
    let (start, count, cqd_count, workers): (usize, usize, usize, usize) =
        (start.parse()?, count.parse()?, cqd_count.parse()?, workers.parse()?);
    let valid_kind = matches!(kind.as_str(), "base" | "pair" | "pair-detail")
        || cfg!(feature = "effective-skill-stats") && matches!(kind.as_str(), "stats-base" | "stats-pair");
    assert!(valid_kind && count > 0 && cqd_count > 0 && workers > 0);
    let mut experiment: Experiment = serde_json::from_reader(io::stdin().lock())?;
    for preset in &mut experiment.pair_presets {
        for mate in &mut preset.teammates {
            for name in &mut mate.players {
                *name = freeze(name);
            }
        }
    }
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| -> io::Result<()> {
        let experiment = &experiment;
        let kind = &kind;
        let next = &next;
        for _ in 0..workers.min(experiment.rows.len()) {
            let sender = sender.clone();
            scope.spawn(move || {
                while let Some(row) = experiment.rows.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if sender.send(measure(row, experiment, kind, start, count, cqd_count)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut output = BufWriter::new(io::stdout().lock());
        for score in receiver {
            serde_json::to_writer(&mut output, &score)?;
            writeln!(output)?;
            output.flush()?;
        }
        Ok(())
    })?;
    Ok(())
}
