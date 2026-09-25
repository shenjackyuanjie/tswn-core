//! 错误类型定义。
//!
//! 定义 [`Ds4Error`] 枚举（覆盖 IO/JSON/配置/解析等错误来源）及 [`Ds4Result<T>`] 类型别名。

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
