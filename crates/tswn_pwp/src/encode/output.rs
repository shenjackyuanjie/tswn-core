//! 同目录临时包与无覆盖提交；失败保留临时目录供诊断，不自动删除数据。
//!
//! 按张量/标签预算检查可用空间，不包含 JSON 和文件系统额外开销，检查不是磁盘预留。
//! 固定暂存目录以 create_dir 抢占，同目标的合作导出者互斥；发布前再次检查目标不存在。
//! 每个 payload 采用 create_new、同步并回读摘要，父目录持久性仍由文件系统提供。
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use super::FileEntry;
use crate::{random, storage};

pub struct OutputDirectory {
    pub staging: PathBuf,
    destination: PathBuf,
}

impl OutputDirectory {
    pub fn create(out: &Path, estimated_bytes: u64) -> Result<Self> {
        let name = out.file_name().context("输出目录必须有末级名称")?;
        let parent = out.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let parent = fs::canonicalize(parent)?;
        let destination = parent.join(name);
        ensure_absent(&destination)?;
        let available = fs2::available_space(&parent).context("读取输出磁盘可用空间")?;
        println!(
            "预计张量与标签 {} 字节（不含 JSON/文件系统开销），可用空间 {} 字节",
            estimated_bytes, available
        );
        ensure!(
            estimated_bytes <= available,
            "输出磁盘空间不足：至少需要 {estimated_bytes} 字节，可用 {available} 字节"
        );
        let mut temporary_name = std::ffi::OsString::from(".");
        temporary_name.push(name);
        temporary_name.push(".encode.tmp");
        let staging = parent.join(temporary_name);
        // 原子创建也阻止两个导出器同时占用同一个目标；旧临时包不覆盖、不自动续跑。
        fs::create_dir(&staging)
            .with_context(|| format!("创建临时编码包 {}；已有临时包时请换输出目录或先检查旧现场", staging.display()))?;
        Ok(Self { staging, destination })
    }

    pub fn commit(&self) -> Result<()> {
        ensure_absent(&self.destination)?;
        fs::rename(&self.staging, &self.destination).context("提交完整编码包")?;
        Ok(())
    }
}

pub fn ensure_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => anyhow::bail!("输出目录已存在，不覆盖：{}", path.display()),
    }
}

pub fn write_payload(dir: &Path, name: &str, bytes: &[u8]) -> Result<FileEntry> {
    let path = dir.join(name);
    let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let sha256 = random::hex(&Sha256::digest(bytes));
    ensure!(
        storage::file_hash(&path)? == sha256,
        "输出文件回读摘要不一致：{}",
        path.display()
    );
    Ok(FileEntry {
        file: name.to_owned(),
        byte_length: bytes.len(),
        sha256,
    })
}
