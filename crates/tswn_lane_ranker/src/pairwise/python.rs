//! 严格 Python 校准器调用与 CSV/JSON 结果解析。
//!
//! 将内嵌脚本写入临时目录，依次尝试 python3/python；结果目录保留用于诊断。
//! 只接受指定展示分列和身份一致的权重追溯，校验有限值、唯一引用和归一化质量。
//! 缺边请求以标记路径交回上层补算；CSV 解析支持引号与嵌入换行，但不负责严格语法验证。

use super::*;

pub(super) fn run_strict_python_calibrator(
    db_path: &str,
    lane_size: usize,
    cqd_threshold: f64,
    folds: usize,
    seed: u64,
) -> anyhow::Result<StrictPythonRun> {
    if !Path::new(db_path).exists() {
        anyhow::bail!("strict Python calibration cannot start: sqlite path does not exist: {db_path}");
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out_dir = std::env::temp_dir().join(format!(
        "tswn_strict_python_calibration_lane{}_{}_{}",
        lane_size,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("create strict Python calibration output dir: {}", out_dir.display()))?;
    let script_path = out_dir.join("strict_python_calibrator.py");
    fs::write(&script_path, STRICT_PYTHON_CALIBRATOR)
        .with_context(|| format!("write embedded strict Python calibrator: {}", script_path.display()))?;

    let mut last_error = None;
    for exe in ["python3", "python"] {
        let output = Command::new(exe)
            .arg(&script_path)
            .arg("--sqlite")
            .arg(db_path)
            .arg("--out")
            .arg(&out_dir)
            .arg("--lane-size")
            .arg(lane_size.to_string())
            .arg("--folds")
            .arg(folds.to_string())
            .arg("--seed")
            .arg(seed.to_string())
            .arg("--raw-min")
            .arg(format!("{:.12}", cqd_threshold))
            .env("TSWN_STRICT_NO_ARTIFACTS", "1")
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let scores = read_strict_python_scores(&out_dir)?;
                read_strict_python_target_trace(&out_dir, lane_size)?;
                return Ok(StrictPythonRun { out_dir, scores });
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let missing_path = out_dir.join("strict_python_missing_rate_pairs.json");
                if missing_path.exists() {
                    anyhow::bail!("STRICT_PYTHON_MISSING_RATE_PAIRS_JSON={}", missing_path.display());
                }
                last_error = Some(format!(
                    "{exe} exited with status {:?}\nstdout:\n{}\nstderr:\n{}",
                    out.status.code(),
                    stdout.trim(),
                    stderr.trim()
                ));
            }
            Err(err) => {
                last_error = Some(format!("failed to execute {exe}: {err}"));
            }
        }
    }
    anyhow::bail!(
        "strict Python calibration failed. The Rust native approximation was intentionally not used because it is not equivalent to the Python exact beta-binomial/L-BFGS-B implementation. Last error: {}",
        last_error.unwrap_or_else(|| "unknown python launcher failure".to_string())
    );
}

