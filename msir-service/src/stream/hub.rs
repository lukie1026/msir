use crate::PERF_MERGE_SEND_CHAN;

use super::{error::StreamError, gop::GopCache, HubToSubsChanTx, MgrToHubChanRx};
use rtmp::{codec, message::RtmpMessage};
use std::collections::HashMap;
use tracing::{debug, info, trace, warn};

pub enum HubEvent {
    SubscriberJoin(String, HubToSubsChanTx),
    SubscriberLeave(String),
}

#[derive(Debug, Default)]
struct MetaCache {
    metadata: Option<RtmpMessage>,
    video_sh: Option<RtmpMessage>,
    audio_sh: Option<RtmpMessage>,
}

#[derive(Debug)]
pub struct Hub {
    meta: MetaCache,
    pub gop: GopCache,
    pub event_rx: MgrToHubChanRx,
    pub subscribers: HashMap<String, HubToSubsChanTx>,
    merge_msgs: Vec<RtmpMessage>,
    start_ts: u32,
}

impl Hub {
    pub fn new(rx: MgrToHubChanRx) -> Self {
        Self {
            gop: GopCache::new(),
            meta: MetaCache::default(),
            event_rx: rx,
            subscribers: HashMap::new(),
            merge_msgs: Vec::with_capacity(64),
            start_ts: 0,
        }
    }

    pub async fn process_hub_ev(&mut self) -> Result<usize, StreamError> {
        match self.event_rx.recv().await {
            Some(ev) => {
                match ev {
                    HubEvent::SubscriberJoin(uid, tx) => {
                        let mut sent_meta = 0;
                        let mut sent_sh = 0;
                        let mut sent_frame = 0;
                        let mut msgs = Vec::with_capacity(self.gop.caches.len() + 3);
                        // send metadata
                        if let Some(meta) = &self.meta.metadata {
                            sent_meta += 1;
                            msgs.push(meta.clone());
                        }
                        // send audio sequence heade
                        if let Some(meta) = &self.meta.audio_sh {
                            sent_sh += 1;
                            msgs.push(meta.clone());
                        }
                        // send video sequence heade
                        if let Some(meta) = &self.meta.video_sh {
                            sent_sh += 1;
                            msgs.push(meta.clone());
                        }
                        // send gopcache
                        for msg in self.gop.caches.iter() {
                            msgs.push(msg.clone());
                        }
                        if let Err(_) = tx.send(msgs) {
                            warn!("Hub send frame to subscriber failed");
                        }
                        sent_frame += self.gop.caches.len();
                        debug!(
                            "Send to {} metadata {} seq header {} av {}, duration {}ms",
                            uid,
                            sent_meta,
                            sent_sh,
                            sent_frame,
                            self.gop.duration()
                        );
                        self.subscribers.insert(uid, tx)
                    }
                    HubEvent::SubscriberLeave(uid) => self.subscribers.remove(&uid),
                };
                Ok(self.subscribers.len())
            }
            None => Err(StreamError::HubClosed),
        }
    }

    pub fn on_metadata(&mut self, msg: RtmpMessage) -> Result<(), StreamError> {
        debug!("Recv metadata {}", msg);
        for (_, subscriber) in self.subscribers.iter() {
            if let Err(_) = subscriber.send(vec![msg.clone()]) {
                warn!("Hub send metadata to subscriber failed");
            }
        }
        self.meta.metadata = Some(msg);
        Ok(())
    }

    pub fn on_frame(&mut self, msg: RtmpMessage) -> Result<(), StreamError> {
        let cur_ts = msg.timestamp().unwrap_or(0);
        self.merge_msgs.push(msg.clone());
        let has_key_frame = msg.is_key_frame();
        // Merge-send msgs to channel in PERF_MERGE_SEND_CHAN for improve performance in mutli-thread mode
        if cur_ts >= (self.start_ts + PERF_MERGE_SEND_CHAN)
            || cur_ts == 0
            || cur_ts < self.start_ts
            || has_key_frame
        {
            for (_, subscriber) in self.subscribers.iter() {
                if let Err(_) = subscriber.send(self.merge_msgs.clone()) {
                    warn!("Hub send frames to subscriber failed");
                }
            }
            self.merge_msgs.clear();
            self.start_ts = cur_ts;
        }

        match &msg {
            RtmpMessage::AudioData { payload, .. } => {
                if codec::is_audio_sequence_header(payload) {
                    self.meta.audio_sh = Some(msg);
                    return Ok(());
                }
            }
            RtmpMessage::VideoData { payload, .. } => {
                if codec::is_video_sequence_header(payload) {
                    self.meta.video_sh = Some(msg);
                    return Ok(());
                }
            }
            _ => {}
        }

        self.gop.cache(msg);
        Ok(())
    }
}
