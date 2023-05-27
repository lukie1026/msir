use msir_core::utils;
use rtmp::connection::RtmpConnType;
use serde_derive::Serialize;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

const STREAM_PRINT_INTVAL: Duration = Duration::from_secs(10);

pub type ConnToStatChanTx = mpsc::UnboundedSender<StatEvent>;
pub type ConnToStatChanRx = mpsc::UnboundedReceiver<StatEvent>;

pub type QueryConnsResponse = oneshot::Sender<HashMap<String, ConnStat>>;
pub type QueryStreamsResponse = oneshot::Sender<HashMap<String, StreamStat>>;
pub type QuerySummariesResponse = oneshot::Sender<SummariesStat>;

pub enum StatEvent {
    CreateConn(String, ConnStat),
    DeleteConn(String, ConnStat),
    UpdateConn(String, ConnStat),

    QueryConn(String, QueryConnsResponse),
    QueryStream(String, QueryStreamsResponse),
    QuerySummaries(QuerySummariesResponse),
}

pub struct Statistic {
    rx: ConnToStatChanRx,
    conns: HashMap<String, ConnStat>,
    streams: HashMap<String, StreamStat>,
    summaries: SummariesStat,
}

impl Statistic {
    pub fn new(rx: ConnToStatChanRx) -> Self {
        Self {
            rx,
            conns: HashMap::new(),
            streams: HashMap::new(),
            summaries: SummariesStat::new(String::from(msir_core::VERSION)),
        }
    }

    pub async fn run(mut self) {
        let intval = 5;
        let mut tick = tokio::time::interval(Duration::from_secs(intval));
        loop {
            tokio::select! {
                event = self.rx.recv() => {
                    if let Some(ev) = event {
                        match ev {
                            StatEvent::CreateConn(uid, cs) => self.on_create_conn(uid, cs),
                            StatEvent::DeleteConn(uid, cs) => self.on_delete_conn(uid, cs),
                            StatEvent::UpdateConn(uid, cs) => self.on_update_conn(uid, cs),

                            StatEvent::QueryConn(filter, tx) => self.on_query_conns(filter, tx),
                            StatEvent::QueryStream(filter, tx) => self.on_query_streams(filter, tx),
                            StatEvent::QuerySummaries(tx) => self.on_query_summaries(tx),
                        }
                    }
                }
                _ = tick.tick() => {
                    self.summaries.update(intval);
                    self.summaries.conns = self.conns.len();
                    self.summaries.streams = self.streams.len();
                    info!("CPU {}% MEM {}MB, streams:{} conns:{}", self.summaries.cpu_percent, self.summaries.mem_mbytes, self.summaries.streams, self.summaries.conns);
                }
            }
        }
    }

    fn on_create_conn(&mut self, uid: String, stat: ConnStat) {
        let stream_key = stat.stream_key.clone();
        let conn_type = stat.conn_type.clone();
        self.conns.insert(uid, stat);
        // PullClient need to first call than player
        if conn_type.is_play() && !self.streams.contains_key(&stream_key) {
            error!("No publish stats record before player stats arrived");
            return;
        }
        let stream = self
            .streams
            .entry(stream_key.clone())
            .or_insert(StreamStat::new(utils::current_time(), conn_type));
        stream.clients += 1;
        stream.can_print(&stream_key, Instant::now());
    }

    fn on_update_conn(&mut self, uid: String, stat: ConnStat) {
        if let Some(conn) = self.conns.get_mut(&uid) {
            let delta_send_bytes = stat.send_bytes - conn.send_bytes;
            conn.audio_count = stat.audio_count;
            conn.video_count = stat.video_count;
            conn.recv_bytes = stat.recv_bytes;
            conn.send_bytes = stat.send_bytes;
            conn.conn_type = stat.conn_type;
            conn.stream_key = stat.stream_key;

            self.streams.get_mut(&conn.stream_key).map(|s| {
                if conn.conn_type.is_publish() {
                    s.audio = stat.audio_count;
                    s.video = stat.video_count;
                    s.recv_bytes = stat.recv_bytes;
                    s.can_print(&conn.stream_key, Instant::now());
                } else {
                    s.send_bytes += delta_send_bytes;
                }
            });
        }
    }

    fn on_delete_conn(&mut self, uid: String, stat: ConnStat) {
        let conn = self.conns.remove(&uid);
        match stat.conn_type.is_publish() {
            true => {
                self.streams.remove(&stat.stream_key);
                info!("StreamStats {} removed", stat.stream_key);
            }
            false => {
                self.streams.get_mut(&stat.stream_key).map(|s| {
                    s.clients -= 1;
                    conn.map(|c| {
                        s.send_bytes += stat.send_bytes - c.send_bytes;
                    })
                });
            }
        }
    }

    fn on_query_streams(&mut self, filter: String, tx: QueryStreamsResponse) {
        if filter.is_empty() {
            let _ = tx.send(self.streams.clone());
        } else {
            let mut res = HashMap::new();
            for (k, v) in &self.streams {
                if k.contains(&filter) {
                    res.insert(k.clone(), v.clone());
                }
            }
            let _ = tx.send(res);
        }
    }

