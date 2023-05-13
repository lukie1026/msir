use msir_core::utils;
use rtmp::connection::RtmpConnType;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::info;

const STREAM_PRINT_INTVAL: Duration = Duration::from_secs(10);

pub enum StatEvent {
    CreateConn(String, ConnStat),
    DeleteConn(String, ConnStat),
    UpdateConn(String, ConnStat),
}

pub struct Statistic {
    rx: mpsc::UnboundedReceiver<StatEvent>,
    conns: HashMap<String, ConnStat>,
    streams: HashMap<String, StreamStat>,
}

impl Statistic {
    pub fn new(rx: mpsc::UnboundedReceiver<StatEvent>) -> Self {
        Self {
            rx,
            conns: HashMap::new(),
            streams: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        while let Some(ev) = self.rx.recv().await {
            match ev {
                StatEvent::CreateConn(uid, cs) => self.on_create_conn(uid, cs),
                StatEvent::DeleteConn(uid, cs) => self.on_delete_conn(uid, cs),
                StatEvent::UpdateConn(uid, cs) => self.on_update_conn(uid, cs),
            }
        }
    }

    fn on_create_conn(&mut self, uid: String, stat: ConnStat) {
        let stream_key = stat.stream_key.clone();
        let conn_type = stat.conn_type.clone();
        self.conns.insert(uid, stat);
        // TODO: RemoteIngester need to first call than player
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
}

struct StreamStat {
    audio: u64,
    video: u64,
    recv_bytes: u64,
    send_bytes: u64,
    publish_type: RtmpConnType,
    start_time: u32,
    clients: u32,
    // Last record for log print
    last_logged: Option<Instant>,
    last_video: u64,
    last_recv_bytes: u64,
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

#[derive(Debug)]
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
    format!("{}b", b)
}
