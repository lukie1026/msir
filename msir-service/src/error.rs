use crate::stream::error::StreamError;
use futures::channel::mpsc::SendError;
use httpflv::error::FlvMuxerError;
use rtmp::{connection::error::ConnectionError, message::error::ReuquestError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Flv muxer error: {0}")]
    FlvError(#[from] FlvMuxerError),

    #[error("Channel send error: {0}")]
    ChanSendError(#[from] SendError),

    #[error("Request parse error: {0}")]
    RequestError(#[from] ReuquestError),

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

    #[error("No subscriber")]
    NoSubscriber,
}