pub(super) fn read_strict_python_scores(out_dir: &Path) -> anyhow::Result<Vec<StrictPythonScoreRow>> {
    let path = out_dir.join("final_total_table_ALL_GROUPS.csv");
    if !path.exists() {
        anyhow::bail!(
            "strict Python calibration finished but did not produce {}; cannot safely apply corrected scores",
            path.display()
        );
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read strict Python total score table: {}", path.display()))?;
    let records = parse_csv_records(&text);
    if records.len() < 2 {
        anyhow::bail!("strict Python total score table is empty: {}", path.display());
    }
    let header = &records[0];
    let idx_group = csv_col(header, "group_id")?;
    // 前端/导出的 C-Score 必须是明确的选择/输出权重。不要在此静默回退至模型 Correct Cqd：pair_score 是
    // 序列化的 UI/导出字段，必须与 selection_weight_cqd 保持一致。
    let idx_correct = csv_col(header, "Selection Weight Cqd Display")
        .or_else(|_| csv_col(header, "selection_weight_cqd"))
        .with_context(|| {
            format!(
                "strict Python final_total_table_ALL_GROUPS.csv is missing selection_weight_cqd / Selection Weight Cqd Display; refusing to display/export model Correct Cqd as C-Score"
            )
        })?;
    let idx_selected = csv_col(header, "resolver_selected").ok();

    let mut out = Vec::new();
    for (line_idx, rec) in records.iter().enumerate().skip(1) {
        if rec.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        let group_id = rec
            .get(idx_group)
            .and_then(|s| s.trim().parse::<GroupId>().ok())
            .with_context(|| format!("parse group_id at strict Python CSV record {}", line_idx + 1))?;
        let correct_score = rec
            .get(idx_correct)
            .and_then(|s| parse_optional_f64(s))
            .with_context(|| format!("parse Selection Weight Cqd Display / selection_weight_cqd for group_id={group_id}"))?;
        if !correct_score.is_finite() {
            anyhow::bail!("strict Python produced non-finite corrected score for group_id={group_id}");
        }
        let resolver_selected = idx_selected
            .and_then(|i| rec.get(i))
            .map(|s| matches!(s.trim(), "1" | "1.0" | "true" | "True"))
            .unwrap_or(false);
        out.push(StrictPythonScoreRow {
            group_id,
            correct_score,
            resolver_selected,
        });
    }
    if out.is_empty() {
        anyhow::bail!(
            "strict Python total score table contains no usable score rows: {}",
            path.display()
        );
    }
    Ok(out)
}

pub(super) fn read_strict_python_target_trace(out_dir: &Path, expected_lane_size: usize) -> anyhow::Result<CorrectTargetTrace> {
    let csv_path = out_dir.join("target_correct_trace_weights.csv");
    let json_path = out_dir.join("target_correct_trace_metadata.json");
    let text = fs::read_to_string(&csv_path).with_context(|| {
        format!(
            "strict Python calibration did not produce a readable Correct trace: {}",
            csv_path.display()
        )
    })?;
    let metadata_json = fs::read_to_string(&json_path).with_context(|| {
        format!(
            "strict Python calibration did not produce readable trace metadata: {}",
            json_path.display()
        )
    })?;
    let metadata: Value =
        serde_json::from_str(&metadata_json).with_context(|| format!("parse Correct trace metadata: {}", json_path.display()))?;
    let trace_version = metadata
        .get("trace_version")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .context("Correct trace metadata is missing trace_version")?
        .to_string();
    let score_mode = metadata
        .get("score_mode")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .context("Correct trace metadata is missing score_mode")?
        .to_string();
    let metadata_lane_size = metadata
        .get("lane_size")
        .and_then(Value::as_u64)
        .context("Correct trace metadata is missing lane_size")? as usize;
    if metadata_lane_size != expected_lane_size {
        anyhow::bail!(
            "Correct trace lane_size mismatch: expected {}, got {}",
            expected_lane_size,
            metadata_lane_size
        );
    }

    let records = parse_csv_records(&text);
    if records.len() < 2 {
        anyhow::bail!("Correct trace CSV is empty: {}", csv_path.display());
    }
    let header = &records[0];
    let idx_version = csv_col(header, "trace_version")?;
    let idx_mode = csv_col(header, "score_mode")?;
    let idx_lane = csv_col(header, "lane_size")?;
    let idx_scope = csv_col(header, "reference_scope")?;
    let idx_group = csv_col(header, "group_id")?;
    let idx_reference_weight = csv_col(header, "reference_weight")?;
    let idx_nominal_weight = csv_col(header, "nominal_weight")?;
    let idx_raw_golden_weight = csv_col(header, "raw_golden_weight")?;
    let idx_common_coefficient = csv_col(header, "common_coefficient")?;
    let idx_coefficient_mean = csv_col(header, "coefficient_mean")?;
    let idx_coefficient_stddev = csv_col(header, "coefficient_stddev")?;
    let idx_correct_target_weight = csv_col(header, "correct_target_weight")?;
    let idx_source = csv_col(header, "source")?;

    let mut seen = HashSet::new();
    let mut weights = Vec::new();
    let mut reference_weight_sum = 0.0_f64;
    for (line_idx, rec) in records.iter().enumerate().skip(1) {
        if rec.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        let row_version = csv_record_field(rec, idx_version, "trace_version", line_idx + 1)?.trim();
        let row_mode = csv_record_field(rec, idx_mode, "score_mode", line_idx + 1)?.trim();
        if row_version != trace_version || row_mode != score_mode {
            anyhow::bail!("Correct trace CSV/metadata identity mismatch at row {}", line_idx + 1);
        }
        let lane_size = csv_record_field(rec, idx_lane, "lane_size", line_idx + 1)?
            .trim()
            .parse::<usize>()
            .with_context(|| format!("parse Correct trace lane_size at row {}", line_idx + 1))?;
        if lane_size != expected_lane_size {
            anyhow::bail!(
                "Correct trace CSV lane_size mismatch at row {}: expected {}, got {}",
                line_idx + 1,
                expected_lane_size,
                lane_size
            );
        }
        let reference_scope = csv_record_field(rec, idx_scope, "reference_scope", line_idx + 1)?.trim().to_string();
        let group_id = csv_record_field(rec, idx_group, "group_id", line_idx + 1)?
            .trim()
            .parse::<GroupId>()
            .with_context(|| format!("parse Correct trace group_id at row {}", line_idx + 1))?;
        let reference_weight = csv_record_field(rec, idx_reference_weight, "reference_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct reference_weight for group_id={group_id}"))?;
        let nominal_weight = csv_record_field(rec, idx_nominal_weight, "nominal_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct nominal_weight for group_id={group_id}"))?;
        let raw_golden_weight = csv_record_field(rec, idx_raw_golden_weight, "raw_golden_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Raw Golden weight for group_id={group_id}"))?;
        let common_coefficient = csv_record_field(rec, idx_common_coefficient, "common_coefficient", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse common Correct coefficient for group_id={group_id}"))?;
        let coefficient_mean = csv_record_field(rec, idx_coefficient_mean, "coefficient_mean", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse mean row coefficient for group_id={group_id}"))?;
        let coefficient_stddev = csv_record_field(rec, idx_coefficient_stddev, "coefficient_stddev", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse row coefficient stddev for group_id={group_id}"))?;
        let correct_target_weight = csv_record_field(rec, idx_correct_target_weight, "correct_target_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct target weight for group_id={group_id}"))?;
        let source = csv_record_field(rec, idx_source, "source", line_idx + 1)?.trim().to_string();
        if reference_scope.is_empty()
            || source.is_empty()
            || !reference_weight.is_finite()
            || reference_weight <= 0.0
            || !nominal_weight.is_finite()
            || nominal_weight.abs() <= 1e-15
            || !raw_golden_weight.is_finite()
            || raw_golden_weight < 0.0
            || !common_coefficient.is_finite()
            || common_coefficient.abs() <= 1e-15
            || !coefficient_mean.is_finite()
            || !coefficient_stddev.is_finite()
            || coefficient_stddev < 0.0
            || !correct_target_weight.is_finite()
            || correct_target_weight.abs() <= 1e-15
            || (nominal_weight - correct_target_weight).abs() > 1e-10
            || (50.0 * common_coefficient - correct_target_weight).abs() > 1e-8
        {
            anyhow::bail!("invalid Correct target trace row for group_id={group_id}");
        }
        if !seen.insert((reference_scope.clone(), group_id)) {
            anyhow::bail!(
                "duplicate Correct target trace row for scope={} group_id={}",
                reference_scope,
                group_id
            );
        }
        reference_weight_sum += reference_weight;
        weights.push(CorrectTargetTraceWeight {
            reference_scope,
            group_id,
            reference_weight,
            nominal_weight,
            raw_golden_weight,
            common_coefficient,
            coefficient_mean,
            coefficient_stddev,
            correct_target_weight,
            source,
        });
    }
    if weights.is_empty() {
        anyhow::bail!("Correct target trace contains no usable reference rows");
    }
    if (reference_weight_sum - 1.0).abs() > 1e-8 {
        anyhow::bail!(
            "Correct target trace normalized weight sum must be 1, got {:.12}",
            reference_weight_sum
        );
    }
    Ok(CorrectTargetTrace {
        trace_version,
        score_mode,
        metadata_json,
        weights,
    })
}

