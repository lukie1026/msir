use crate::{
    rtmp_pull::{start_pull_task, RtmpPull},
    statistic::ConnToStatChanTx,
    utils, STATIC_PULL_ADDRESS,
};

use self::{
    error::StreamError,
    hub::{Hub, HubEvent},
};
use rtmp::message::RtmpMessage;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, trace, warn, Instrument};

pub mod error;
pub mod gop;
pub mod hub;

type HubToSubsChanTx = mpsc::UnboundedSender<Vec<RtmpMessage>>;
type HubToSubsChanRx = mpsc::UnboundedReceiver<Vec<RtmpMessage>>;

type MgrToHubChanTx = mpsc::UnboundedSender<HubEvent>;
type MgrToHubChanRx = mpsc::UnboundedReceiver<HubEvent>;

pub type ConnToMgrChanTx = mpsc::UnboundedSender<StreamEvent>;
pub type ConnToMgrChanRx = mpsc::UnboundedReceiver<StreamEvent>;

#[derive(Debug)]
pub enum RoleType {
    Subscriber,
    Publisher,
}

#[derive(Debug)]
pub enum Token {
    Failure(StreamError),
    PublisherToken(Hub),
    SubscriberToken(HubToSubsChanRx),
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
    pool: HashMap<String, MgrToHubChanTx>,
    conn_rx: ConnToMgrChanRx,
    conn_tx: ConnToMgrChanTx,
    stat_tx: ConnToStatChanTx,
}

impl Manager {
    pub fn new(
        conn_rx: ConnToMgrChanRx,
        conn_tx: ConnToMgrChanTx,
        stat_tx: ConnToStatChanTx,
    ) -> Self {
        Self {
            conn_rx,
            conn_tx,
            stat_tx,
            pool: HashMap::new(),
        }
    }

    pub async fn run(mut self) -> Result<(), StreamError> {
        while let Some(ev) = self.conn_rx.recv().await {
            match ev {
                StreamEvent::Register(ev) => self.register(ev).await,
                StreamEvent::Unregister(ev) => self.unregister(ev).await,
            }
        }
        Ok(())
    }

    async fn register(&mut self, ev: RegisterEv) {
        let hub_ev_tx = self.pool.get(&ev.stream_key);
        debug!(
            "Recv register {} {:?} {} exist {}",
            ev.uid,
            ev.role,
            ev.stream_key,
            hub_ev_tx.is_some(),
        );
        let token = match ev.role {
            RoleType::Publisher => {
                if hub_ev_tx.is_some() {
                    Token::Failure(StreamError::DuplicatePublish)
                } else {
                    let (tx, rx) = mpsc::unbounded_channel();
                    self.pool.insert(ev.stream_key, tx);
                    Token::PublisherToken(Hub::new(rx))
                }
            }
            RoleType::Subscriber => {
                if let Some(hub_ev_tx) = hub_ev_tx {
                    let (tx, rx) = mpsc::unbounded_channel();
                    if let Err(_) = hub_ev_tx.send(HubEvent::SubscriberJoin(ev.uid, tx)) {
                        Token::Failure(StreamError::DisconnectHub)
                    } else {
                        Token::SubscriberToken(rx)
                    }
                } else {
                    // Token::Failure(StreamError::NoPublish)
                    let (hub_tx, hub_rx) = mpsc::unbounded_channel();
                    self.pool.insert(ev.stream_key.clone(), hub_tx.clone());
                    // New rtmp client
                    let uid = utils::gen_uid();
                    let mut rtmp = RtmpPull::new(
                        uid.clone(),
                        Hub::new(hub_rx),
                        self.conn_tx.clone(),
                        self.stat_tx.clone(),
                    );
                    rtmp.on_create_conn(ev.stream_key.clone());
                    let vecs: Vec<&str> = ev.stream_key.split('/').collect();
                    let tc_url = format!("{}/{}", STATIC_PULL_ADDRESS, vecs[1]);
                    let stream = vecs[2].to_string();
                    let stream_key = ev.stream_key;
                    tokio::spawn(
                        async move {
                            if let Err(e) = start_pull_task(&mut rtmp, tc_url, stream).await {
                                error!("Failed to transfer; error={}", e);
                            }
                            rtmp.on_delete_conn(stream_key.clone());
                            rtmp.unregister(stream_key);
                        }
                        .instrument(tracing::info_span!("RTMP-PULL", uid)),
                    );

                    let (sub_tx, sub_rx) = mpsc::unbounded_channel();
                    if let Err(_) = hub_tx.send(HubEvent::SubscriberJoin(ev.uid, sub_tx)) {
                        Token::Failure(StreamError::DisconnectHub)
                    } else {
                        Token::SubscriberToken(sub_rx)
                    }
                }
            }
        };
        if let Err(_) = ev.ret.send(token) {
            error!("Response token falied");
        }
    }

    async fn unregister(&mut self, ev: UnregisterEv) {
        let hub_ev_tx = self.pool.get(&ev.stream_key);
        debug!("Recv unregister {} {:?} {}", ev.uid, ev.role, ev.stream_key);

        if let Some(hub_ev_tx) = hub_ev_tx {
            match ev.role {
                RoleType::Subscriber => {
                    if let Err(_) = hub_ev_tx.send(HubEvent::SubscriberLeave(ev.uid)) {
                        warn!("Send unregister to hub failed");
                    }
                }
                RoleType::Publisher => {
                    self.pool.remove(&ev.stream_key);
                }
            }
        }
    }
}
