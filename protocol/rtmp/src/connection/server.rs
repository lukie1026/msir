use tokio::net::TcpStream;

use crate::handshake;
use crate::handshake::simple_hs::SimpleHandshake;

use super::{context::Context, error::ConnectionError};

pub struct Server {
    ctx: Context,
}

impl Server {
    pub fn new(io: TcpStream) -> Self {
        Self {
            ctx: Context::new(io),
        }
    }
    pub fn handshake(&mut self) -> Result<(), ConnectionError> {
        let hs = SimpleHandshake {};
        let hs_ctx = handshake::context::Context::new();
        // hs.handshake_with_client(hs_ctx, &mut inbound).await?;
        Ok(())
    }
}