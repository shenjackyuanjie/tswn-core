//! 错误类型定义。
//!
//! [`Ds4Error`] 区分原始 I/O 错误、解析错误文本、CLI 用法错误和缺失配置的路径列表。
//! JSON 与外部程序失败也会转换为 Parse 文本，不存在单独的 JSON 或子进程错误分支。
//! [`Ds4Result`] 由同步库接口返回，宿主决定如何显示错误及处理已经产生的部分输出。

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

pub type Ds4Result<T> = Result<T, Ds4Error>;

#[derive(Debug)]
pub enum Ds4Error {
    Io(io::Error),
    Parse(String),
    Cli(String),
    MissingConfig { checked: Vec<PathBuf> },
}

impl Ds4Error {
    pub fn parse(message: impl Into<String>) -> Self { Self::Parse(message.into()) }

    pub fn cli(message: impl Into<String>) -> Self { Self::Cli(message.into()) }
}

impl Display for Ds4Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Parse(message) => write!(f, "parse error: {message}"),
            Self::Cli(message) => write!(f, "cli error: {message}"),
            Self::MissingConfig { checked } => {
                write!(f, "no config file found, checked: ")?;
                for (index, path) in checked.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", path.display())?;
                }
                Ok(())
            }
        }
    }
}

impl Error for Ds4Error {}

impl From<io::Error> for Ds4Error {
    fn from(value: io::Error) -> Self { Self::Io(value) }
}
