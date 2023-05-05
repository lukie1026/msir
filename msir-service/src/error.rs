use crate::stream::error::StreamError;
use rtmp::connection::error::ConnectionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] ConnectionError),

    #[error("Hub error: {0}")]
    HubError(#[from] StreamError),

    #[error("Register failed: {0}")]
    RegisterFailed(String),

    #[error("The token is invalid")]
    InvalidToken,
}
