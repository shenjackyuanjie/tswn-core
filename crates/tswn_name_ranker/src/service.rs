use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Instant;
use std::{fs, process::Command};

use anyhow::Context;
use tswn_core::namerena::{
    NamerenaInput, PreparedPlayer, PreparedRoster,
    eval_name::{DEFAULT_EVAL_RQ, WIN_RATE_EVAL_RQ},
};

use crate::{
    abcp_calibration::Calibrator,
    db::Db,
    model::{NameRow, RecomputeRequest, Status, TargetRow},
    name_profile::player_text_type,
    parser, ranker,
};

const WIN_RATE_SAMPLES: usize = 10_000;
const ARCHIVE_SCORE_THRESHOLD: f64 = 48.0;
const TEXT_TYPE_VERSION: &str = "single_effective_skill_passive_85_v2";

#[derive(Clone)]
pub struct Service {
    pub db: Db,
    pub status: Arc<Mutex<Status>>,
    calibrator: Arc<Calibrator>,
}

impl Service {
    pub fn new(db: Db) -> anyhow::Result<Self> {
        refresh_text_types_if_needed(&db)?;
        Ok(Self {
            db,
            status: Arc::new(Mutex::new(Status::default())),
            calibrator: Arc::new(Calibrator::embedded()?),
        })
    }

    fn legal_abcp_pairs(&self) -> anyhow::Result<std::collections::HashSet<(String, String)>> {
        Ok(self.calibrator.legal_pairs(&self.db.abcp_candidates()?))
    }

    pub fn export_results(&self) -> anyhow::Result<String> { self.db.export_results(&self.legal_abcp_pairs()?) }

    pub fn result_details(&self) -> anyhow::Result<Vec<crate::model::ResultDetailRow>> {
        self.db.result_details(&self.legal_abcp_pairs()?)
    }

