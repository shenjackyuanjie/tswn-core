use std::collections::{HashMap, HashSet};

use anyhow::Context;
use serde::Deserialize;

const PREDICTED_PARTNERS: usize = 10;
const EXPANDED_THRESHOLD: f64 = 4500.0;

#[derive(Deserialize)]
struct Parameters {
    ridge_mean: Vec<f64>,
    ridge_scale: Vec<f64>,
    ridge_coef: Vec<f64>,
    ridge_intercept: f64,
    hist_baseline: f64,
    hist_trees: Vec<Vec<TreeNode>>,
    affine_slope: f64,
    affine_intercept: f64,
}

#[derive(Deserialize)]
struct TreeNode {
    value: f64,
    feature: usize,
    threshold: f64,
    left: usize,
    right: usize,
    leaf: bool,
}

pub struct Calibrator {
    parameters: Parameters,
    features: HashMap<String, Vec<f64>>,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub a: String,
    pub b: String,
    pub raw: f64,
    pub expanded: bool,
    pub coefficient_a: f64,
    pub coefficient_b: f64,
}

impl Calibrator {
    pub fn embedded() -> anyhow::Result<Self> {
        let parameters: Parameters = serde_json::from_str(include_str!("../abcp_directional_calibrator.json"))?;
        anyhow::ensure!(
            parameters.ridge_mean.len() == 231
                && parameters.ridge_scale.len() == 231
                && parameters.ridge_coef.len() == 231
                && parameters.hist_trees.len() == 180,
            "invalid embedded ABCP calibrator"
        );
        let mut features = HashMap::new();
        for (line_number, line) in include_str!("../abcp_calibration_features.tsv").lines().enumerate() {
            let mut fields = line.split('\t');
            let name = fields.next().unwrap_or_default();
            let values = fields
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
                .with_context(|| format!("invalid ABCP feature row {}", line_number + 1))?;
            if values.len() == 46 {
                features.insert(name.to_owned(), values);
            }
        }
        Ok(Self { parameters, features })
    }

    fn directional_features(&self, raw: f64, a: &str, b: &str) -> Option<Vec<f64>> {
        let fa = self.features.get(a)?;
        let fb = self.features.get(b)?;
        let (low, high) = if cfz(fa) <= cfz(fb) { (fa, fb) } else { (fb, fa) };
        let mut x = Vec::with_capacity(231);
        x.push(raw / 100.0);
        x.extend(low.iter().zip(high).map(|(a, b)| a + b));
        x.extend(low.iter().zip(high).map(|(a, b)| b - a));
        x.extend(low.iter().zip(high).map(|(a, b)| a * b / (a.abs() + b.abs()).max(1.0)));
        x.extend(low);
        x.extend(high);
        Some(x)
    }

    fn predict_directional(&self, raw: f64, a: &str, b: &str) -> Option<(f64, f64)> {
        let x = self.directional_features(raw, a, b)?;
        let residual = self.parameters.ridge_intercept
            + x.iter()
                .zip(&self.parameters.ridge_mean)
                .zip(&self.parameters.ridge_scale)
                .zip(&self.parameters.ridge_coef)
                .map(|(((x, mean), scale), coefficient)| ((x - mean) / scale) * coefficient)
                .sum::<f64>();
        let ridge = raw / 100.0 + residual;
        let mut hist = self.parameters.hist_baseline;
        for tree in &self.parameters.hist_trees {
            let mut index = 0;
            loop {
                let node = &tree[index];
                if node.leaf {
                    hist += node.value;
                    break;
                }
                index = if x[node.feature] <= node.threshold {
                    node.left
                } else {
                    node.right
                };
            }
        }
        Some((ridge, raw / 100.0 + hist))
    }

    pub fn legal_pairs(&self, candidates: &[Candidate]) -> HashSet<(String, String)> {
        let mut legal = HashSet::new();
        let mut ranked = [HashMap::<&str, Vec<(f64, usize)>>::new(), HashMap::new(), HashMap::new()];
        for (index, edge) in candidates.iter().enumerate() {
            if edge.expanded && edge.raw > EXPANDED_THRESHOLD {
                legal.insert((edge.a.clone(), edge.b.clone()));
            }
            let affine = self.parameters.affine_slope * (edge.raw / 100.0) + self.parameters.affine_intercept;
            let mut values = vec![(2, affine)];
            if let Some((ridge, hist)) = self.predict_directional(edge.raw, &edge.a, &edge.b) {
                values.push((0, hist));
                values.push((1, ridge));
            }
            for (model, value) in values {
                let z = logit(value);
                ranked[model].entry(&edge.a).or_default().push((z + edge.coefficient_b.ln(), index));
                if edge.a != edge.b {
                    ranked[model].entry(&edge.b).or_default().push((z + edge.coefficient_a.ln(), index));
                }
            }
        }
        for by_name in &mut ranked {
            for rows in by_name.values_mut() {
                rows.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                for &(_, index) in rows.iter().take(PREDICTED_PARTNERS) {
                    let edge = &candidates[index];
                    legal.insert((edge.a.clone(), edge.b.clone()));
                }
            }
        }
        legal
    }
}

fn cfz(features: &[f64]) -> i32 {
    ((features[1] - features[2] + features[3] + features[5] - features[6]) * 2.0 + features[4] + features[7]) as i32
}

fn logit(percent: f64) -> f64 {
    let p = (percent / 100.0).clamp(1e-6, 1.0 - 1e-6);
    (p / (1.0 - p)).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_model_loads_and_predicts() {
        let model = Calibrator::embedded().unwrap();
        let (a, fa) = model.features.iter().next().unwrap();
        assert_eq!(fa.len(), 46);
        assert_eq!(model.directional_features(4700.0, a, a).unwrap().len(), 231);
        let (ridge, hist) = model.predict_directional(4700.0, a, a).unwrap();
        assert!(ridge.is_finite() && hist.is_finite());
    }

    #[test]
    fn every_model_contributes_its_top_ten() {
        let model = Calibrator::embedded().unwrap();
        let names = model.features.keys().take(13).cloned().collect::<Vec<_>>();
        let candidates = names[1..]
            .iter()
            .enumerate()
            .map(|(i, partner)| Candidate {
                a: names[0].clone(),
                b: partner.clone(),
                raw: 4501.0 + i as f64,
                expanded: false,
                coefficient_a: 1.0,
                coefficient_b: 1.0,
            })
            .collect::<Vec<_>>();
        let legal = model.legal_pairs(&candidates);
        assert_eq!(legal.len(), 12);
    }
}
