use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("plugin load failed: {0}")]
    Load(String),
    #[error("abi mismatch: plugin={plugin} host={host}")]
    AbiMismatch { plugin: u32, host: u32 },
    #[error("unknown node type '{0}'")]
    UnknownType(String),
    #[error("graph error: {0}")]
    Graph(String),
    #[error("engine error: {0}")]
    Engine(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
