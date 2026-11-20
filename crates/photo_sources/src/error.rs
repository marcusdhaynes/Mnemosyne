use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    Io { path: Option<PathBuf>, source: io::Error },
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    Message(String),
    Conflict(String),
    /// Documented stub: no network call is made.
    Stub { feature: &'static str, route: &'static str },
    /// Destructive file ops require an explicit confirmed=true.
    ConfirmationRequired(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path: Some(p), source } => write!(f, "{p}: {source}", p = p.display()),
            Error::Io { path: None, source } => write!(f, "{source}"),
            Error::Zip(e) => write!(f, "zip: {e}"),
            Error::Json(e) => write!(f, "json: {e}"),
            Error::Message(m) => write!(f, "{m}"),
            Error::Conflict(m) => write!(f, "conflict: {m}"),
            Error::Stub { feature, route } => {
                write!(f, "{feature} is not connected in this build. {route}")
            }
            Error::ConfirmationRequired(m) => {
                write!(f, "refusing {m} without confirmed=true")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::Io { path: None, source }
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(value: zip::result::ZipError) -> Self {
        Error::Zip(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::Json(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn io_at(path: impl Into<PathBuf>, source: io::Error) -> Error {
    Error::Io { path: Some(path.into()), source }
}
