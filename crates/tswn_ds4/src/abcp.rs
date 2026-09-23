//! 调用 DS4 的 ABCP5 模型并准备二人组、三人组输入。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Ds4Error, Ds4Result};
use crate::model::engine::NameFeature;
use crate::output::{AtomicFileWriter, write_bytes_atomic};

const PAIR_TYPES: [&str; 3] = ["FC", "WC", "RH"];

fn model_dir(root: &Path) -> PathBuf {
    std::env::var_os("TSWN_DS4_ABCP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("abcp5"))
}

pub fn predict(root: &Path, input: &Path, output: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    let mut has_input = false;
    for line in BufReader::new(fs::File::open(input)?).lines() {
        if !line?.trim().is_empty() {
            has_input = true;
            break;
        }
    }
    if !has_input {
        write_bytes_atomic(output, b"")?;
        return Ok(0);
    }
    let dir = model_dir(root);
    let dir = if dir.is_absolute() {
        dir
    } else {
        std::env::current_dir()?.join(dir)
    };
    let exe = dir.join("abcp5.exe");
    let model = dir.join("model4.onnx");
    let scale = dir.join("scale.txt");
    for path in [&exe, &model, &scale] {
        if !path.is_file() {
            return Err(Ds4Error::parse(format!("缺少 ABCP5 文件: {}", path.display())));
        }
    }
    let result = Command::new(&exe)
        .current_dir(&dir)
        .arg(&model)
        .arg(input)
        .arg(&scale)
        .arg(threads.max(1).to_string())
        .arg(sieve.to_string())
        .arg(output)
        .output()?;
    if !result.status.success() {
        return Err(Ds4Error::parse(format!(
            "ABCP5 执行失败: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        )));
    }
    Ok(BufReader::new(fs::File::open(output)?).lines().count())
}

pub fn prepare_three_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    let work = root.join("tmp").join("abcp5_three");
    fs::create_dir_all(&work)?;
    let input = work.join("input.txt");
    let predicted = work.join("predicted.txt");
    let mut seen = HashSet::new();
    let mut input_writer = AtomicFileWriter::new(&input)?;
    for kind in PAIR_TYPES {
        for_each_pair_name(&root.join("out").join(format!("{kind}.txt")), |duo| {
            if seen.insert(duo.to_owned()) {
                writeln!(input_writer.writer(), "{duo}")?;
            }
            Ok(())
        })?;
    }
    input_writer.commit()?;
    let count = predict(root, &input, &predicted, sieve, threads)?;
    classify_pairs(&predicted, &root.join("tmp"))?;
    Ok(count)
}

pub fn predict_final_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    let dir = root.join("abcp5");
    fs::create_dir_all(&dir)?;
    let input = dir.join("input.txt");
    let mut input_writer = AtomicFileWriter::new(&input)?;
    for kind in PAIR_TYPES {
        for_each_pair_name(&root.join("out").join(format!("{kind}.txt")), |duo| {
            writeln!(input_writer.writer(), "{duo}")?;
            Ok(())
        })?;
    }
    input_writer.commit()?;
    let result = dir.join("result.txt");
    let count = predict(root, &input, &result, sieve, threads)?;
    let mut without_score = AtomicFileWriter::new(&dir.join("result_without_score.txt"))?;
    for line in BufReader::new(fs::File::open(&result)?).lines() {
        let line = line?;
        if let Some((_, duo)) = line.trim_end_matches('\r').split_once(char::is_whitespace) {
            writeln!(without_score.writer(), "{}", duo.trim_start())?;
        }
    }
    without_score.commit()?;
    Ok(count)
}

fn for_each_pair_name(path: &Path, mut visit: impl FnMut(&str) -> Ds4Result<()>) -> Ds4Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for line in BufReader::new(fs::File::open(path)?).lines() {
        let line = line?;
        if let Some((score, duo)) = line.trim_end_matches('\r').split_once(char::is_whitespace)
            && score.parse::<f64>().is_ok()
            && !duo.trim().is_empty()
        {
            visit(duo.trim())?;
        }
    }
    Ok(())
}

fn classify_pairs(predicted: &Path, tmp: &Path) -> Ds4Result<()> {
    let mut outputs = [
        AtomicFileWriter::new(&tmp.join("new_three_FC.txt"))?,
        AtomicFileWriter::new(&tmp.join("new_three_WC.txt"))?,
        AtomicFileWriter::new(&tmp.join("new_three_RH.txt"))?,
    ];
    for line in BufReader::new(fs::File::open(predicted)?).lines() {
        let line = line?;
        let Some((raw_score, duo)) = line.trim_end_matches('\r').split_once(char::is_whitespace) else {
            continue;
        };
        let score = raw_score
            .parse::<f64>()
            .map_err(|_| Ds4Error::parse(format!("ABCP5 分数无效: {raw_score}")))?;
        let Some((left, right)) = duo.trim().split_once('+') else {
            continue;
        };
        let mut x = NameFeature::from_full_name(left.trim())?;
        let mut y = NameFeature::from_full_name(right.trim())?;
        let original_x = x.name_base;
        let original_y = y.name_base;
        for k in 7..128 {
            if original_y[k - 1] == original_x[k] {
                x.name_base[k] = x.name_base[k].max(original_y[k]);
            }
            if original_x[k - 1] == original_y[k] {
                y.name_base[k] = y.name_base[k].max(original_x[k]);
            }
        }
        x.x = x.recompute_x();
        y.x = y.recompute_x();
        if cfz(&x) < cfz(&y) {
            std::mem::swap(&mut x, &mut y);
        }
        let converted = (score * 2.0 + 200.0) as i32;
        if y.x[29] >= 25.0 {
            write!(outputs[0].writer(), "{converted} {}+{}\r\n", x.name, y.name)?;
        }
        if y.x[29] < 35.0 && x.x[29] < 35.0 {
            write!(outputs[1].writer(), "{converted} {}+{}\r\n", x.name, y.name)?;
        }
        if fs_gate(&y.x) {
            write!(outputs[2].writer(), "{converted} {}+{}\r\n", x.name, y.name)?;
        }
    }
    for output in outputs {
        output.commit()?;
    }
    Ok(())
}

fn cfz(feature: &NameFeature) -> i32 {
    ((feature.x[1] - feature.x[2] + feature.x[3] + feature.x[5] - feature.x[6]) * 2.0 + feature.x[4] + feature.x[7]) as i32
}

pub fn fs_gate(x: &[f64; 46]) -> bool { x[31] * 2.0 + x[32] + 0.01 * x[31] * x[19] + 0.01 * x[31] * x[36] >= 50.0 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_matches_ds4_cpp_sample() {
        let root = std::env::temp_dir().join(format!("tswn-ds4-classify-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create temp dir");
        let input = root.join("predicted.txt");
        fs::write(&input, "4500 alpha@teamA+beta@teamA\n4700 gamma@teamA+delta@teamA\n").expect("write predictions");
        classify_pairs(&input, &root).expect("classify");
        assert_eq!(fs::read_to_string(root.join("new_three_FC.txt")).expect("FC"), "");
        assert_eq!(
            fs::read_to_string(root.join("new_three_WC.txt")).expect("WC"),
            "9200 alpha@teamA+beta@teamA\r\n9600 delta@teamA+gamma@teamA\r\n"
        );
        assert_eq!(fs::read_to_string(root.join("new_three_RH.txt")).expect("RH"), "");
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
