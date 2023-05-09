use crate::stream::{error::StreamError, hub::HubEvent};
use rtmp::{connection::error::ConnectionError, message::RtmpMessage};
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

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

    #[error("Publish is done")]
    PublishDone,

    #[error("Channel send failed: {0}")]
    SendCh(#[from] SendError<HubEvent>),
}
