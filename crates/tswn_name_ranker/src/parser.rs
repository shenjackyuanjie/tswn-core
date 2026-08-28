use anyhow::{Context, bail};

pub fn parse_names(text: &str) -> anyhow::Result<Vec<String>> {
    let names: Vec<_> = text.lines().map(str::trim).filter(|x| !x.is_empty()).map(str::to_owned).collect();
    if names.is_empty() {
        bail!("没有可加入的号");
    }
    if names.iter().any(|x| x.contains('\n') || x.contains('\r')) {
        bail!("号不能包含换行");
    }
    Ok(names)
}

pub fn parse_targets(text: &str) -> anyhow::Result<Vec<(f64, String, String, String)>> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((weight, raw)) = line.split_once('\t') else {
            bail!("靶子第 {} 行缺少制表符", idx + 1);
        };
        let weight: f64 = weight.trim().parse().with_context(|| format!("靶子第 {} 行权重无效", idx + 1))?;
        if !weight.is_finite() || weight <= 0.0 {
            bail!("靶子第 {} 行权重必须为正数", idx + 1);
        }
        let raw = raw.trim().to_string();
        let Some((left, right)) = raw.split_once('+') else {
            bail!("靶子第 {} 行不是双号组合", idx + 1);
        };
        if left.trim().is_empty() || right.trim().is_empty() {
            bail!("靶子第 {} 行组合不完整", idx + 1);
        }
        let left = left.trim().to_string();
        let right = right.trim().to_string();
        out.push((weight, raw, left, right));
    }
    if out.is_empty() {
        bail!("没有解析到靶子");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_file_is_strict() {
        let x = "# header\n1.2\ta@x+b@x\n";
        assert_eq!(parse_targets(x).unwrap().len(), 1);
        assert!(parse_targets("1.2\ta@x+b@x\nnot a target").is_err());
    }
}
