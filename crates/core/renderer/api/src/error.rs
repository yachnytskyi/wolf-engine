#[derive(Debug)]
pub enum Error {
    Init(&'static str),
    Runtime(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Init(m) => write!(f, "init error: {m}"),
            Error::Runtime(m) => write!(f, "runtime error: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
