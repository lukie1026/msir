use tokio::net::TcpStream;
use self::error::HandshakeError;

mod context;
mod simple_hs;
mod complex_hs;
pub mod error;

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
    pub async fn handshake(&mut self, io: &mut TcpStream) -> Result<(), HandshakeError> {
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
    io: TcpStream,
}

impl Client {
    pub fn new(io: TcpStream) -> Self {
        Self {
            simple: simple_hs::SimpleHandshake {},
            ctx: context::Context::new(),
            io,
        }
    }
    pub async fn handshake(&mut self) -> Result<(), HandshakeError> {
        self.simple
            .handshake_with_client(&mut self.ctx, &mut self.io)
            .await
    }
}
