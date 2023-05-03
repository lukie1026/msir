use std::io;
use thiserror::Error;

use crate::chunk::error::ChunkError;
use crate::handshake::error::HandshakeError;
use crate::message::error::{ReuquestError, MessageEncodeError};

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Chunk IO error: {0}")]
    ChunkIo(#[from] ChunkError),

    #[error("Handshake failed: {0}")]
    Handshake(#[from] HandshakeError),

    #[error("Receive unexpected message")]
    UnexpectedMessage,

    #[error("A connect_app msg is invalid")]
    InvalidConnectApp,

    #[error("Parse tcUrl failed: {0}")]
    InvalidTcurl(#[from] ReuquestError),

    #[error("Encode rtmp message failed: {0}")]
    RtmpMessageEncode(#[from] MessageEncodeError),

    // Failed to read the values
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
