#[derive(Debug)]
pub struct FitResult {
    pub scores: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub strength_variance: f64,
    pub loss_variance: f64,
    pub process_variance: f64,
    pub iterations: usize,
    pub converged: bool,
    pub final_change: f64,
    pub correction_reliability: f64,
}

#[derive(Clone, Copy)]
struct Edge {
    i: usize,
    j: usize,
    z: f64,
    noise: f64,
}

const PARAMETER_ELASTICITY: f64 = 0.35;
const PARAMETER_DAMPING: f64 = 0.25;
const PARAMETER_PROFILE_WEIGHTS: [f64; TOP_PARTNERS] = [0.50, 0.25, 0.15, 0.10];

pub fn fit_and_rank(
    scores: &[Vec<f64>],
    allowed: &[Vec<bool>],
    _effective_samples: f64,
    mut progress: impl FnMut(usize),
) -> anyhow::Result<FitResult> {
    let n = scores.len();
    if n < TOP_PARTNERS {
        anyhow::bail!("至少需要 {TOP_PARTNERS} 个号");
    }
    if allowed.len() != n || scores.iter().any(|x| x.len() != n) || allowed.iter().any(|x| x.len() != n) {
        anyhow::bail!("评分矩阵尺寸不一致");
    }
    if (0..n).any(|i| (0..n).filter(|&j| allowed[i][j]).count() < TOP_PARTNERS) {
        anyhow::bail!("存在合法搭档不足 {TOP_PARTNERS} 个的号");
    }

    let mut log_coefficients = vec![0.0_f64; n];
    let mut strengths = vec![0.0_f64; n];
    let mut parameter_strengths = vec![0.0_f64; n];
    let mut previous_choices = vec![Vec::<usize>::new(); n];
    let mut iterations = 0;
    let mut converged = false;
    let mut final_change = f64::INFINITY;

    for iteration in 1..=500 {
        let coefficients = log_coefficients.iter().map(|x| x.exp()).collect::<Vec<_>>();
        let mut choices = vec![Vec::<usize>::new(); n];
        for i in 0..n {
            let mut candidates = (0..n)
                .filter(|&j| allowed[i][j])
                .map(|j| (scores[i][j] * coefficients[j], j))
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            choices[i] = candidates[..TOP_PARTNERS].iter().map(|x| x.1).collect();
            strengths[i] = candidates[..TOP_PARTNERS].iter().map(|x| x.0).sum::<f64>() / TOP_PARTNERS as f64;
            parameter_strengths[i] = candidates[..TOP_PARTNERS]
                .iter()
                .zip(PARAMETER_PROFILE_WEIGHTS)
                .map(|((_, partner), weight)| weight * scores[i][*partner])
                .sum::<f64>();
        }
        let mean_log_parameter_strength = parameter_strengths.iter().map(|x| x.ln()).sum::<f64>() / n as f64;
        let mut next = log_coefficients
            .iter()
            .zip(&parameter_strengths)
            .map(|(old, parameter_strength)| {
                let target = -PARAMETER_ELASTICITY * (parameter_strength.ln() - mean_log_parameter_strength);
                (1.0 - PARAMETER_DAMPING) * old + PARAMETER_DAMPING * target
            })
            .collect::<Vec<_>>();
        let mean_log_coefficient = next.iter().sum::<f64>() / n as f64;
        for value in &mut next {
            *value -= mean_log_coefficient;
        }
        final_change = next.iter().zip(&log_coefficients).map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max);
        let choices_stable = choices == previous_choices;
        log_coefficients = next;
        previous_choices = choices;
        iterations = iteration;
        progress(iteration);
        if final_change < 1e-9 && choices_stable {
            converged = true;
            break;
        }
    }

    // 使用最终同步参数再计算一次，使返回的分数和系数描述完全相同的不动点状态。
    let coefficients = log_coefficients.iter().map(|x| x.exp()).collect::<Vec<_>>();
    for i in 0..n {
        let mut candidates = (0..n)
            .filter(|&j| allowed[i][j])
            .map(|j| (scores[i][j] * coefficients[j], j))
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        strengths[i] = candidates[..TOP_PARTNERS].iter().map(|x| x.0).sum::<f64>() / TOP_PARTNERS as f64;
    }
    let mean_log_strength = strengths.iter().map(|x| x.ln()).sum::<f64>() / n as f64;
    let strength_variance = strengths.iter().map(|x| (x.ln() - mean_log_strength).powi(2)).sum::<f64>() / n as f64;
    Ok(FitResult {
        scores: strengths,
        coefficients,
        strength_variance,
        loss_variance: 0.0,
        process_variance: 0.0,
        iterations,
        converged,
        final_change,
        correction_reliability: PARAMETER_ELASTICITY,
    })
}

