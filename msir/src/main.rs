#![warn(rust_2018_idioms)]

use msir_core::transport::Transport;
use msir_service::rtmp_service::RtmpService;
use msir_service::statistic::Statistic;
use msir_service::stream::{Manager, StreamEvent};
use msir_service::utils;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::mpsc;
use tracing::{debug, error, info, info_span, instrument, trace, Instrument};
use tracing_subscriber;

use futures::FutureExt;
use std::error::Error;
use std::time::Duration;
use std::{env, io};

//#[tokio::main(worker_threads = 8)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let file_appender = tracing_appender::rolling::never("/tmp", "tracing.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
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
    tokio::spawn(res_mgr.instrument(tracing::info_span!("STREAM-MGR")));
    tokio::spawn(proc_stat());
    tokio::spawn(async move {
        if let Err(err) = rtmp_server_start(tx).await {
            error!("Rtmp server error: {}\n", err);
        }
    });

    signal::ctrl_c().await?;
    info!("msir exit...");
    Ok(())
}

async fn resource_manager_start(
    receiver: mpsc::UnboundedReceiver<StreamEvent>,
) -> Result<(), Box<dyn Error>> {
    Manager::new(receiver).run().await?;
    Ok(())
}

async fn rtmp_server_start(tx: mpsc::UnboundedSender<StreamEvent>) -> Result<(), Box<dyn Error>> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

    info!("Listening on: {}", listen_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let uid = utils::gen_uid();
        let rtmp_service = rtmp_service(inbound, uid.clone(), tx.clone()).map(|r| {
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
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> Result<(), Box<dyn Error>> {
    RtmpService::new(Transport::new(inbound), Some(uid))
        .await?
        .run(tx)
        .await?;
    Ok(())
}

#[cfg(target_os = "linux")]
async fn proc_stat() {
    use procfs::process::Stat;
    let intval = 5;
    let mut interval = tokio::time::interval(Duration::from_secs(intval));
    let mut last_stat: Option<Stat> = None;
    loop {
        interval.tick().await;
        let curr = procfs::process::Process::myself().unwrap().stat().unwrap();
        let memory = curr.rss * procfs::page_size();
        if let Some(last) = last_stat {
            let cpu = (100 * (curr.utime + curr.stime - last.utime - last.stime)) as f32
                / intval as f32
                / procfs::ticks_per_second() as f32;
            info!("CPU {}% MEM {}MB", cpu, memory / 1024 / 1024);
        }

        last_stat = Some(curr);
    }
}

#[cfg(not(target_os = "linux"))]
async fn proc_stat() {}
