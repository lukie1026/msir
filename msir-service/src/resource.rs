use std::collections::HashMap;

use rtmp::message::RtmpMessage;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::error::ServiceError;

#[derive(Debug)]
pub enum RoleType {
    Consumer,
    Producer,
}

#[derive(Debug)]
pub enum Token {
    Failure(String),
    ProducerToken(Hub),
    ComsumerToken(mpsc::UnboundedReceiver<RtmpMessage>),
}

#[derive(Debug)]
pub struct RegisterEv {
    pub uid: Uuid,
    pub role: RoleType,
    pub resource_id: String,
    pub ret: oneshot::Sender<Token>,
}

impl RegisterEv {
    pub fn new(
        uid: Uuid,
        resource_id: String,
        role: RoleType,
        ret: oneshot::Sender<Token>,
    ) -> Self {
        Self {
            uid,
            resource_id,
            role,
            ret,
        }
    }
}

pub struct UnregisterEv {
    pub uid: Uuid,
    pub role: RoleType,
    pub resource_id: String,
}

impl UnregisterEv {
    pub fn new(uid: Uuid, resource_id: String, role: RoleType) -> Self {
        Self {
            uid,
            role,
            resource_id,
        }
    }
}

pub enum ResourceEvent {
    Register(RegisterEv),
    Unregister(UnregisterEv),
}

pub struct Manager {
    pool: HashMap<String, mpsc::UnboundedSender<HubEvent>>,
    receiver: mpsc::UnboundedReceiver<ResourceEvent>,
}

impl Manager {
    pub fn new(receiver: mpsc::UnboundedReceiver<ResourceEvent>) -> Self {
        Self {
            receiver,
            pool: HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), ServiceError> {
        info!("Resource Manager daemon start...");
        while let Some(ev) = self.receiver.recv().await {
            match ev {
                ResourceEvent::Register(ev) => self.register(ev).await,
                ResourceEvent::Unregister(ev) => self.unregister(ev).await,
            }
        }
        Ok(())
    }

    async fn register(&mut self, ev: RegisterEv) {
        let hub_ev_tx = self.pool.get(&ev.resource_id);
        info!(
            "received register [{}]: {:?} {:?} {:?}",
            hub_ev_tx.is_none(),
            ev.uid,
            ev.role,
            ev.resource_id
        );
        let token = match ev.role {
            RoleType::Producer => {
                if hub_ev_tx.is_some() {
                    Token::Failure("duplicate publish".to_string())
                } else {
                    let (tx, rx) = mpsc::unbounded_channel();
                    self.pool.insert(ev.resource_id, tx);
                    Token::ProducerToken(Hub::new(rx))
                }
            }
            RoleType::Consumer => {
                if let Some(hub_ev_tx) = hub_ev_tx {
                    let (tx, rx) = mpsc::unbounded_channel();
                    if let Err(_) = hub_ev_tx.send(HubEvent::ComsumerJoin(ev.uid, tx)) {
                        Token::Failure("send to hub failed".to_string())
                    } else {
                        Token::ComsumerToken(rx)
                    }
                } else {
                    Token::Failure("no publish".to_string())
                }
            }
        };
        if let Err(_) = ev.ret.send(token) {
            error!("Response token falied");
        }
    }

    async fn unregister(&mut self, ev: UnregisterEv) {
        let hub_ev_tx = self.pool.get(&ev.resource_id);
        info!(
            "received unregister [{}]: {:?} {:?} {:?}",
            hub_ev_tx.is_none(),
            ev.uid,
            ev.role,
            ev.resource_id
        );

        if let Some(hub_ev_tx) = hub_ev_tx {
            match ev.role {
                RoleType::Consumer => {
                    if let Err(_) = hub_ev_tx.send(HubEvent::ComsumerLeave(ev.uid)) {
                        warn!("send to hub failed");
                    }
                }
                RoleType::Producer => {
                    self.pool.remove(&ev.resource_id);
                }
            }
        } else {
            warn!("unregister failed for no publish");
        }
    }
}

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
