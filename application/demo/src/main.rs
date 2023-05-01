//! A proxy that forwards data to another server and forwards that server's
//! responses back to clients.
//!
//! Because the Tokio runtime uses a thread pool, each TCP connection is
//! processed concurrently with all other TCP connections across multiple
//! threads.
//!
//! You can showcase this by running this in one terminal:
//!
//!     cargo run --example proxy
//!
//! This in another terminal
//!
//!     cargo run --example echo
//!
//! And finally this in another terminal
//!
//!     cargo run --example connect 127.0.0.1:8081
//!
//! This final terminal will connect to our proxy, which will in turn connect to
//! the echo server, and you'll be able to see data flowing between them.

#![warn(rust_2018_idioms)]

use rtmp::chunk::ChunkCodec;
use rtmp::handshake::context::Context;
use rtmp::handshake::simple_hs::SimpleHandshake;
use rtmp::message::RtmpMessage;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, info_span, instrument, trace};
use tracing_subscriber;

use futures::FutureExt;
use std::env;
use std::error::Error;

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        // enable everything
        .with_max_level(tracing::Level::TRACE)
        // sets this to be the default, global collector for this application.
        .init();

    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8081".to_string());

    info!("Listening on: {}", listen_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let rtmp_service = rtmp_service(inbound).map(|r| {
            if let Err(e) = r {
                error!("Failed to transfer; error={}", e);
            }
        });

        tokio::spawn(rtmp_service);
    }

    Ok(())
}

#[instrument]
async fn rtmp_service(mut inbound: TcpStream) -> Result<(), Box<dyn Error>> {
    let hs = SimpleHandshake {};
    let hs_ctx = Context::new();
    hs.handshake_with_client(hs_ctx, &mut inbound).await?;

    let mut chunk = ChunkCodec::new(inbound);
    loop {
        let msg = chunk.recv_rtmp_message().await?;

        trace!("Got rtmp message: {:?}", msg);

        let (app, reply) = chunk.handle_connect(msg);
        if reply {
            trace!("Relay connect: {:?}", app);
            chunk.relay_connect(app).await?;
            
        }
        
    }

    Ok(())
}
