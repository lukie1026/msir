use crate::http_server::http_server_start;
use crate::rtmp_server::rtmp_server_start;
use clap::{value_parser, Arg, Command};
use config::LogConfig;
use msir_service::statistic::{ConnToStatChanTx, StatEvent, Statistic};
use msir_service::stream::{Manager, StreamEvent};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use std::{io, process};
use tokio::runtime;
use tokio::signal;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::{error, info, Instrument};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber;
// use tokio_metrics::RuntimeMonitor;

mod config;
mod http_server;
mod rtmp_server;

// #[tokio::main(flavor = "multi_thread")]
// #[tokio::main(flavor = "current_thread")]
fn main() -> Result<(), Box<dyn Error>> {
    // let handle = tokio::runtime::Handle::current();

    let matches = Command::new("MSIR")
        .bin_name("msir")
        .arg(
            Arg::new("config_file_path")
                .long("config")
                .short('c')
                .value_name("path")
                .help("Specify the configuration file path.")
                .value_parser(value_parser!(String)),
        )
        .get_matches();

    let cfg = if let Some(path) = matches.get_one::<String>("config_file_path") {
        config::load(path)?
    } else {
        config::Config::default()
    };

    info!("MSIR start...");

    let _guard = log_init(&cfg.log.unwrap());

    let rt = match cfg.worker.unwrap().mode.as_str() {
        "single" => runtime::Builder::new_current_thread()
            .enable_all()
            .build()?,
        _ => runtime::Builder::new_multi_thread() /*.worker_threads(2)*/
            .enable_all()
            .build()?,
    };

    rt.block_on(async {
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

        let stat_tx_c = stat_tx.clone();
        let stream_tx_c = stream_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = rtmp_server_start(stream_tx_c, stat_tx_c, &cfg.rtmp.unwrap()).await {
                error!("Start rtmp server error: {}\n", err);
                process::exit(-1);
            }
        });
        tokio::spawn(async move {
            if let Err(err) = http_server_start(stream_tx, stat_tx, &cfg.http.unwrap()).await {
                error!("Start http server error: {}\n", err);
                process::exit(-1);
            }
        });

        tokio::spawn(proc_stat().instrument(tracing::info_span!("PROC-BG")));

        signal::ctrl_c().await.unwrap();
    });

    info!("MSIR exit...");
    Ok(())
}

fn log_init(config: &LogConfig) -> Option<WorkerGuard> {
    let level = match config.level.as_str() {
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        "info" => tracing::Level::INFO,
        "debug" => tracing::Level::DEBUG,
        "trace" => tracing::Level::TRACE,
        _ => tracing::Level::ERROR,
    };
    if let Some(file) = &config.file {
        let path = Path::new(file);
        let rolling = match &config.rolling {
            Some(r) => r.as_str(),
            None => "never",
        };
        let file_appender = match rolling {
            "hour" => tracing_appender::rolling::hourly(
                path.parent().unwrap().as_os_str(),
                path.file_name().unwrap(),
            ),
            "daily" => tracing_appender::rolling::daily(
                path.parent().unwrap().as_os_str(),
                path.file_name().unwrap(),
            ),
            _ => tracing_appender::rolling::never(
                path.parent().unwrap().as_os_str(),
                path.file_name().unwrap(),
            ),
        };
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::fmt()
            .with_max_level(level)
            .with_writer(non_blocking) // write to file
            .with_ansi(false) // disable color if write to file
            .init();
        Some(_guard)
    } else {
        tracing_subscriber::fmt()
            .with_max_level(level)
            .with_writer(io::stdout)
            .with_ansi(true)
            .init();
        None
    }
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
