use super::{context::Context, error::HandshakeError, RTMP_VERSION, RTMP_HANDSHAKE_SIZE};
use tokio::{net::TcpStream, io::AsyncWriteExt};
use tracing::{trace, info, error, info_span, instrument};

pub struct SimpleHandshake {}

impl SimpleHandshake {
    pub async fn handshake_with_server(&self, mut ctx: Context, io: &mut TcpStream) -> Result<(), HandshakeError>{

        ctx.create_c0c1()?;

        io.write_all(&ctx.c0c1[0..]).await?;

        ctx.read_s0s1s2(io).await?;

        if ctx.s0s1s2[0] != RTMP_VERSION {
            return Err(HandshakeError::InvalidVersion(ctx.s0s1s2[0]));
        }

        // for simple handshake, copy s1 to c2.
        ctx.c2.clear();
        ctx.c2.extend_from_slice(&ctx.s0s1s2[1..RTMP_HANDSHAKE_SIZE+1]);

        io.write_all(&ctx.c2[0..]).await?;

        info!("Simple handshake completed");

        Ok(())
    }
    pub async fn handshake_with_client(&self, mut ctx: Context, io: &mut TcpStream) -> Result<(), HandshakeError>{
        
        ctx.read_c0c1(io).await?;

        trace!("Read c0c1 len {}", ctx.c0c1.len());

        if ctx.c0c1[0] != RTMP_VERSION {
            return Err(HandshakeError::InvalidVersion(ctx.c0c1[0]));
        }

        trace!("Version check pass");

        ctx.create_s0s1s2()?;

        io.write_all(&ctx.s0s1s2).await?;

        trace!("Send s0s1s2 len {}", ctx.s0s1s2.len());

        ctx.read_c2(io).await?;

        trace!("Read c2 len {}", ctx.c2.len());

        info!("Simple handshake completed");

        Ok(())
    }
}