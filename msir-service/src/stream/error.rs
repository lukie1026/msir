use thiserror::Error;

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("Stream has been published")]
    DuplicatePublish,

    #[error("Do not connect to hub")]
    DisconnectHub,

    #[error("There was no publish")]
    NoPublish,
}
