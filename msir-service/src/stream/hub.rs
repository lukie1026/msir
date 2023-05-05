use rtmp::message::RtmpMessage;
use tokio::sync::mpsc;
use uuid::Uuid;

pub enum HubEvent {
    ComsumerJoin(Uuid, mpsc::UnboundedSender<RtmpMessage>),
    ComsumerLeave(Uuid),
}

#[derive(Debug)]
pub struct Hub {
    pub receiver: mpsc::UnboundedReceiver<HubEvent>,
    pub comsumers: Vec<mpsc::Sender<RtmpMessage>>,
}

impl Hub {
    pub fn new(rx: mpsc::UnboundedReceiver<HubEvent>) -> Self {
        Self {
            receiver: rx,
            comsumers: vec![],
        }
    }
}
