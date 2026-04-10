use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::Ds3Result;
use crate::output::AtomicFileWriter;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DedupStats {
    pub new_unique: usize,
    pub old_hits: usize,
    pub remaining: usize,
}

pub fn remove_duplicates(new_file: &Path, old_file: &Path, output_file: &Path) -> Ds3Result<DedupStats> {
    let new_content = read_optional_file(new_file)?;
    let old_content = read_optional_file(old_file)?;

    let mut set = HashSet::new();
    let mut insertion_order = Vec::new();
    for line in split_like_cpp(&new_content) {
        if !line.is_empty() && set.insert(line.to_string()) {
            insertion_order.push(line.to_string());
        }
    }
    let new_unique = set.len();

    let mut old_hits = 0usize;
    for line in split_like_cpp(&old_content) {
        if !line.is_empty() && set.remove(line) {
            old_hits += 1;
        }
    }

    let mut rows = Vec::new();
    for row in insertion_order.into_iter().rev() {
        if set.contains(&row) {
            rows.push(row);
        }
    }

    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();
    for row in &rows {
        out.write_all(row.as_bytes())?;
        out.write_all(b"\r\n")?;
    }
    writer.commit()?;

    Ok(DedupStats {
        new_unique,
        old_hits,
        remaining: rows.len(),
    })
}

fn read_optional_file(path: &Path) -> Ds3Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(fs::read_to_string(path)?)
}

fn split_like_cpp(source: &str) -> impl Iterator<Item = &str> { source.split(['\n', '\r']).filter(|line| !line.is_empty()) }

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
        let root = temp_dir("tswn-ds3-dedup");
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
