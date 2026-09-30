//! 调用 DS4 的 ABCP5 模型并准备二人组、三人组输入。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Ds4Error, Ds4Result};
use crate::model::engine::NameFeature;
use crate::output::{AtomicFileWriter, append_file, write_bytes_atomic};

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
    let input = std::path::absolute(input)?;
    let output = std::path::absolute(output)?;
    let diagnostic = output.with_extension("failure.txt");
    let mut failures = String::new();
    for attempt in thread_attempts(threads) {
        // 每次尝试覆盖中间输出，失败的部分结果不能进入历史文件。
        write_bytes_atomic(&output, b"")?;
        eprintln!("[ABCP5] 开始预测，线程数 {attempt}");
        let result = Command::new(&exe)
            .current_dir(&dir)
            .arg(&model)
            .arg(&input)
            .arg(&scale)
            .arg(attempt.to_string())
            .arg(sieve.to_string())
            .arg(&output)
            .output();
        match result {
            Ok(result) if result.status.success() => {
                return BufReader::new(fs::File::open(&output)?).lines().try_fold(0, |count, line| {
                    line?;
                    Ok(count + 1)
                });
            }
            Ok(result) => failures.push_str(&format!(
                "threads={attempt}, status={}\n{}\n{}\n",
                result.status,
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            )),
            Err(error) => failures.push_str(&format!("threads={attempt}: {error}\n")),
        }
        write_bytes_atomic(&diagnostic, failures.as_bytes())?;
    }
    Err(Ds4Error::parse(format!(
        "ABCP5 自动重试失败，输入已保留，诊断: {}",
        diagnostic.display()
    )))
}

fn thread_attempts(threads: usize) -> Vec<usize> {
    let mut attempts = vec![threads.max(1), (threads / 2).max(1), (threads / 4).max(1), 1];
    attempts.dedup();
    attempts
}

pub fn prepare_three_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    prepare_three_pairs_with(root, |input, output| predict(root, input, output, sieve, threads))
}

fn prepare_three_pairs_with(root: &Path, mut predict: impl FnMut(&Path, &Path) -> Ds4Result<usize>) -> Ds4Result<usize> {
    let work = root.join("tmp").join("abcp5_three");
    fs::create_dir_all(&work)?;
    let input = work.join("input.txt");
    let predicted = work.join("predicted.txt");
    let mut seen = HashSet::new();
    let mut input_writer = AtomicFileWriter::new(&input)?;
    let mut fresh = Vec::new();
    for kind in PAIR_TYPES {
        let cache = root.join("file").join(format!("{kind}_old.txt"));
        let mut cached = load_cache(&cache)?;
        let mut names = Vec::new();
        for_each_pair_name(&root.join("out").join(format!("{kind}.txt")), |duo| {
            if cached.insert(duo.to_owned()) {
                names.push(duo.to_owned());
                if seen.insert(duo.to_owned()) {
                    writeln!(input_writer.writer(), "{duo}")?;
                }
            }
            Ok(())
        })?;
        fresh.push((cache, names));
    }
    input_writer.commit()?;
    let count = if seen.is_empty() {
        write_bytes_atomic(&predicted, b"")?;
        0
    } else {
        predict(&input, &predicted)?
    };
    classify_pairs(&predicted, &root.join("tmp"))?;
    // 分类成功后才记录已评测组合，包括未通过阈值的组合。
    if !seen.is_empty() {
        for (cache, names) in fresh {
            append_cache(&cache, &names)?;
        }
    }
    Ok(count)
}

pub fn predict_final_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    predict_final_pairs_with(root, |input, output| predict(root, input, output, sieve, threads))
}

