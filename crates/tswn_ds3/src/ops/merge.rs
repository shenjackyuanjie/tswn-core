//! 输入文件合并操作。
//!
//! 按文件名排序读取输入目录下的普通文件，并把原始字节顺序写入目标文件。
//! 该行为用于复刻原版 DS3 工具“先合并新增输入再进入去重/排序”的处理链。

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::Ds3Result;
use crate::output::AtomicFileWriter;

pub fn merge_input_files(input_dir: &Path, output_file: &Path) -> Ds3Result<usize> {
    let mut entries = Vec::new();
    if input_dir.exists() {
        for entry in fs::read_dir(input_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                entries.push(path);
            }
        }
    }
    entries.sort();

    let mut writer = AtomicFileWriter::new(output_file)?;
    let out = writer.writer();
    let mut merged_files = 0usize;

    for path in entries {
        let content = fs::read(&path)?;
        out.write_all(&content)?;
        merged_files += 1;
    }

    writer.commit()?;
    Ok(merged_files)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::merge_input_files;

    fn temp_dir(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn merge_files_in_lexicographic_order() {
        let root = temp_dir("tswn-ds3-merge");
        let input_dir = root.join("input");
        fs::create_dir_all(&input_dir).expect("create input dir");
        fs::write(input_dir.join("b.txt"), "B\n").expect("write b");
        fs::write(input_dir.join("a.txt"), "A\n").expect("write a");
        fs::write(input_dir.join("c.txt"), "C\n").expect("write c");

        let output = root.join("tmp").join("new.txt");
        let count = merge_input_files(&input_dir, &output).expect("merge");
        let content = fs::read_to_string(&output).expect("read output");

        assert_eq!(count, 3);
        assert_eq!(content, "A\nB\nC\n");

        fs::remove_dir_all(&root).expect("cleanup");
    }
}
