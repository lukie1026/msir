use self::{error::StreamError, hub::HubEvent};
use rtmp::message::RtmpMessage;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn, Instrument};

pub mod error;
pub mod gop;
pub mod hub;

#[derive(Debug)]
pub enum RoleType {
    Consumer,
    Producer,
}

#[derive(Debug)]
pub enum Token {
    Failure(StreamError),
    ProducerToken(mpsc::UnboundedSender<HubEvent>),
    ComsumerToken(mpsc::UnboundedReceiver<RtmpMessage>),
}

#[derive(Debug)]
pub struct RegisterEv {
    pub uid: String,
    pub role: RoleType,
    pub stream_key: String,
    pub ret: oneshot::Sender<Token>,
}

pub struct UnregisterEv {
    pub uid: String,
    pub role: RoleType,
    pub stream_key: String,
}

pub enum StreamEvent {
    Register(RegisterEv),
    Unregister(UnregisterEv),
}

pub struct Manager {
    pool: HashMap<String, mpsc::UnboundedSender<HubEvent>>,
    receiver: mpsc::UnboundedReceiver<StreamEvent>,
}

impl Manager {
    pub fn new(receiver: mpsc::UnboundedReceiver<StreamEvent>) -> Self {
        Self {
            receiver,
            pool: HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), StreamError> {
        info!("Resource Manager daemon start...");
        while let Some(ev) = self.receiver.recv().await {
            match ev {
                StreamEvent::Register(ev) => self.register(ev).await,
                StreamEvent::Unregister(ev) => self.unregister(ev).await,
            }
        }
        Ok(())
    }

    async fn register(&mut self, ev: RegisterEv) {
        let hub_ev_tx = self.pool.get(&ev.stream_key);
        info!(
            "received register [{}]: {:?} {:?} {:?}",
            hub_ev_tx.is_none(),
            ev.uid,
            ev.role,
            ev.stream_key
        );
        let token = match ev.role {
            RoleType::Producer => {
                if hub_ev_tx.is_some() {
                    Token::Failure(StreamError::DuplicatePublish)
                } else {
                    let (tx, rx) = mpsc::unbounded_channel();
                    self.pool.insert(ev.stream_key.clone(), tx.clone());
                    let hub_service = hub::hub_service_start;
                    let _ = tx.send(HubEvent::Publish());
                    tokio::spawn(
                        hub_service(ev.stream_key, rx)
                            .instrument(tracing::info_span!("HUB", uid = ev.uid.to_string())),
                    );
                    Token::ProducerToken(tx)
                }
            }
            RoleType::Consumer => {
                if let Some(hub_ev_tx) = hub_ev_tx {
                    let (tx, rx) = mpsc::unbounded_channel();
                    if let Err(_) = hub_ev_tx.send(HubEvent::ComsumerJoin(ev.uid, tx)) {
                        Token::Failure(StreamError::DisconnectHub)
                    } else {
                        Token::ComsumerToken(rx)
                    }
                } else {
                    Token::Failure(StreamError::NoPublish)
                }
            }
        };
        if let Err(_) = ev.ret.send(token) {
            error!("Response token falied");
        }
    }

    async fn unregister(&mut self, ev: UnregisterEv) {
        let hub_ev_tx = self.pool.get(&ev.stream_key);
        info!(
            "received unregister [{}]: {:?} {:?} {:?}",
            hub_ev_tx.is_none(),
            ev.uid,
            ev.role,
            ev.stream_key
        );

        if let Some(hub_ev_tx) = hub_ev_tx {
            match ev.role {
                RoleType::Consumer => {
                    if let Err(_) = hub_ev_tx.send(HubEvent::ComsumerLeave(ev.uid)) {
                        warn!("send to hub failed");
                    }
                }
                RoleType::Producer => {
                    self.pool.remove(&ev.stream_key).and_then(|tx| {
                        let _ = tx.send(HubEvent::PublishDone());
                        Some(tx)
                    });
                }
            }
        } else {
            warn!("unregister failed for no publish");
        }
    }
}
