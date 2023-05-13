use anyhow::Result;
use futures::FutureExt;
use msir_core::transport::Transport;
use msir_service::{rtmp_service::RtmpService, statistic::StatEvent, stream::StreamEvent, utils};
use std::env;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{error, info, Instrument};

pub async fn rtmp_server_start(
    stream_tx: mpsc::UnboundedSender<StreamEvent>,
    stat_tx: mpsc::UnboundedSender<StatEvent>,
) -> Result<()> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

    info!("Listening on: {}", listen_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let uid = utils::gen_uid();
        let rtmp_service = rtmp_service(inbound, uid.clone(), stream_tx.clone(), stat_tx.clone())
            .map(|r| {
                if let Err(e) = r {
                    error!("Failed to transfer; error={}", e);
                }
            });

        tokio::spawn(rtmp_service.instrument(tracing::info_span!("RTMP-CONN", uid)));
    }

    Ok(())
}

// #[instrument]
async fn rtmp_service(
    inbound: TcpStream,
    uid: String,
    stream: mpsc::UnboundedSender<StreamEvent>,
    stat: mpsc::UnboundedSender<StatEvent>,
) -> Result<()> {
    RtmpService::new(Transport::new(inbound), Some(uid), stream, stat)
        .await?
        .run()
        .await?;
    Ok(())
}
