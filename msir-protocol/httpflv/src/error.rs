use std::io;

use rtmp::message::error::MessageEncodeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FlvMuxerError {
    #[error("Encode metadata error: {0}")]
    EncodeError(#[from] MessageEncodeError),

    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
