use msir_core::transport::Transport;

use self::error::HandshakeError;

mod complex_hs;
mod context;
pub mod error;
mod simple_hs;

const RTMP_VERSION: u8 = 3;
const RTMP_HANDSHAKE_SIZE: usize = 1536;

pub struct Server {
    simple: simple_hs::SimpleHandshake,
    complex: complex_hs::ComplexHandshake,
    ctx: context::Context,
}

impl Server {
    pub fn new() -> Self {
        Self {
            simple: simple_hs::SimpleHandshake {},
            complex: complex_hs::ComplexHandshake {},
            ctx: context::Context::new(),
        }
    }
    pub async fn handshake(&mut self, io: &mut Transport) -> Result<(), HandshakeError> {
        match self.complex.handshake_with_client(&mut self.ctx, io).await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if let HandshakeError::TrySimpleHandshake = err {
                    self.simple.handshake_with_client(&mut self.ctx, io).await?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

pub struct Client {
    simple: simple_hs::SimpleHandshake,
    ctx: context::Context,
}

impl Client {
    pub fn new() -> Self {
        Self {
            simple: simple_hs::SimpleHandshake {},
            ctx: context::Context::new(),
        }
    }
    pub async fn handshake(&mut self, io: &mut Transport) -> Result<(), HandshakeError> {
        self.simple.handshake_with_server(&mut self.ctx, io).await
    }
}
