use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("invalid parameter: {0}")]
    Param(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

impl SdkError {
    pub fn param(msg: impl Into<String>) -> Self {
        Self::Param(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl From<std::io::Error> for SdkError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SdkError>;