#[allow(dead_code)]
fn fit_and_rank_legacy(
    scores: &[Vec<f64>],
    allowed: &[Vec<bool>],
    effective_samples: f64,
    mut progress: impl FnMut(usize),
) -> anyhow::Result<FitResult> {
    let n = scores.len();
    if n < TOP_PARTNERS {
        anyhow::bail!("至少需要 {TOP_PARTNERS} 个号");
    }
    if allowed.len() != n || scores.iter().any(|x| x.len() != n) || allowed.iter().any(|x| x.len() != n) {
        anyhow::bail!("评分矩阵尺寸不一致");
    }
    let mut edges = Vec::new();
    for i in 0..n {
        for j in i..n {
            if allowed[i][j] {
                let p = (scores[i][j] / 100.0).clamp(1e-6, 1.0 - 1e-6);
                edges.push(Edge {
                    i,
                    j,
                    z: (p / (1.0 - p)).ln(),
                    noise: 1.0 / (effective_samples * p * (1.0 - p)),
                });
            }
        }
    }
    if (0..n).any(|i| edges.iter().filter(|e| e.i == i || e.j == i).count() < TOP_PARTNERS) {
        anyhow::bail!("存在合法搭档不足 {TOP_PARTNERS} 个的号");
    }
    let mean = edges.iter().map(|e| e.z).sum::<f64>() / edges.len() as f64;
    let total_var = edges.iter().map(|e| (e.z - mean).powi(2)).sum::<f64>() / edges.len() as f64;
    let eps = f64::EPSILON.sqrt();
    let mut strength_variance = (total_var / 4.0).max(eps);
    let mut loss_variance = (total_var / 3.0).max(eps);
    let mut process_variance = (total_var / 3.0).max(eps);
    let mut beta = vec![0.0; n + 1];
    beta[0] = mean + loss_variance.sqrt() * (2.0 / std::f64::consts::PI).sqrt();
    let mut iterations = 0;
    let mut converged = false;
    let mut final_change = f64::INFINITY;
    for iteration in 1..=500 {
        let mut expected_loss = Vec::with_capacity(edges.len());
        let mut expected_loss_sq = Vec::with_capacity(edges.len());
        for e in &edges {
            let residual = e.z - beta[0] - beta[e.i + 1] - beta[e.j + 1];
            let symmetric = process_variance + e.noise;
            let sum = loss_variance + symmetric;
            let posterior_mean = -loss_variance / sum * residual;
            let posterior_sd = (loss_variance * symmetric / sum).sqrt();
            let alpha = -posterior_mean / posterior_sd;
            let mills = inverse_mills(alpha);
            expected_loss.push(posterior_mean + posterior_sd * mills);
            expected_loss_sq
                .push(posterior_mean * posterior_mean + posterior_sd * posterior_sd + posterior_mean * posterior_sd * mills);
        }
        let weights = edges.iter().map(|e| 1.0 / (process_variance + e.noise)).collect::<Vec<_>>();
        let targets = edges.iter().zip(&expected_loss).map(|(e, u)| e.z + u).collect::<Vec<_>>();
        beta = solve_full(n, &edges, &weights, &targets, 1.0 / strength_variance)?;
        let trace = posterior_trace(n, &edges, &weights, 1.0 / strength_variance)?;
        let next_strength = (beta[1..].iter().map(|a| a * a).sum::<f64>() + trace) / n as f64;
        let next_loss = expected_loss_sq.iter().sum::<f64>() / expected_loss_sq.len() as f64;
        let next_process = edges
            .iter()
            .zip(&expected_loss)
            .zip(&expected_loss_sq)
            .map(|((e, u), u2)| {
                let residual = e.z - beta[0] - beta[e.i + 1] - beta[e.j + 1];
                residual * residual + 2.0 * residual * u + u2 - e.noise
            })
            .sum::<f64>()
            / edges.len() as f64;
        let change = relative(next_strength, strength_variance)
            .max(relative(next_loss, loss_variance))
            .max(relative(next_process.max(eps), process_variance));
        strength_variance = next_strength.max(eps);
        loss_variance = next_loss.max(eps);
        process_variance = next_process.max(eps);
        iterations = iteration;
        progress(iteration);
        final_change = change;
        if change < 1e-6 {
            converged = true;
            break;
        }
    }
    let weights = edges.iter().map(|e| 1.0 / (process_variance + e.noise)).collect::<Vec<_>>();
    let mut expected_loss = Vec::with_capacity(edges.len());
    for e in &edges {
        let residual = e.z - beta[0] - beta[e.i + 1] - beta[e.j + 1];
        let symmetric = process_variance + e.noise;
        let sum = loss_variance + symmetric;
        let m = -loss_variance / sum * residual;
        let sd = (loss_variance * symmetric / sum).sqrt();
        expected_loss.push(m + sd * inverse_mills(-m / sd));
    }
    let targets = edges.iter().zip(&expected_loss).map(|(e, u)| e.z + u).collect::<Vec<_>>();
    beta = solve_full(n, &edges, &weights, &targets, 1.0 / strength_variance)?;
    let measurement_variance = edges.iter().map(|e| e.noise).sum::<f64>() / edges.len() as f64;
    // 标准差份额略显激进，而方差份额过于保守。二者的几何平均让修正保持数据驱动，并提供适中的收缩。
    // 焦点名称是条件变量，因而不属于这部分噪声。
    let strength_sd = strength_variance.sqrt();
    let noise_variance = loss_variance + process_variance + measurement_variance;
    let noise_sd = noise_variance.sqrt();
    let sd_reliability = strength_sd / (strength_sd + noise_sd);
    let variance_reliability = strength_variance / (strength_variance + noise_variance);
    let correction_reliability = (sd_reliability * variance_reliability).sqrt();
    let mut choices = vec![Vec::<(f64, f64, f64, usize)>::new(); n];
    for e in &edges {
        let qi = e.z - correction_reliability * beta[e.j + 1];
        let qj = e.z - correction_reliability * beta[e.i + 1];
        // 前沿截距包含总体平均潜在兼容性损失。该成分无法归因至配对的任一成员，因此改在观测总体上居中地
        // 对称分配。这也使恒定 50% 矩阵恰好锚定在 50%。
        let allocated_i = (qi + mean + correction_reliability * beta[e.i + 1]) / 2.0;
        let allocated_j = (qj + mean + correction_reliability * beta[e.j + 1]) / 2.0;
        choices[e.i].push((e.z, qi, allocated_i, e.j));
        if e.i != e.j {
            choices[e.j].push((e.z, qj, allocated_j, e.i));
        }
    }
    let mut result = vec![0.0; n];
    for i in 0..n {
        choices[i].sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.3.cmp(&b.3)));
        let q = choices[i][..TOP_PARTNERS].iter().map(|x| x.2).sum::<f64>() / TOP_PARTNERS as f64;
        result[i] = 100.0 / (1.0 + (-q).exp());
    }
    Ok(FitResult {
        scores: result,
        coefficients: beta[1..].iter().map(|a| (-correction_reliability * a).exp()).collect(),
        strength_variance,
        loss_variance,
        process_variance,
        iterations,
        converged,
        final_change,
        correction_reliability,
    })
}

