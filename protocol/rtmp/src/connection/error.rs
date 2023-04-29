use thiserror::Error;
use std::io;

use crate::chunk::error::ChunkError;

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Chunk IO error")]
    ChunkIo(#[from] ChunkError),

    // Failed to read the values
    #[error("An IO error occurred: {0}")]
    Io(#[from] io::Error),
}
