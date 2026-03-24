use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

pub type Ds3Result<T> = Result<T, Ds3Error>;

#[derive(Debug)]
pub enum Ds3Error {
    Io(io::Error),
    Parse(String),
    Cli(String),
    MissingConfig { checked: Vec<PathBuf> },
}

impl Ds3Error {
    pub fn parse(message: impl Into<String>) -> Self { Self::Parse(message.into()) }

    pub fn cli(message: impl Into<String>) -> Self { Self::Cli(message.into()) }
}

impl Display for Ds3Error {
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

impl Error for Ds3Error {}

impl From<io::Error> for Ds3Error {
    fn from(value: io::Error) -> Self { Self::Io(value) }
}
