//! 去重操作。
//!
//! `remove_duplicates` 读取已排序的评分文件并去除重复名条目，返回 [`DedupStats`] 统计信息。

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::error::Ds4Result;
use crate::output::AtomicFileWriter;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DedupStats {
    pub new_unique: usize,
    pub old_hits: usize,
    pub remaining: usize,
}

pub fn remove_duplicates(new_file: &Path, old_file: &Path, output_file: &Path) -> Ds4Result<DedupStats> {
    let mut set = HashSet::new();
    let mut insertion_order = Vec::new();
    visit_cpp_lines(new_file, |line| {
        if set.insert(line.to_owned()) {
            insertion_order.push(line.to_owned());
        }
    })?;
    let new_unique = set.len();

    let mut old_hits = 0usize;
    visit_cpp_lines(old_file, |line| {
        if set.remove(line) {
            old_hits += 1;
        }
    })?;

    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();
    let mut remaining = 0;
    for row in insertion_order.into_iter().rev() {
        if set.contains(&row) {
            out.write_all(row.as_bytes())?;
            out.write_all(b"\r\n")?;
            remaining += 1;
        }
    }
    writer.commit()?;

    Ok(DedupStats {
        new_unique,
        old_hits,
        remaining,
    })
}

fn visit_cpp_lines(path: &Path, mut visit: impl FnMut(&str)) -> Ds4Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for line in BufReader::new(fs::File::open(path)?).lines() {
        for part in line?.split('\r') {
            if !part.is_empty() {
                visit(part);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::remove_duplicates;

    fn temp_dir(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn dedup_removes_old_hits() {
        let root = temp_dir("tswn-ds4-dedup");
        let new_file = root.join("new.txt");
        let old_file = root.join("old.txt");
        let out_file = root.join("new_dup.txt");

        fs::write(&new_file, "a\nb\nb\nc\n\r\nd\n").expect("write new");
        fs::write(&old_file, "b\nx\nd\n").expect("write old");

        let stats = remove_duplicates(&new_file, &old_file, &out_file).expect("dedup");
        let output = fs::read_to_string(&out_file).expect("read output");

        assert_eq!(stats.new_unique, 4);
        assert_eq!(stats.old_hits, 2);
        assert_eq!(stats.remaining, 2);
        assert_eq!(output, "c\r\na\r\n");

        fs::remove_dir_all(&root).expect("cleanup");
    }
}
