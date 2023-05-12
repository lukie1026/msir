use super::{context::Context, error::HandshakeError, RTMP_HANDSHAKE_SIZE, RTMP_VERSION};
use msir_core::transport::Transport;
use tracing::{debug, error, info, info_span, instrument, trace};

pub struct ComplexHandshake {}

impl ComplexHandshake {
    pub async fn handshake_with_server(
        &self,
        ctx: &mut Context,
        io: &mut Transport,
    ) -> Result<(), HandshakeError> {
        trace!("Complex handshake do not implement, try simple");

        Err(HandshakeError::TrySimpleHandshake)
    }
    pub async fn handshake_with_client(
        &self,
        ctx: &mut Context,
        io: &mut Transport,
    ) -> Result<(), HandshakeError> {
        trace!("Complex handshake do not implement, try simple");

        Err(HandshakeError::TrySimpleHandshake)
    }
}