fn solve_full(n: usize, edges: &[Edge], weights: &[f64], targets: &[f64], penalty: f64) -> anyhow::Result<Vec<f64>> {
    let mul = |v: &[f64]| {
        let mut out = vec![0.0; n + 1];
        for (k, e) in edges.iter().enumerate() {
            let q = weights[k] * (v[0] + v[e.i + 1] + v[e.j + 1]);
            out[0] += q;
            out[e.i + 1] += q;
            out[e.j + 1] += q;
        }
        for i in 0..n {
            out[i + 1] += penalty * v[i + 1];
        }
        out
    };
    let mut b = vec![0.0; n + 1];
    for ((e, w), y) in edges.iter().zip(weights).zip(targets) {
        let q = w * y;
        b[0] += q;
        b[e.i + 1] += q;
        b[e.j + 1] += q;
    }
    cg(&b, mul)
}
fn posterior_trace(n: usize, edges: &[Edge], weights: &[f64], penalty: f64) -> anyhow::Result<f64> {
    let mul = |v: &[f64]| {
        let mut out = vec![0.0; n];
        for (k, e) in edges.iter().enumerate() {
            let q = weights[k] * (v[e.i] + v[e.j]);
            out[e.i] += q;
            out[e.j] += q;
        }
        for i in 0..n {
            out[i] += penalty * v[i];
        }
        out
    };
    let probes = 4;
    let mut trace = 0.0;
    for p in 0..probes {
        let v = (0..n).map(|i| sign(p, i)).collect::<Vec<_>>();
        let cv = cg(&v, &mul)?;
        trace += dot(&v, &cv);
    }
    Ok(trace / probes as f64)
}
fn cg(b: &[f64], mul: impl Fn(&[f64]) -> Vec<f64>) -> anyhow::Result<Vec<f64>> {
    let mut x = vec![0.0; b.len()];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rr = dot(&r, &r);
    let initial = rr.sqrt();
    if initial == 0.0 {
        return Ok(x);
    }
    for _ in 0..b.len() * 3 {
        let ap = mul(&p);
        let d = dot(&p, &ap);
        if d <= 0.0 {
            anyhow::bail!("拟合矩阵不是正定的");
        }
        let alpha = rr / d;
        for i in 0..x.len() {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let next = dot(&r, &r);
        if next.sqrt() <= initial * 1e-10 {
            return Ok(x);
        }
        let q = next / rr;
        for i in 0..p.len() {
            p[i] = r[i] + q * p[i];
        }
        rr = next;
    }
    Ok(x)
}
fn inverse_mills(alpha: f64) -> f64 {
    if alpha > 8.0 {
        return alpha + 1.0 / alpha;
    }
    if alpha < -8.0 {
        return 0.0;
    }
    normal_pdf(alpha) / (1.0 - normal_cdf(alpha)).max(f64::MIN_POSITIVE)
}
fn normal_pdf(x: f64) -> f64 { (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt() }
fn normal_cdf(x: f64) -> f64 {
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.2316419 * a);
    let p =
        1.0 - normal_pdf(a) * t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0.0 { p } else { 1.0 - p }
}
fn relative(a: f64, b: f64) -> f64 { (a - b).abs() / b.max(f64::MIN_POSITIVE) }
fn sign(probe: usize, i: usize) -> f64 {
    let mut x = (probe as u64 + 1).wrapping_mul(0x9E3779B97F4A7C15) ^ (i as u64 + 1).wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    if x & 1 == 0 { 1.0 } else { -1.0 }
}
fn dot(a: &[f64], b: &[f64]) -> f64 { a.iter().zip(b).map(|(x, y)| x * y).sum() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_scores_stay_equal() {
        let s = vec![vec![50.0; 5]; 5];
        let a = vec![vec![true; 5]; 5];
        let r = fit_and_rank(&s, &a, 100_000.0, |_| {}).unwrap();
        assert!(r.scores.iter().all(|x| (*x - 50.0).abs() < 1e-6));
        assert!(r.coefficients.iter().all(|x| (*x - 1.0).abs() < 1e-6));
    }

    #[test]
    fn parameters_are_the_normalized_inverse_selected_raw_profile() {
        let mut s = vec![vec![48.0; 6]; 6];
        for i in 0..6 {
            for j in 0..6 {
                s[i][j] += (i + j) as f64 * 0.2;
            }
        }
        let a = vec![vec![true; 6]; 6];
        let r = fit_and_rank(&s, &a, 10_000.0, |_| {}).unwrap();
        assert!(r.converged);
        let parameter_strengths = (0..s.len())
            .map(|i| {
                let mut candidates = (0..s.len()).map(|j| (s[i][j] * r.coefficients[j], j)).collect::<Vec<_>>();
                candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                candidates[..TOP_PARTNERS]
                    .iter()
                    .zip(PARAMETER_PROFILE_WEIGHTS)
                    .map(|((_, j), weight)| weight * s[i][*j])
                    .sum::<f64>()
            })
            .collect::<Vec<_>>();
        let mean_log_strength = parameter_strengths.iter().map(|x| x.ln()).sum::<f64>() / parameter_strengths.len() as f64;
        for (coefficient, strength) in r.coefficients.iter().zip(&parameter_strengths) {
            let expected = (-PARAMETER_ELASTICITY * (strength.ln() - mean_log_strength)).exp();
            assert!((coefficient - expected).abs() < 1e-7);
        }
        let geometric_mean = (r.coefficients.iter().map(|x| x.ln()).sum::<f64>() / r.coefficients.len() as f64).exp();
        assert!((geometric_mean - 1.0).abs() < 1e-9);
    }
}
pub const TOP_PARTNERS: usize = 4;
