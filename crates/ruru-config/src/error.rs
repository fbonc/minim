use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("configuration directory is unavailable")]
    ConfigDirectoryUnavailable,
    #[error("configuration I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("configuration version {0} is unsupported")]
    UnsupportedVersion(u32),
    #[error("credential storage failed: {0}")]
    Credential(String),
}

pub type Result<T> = std::result::Result<T, Error>;
