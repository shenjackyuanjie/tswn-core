//! 工作目录与文件路径管理。
//!
//! [`Store`] 封装各类文件目录（输入、新增、临时、输出）的路径构造逻辑，
//! 以及各模式对应的文件命名约定。构造路径不创建目录，也不做规范化或符号链接检查。
//! 调用方拥有 root 并负责隔离工作目录；全流程会清空 tmp，因此不能将其他数据放在其中。

use std::path::{Path, PathBuf};

use crate::config::{PairMode, SingleMode};

/// DS4 数据目录布局：持有 root，并派生 input/file/new/out/tmp 等标准路径。
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// 以数据根目录构造 Store。
    pub fn new(root: PathBuf) -> Self { Self { root } }

    /// 返回数据根目录。
    pub fn root(&self) -> &Path { &self.root }

    /// 返回 input 输入目录。
    pub fn input_dir(&self) -> PathBuf { self.root.join("input") }
    /// 返回 file 历史数据目录。
    pub fn file_dir(&self) -> PathBuf { self.root.join("file") }
    /// 返回 new 新结果目录。
    pub fn new_dir(&self) -> PathBuf { self.root.join("new") }
    /// 返回 out 输出目录。
    pub fn out_dir(&self) -> PathBuf { self.root.join("out") }
    /// 返回 tmp 中间产物目录。
    pub fn tmp_dir(&self) -> PathBuf { self.root.join("tmp") }

    /// 返回 tmp/new.txt 合并输入路径。
    pub fn tmp_new_file(&self) -> PathBuf { self.tmp_dir().join("new.txt") }
    /// 返回 tmp/new_dup.txt 去重结果路径。
    pub fn tmp_new_dup_file(&self) -> PathBuf { self.tmp_dir().join("new_dup.txt") }
    /// 返回 tmp/blank.txt 空行过滤结果路径。
    pub fn tmp_blank_file(&self) -> PathBuf { self.tmp_dir().join("blank.txt") }

    /// 返回 file/old.txt 历史总表路径。
    pub fn file_old(&self) -> PathBuf { self.file_dir().join("old.txt") }

    /// 返回指定单评模式的 tmp 新评分文件路径。
    pub fn tmp_new_mode_file(&self, mode: SingleMode) -> PathBuf { self.tmp_dir().join(format!("new_{}.txt", mode.as_str())) }

    /// 返回指定单评模式的历史评分文件路径。
    pub fn file_old_mode_file(&self, mode: SingleMode) -> PathBuf { self.file_dir().join(format!("old_{}.txt", mode.as_str())) }

    /// 返回指定单评模式的历史 PTT 文件路径。
    pub fn file_old_mode_ptt_file(&self, mode: SingleMode) -> PathBuf {
        self.file_dir().join(format!("old_{}_ptt.txt", mode.as_str()))
    }

    /// 返回指定单评模式的新评分输出路径。
    pub fn new_mode_file(&self, mode: SingleMode) -> PathBuf { self.new_dir().join(format!("new_{}.txt", mode.as_str())) }

    /// 返回指定单评模式的新 PTT 输出路径。
    pub fn new_mode_ptt_file(&self, mode: SingleMode) -> PathBuf { self.new_dir().join(format!("new_{}_ptt.txt", mode.as_str())) }

    /// 返回指定配对模式的输出文件路径（大写模式名）。
    pub fn out_pair_file(&self, mode: PairMode) -> PathBuf {
        self.out_dir().join(format!("{}.txt", mode.as_str().to_ascii_uppercase()))
    }
}
