use rml_amf0::{Amf0DeserializationError, Amf0SerializationError};
use thiserror::Error;

use std::io;

/// Error state when deserialization errors occur
/// Enumeration that represents the various errors that may occur while trying to
/// deserialize a RTMP message
#[derive(Debug, Error)]
pub enum MessageDecodeError {
    /// The bytes or amf0 values contained in the message were not what were expected, and thus
    /// the message could not be parsed.
    #[error("The {0} field of the message was not expected")]
    InvalidFormat(String),

    // #[error("Can not find request for command:{0} transcation_id:{1}")]
    // NoRequest(String, f64),

    /// The bytes in the message that were expected to be AMF0 values were not properly encoded,
    /// and thus could not be read
    #[error("Can not decode message: {0}")]
    AmfDecodeFailed(#[from] Amf0DeserializationError),

    /// Failed to read the values from the input buffer
    #[error("An IO error occurred while reading the input: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum MessageEncodeError {
    /// The bytes or amf0 values contained in the message were not what were expected, and thus
    /// the message could not be parsed.
    #[error("The message was not encoded in an expected format")]
    InvalidFormat,

    #[error("Can not encode message: {0}")]
    AmfDecodeFailed(#[from] Amf0SerializationError),

    /// Failed to read the values from the input buffer
    #[error("An IO error occurred while reading the input: {0}")]
    Io(#[from] io::Error),
}
