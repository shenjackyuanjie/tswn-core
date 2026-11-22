//! 调用 DS4 的 ABCP5 模型并准备二人组、三人组输入。
//!
//! 可执行文件、ONNX 和 scale 从环境指定目录或工作目录 abcp5 读取，不随库分发。
//! 预测失败时按线程数逐步减小重试，保留输入并写诊断；成功分类后才更新候选缓存，
//! 其中未过筛候选也标记为已评测。缓存不含模型/阈值版本，参数变化应使用新工作目录。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Ds4Error, Ds4Result};
use crate::model::engine::NameFeature;
use crate::output::{AtomicFileWriter, append_file, write_bytes_atomic};

/// 参与 ABCP5 预测的二人配对类型，同时决定缓存与 out 文件名。
const PAIR_TYPES: [&str; 3] = ["FC", "WC", "RH"];

/// 解析 ABCP5 资源目录：优先环境变量 TSWN_DS4_ABCP_DIR，否则取 root/abcp5。
fn model_dir(root: &Path) -> PathBuf {
    std::env::var_os("TSWN_DS4_ABCP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("abcp5"))
}

/// 调用外部 ABCP5 预测器（`abcp5.exe`、`model4.onnx`、`scale.txt`）为 input 中的名字打分并原子写入 output，返回结果行数。
/// input 无有效行时直接写空输出并返回 0；预测失败时按 threads、threads/2、threads/4、1 依次降档重试，
/// 每次尝试覆盖中间输出，诊断信息累积写入 output 同名的 `.failure.txt`。
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

/// 生成降档重试的线程序列：threads、threads/2、threads/4、1，去重且至少为 1。
fn thread_attempts(threads: usize) -> Vec<usize> {
    let mut attempts = vec![threads.max(1), (threads / 2).max(1), (threads / 4).max(1), 1];
    attempts.dedup();
    attempts
}

/// 用指定 sieve 与线程数准备三方配对（three pairs）的预测输入，返回处理的记录数。
pub fn prepare_three_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    prepare_three_pairs_with(root, |input, output| predict(root, input, output, sieve, threads))
}

/// 用注入的预测函数准备三人配对输入：按缓存跳过已评测组合，分类成功后才更新缓存。
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

/// 用指定 sieve 与线程数对最终配对集合执行 ABCP 预测，返回预测结果行数。
pub fn predict_final_pairs(root: &Path, sieve: i32, threads: usize) -> Ds4Result<usize> {
    predict_final_pairs_with(root, |input, output| predict(root, input, output, sieve, threads))
}

/// 最终配对预测：汇总三类二人、跳过缓存、追加结果并排序，同时输出去分数版本。
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

/// 读取已评测组合缓存为集合；文件不存在时返回空集。
fn load_cache(path: &Path) -> Ds4Result<HashSet<String>> {
    match fs::File::open(path) {
        Ok(file) => Ok(BufReader::new(file).lines().collect::<Result<HashSet<_>, _>>()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashSet::new()),
        Err(error) => Err(error.into()),
    }
}

/// 向缓存文件追加组合名，必要时创建父目录。
fn append_cache(path: &Path, names: &[String]) -> Ds4Result<()> {
    fs::create_dir_all(path.parent().expect("缓存目录"))?;
    let mut writer = std::io::BufWriter::new(fs::OpenOptions::new().create(true).append(true).open(path)?);
    for name in names {
        writeln!(writer, "{name}")?;
    }
    writer.flush()?;
    Ok(())
}

/// 按分数降序重排预测结果并原子重写；无法解析的行丢弃。
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

/// 遍历评分文件中的二人组合名，跳过空行与无效行。
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

/// 把 ABCP5 预测的二人按特征门控分类写入 FC/WC/RH 三人输入文件。
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

/// 计算 C++ 兼容的 CFZ 分（x[1..7] 线性组合），用于二人成员排序。
fn cfz(feature: &NameFeature) -> i32 {
    ((feature.x[1] - feature.x[2] + feature.x[3] + feature.x[5] - feature.x[6]) * 2.0 + feature.x[4] + feature.x[7]) as i32
}

/// FS 特征门控判定：按 x[31]、x[32] 与交叉项的线性组合是否达到 50.0，决定样本是否通过 FS 筛选。
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
