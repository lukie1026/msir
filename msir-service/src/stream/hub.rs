use std::collections::HashMap;

use rtmp::{codec, message::RtmpMessage};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{error::StreamError, gop::GopCache};

pub enum HubEvent {
    ComsumerJoin(String, mpsc::UnboundedSender<RtmpMessage>),
    ComsumerLeave(String),
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
    pub receiver: mpsc::UnboundedReceiver<HubEvent>,
    pub comsumers: HashMap<String, mpsc::UnboundedSender<RtmpMessage>>,
}

impl Hub {
    pub fn new(rx: mpsc::UnboundedReceiver<HubEvent>) -> Self {
        Self {
            gop: GopCache::new(),
            meta: MetaCache::default(),
            receiver: rx,
            comsumers: HashMap::new(),
        }
    }

    pub async fn process_hub_ev(&mut self) -> Result<(), StreamError> {
        match self.receiver.recv().await {
            Some(ev) => {
                match ev {
                    HubEvent::ComsumerJoin(uid, tx) => {
                        // send metadata
                        if let Some(meta) = &self.meta.metadata {
                            if let Err(e) = tx.send(meta.clone()) {
                                warn!("Hub send metadata to comsumer failed: {:?}", e);
                            }
                        }
                        // send audio sequence heade
                        if let Some(meta) = &self.meta.audio_sh {
                            if let Err(e) = tx.send(meta.clone()) {
                                warn!("Hub send audio sh to comsumer failed: {:?}", e);
                            }
                        }
                        // send video sequence heade
                        if let Some(meta) = &self.meta.video_sh {
                            if let Err(e) = tx.send(meta.clone()) {
                                warn!("Hub send video sh to comsumer failed: {:?}", e);
                            }
                        }
                        // send gopcache
                        for msg in self.gop.caches.iter() {
                            if let Err(e) = tx.send(msg.clone()) {
                                warn!("Hub send avdata to comsumer failed: {:?}", e);
                            }
                        }
                        self.comsumers.insert(uid, tx)
                    }
                    HubEvent::ComsumerLeave(uid) => self.comsumers.remove(&uid),
                };
                Ok(())
            }
            None => Err(StreamError::HubClosed),
        }
    }

    pub fn on_metadata(&mut self, msg: RtmpMessage) -> Result<(), StreamError> {
        self.meta.metadata = Some(msg.clone());
        for (_, comsumer) in self.comsumers.iter() {
            if let Err(e) = comsumer.send(msg.clone()) {
                warn!("Hub send metadata to comsumer failed: {:?}", e);
            }
        }
        Ok(())
    }

    pub fn on_frame(&mut self, msg: RtmpMessage) -> Result<(), StreamError> {
        for (_, comsumer) in self.comsumers.iter() {
            if let Err(e) = comsumer.send(msg.clone()) {
                warn!("Hub send avdata to comsumer failed: {:?}", e);
            }
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
