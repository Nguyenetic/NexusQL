//! Error types for NexusSQL
//! PROPRIETARY

use thiserror::Error;

pub type Result<T> = std::result::Result<T, NexusError>;

#[derive(Error, Debug)]
pub enum NexusError {
    #[error("Engine '{0}' not found")]
    EngineNotFound(String),

    #[error("Query execution failed: {0}")]
    QueryError(String),

    #[error("Engine error in '{engine}': {message}")]
    EngineError { engine: String, message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Resource exhausted: {0}")]
    ResourceError(String),

    #[error("Sled database error: {0}")]
    SledError(#[from] sled::Error),

    #[error("Serialization error: {0}")]
    SerializeError(#[from] Box<bincode::ErrorKind>),

    #[error("Execution error: {0}")]
    ExecutionError(String),
}
