use msir_core::utils;
use rtmp::connection::RtmpConnType;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, UnboundedSender};

pub enum StatEvent {
    CreateConn(String, ConnStat),
    DeleteConn(String, ConnStat),
    UpdateConn(String, ConnStat),
}

pub struct Statistic {
    tx: mpsc::UnboundedSender<StatEvent>,
    rx: mpsc::UnboundedReceiver<StatEvent>,
    conns: HashMap<String, ConnStat>,
    streams: HashMap<String, StreamStat>,
}

impl Statistic {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<StatEvent>();
        Self {
            rx,
            tx,
            conns: HashMap::new(),
            streams: HashMap::new(),
        }
    }

    pub fn tx(&mut self) -> UnboundedSender<StatEvent> {
        self.tx.clone()
    }

    pub async fn run(&mut self) {
        while let Some(ev) = self.rx.recv().await {
            match ev {
                StatEvent::CreateConn(uid, cs) => self.on_create_conn(uid, cs),
                StatEvent::DeleteConn(uid, cs) => self.on_delete_conn(uid, cs),
                StatEvent::UpdateConn(uid, cs) => self.on_update_conn(uid, cs),
            }
        }
    }

    fn on_create_conn(&mut self, uid: String, stat: ConnStat) {
        let conn = self.conns.insert(uid, stat).unwrap();
        // TODO: RemoteIngester need to first call than player
        let stream = self
            .streams
            .entry(conn.stream_key.clone())
            .or_insert(StreamStat::new(utils::current_time(), conn.conn_type));
        stream.clients += 1;
    }

    fn on_update_conn(&mut self, uid: String, stat: ConnStat) {
        if let Some(conn) = self.conns.get_mut(&uid) {
            let delta_audio = stat.audio - conn.audio;
            let delta_video = stat.video - conn.video;
            let delta_recv_bytes = stat.recv_bytes - conn.recv_bytes;
            let delta_send_bytes = stat.send_bytes - conn.send_bytes;
            conn.audio = stat.audio;
            conn.video = stat.video;
            conn.recv_bytes = stat.recv_bytes;
            conn.send_bytes = stat.send_bytes;
            conn.conn_type = stat.conn_type;
            conn.stream_key = stat.stream_key;

            if let Some(stream) = self.streams.get_mut(&conn.stream_key) {
                if conn.conn_type.is_publish() {
                    stream.audio += delta_audio;
                    stream.video += delta_video;
                    stream.recv_bytes += delta_recv_bytes;
                    stream.send_bytes += delta_send_bytes;
                }
                if conn.conn_type.is_play() {
                    stream.send_bytes += delta_send_bytes;
                }
            }
        }
    }

    fn on_delete_conn(&mut self, uid: String, stat: ConnStat) {
        match stat.conn_type.is_publish() {
            true => {
                self.streams.remove(&stat.stream_key);
            }
            false => {
                self.streams
                    .get_mut(&stat.stream_key)
                    .map(|s| s.clients += 1);
            }
        }
        self.conns.remove(&uid);
    }
}

pub struct StreamStat {
    audio: u64,
    video: u64,
    recv_bytes: u64,
    send_bytes: u64,
    publish_type: RtmpConnType,
    start_time: u32,
    clients: u32,
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
        }
    }
}

pub struct ConnStat {
    conn_time: u32,
    stream_key: String,
    conn_type: RtmpConnType,
    pub recv_bytes: u64,
    pub send_bytes: u64,
    pub audio: u64,
    pub video: u64,
}

impl ConnStat {
    pub fn new(stream_key: String, conn_type: RtmpConnType) -> Self {
        Self {
            stream_key,
            conn_type,
            conn_time: 0, // TODO: get current time
            recv_bytes: 0,
            send_bytes: 0,
            audio: 0,
            video: 0,
        }
    }
}
