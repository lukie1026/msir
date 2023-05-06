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

use msir_service::rtmp_service::RtmpService;
use msir_service::stream::{Manager, StreamEvent};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, info_span, instrument, trace, Instrument};
use tracing_subscriber;

use futures::FutureExt;
use std::error::Error;
use std::{env, io};
use uuid::Uuid;

#[tokio::main(worker_threads = 8)]
async fn main() -> Result<(), Box<dyn Error>> {
    let file_appender = tracing_appender::rolling::never("/tmp", "tracing.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(io::stdout)
        .with_writer(non_blocking) // write to file
        .with_ansi(false) // disable color if write to file
        .init();

    let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();
    let res_mgr = resource_manager_start(rx).map(|r| {
        if let Err(e) = r {
            error!("Resource manager error={}", e);
        }
    });
    tokio::spawn(res_mgr.instrument(tracing::info_span!("RES-MGR")));

    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

    info!("Listening on: {}", listen_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let uid = Uuid::new_v4();
        let rtmp_service = rtmp_service(inbound, uid, tx.clone()).map(|r| {
            if let Err(e) = r {
                error!("Failed to transfer; error={}", e);
            }
        });

        tokio::spawn(
            rtmp_service.instrument(tracing::info_span!("RTMP-CONN", uid = uid.to_string())),
        );
    }

    Ok(())
}

async fn resource_manager_start(
    receiver: mpsc::UnboundedReceiver<StreamEvent>,
) -> Result<(), Box<dyn Error>> {
    Manager::new(receiver).run().await?;
    Ok(())
}

// #[instrument]
async fn rtmp_service(
    inbound: TcpStream,
    uid: Uuid,
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> Result<(), Box<dyn Error>> {
    RtmpService::new(inbound, Some(uid)).await?.run(tx).await?;
    Ok(())
}
