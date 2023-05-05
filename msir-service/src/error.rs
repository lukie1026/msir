use rtmp::connection::error::ConnectionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] ConnectionError),

    #[error("Register failed: {0}")]
    RegisterFailed(String),
}
