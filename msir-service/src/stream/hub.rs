use std::collections::HashMap;

use rtmp::message::RtmpMessage;
use tokio::sync::mpsc;
use tracing::{info, warn};
use uuid::Uuid;

use super::{error::StreamError, gop::GopCache};

pub enum HubEvent {
    ComsumerJoin(Uuid, mpsc::UnboundedSender<RtmpMessage>),
    ComsumerLeave(Uuid),
}

#[derive(Debug)]
pub struct Hub {
    pub metadata: Option<RtmpMessage>,
    pub gop: GopCache,
    pub receiver: mpsc::UnboundedReceiver<HubEvent>,
    pub comsumers: HashMap<Uuid, mpsc::UnboundedSender<RtmpMessage>>,
}

impl Hub {
    pub fn new(rx: mpsc::UnboundedReceiver<HubEvent>) -> Self {
        Self {
            metadata: None,
            gop: GopCache::new(),
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
                        if let Some(meta) = &self.metadata {
                            if let Err(e) = tx.send(meta.clone()) {
                                warn!("Hub send metadata to comsumer failed: {:?}", e);
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
        self.metadata = Some(msg.clone());
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
        self.gop.cache(msg);
        Ok(())
    }
}
