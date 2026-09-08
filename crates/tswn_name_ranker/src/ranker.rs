#[derive(Debug)]
pub struct FitResult {
    pub scores: Vec<f64>,
    pub coefficients: Vec<f64>,
    pub strength_variance: f64,
    pub iterations: usize,
    pub converged: bool,
    pub final_change: f64,
    pub correction_reliability: f64,
}

const PARAMETER_ELASTICITY: f64 = 0.35;
const PARAMETER_DAMPING: f64 = 0.25;
const PARAMETER_PROFILE_WEIGHTS: [f64; TOP_PARTNERS] = [0.50, 0.25, 0.15, 0.10];

pub fn fit_and_rank(scores: &[Vec<f64>], allowed: &[Vec<bool>], mut progress: impl FnMut(usize)) -> anyhow::Result<FitResult> {
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
        iterations,
        converged,
        final_change,
        correction_reliability: PARAMETER_ELASTICITY,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_scores_stay_equal() {
        let s = vec![vec![50.0; 5]; 5];
        let a = vec![vec![true; 5]; 5];
        let r = fit_and_rank(&s, &a, |_| {}).unwrap();
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
        let r = fit_and_rank(&s, &a, |_| {}).unwrap();
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