pub(super) fn csv_record_field<'a>(record: &'a [String], index: usize, name: &str, row_number: usize) -> anyhow::Result<&'a str> {
    record
        .get(index)
        .map(String::as_str)
        .with_context(|| format!("Correct trace row {row_number} is missing {name}"))
}

pub(super) fn csv_col(header: &[String], name: &str) -> anyhow::Result<usize> {
    header
        .iter()
        .position(|h| h.trim() == name)
        .with_context(|| format!("missing required CSV column `{name}` in strict Python output"))
}

pub(super) fn parse_optional_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("nan") || t.eq_ignore_ascii_case("none") {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

pub(super) fn parse_csv_records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                record.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            _ => field.push(ch),
        }
    }
    if in_quotes || !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

pub(super) fn read_strict_python_missing_rate_request(path: &Path) -> anyhow::Result<StrictPythonMissingRateRequest> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read strict Python missing-rate request: {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse strict Python missing-rate request JSON: {}", path.display()))?;
    let context = value
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("strict_python_dynamic_rate_request")
        .to_string();
    let pairs_v = value
        .get("pairs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing-rate request has no pairs array: {}", path.display()))?;
    let mut pairs = Vec::with_capacity(pairs_v.len());
    for p in pairs_v {
        let a = p
            .get("group_a")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("missing-rate request pair missing group_a"))? as GroupId;
        let b = p
            .get("group_b")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("missing-rate request pair missing group_b"))? as GroupId;
        if a != b {
            pairs.push(ordered_group_id_pair(a, b));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    let out_dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    Ok(StrictPythonMissingRateRequest { out_dir, pairs, context })
}

pub(super) fn extract_missing_rate_request_path(err: &anyhow::Error) -> Option<PathBuf> {
    let s = err.to_string();
    let marker = "STRICT_PYTHON_MISSING_RATE_PAIRS_JSON=";
    let idx = s.find(marker)?;
    let rest = &s[(idx + marker.len())..];
    let path = rest.lines().next().unwrap_or(rest).trim();
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}
