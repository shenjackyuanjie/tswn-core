use std::path::{Path, PathBuf};

use crate::config::{PairMode, SingleMode};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    pub fn root(&self) -> &Path { &self.root }

    pub fn input_dir(&self) -> PathBuf { self.root.join("input") }
    pub fn file_dir(&self) -> PathBuf { self.root.join("file") }
    pub fn new_dir(&self) -> PathBuf { self.root.join("new") }
    pub fn out_dir(&self) -> PathBuf { self.root.join("out") }
    pub fn tmp_dir(&self) -> PathBuf { self.root.join("tmp") }

    pub fn tmp_new_file(&self) -> PathBuf { self.tmp_dir().join("new.txt") }
    pub fn tmp_new_dup_file(&self) -> PathBuf { self.tmp_dir().join("new_dup.txt") }
    pub fn tmp_blank_file(&self) -> PathBuf { self.tmp_dir().join("blank.txt") }

    pub fn file_old(&self) -> PathBuf { self.file_dir().join("old.txt") }

    pub fn tmp_new_mode_file(&self, mode: SingleMode) -> PathBuf { self.tmp_dir().join(format!("new_{}.txt", mode.as_str())) }

    pub fn file_old_mode_file(&self, mode: SingleMode) -> PathBuf { self.file_dir().join(format!("old_{}.txt", mode.as_str())) }

    pub fn file_old_mode_ptt_file(&self, mode: SingleMode) -> PathBuf {
        self.file_dir().join(format!("old_{}_ptt.txt", mode.as_str()))
    }

    pub fn new_mode_file(&self, mode: SingleMode) -> PathBuf { self.new_dir().join(format!("new_{}.txt", mode.as_str())) }

    pub fn new_mode_ptt_file(&self, mode: SingleMode) -> PathBuf { self.new_dir().join(format!("new_{}_ptt.txt", mode.as_str())) }

    pub fn out_pair_file(&self, mode: PairMode) -> PathBuf {
        self.out_dir().join(format!("{}.txt", mode.as_str().to_ascii_uppercase()))
    }
}
