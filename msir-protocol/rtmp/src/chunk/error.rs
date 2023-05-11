use std::io;
use thiserror::Error;

use crate::message::error::{MessageDecodeError, MessageEncodeError};

use super::transport::TransportError;

#[derive(Debug, Error)]
pub enum ChunkError {
    // Invalid fmt
    #[error("Fresh chunk expect fmt=0, actual={0}, csid={1}")]
    InvalidFmtRule1(u8, u32),

    // For existed chunk, fmt should not be 0
    #[error("Existed chunk expect fmt!=0, actual={0}, csid={1}")]
    InvalidFmtRule2(u8, u32),

    // Msg in chunk cache, size changed
    #[error("Msg in chunk cache, size={0} cannot change to {1}")]
    InvalidMsgLengthRule1(usize, usize),

    #[error("Not found extend-timestamp")]
    InvalidExTimestamp,

    #[error("Decode message failed: {0}")]
    DecodeMessageFailed(#[from] MessageDecodeError),

    #[error("Encode message failed: {0}")]
    EncodeMessageFailed(#[from] MessageEncodeError),

    #[error("Transport IO: {0}")]
    TransportIO(#[from] TransportError),

    #[error("Rtmp csid is large than 64, csid={0}")]
    LargeCsid(u32),

    // Failed to read the values
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
