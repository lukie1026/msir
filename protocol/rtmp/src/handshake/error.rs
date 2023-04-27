use thiserror::Error;

use std::io;

#[derive(Debug, Error)]
pub enum HandshakeError {
    /// Invalid RTMP version
    #[error("Not support version {0}")]
    InvalidVersion(u8),

    /// Failed to read the values from the input buffer
    #[error("An IO error occurred while reading the input: {0}")]
    Io(#[from] io::Error),
}