    pub fn add_names(&self, text: &str) -> anyhow::Result<usize> {
        let mut count = 0;
        for raw in parser::parse_names(text)? {
            let player = build_player(&raw).with_context(|| format!("解析号：{raw}"))?;
            let text_type = player_text_type(&player);
            let diy = tswn_core::cli_api::to_diy_prepared(&player, false, true).with_context(|| format!("导出号：{raw}"))?;
            if self.db.add_name(&raw, &diy, &text_type)? {
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn import_targets(&self, text: &str) -> anyhow::Result<usize> {
        let rows = parser::parse_targets(text)?;
        let count = rows.len();
        self.db.replace_targets(&rows)?;
        Ok(count)
    }

    pub fn set_names_expanded(&self, text: &str, expanded: bool) -> anyhow::Result<usize> {
        let names = parser::parse_names(text)?;
        self.db.set_names_expanded(&names, expanded)
    }

    /// 仅测量与一个名称相连的缺失合法边。这便于探查手动扩展的候选项，无需为每个待处理名称安排完整重算。
    pub fn measure_one(&self, raw: &str) -> anyhow::Result<usize> {
        let names = self.db.names_for_run(false)?;
        let target = names
            .iter()
            .position(|(name, _)| name.raw == raw)
            .with_context(|| format!("号不存在：{raw}"))?;
        let targets = self.db.targets()?;
        if targets.is_empty() {
            anyhow::bail!("请先导入靶子");
        }
        let legal = self.legal_abcp_pairs()?;
        let signature = self.db.signature()?;
        let mut measured = Vec::new();
        for j in 0..names.len() {
            if !is_legal_pair(&legal, &names[target].0.raw, &names[j].0.raw) {
                continue;
            }
            if self.db.score(names[target].0.id, names[j].0.id, &signature, WIN_RATE_SAMPLES)?.is_some() {
                continue;
            }
            measured.push((
                names[target].0.id,
                names[j].0.id,
                self.compute_pair(target, j, &names, &targets)?,
            ));
        }
        let count = measured.len();
        if !measured.is_empty() {
            self.db.save_scores_bulk(&measured, WIN_RATE_SAMPLES, &signature)?;
        }
        Ok(count)
    }

    pub fn queue(&self, req: RecomputeRequest) -> anyhow::Result<()> {
        {
            let mut status = self.status.lock().unwrap();
            if status.state == "running" {
                anyhow::bail!("排名任务正在运行");
            }
            *status = Status {
                state: "running".into(),
                message: "准备评分".into(),
                ..Default::default()
            };
        }
        let service = self.clone();
        thread::spawn(move || {
            if let Err(err) = service.run(req) {
                let mut status = service.status.lock().unwrap();
                status.state = "error".into();
                status.message = format!("{err:#}");
            }
        });
        Ok(())
    }

    fn run(&self, req: RecomputeRequest) -> anyhow::Result<()> {
        let requested_workers = req.outer_workers.unwrap_or(0);
        let skip_archived = req.skip_archived.unwrap_or(true);
        let all_names = self.db.names_for_run(skip_archived)?;
        let archived_ids = if skip_archived {
            std::collections::HashSet::new()
        } else {
            self.db.archived_name_ids()?
        };
        let targets = Arc::new(self.db.targets()?);
        if targets.is_empty() {
            anyhow::bail!("请先导入靶子");
        }
        self.ensure_abcp(&all_names, requested_workers)?;
        let legal_pairs = self.legal_abcp_pairs()?;
        if legal_pairs.is_empty() {
            anyhow::bail!("三模型预计贡献 Top10 中没有合法组合");
        }
        let mut active = vec![true; all_names.len()];
        loop {
            let remove = (0..all_names.len())
                .filter(|&i| active[i])
                .filter(|&i| {
                    (0..all_names.len())
                        .filter(|&j| {
                            active[j]
                                && !(archived_ids.contains(&all_names[i].0.id) && archived_ids.contains(&all_names[j].0.id))
                                && is_legal_pair(&legal_pairs, &all_names[i].0.raw, &all_names[j].0.raw)
                        })
                        .count()
                        < ranker::TOP_PARTNERS
                })
                .collect::<Vec<_>>();
            if remove.is_empty() {
                break;
            }
            for i in remove {
                active[i] = false;
            }
        }
        let names = Arc::new(
            all_names
                .into_iter()
                .enumerate()
                .filter(|(i, _)| active[*i])
                .map(|(_, name)| name)
                .collect::<Vec<_>>(),
        );
        if names.len() < ranker::TOP_PARTNERS {
            anyhow::bail!(
                "三模型预计贡献 Top10 中拥有至少 {} 个合法搭档的号不足 {} 个",
                ranker::TOP_PARTNERS,
                ranker::TOP_PARTNERS
            );
        }

        let signature = Arc::new(self.db.signature()?);
        let n = names.len();
        let mut allowed = vec![vec![false; n]; n];
        let mut total = 0;
        let mut matrix = vec![vec![0.0; n]; n];
        let mut missing = Vec::new();
        let mut cached = 0;
        for i in 0..n {
            for j in i..n {
                if archived_ids.contains(&names[i].0.id) && archived_ids.contains(&names[j].0.id) {
                    continue;
                }
                if !is_legal_pair(&legal_pairs, &names[i].0.raw, &names[j].0.raw) {
                    continue;
                }
                allowed[i][j] = true;
                allowed[j][i] = true;
                total += 1;
                if let Some(score) = self.db.score(names[i].0.id, names[j].0.id, &signature, WIN_RATE_SAMPLES)? {
                    matrix[i][j] = score;
                    matrix[j][i] = score;
                    cached += 1;
                } else {
                    missing.push((i, j));
                }
            }
        }

        let workers = resolve_workers(requested_workers, missing.len());
        let mode = if requested_workers == 0 {
            "dynamic_queue"
        } else {
            "static_chunks"
        };
        {
            let mut status = self.status.lock().unwrap();
            status.pair_done = cached;
            status.pair_total = total;
            status.target_total = targets.len();
            status.message =
                format!("固定 10000 局/靶子，outer_workers={workers}，inner_workers=1，mode={mode}，cached={cached}");
        }

        if !missing.is_empty() {
            let measured = Arc::new(AtomicUsize::new(0));
            let missing = Arc::new(missing);
            let (tx, rx) = mpsc::channel::<anyhow::Result<(usize, usize, f64)>>();
            let started = Instant::now();
            thread::scope(|scope| {
                if requested_workers == 0 {
                    let next = Arc::new(AtomicUsize::new(0));
                    for _ in 0..workers {
                        let tx = tx.clone();
                        let next = next.clone();
                        let missing = missing.clone();
                        let names = names.clone();
                        let targets = targets.clone();
                        let service = self.clone();
                        let measured = measured.clone();
                        scope.spawn(move || {
                            loop {
                                let index = next.fetch_add(1, Ordering::Relaxed);
                                let Some(&(i, j)) = missing.get(index) else {
                                    break;
                                };
                                let result = service.compute_pair(i, j, &names, &targets).map(|score| (i, j, score));
                                if result.is_ok() {
                                    let current = measured.fetch_add(1, Ordering::Relaxed) + 1;
                                    if current % 10 == 0 {
                                        service.update_pair_progress(cached, current, total, workers, mode, started);
                                    }
                                }
                                if tx.send(result).is_err() {
                                    break;
                                }
                            }
                        });
                    }
                } else {
                    let chunk_size = missing.len().div_ceil(workers);
                    for worker in 0..workers {
                        let start = worker * chunk_size;
                        let end = (start + chunk_size).min(missing.len());
                        if start >= end {
                            continue;
                        }
                        let tx = tx.clone();
                        let missing = missing.clone();
                        let names = names.clone();
                        let targets = targets.clone();
                        let service = self.clone();
                        let measured = measured.clone();
                        scope.spawn(move || {
                            for &(i, j) in &missing[start..end] {
                                let result = service.compute_pair(i, j, &names, &targets).map(|score| (i, j, score));
                                if result.is_ok() {
                                    let current = measured.fetch_add(1, Ordering::Relaxed) + 1;
                                    if current % 10 == 0 {
                                        service.update_pair_progress(cached, current, total, workers, mode, started);
                                    }
                                }
                                if tx.send(result).is_err() {
                                    break;
                                }
                            }
                        });
                    }
                }
                drop(tx);
                let mut save_batch = Vec::with_capacity(100);
                for result in rx {
                    let (i, j, score) = result?;
                    matrix[i][j] = score;
                    matrix[j][i] = score;
                    save_batch.push((names[i].0.id, names[j].0.id, score));
                    if save_batch.len() == 100 {
                        self.db.save_scores_bulk(&save_batch, WIN_RATE_SAMPLES, &signature)?;
                        save_batch.clear();
                    }
                }
                if !save_batch.is_empty() {
                    self.db.save_scores_bulk(&save_batch, WIN_RATE_SAMPLES, &signature)?;
                }
                let current = measured.load(Ordering::Relaxed);
                self.update_pair_progress(cached, current, total, workers, mode, started);
                Ok::<(), anyhow::Error>(())
            })?;
        }

        let mut active_indices = (0..n).collect::<Vec<_>>();
        let mut archive_rows = Vec::<(i64, String, f64)>::new();
        let (rows, fit, display_scale, display_offset) = loop {
            let insufficient = active_indices
                .iter()
                .copied()
                .filter(|&i| active_indices.iter().filter(|&&j| allowed[i][j]).count() < ranker::TOP_PARTNERS)
                .collect::<Vec<_>>();
            if !insufficient.is_empty() {
                for &i in &insufficient {
                    archive_rows.push((names[i].0.id, format!("legal_partners_below_{}", ranker::TOP_PARTNERS), 0.0));
                }
                active_indices.retain(|i| !insufficient.contains(i));
                continue;
            }
            if active_indices.len() < ranker::TOP_PARTNERS {
                anyhow::bail!(
                    "封存后拥有至少 {} 个合法搭档的号不足 {} 个",
                    ranker::TOP_PARTNERS,
                    ranker::TOP_PARTNERS
                );
            }
            let m = active_indices.len();
            let sub_matrix = active_indices
                .iter()
                .map(|&i| active_indices.iter().map(|&j| matrix[i][j]).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let sub_allowed = active_indices
                .iter()
                .map(|&i| active_indices.iter().map(|&j| allowed[i][j]).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let status = self.status.clone();
            let fit = ranker::fit_and_rank(&sub_matrix, &sub_allowed, |step| status.lock().unwrap().iteration = step)?;
            // 保持自洽排序，但将其显示尺度校准回总体的直接前四平均值。归档阈值在这一熟悉的 win-rate 尺度上
            // 定义；若直接应用于加权贡献，系数收缩时可能归档几乎整个总体。
            let base_scores = (0..m)
                .map(|i| {
                    let mut values = (0..m).filter(|&j| sub_allowed[i][j]).map(|j| sub_matrix[i][j]).collect::<Vec<_>>();
                    values.sort_by(|a, b| b.total_cmp(a));
                    values[..ranker::TOP_PARTNERS].iter().sum::<f64>() / ranker::TOP_PARTNERS as f64
                })
                .collect::<Vec<_>>();
            let model_mean = fit.scores.iter().sum::<f64>() / m as f64;
            let base_mean = base_scores.iter().sum::<f64>() / m as f64;
            let denominator = fit.scores.iter().map(|score| (score - model_mean).powi(2)).sum::<f64>();
            let display_scale = if denominator > f64::EPSILON {
                fit.scores
                    .iter()
                    .zip(&base_scores)
                    .map(|(model, base)| (model - model_mean) * (base - base_mean))
                    .sum::<f64>()
                    / denominator
            } else {
                1.0
            };
            let display_offset = base_mean - display_scale * model_mean;
            let strengths = fit.scores.iter().map(|score| score * display_scale + display_offset).collect::<Vec<_>>();
            let below = strengths
                .iter()
                .enumerate()
                .filter(|(_, score)| **score < ARCHIVE_SCORE_THRESHOLD)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if !below.is_empty() {
                let removed = below.iter().map(|&i| active_indices[i]).collect::<Vec<_>>();
                for (&local, &global) in below.iter().zip(&removed) {
                    archive_rows.push((
                        names[global].0.id,
                        format!("score_below_{ARCHIVE_SCORE_THRESHOLD:.3}"),
                        strengths[local],
                    ));
                }
                active_indices.retain(|i| !removed.contains(i));
                continue;
            }
            let mut order = (0..m).collect::<Vec<_>>();
            order.sort_by(|&a, &b| {
                strengths[b]
                    .total_cmp(&strengths[a])
                    .then_with(|| names[active_indices[a]].0.raw.cmp(&names[active_indices[b]].0.raw))
            });
            let rows = order
                .iter()
                .enumerate()
                .map(|(rank, &i)| {
                    let global = active_indices[i];
                    (names[global].0.id, rank + 1, strengths[i], fit.coefficients[i], strengths[i])
                })
                .collect::<Vec<_>>();
            break (rows, fit, display_scale, display_offset);
        };
        let archived_count = if skip_archived {
            self.db.archive_names(&archive_rows)?
        } else {
            self.db.replace_archived_names(&archive_rows)?
        };
        self.db.replace_results(&rows)?;
        let mut status = self.status.lock().unwrap();
        status.state = "ready".into();
        status.iteration = fit.iterations;
        status.message = format!(
            "完成；自洽系数迭代 {} 轮，converged={}，final change={:.3e}，correction reliability={:.8}，display scale={:.8}，display offset={:.8}，archived={}，skip_archived={}，strength variance={:.8}；固定 10000 局/靶子",
            fit.iterations,
            fit.converged,
            fit.final_change,
            fit.correction_reliability,
            display_scale,
            display_offset,
            archived_count,
            skip_archived,
            fit.strength_variance
        );
        Ok(())
    }

    fn compute_pair(&self, i: usize, j: usize, names: &[(NameRow, String)], targets: &[TargetRow]) -> anyhow::Result<f64> {
        let weight_sum: f64 = targets.iter().map(|target| target.weight).sum();
        let mut weighted = 0.0;
        for target in targets {
            let groups = vec![
                vec![names[i].1.clone(), names[j].1.clone()],
                vec![target.left_name.clone(), target.right_name.clone()],
            ];
            let summary = tswn_core::win_rate::groups_win_rate(&groups, WIN_RATE_SAMPLES, WIN_RATE_EVAL_RQ, 1)
                .with_context(|| format!("评分 {} + {} 对 {}", names[i].0.raw, names[j].0.raw, target.raw))?;
            weighted += target.weight * summary.win_rate_percent();
        }
        let score = weighted / weight_sum;
        Ok(score)
    }

    fn ensure_abcp(&self, names: &[(NameRow, String)], requested_workers: usize) -> anyhow::Result<()> {
        let available = names.iter().map(|n| n.0.raw.as_str()).collect::<std::collections::HashSet<_>>();
        let pending = self
            .db
            .pending_abcp_names()?
            .into_iter()
            .filter(|raw| available.contains(raw.as_str()))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(());
        }
        let pending_set = pending.iter().collect::<std::collections::HashSet<_>>();
        let mut pairs = Vec::new();
        for i in 0..names.len() {
            for j in i..names.len() {
                let a = &names[i].0.raw;
                let b = &names[j].0.raw;
                if pending_set.contains(a) || pending_set.contains(b) {
                    pairs.push((a.clone(), b.clone()));
                }
            }
        }
        self.status.lock().unwrap().message = format!("computing ABCP for {} new names, {} pairs", pending.len(), pairs.len());
        let dir = std::env::var_os("NAME_RANKER_ABCP_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("abcp5"));
        let exe = dir.join("abcp5.exe");
        let model = dir.join("model4.onnx");
        let scale = dir.join("scale.txt");
        for path in [&exe, &model, &scale] {
            if !path.is_file() {
                anyhow::bail!("缺少 ABCP 文件：{}", path.display());
            }
        }
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
        let input = std::env::temp_dir().join(format!("tswn_name_ranker_abcp_{stamp}.txt"));
        let output = std::env::temp_dir().join(format!("tswn_name_ranker_abcp_{stamp}.result.txt"));
        let text = pairs.iter().map(|(a, b)| format!("{a}+{b}\n")).collect::<String>();
        fs::write(&input, text)?;
        let workers = if requested_workers > 0 {
            requested_workers
        } else {
            thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
        };
        let result = Command::new(&exe)
            .current_dir(&dir)
            .arg(&model)
            .arg(&input)
            .arg(&scale)
            .arg(workers.to_string())
            .arg(crate::db::ABCP_OUTPUT_THRESHOLD.to_string())
            .arg(&output)
            .output()
            .with_context(|| format!("启动 ABCP：{}", exe.display()))?;
        let _ = fs::remove_file(&input);
        if !result.status.success() {
            let _ = fs::remove_file(&output);
            anyhow::bail!("ABCP 失败：{}", String::from_utf8_lossy(&result.stderr));
        }
        let mut scored = Vec::new();
        for line in fs::read_to_string(&output)?.lines() {
            let Some((score, duo)) = line.trim().split_once(char::is_whitespace) else {
                continue;
            };
            let score: f64 = score.parse()?;
            let Some((a, b)) = duo.trim().split_once('+') else {
                anyhow::bail!("ABCP 输出组合无效：{duo}");
            };
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            scored.push((a.to_owned(), b.to_owned(), score));
        }
        let _ = fs::remove_file(&output);
        self.db.save_abcp_results(&pairs, &scored, &pending)?;
        self.status.lock().unwrap().message = format!(
            "ABCP complete: {} pairs above {:.0}",
            scored.len(),
            crate::db::ABCP_OUTPUT_THRESHOLD
        );
        Ok(())
    }

    fn update_pair_progress(&self, cached: usize, measured: usize, total: usize, workers: usize, mode: &str, started: Instant) {
        let done = cached + measured;
        let elapsed = started.elapsed().as_secs_f64();
        let rate = if elapsed > 0.0 { measured as f64 / elapsed } else { 0.0 };
        let eta = if rate > 0.0 {
            total.saturating_sub(done) as f64 / rate
        } else {
            0.0
        };
        let mut status = self.status.lock().unwrap();
        status.pair_done = done;
        status.target_done = 0;
        status.message = format!(
            "computing missing rates {done}/{total}, {rate:.2} pair/s, elapsed {}, eta {}, outer_workers={workers}, inner_threads=1, mode={mode}",
            format_duration(elapsed),
            format_duration(eta),
        );
    }
}

fn is_legal_pair(pairs: &std::collections::HashSet<(String, String)>, a: &str, b: &str) -> bool {
    if a <= b {
        pairs.contains(&(a.to_owned(), b.to_owned()))
    } else {
        pairs.contains(&(b.to_owned(), a.to_owned()))
    }
}

fn resolve_workers(requested: usize, total: usize) -> usize {
    if requested > 0 {
        return requested.max(1).min(total.max(1));
    }
    thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1).min(total.max(1))
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

fn build_player(raw: &str) -> anyhow::Result<PreparedPlayer> {
    let input = NamerenaInput::from_raw_groups(&[vec![raw.to_owned()]])?;
    let mut roster = match PreparedRoster::build(&input, DEFAULT_EVAL_RQ) {
        Ok(roster) => roster,
        Err(error) => match error {},
    };
    roster.players.pop().ok_or_else(|| anyhow::anyhow!("没有构建出角色"))
}

fn refresh_text_types_if_needed(db: &Db) -> anyhow::Result<()> {
    if db.text_type_version()?.as_deref() == Some(TEXT_TYPE_VERSION) {
        return Ok(());
    }
    let mut updates = Vec::new();
    for (row, _) in db.names()? {
        let player = build_player(&row.raw).with_context(|| format!("解析已有号以更新 Text-Type：{}", row.raw))?;
        updates.push((row.id, player_text_type(&player)));
    }
    db.replace_text_types(&updates, TEXT_TYPE_VERSION)
}
