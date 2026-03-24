use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Ds3Result;

pub struct AtomicFileWriter {
    final_path: PathBuf,
    temp_path: PathBuf,
    writer: Option<BufWriter<File>>,
    committed: bool,
}

impl AtomicFileWriter {
    pub fn new(final_path: &Path) -> Ds3Result<Self> {
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

    pub fn commit(mut self) -> Ds3Result<()> {
        let mut writer = self.writer.take().expect("writer should exist before commit");
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);

        if self.final_path.exists() {
            fs::remove_file(&self.final_path)?;
        }
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

pub fn write_bytes_atomic(path: &Path, content: &[u8]) -> Ds3Result<()> {
    let mut writer = AtomicFileWriter::new(path)?;
    writer.writer().write_all(content)?;
    writer.commit()
}

pub fn append_file(source: &Path, destination: &Path) -> Ds3Result<()> {
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
