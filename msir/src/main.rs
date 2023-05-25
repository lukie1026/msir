use crate::http_server::http_server_start;
use crate::rtmp_server::rtmp_server_start;
use msir_service::statistic::{ConnToStatChanTx, StatEvent, Statistic};
use msir_service::stream::{Manager, StreamEvent};
use std::error::Error;
use std::io;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{debug, error, info, info_span, instrument, trace, Instrument};
use tracing_subscriber;
// use tokio_metrics::RuntimeMonitor;

mod http_server;
mod rtmp_server;

#[tokio::main(flavor = "multi_thread")]
// #[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // let handle = tokio::runtime::Handle::current();

    let file_appender = tracing_appender::rolling::never("/tmp", "tracing.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(io::stdout)
        .with_writer(non_blocking) // write to file
        .with_ansi(false) // disable color if write to file
        .init();

    info!("MSIR start...");

    // {
    //     let runtime_monitor = RuntimeMonitor::new(&handle);
    //     tokio::spawn(async move {
    //         for interval in runtime_monitor.intervals() {
    //             // pretty-print the metric interval
    //             info!("{:#?}", interval);
    //             // wait 500ms
    //             tokio::time::sleep(Duration::from_secs(3)).await;
    //         }
    //     });
    // }

    let stat_tx = statistic_bg_start();
    let stream_tx = stream_mgr_start(stat_tx.clone());

    tokio::spawn(proc_stat().instrument(tracing::info_span!("PROC-BG")));
    let stream_tx_c = stream_tx.clone();
    let stat_tx_c = stat_tx.clone();
    tokio::spawn(async move {
        if let Err(err) = rtmp_server_start(stream_tx_c, stat_tx_c).await {
            error!("Start rtmp server error: {}\n", err);
        }
    });
    tokio::spawn(async move {
        if let Err(err) = http_server_start(stream_tx, stat_tx).await {
            error!("Start http server error: {}\n", err);
        }
    });

    signal::ctrl_c().await?;
    info!("MSIR exit...");
    Ok(())
}

fn statistic_bg_start() -> UnboundedSender<StatEvent> {
    let (tx, rx) = mpsc::unbounded_channel::<StatEvent>();
    let stat_bg = Statistic::new(rx);
    tokio::spawn(stat_bg.run().instrument(tracing::info_span!("STAT-BG")));
    tx
}

fn stream_mgr_start(stat_tx: ConnToStatChanTx) -> UnboundedSender<StreamEvent> {
    let (tx, rx) = mpsc::unbounded_channel::<StreamEvent>();
    let stream_mgr = Manager::new(rx, tx.clone(), stat_tx);
    tokio::spawn(
        stream_mgr
            .run()
            .instrument(tracing::info_span!("STREAM-MGR")),
    );
    tx
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