fn predict_final_pairs_with(root: &Path, mut predict: impl FnMut(&Path, &Path) -> Ds4Result<usize>) -> Ds4Result<usize> {
    let dir = root.join("abcp5");
    fs::create_dir_all(&dir)?;
    let input = root.join("tmp/two_new.txt");
    let cache = root.join("file/two_old.txt");
    let mut cached = load_cache(&cache)?;
    let mut fresh = Vec::new();
    let mut input_writer = AtomicFileWriter::new(&input)?;
    for kind in PAIR_TYPES {
        for_each_pair_name(&root.join("out").join(format!("{kind}.txt")), |duo| {
            if cached.insert(duo.to_owned()) {
                writeln!(input_writer.writer(), "{duo}")?;
                fresh.push(duo.to_owned());
            }
            Ok(())
        })?;
    }
    input_writer.commit()?;
    let result = dir.join("result.txt");
    let count = if fresh.is_empty() {
        0
    } else {
        let predicted = dir.join("two_new_result.txt");
        let count = predict(&input, &predicted)?;
        append_file(&predicted, &result)?;
        append_cache(&cache, &fresh)?;
        count
    };
    sort_predictions(&result)?;
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

fn load_cache(path: &Path) -> Ds4Result<HashSet<String>> {
    match fs::File::open(path) {
        Ok(file) => Ok(BufReader::new(file).lines().collect::<Result<HashSet<_>, _>>()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashSet::new()),
        Err(error) => Err(error.into()),
    }
}

fn append_cache(path: &Path, names: &[String]) -> Ds4Result<()> {
    fs::create_dir_all(path.parent().expect("缓存目录"))?;
    let mut writer = std::io::BufWriter::new(fs::OpenOptions::new().create(true).append(true).open(path)?);
    for name in names {
        writeln!(writer, "{name}")?;
    }
    writer.flush()?;
    Ok(())
}

fn sort_predictions(path: &Path) -> Ds4Result<()> {
    let mut rows = Vec::new();
    if path.exists() {
        for line in BufReader::new(fs::File::open(path)?).lines() {
            let line = line?;
            if let Some((score, _)) = line.split_once(char::is_whitespace)
                && let Ok(score) = score.parse::<f64>()
            {
                rows.push((score, line));
            }
        }
    }
    rows.sort_by(|left, right| right.0.total_cmp(&left.0));
    let mut writer = AtomicFileWriter::new(path)?;
    for (_, line) in rows {
        writeln!(writer.writer(), "{line}")?;
    }
    writer.commit()
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
    fn incremental_predictions_preserve_history_and_cache_rejected_pairs() {
        let root = std::env::temp_dir().join(format!("ds4-cache-{}", std::process::id()));
        fs::create_dir_all(root.join("out")).unwrap();
        fs::write(
            root.join("out/FC.txt"),
            "9000 alpha@teamA+beta@teamA\n9000 gamma@teamA+delta@teamA\n",
        )
        .unwrap();
        fs::write(root.join("out/WC.txt"), "9000 alpha@teamA+beta@teamA\n").unwrap();
        let predict = |input: &Path, output: &Path| {
            assert_eq!(fs::read_to_string(input)?.lines().count(), 2);
            fs::write(output, "4500 alpha@teamA+beta@teamA\n")?;
            Ok(1)
        };
        assert_eq!(predict_final_pairs_with(&root, predict).unwrap(), 1);
        let original = fs::read_to_string(root.join("abcp5/result.txt")).unwrap();
        assert_eq!(
            predict_final_pairs_with(&root, |_, _| panic!("缓存命中不应重新预测")).unwrap(),
            0
        );
        assert_eq!(fs::read_to_string(root.join("abcp5/result.txt")).unwrap(), original);
        assert_eq!(fs::read_to_string(root.join("file/two_old.txt")).unwrap().lines().count(), 2);

        assert_eq!(prepare_three_pairs_with(&root, predict).unwrap(), 1);
        assert_eq!(
            prepare_three_pairs_with(&root, |_, _| panic!("三人分类缓存应独立生效")).unwrap(),
            0
        );
        assert_eq!(fs::metadata(root.join("tmp/new_three_WC.txt")).unwrap().len(), 0);
        assert_eq!(fs::read_to_string(root.join("file/FC_old.txt")).unwrap().lines().count(), 2);
        assert_eq!(fs::read_to_string(root.join("file/WC_old.txt")).unwrap().lines().count(), 1);

        fs::write(root.join("out/FC.txt"), "9000 new@teamA+beta@teamA\n").unwrap();
        assert!(predict_final_pairs_with(&root, |_, _| Err(Ds4Error::parse("模拟预测失败"))).is_err());
        assert_eq!(fs::read_to_string(root.join("abcp5/result.txt")).unwrap(), original);
        assert_eq!(fs::read_to_string(root.join("file/two_old.txt")).unwrap().lines().count(), 2);
        predict_final_pairs_with(&root, |_, output| {
            fs::write(output, "4800 new@teamA+beta@teamA\n")?;
            Ok(1)
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.join("abcp5/result.txt")).unwrap(),
            "4800 new@teamA+beta@teamA\n4500 alpha@teamA+beta@teamA\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retries_reduce_threads_without_duplicate_attempts() {
        assert_eq!(thread_attempts(20), [20, 10, 5, 1]);
        assert_eq!(thread_attempts(3), [3, 1]);
        assert_eq!(thread_attempts(0), [1]);
    }

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
