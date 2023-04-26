use msir_core::transport::TransportError;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HandshakeError {
    /// Invalid RTMP version
    #[error("Not support version {0}")]
    InvalidVersion(u8),

    #[error("Complex handshake failed, try simple handshake")]
    TrySimpleHandshake,

    #[error("Transport IO: {0}")]
    TransportIO(#[from] TransportError),

    /// Failed to read the values
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
