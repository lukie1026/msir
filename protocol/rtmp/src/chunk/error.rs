use thiserror::Error;
use std::io;

use crate::message::error::{MessageDecodeError, MessageEncodeError};

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

    // Failed to read the values
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
