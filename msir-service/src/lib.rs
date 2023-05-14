use std::time::Duration;

pub mod error;
pub mod rtmp_pull;
pub mod rtmp_service;
pub mod statistic;
pub mod stream;
pub mod utils;

const CONN_PRINT_INTVAL: Duration = Duration::from_secs(5);
const PERF_MERGE_SEND_MSG: u32 = 350;
const PERF_MERGE_SEND_CHAN: u32 = 170;

const STATIC_PULL_ADDRESS: &str = "rtmp://127.0.0.1";
