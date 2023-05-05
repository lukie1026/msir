use std::collections::HashMap;

use rtmp::message::RtmpMessage;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::error::StreamError;

pub enum HubEvent {
    ComsumerJoin(Uuid, mpsc::UnboundedSender<RtmpMessage>),
    ComsumerLeave(Uuid),
}

#[derive(Debug)]
pub struct Hub {
    pub receiver: mpsc::UnboundedReceiver<HubEvent>,
    pub comsumers: HashMap<Uuid, mpsc::UnboundedSender<RtmpMessage>>,
}

impl Hub {
    pub fn new(rx: mpsc::UnboundedReceiver<HubEvent>) -> Self {
        Self {
            receiver: rx,
            comsumers: HashMap::new(),
        }
    }

    pub async fn process_hub_ev(&mut self) -> Result<(), StreamError> {
        match self.receiver.recv().await {
            Some(ev) => {
                match ev {
                    HubEvent::ComsumerJoin(uid, tx) => self.comsumers.insert(uid, tx),
                    HubEvent::ComsumerLeave(uid) => self.comsumers.remove(&uid),
                };
                Ok(())
            }
            None => Err(StreamError::HubClosed),
        }
    }
}