    fn on_query_conns(&mut self, filter: String, tx: QueryConnsResponse) {
        if filter.is_empty() {
            let _ = tx.send(self.conns.clone());
        } else {
            let mut res = HashMap::new();
            for (k, v) in &self.conns {
                if k.contains(&filter) {
                    res.insert(k.clone(), v.clone());
                }
            }
            let _ = tx.send(res);
        }
    }

    fn on_query_summaries(&mut self, tx: QuerySummariesResponse) {
        self.summaries.streams = self.streams.len();
        self.summaries.conns = self.conns.len();

        let _ = tx.send(self.summaries.clone());
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamStat {
    pub audio: u64,
    pub video: u64,
    pub recv_bytes: u64,
    pub send_bytes: u64,
    pub publish_type: RtmpConnType,
    pub start_time: u32,
    pub clients: u32,
    // Last record for log print
    #[serde(skip_serializing)]
    last_logged: Option<Instant>,
    #[serde(skip_serializing)]
    last_video: u64,
    #[serde(skip_serializing)]
    last_recv_bytes: u64,
    #[serde(skip_serializing)]
    last_send_bytes: u64,
}

impl StreamStat {
    pub fn new(start_time: u32, publish_type: RtmpConnType) -> Self {
        Self {
            audio: 0,
            video: 0,
            recv_bytes: 0,
            send_bytes: 0,
            clients: 0,
            publish_type,
            start_time,
            last_logged: None,
            last_video: 0,
            last_recv_bytes: 0,
            last_send_bytes: 0,
        }
    }

    fn can_print(&mut self, stream: &str, now: Instant) {
        let dur = self.last_logged.map(|last| now.duration_since(last));
        let mut fps = 0;
        let mut recv_rate = 0;
        let mut send_rate = 0;
        if let Some(d) = dur {
            if d < STREAM_PRINT_INTVAL {
                return;
            }
            fps = (self.video - self.last_video) / d.as_secs();
            recv_rate = (self.recv_bytes - self.last_recv_bytes) / d.as_secs();
            send_rate = (self.send_bytes - self.last_send_bytes) / d.as_secs();
        }
        self.last_logged = Some(now);
        self.last_video = self.video;
        self.last_recv_bytes = self.recv_bytes;
        self.last_send_bytes = self.send_bytes;
        info!(
            "StreamStats {} has clients {}, fps={}, in={}, out={}",
            stream,
            self.clients,
            fps,
            format_bw(recv_rate),
            format_bw(send_rate)
        );
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnStat {
    pub conn_time: u32,
    pub stream_key: String,
    pub conn_type: RtmpConnType,
    pub recv_bytes: u64,
    pub send_bytes: u64,
    pub audio_count: u64,
    pub video_count: u64,
}

impl ConnStat {
    pub fn new(stream_key: String, conn_type: RtmpConnType) -> Self {
        Self {
            stream_key,
            conn_type,
            conn_time: utils::current_time(),
            recv_bytes: 0,
            send_bytes: 0,
            audio_count: 0,
            video_count: 0,
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Serialize)]
pub struct SummariesStat {
    pub version: String,
    pub uptime_sec: u32,
    pub pid: i32,
    pub ppid: i32,
    pub threads: i64,
    pub mem_mbytes: u64,
    pub cpu_percent: f32,
    pub streams: usize,
    pub conns: usize,

    #[serde(skip_serializing)]
    last_stat: procfs::process::Stat,
}

#[cfg(target_os = "linux")]
impl SummariesStat {
    fn new(ver: String) -> Self {
        let stat = procfs::process::Process::myself().unwrap().stat().unwrap();
        Self {
            version: ver,
            uptime_sec: utils::current_time() - stat.starttime().unwrap().timestamp() as u32,
            pid: stat.pid,
            ppid: stat.ppid,
            threads: stat.num_threads,
            mem_mbytes: stat.rss * procfs::page_size() / 1024 / 1024,
            cpu_percent: 0.0,
            last_stat: stat,
            streams: 0,
            conns: 0,
        }
    }

    fn update(&mut self, intval: u64) {
        let stat = procfs::process::Process::myself().unwrap().stat().unwrap();
        self.uptime_sec = utils::current_time() - stat.starttime().unwrap().timestamp() as u32;
        self.mem_mbytes = stat.rss * procfs::page_size() / 1024 / 1024;
        self.cpu_percent =
            (100 * (stat.utime + stat.stime - self.last_stat.utime - self.last_stat.stime)) as f32
                / intval as f32
                / procfs::ticks_per_second() as f32;
        self.last_stat = stat;
    }
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Serialize)]
pub struct SummariesStat {
    pub version: String,
    pub streams: usize,
    pub conns: usize,
}

#[cfg(not(target_os = "linux"))]
impl SummariesStat {
    fn new(ver: String) -> Self {
        Self {
            version: ver,
            streams: 0,
            conns: 0,
        }
    }

    fn update(&mut self, intval: u64) {}
}

const KBIT: u64 = 1024;
const MBIT: u64 = 1024 * 1024;
const GBIT: u64 = 1024 * 1024 * 1024;
fn format_bw(bytes: u64) -> String {
    let b = bytes * 8;
    if b >= GBIT {
        return format!("{:.2} Gb", b as f32 / GBIT as f32);
    }
    if b >= MBIT {
        return format!("{:.2} Mb", b as f32 / MBIT as f32);
    }
    if b >= KBIT {
        return format!("{:.2} Kb", b as f32 / KBIT as f32);
    }
    format!("{} b", b)
}
