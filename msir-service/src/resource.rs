use std::{collections::HashMap, hash::Hash};

use rtmp::message::RtmpMessage;
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot,
};
use tracing::{error, info};
use uuid::Uuid;

use crate::error::{self, ServiceError};

#[derive(Debug)]
pub enum RoleType {
    Consumer,
    Producer,
}

// pub enum Message {
//     AvData(RtmpMessage),
//     AvDataVec(Vec<RtmpMessage>),
// }

#[derive(Debug)]
pub enum Token {
    Failure(String),
    ProducerToken(Hub),
    ComsumerToken(mpsc::UnboundedReceiver<RtmpMessage>),
}

#[derive(Debug)]
pub struct RegisterEvent {
    pub uid: Uuid,
    pub role: RoleType,
    pub resource_id: String,
    pub ret: oneshot::Sender<Token>,
}

impl RegisterEvent {
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

pub struct Manager {
    pool: HashMap<String, mpsc::UnboundedSender<HubEvent>>,
    receiver: mpsc::UnboundedReceiver<RegisterEvent>,
}

impl Manager {
    pub fn new(receiver: mpsc::UnboundedReceiver<RegisterEvent>) -> Self {
        Self {
            receiver,
            pool: HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), ServiceError> {
        info!("Resource Manager daemon start...");
        while let Some(reg) = self.receiver.recv().await {
            let hub_ev_tx = self.pool.get(&reg.resource_id);
            info!(
                "received[{}]: {:?} {:?} {:?}",
                hub_ev_tx.is_none(),
                reg.uid,
                reg.role,
                reg.resource_id
            );
            let token = match reg.role {
                RoleType::Producer => {
                    if hub_ev_tx.is_some() {
                        Token::Failure("duplicate publish".to_string())
                    } else {
                        let (tx, rx) = mpsc::unbounded_channel();
                        self.pool.insert(reg.resource_id, tx);
                        Token::ProducerToken(Hub::new(rx))
                    }
                }
                RoleType::Consumer => {
                    if let Some(hub_ev_tx) = hub_ev_tx {
                        let (tx, rx) = mpsc::unbounded_channel();
                        if let Err(_) = hub_ev_tx.send(HubEvent::ComsumerJoin(reg.uid, tx)) {
                            Token::Failure("send to hub failed".to_string())
                        } else {
                            Token::ComsumerToken(rx)
                        }
                    } else {
                        Token::Failure("no publish".to_string())
                    }
                }
            };
            if let Err(_) = reg.ret.send(token) {
                error!("Response token falied");
            }
        }
        Ok(())
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
