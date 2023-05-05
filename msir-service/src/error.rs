use rtmp::connection::error::ConnectionError;
use thiserror::Error;
// use tokio::sync::oneshot::error::RecvError;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] ConnectionError),

    #[error("Stream has been published")]
    DuplicatePublish,

    #[error("Register failed: {0}")]
    RegisterFailed(String),
}
