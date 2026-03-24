use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Ds3Error, Ds3Result};
use crate::output::AtomicFileWriter;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SortOptions {
    pub score_number: usize,
    pub sort_key_zero_based: usize,
    pub output_score: bool,
}

#[derive(Debug, Clone)]
struct ScoredRow {
    scores: Vec<f64>,
    name: String,
}

pub fn sort_scored_file(input_file: &Path, output_file: &Path, options: &SortOptions) -> Ds3Result<usize> {
    if options.score_number == 0 {
        return Err(Ds3Error::parse("score_number must be >= 1"));
    }
    if options.sort_key_zero_based >= options.score_number {
        return Err(Ds3Error::parse(format!(
            "sort_key {} out of range for score_number {}",
            options.sort_key_zero_based + 1,
            options.score_number
        )));
    }

    let input = if input_file.exists() {
        fs::read_to_string(input_file)?
    } else {
        String::new()
    };

    let mut rows = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(row) = parse_row(line, options.score_number) {
            rows.push(row);
        }
    }

    rows.sort_by(|left, right| {
        let left_key = left.scores[options.sort_key_zero_based];
        let right_key = right.scores[options.sort_key_zero_based];
        right_key.total_cmp(&left_key).then_with(|| right.name.cmp(&left.name))
    });

    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();

    let mut previous_name: Option<&str> = None;
    let mut previous_key = f64::NAN;
    let mut written = 0usize;

    for row in &rows {
        let key = row.scores[options.sort_key_zero_based];
        let is_duplicate = previous_name.is_some_and(|name| name == row.name.as_str() && previous_key == key);
        if is_duplicate {
            continue;
        }

        if options.output_score {
            for score in row.scores.iter().take(options.score_number) {
                write!(out, "{score:.3} ")?;
            }
        }
        write!(out, "{}\r\n", row.name)?;

        previous_name = Some(&row.name);
        previous_key = key;
        written += 1;
    }

    writer.commit()?;
    Ok(written)
}

fn parse_row(line: &str, score_number: usize) -> Option<ScoredRow> {
    let mut scores = Vec::with_capacity(score_number);
    let bytes = line.as_bytes();
    let mut index = 0usize;

    for _ in 0..score_number {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        scores.push(line[start..index].parse::<f64>().ok()?);
    }

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let name = line[index..].to_string();
    Some(ScoredRow { scores, name })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{SortOptions, sort_scored_file};

    fn temp_dir(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn sort_works_and_dedups_same_key_same_name() {
        let root = temp_dir("tswn-ds3-sort");
        let input_file = root.join("in.txt");
        let output_file = root.join("out.txt");
        fs::write(&input_file, "1.0 2.0 alpha\n1.0 2.0 alpha\n3.0 1.0 beta\n2.0 9.0 gamma\n").expect("write input");

        let options = SortOptions {
            score_number: 2,
            sort_key_zero_based: 0,
            output_score: true,
        };
        let written = sort_scored_file(&input_file, &output_file, &options).expect("sort");
        let output = fs::read_to_string(&output_file).expect("read output");

        assert_eq!(written, 3);
        assert_eq!(output, "3.000 1.000 beta\r\n2.000 9.000 gamma\r\n1.000 2.000 alpha\r\n");

        fs::remove_dir_all(&root).expect("cleanup");
    }
}
