//! 原子化文件写入工具。
//!
//! [`AtomicFileWriter`] 在目标同目录暂存内容，刷新并同步文件后，用一次重命名替换目标。
//! 写入或重命名失败时不主动删除旧目标，未提交的临时文件由 Drop 尽力清理。
//! 这里只保证单文件发布边界，不同步父目录，也不把多个文件更新变成事务。
//! [`append_file`] 则直接追加目标文件，失败时可能留下部分追加内容。

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Ds4Result;

/// 同目录暂存的单文件写入器；调用方必须显式提交才能发布内容。
///
/// 持有暂存文件和缓冲区的所有权；直接丢弃不会替换目标，清理失败不会从 Drop 抛错。
/// 同一目标仍应由调用方串行写入，本类型不提供跨进程锁或多文件事务。
pub struct AtomicFileWriter {
    final_path: PathBuf,
    temp_path: PathBuf,
    writer: Option<BufWriter<File>>,
    committed: bool,
}

impl AtomicFileWriter {
    pub fn new(final_path: &Path) -> Ds4Result<Self> {
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or(0);
        let temp_name = format!(
            "{}.tmp-{}-{timestamp}",
            final_path.file_name().and_then(|name| name.to_str()).unwrap_or("atomic"),
            std::process::id()
        );
        let temp_path = final_path.with_file_name(temp_name);
        let file = File::create(&temp_path)?;

        Ok(Self {
            final_path: final_path.to_path_buf(),
            temp_path,
            writer: Some(BufWriter::new(file)),
            committed: false,
        })
    }

    pub fn writer(&mut self) -> &mut BufWriter<File> { self.writer.as_mut().expect("writer should exist before commit") }

    /// 消费写入器，先 flush/sync_all 再关闭暂存文件并替换目标。
    ///
    /// 任一步失败均返回 I/O 错误；不以删除旧目标的方式降级重试，尤其保留 Windows
    /// 文件占用时的旧输出。成功后暂存路径不再存在。
    pub fn commit(mut self) -> Ds4Result<()> {
        let mut writer = self.writer.take().expect("writer should exist before commit");
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);

        fs::rename(&self.temp_path, &self.final_path)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for AtomicFileWriter {
    fn drop(&mut self) {
        if !self.committed && self.temp_path.exists() {
            let _ = fs::remove_file(&self.temp_path);
        }
    }
}

pub fn write_bytes_atomic(path: &Path, content: &[u8]) -> Ds4Result<()> {
    let mut writer = AtomicFileWriter::new(path)?;
    writer.writer().write_all(content)?;
    writer.commit()
}

pub fn append_file(source: &Path, destination: &Path) -> Ds4Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut input = File::open(source)?;
    let mut output = OpenOptions::new().create(true).append(true).open(destination)?;
    let mut buffer = [0u8; 8192];

    loop {
        let read_size = input.read(&mut buffer)?;
        if read_size == 0 {
            break;
        }
        output.write_all(&buffer[..read_size])?;
    }

    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn commit_replaces_existing_file() {
        let root = temp_dir("tswn-ds4-replace");
        let path = root.join("result.txt");
        fs::write(&path, b"old").unwrap();
        write_bytes_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dropping_uncommitted_writer_removes_temporary_file() {
        let root = temp_dir("tswn-ds4-drop-writer");
        let path = root.join("result.txt");
        fs::write(&path, b"old").unwrap();
        let mut writer = AtomicFileWriter::new(&path).unwrap();
        writer.writer().write_all(b"new").unwrap();
        let temporary = writer.temp_path.clone();
        drop(writer);
        assert!(!temporary.exists());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn occupied_destination_preserves_old_content_and_cleans_temporary_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = temp_dir("tswn-ds4-occupied-replace");
        let path = root.join("result.txt");
        fs::write(&path, b"old").unwrap();
        let mut writer = AtomicFileWriter::new(&path).unwrap();
        writer.writer().write_all(b"new").unwrap();
        let temporary = writer.temp_path.clone();
        let occupied = OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
        assert!(writer.commit().is_err());
        assert!(!temporary.exists());
        drop(occupied);
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_commit_preserves_existing_file() {
        let root = temp_dir("tswn-ds4-failed-replace");
        let path = root.join("result.txt");
        fs::write(&path, b"old").unwrap();
        let mut writer = AtomicFileWriter::new(&path).unwrap();
        writer.writer().write_all(b"new").unwrap();
        fs::remove_file(&writer.temp_path).unwrap();
        assert!(writer.commit().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
