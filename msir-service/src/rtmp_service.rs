use crate::{
    error::ServiceError,
    stream::{hub::HubEvent, RegisterEv, RoleType, StreamEvent, Token, UnregisterEv},
    utils::gen_uid,
};
use msir_core::transport::Transport;
use rtmp::connection::RtmpConnType;
use rtmp::connection::{server as rtmp_conn, RtmpCtrlAction};
use rtmp::message::request::Request;
use rtmp::message::RtmpMessage;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::{debug, error, info, trace, warn};

pub struct RtmpService {
    uid: String,
    rtmp: rtmp_conn::Server,
}

impl RtmpService {
    pub async fn new(io: Transport, uid: Option<String>) -> Result<Self, ServiceError> {
        let rtmp = rtmp_conn::Server::new(io).await?;
        let uid = uid.unwrap_or_else(|| gen_uid());
        Ok(Self { uid, rtmp })
    }
    pub async fn run(
        &mut self,
        mgr_tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<(), ServiceError> {
        loop {
            // connect with client and identify conn type
            let req = self.rtmp.identify_client().await?;
            // start publish/play
            match req.conn_type {
                RtmpConnType::Play => {
                    info!("Start play...");
                    self.rtmp.start_play().await?;
                }
                RtmpConnType::FmlePublish => {
                    info!("Start fmle publish...");
                    self.rtmp.start_fmle_publish().await?;
                }
                RtmpConnType::FlashPublish => {
                    info!("Start flash publish...");
                    self.rtmp.start_flash_publish().await?;
                }
                RtmpConnType::HaivisionPublish => {
                    info!("Start haivision publish...");
                    self.rtmp.start_haivision_publish().await?;
                }
                _ => {}
            }
            let token = self.register(&mgr_tx, &req).await?;
            info!("Register {:?} succeed", self.uid);
            let ret = match req.conn_type.is_publish() {
                true => self.publishing(&req, token).await,
                false => self.playing(&req, token).await,
            };
            info!("Unegister {:?} {:?} succeed", self.uid, req.conn_type);
            self.unregister(&mgr_tx, &req).await;
            match ret? {
                Some(act) => match act {
                    RtmpCtrlAction::Republish | RtmpCtrlAction::Close => continue,
                    _ => return Ok(()),
                },
                None => return Ok(()),
            }
        }
    }

    async fn register(
        &mut self,
        mgr_tx: &mpsc::UnboundedSender<StreamEvent>,
        req: &Request,
    ) -> Result<Token, ServiceError> {
        let stream_key = req.app_stream();
        let role = match req.conn_type.is_publish() {
            true => RoleType::Producer,
            false => RoleType::Consumer,
        };
        let (reg_tx, reg_rx) = oneshot::channel();
        let msg = StreamEvent::Register(RegisterEv {
            uid: self.uid.clone(),
            stream_key,
            role,
            ret: reg_tx,
        });
        if let Err(_) = mgr_tx.send(msg) {
            return Err(ServiceError::RegisterFailed(
                "send register event failed".to_string(),
            ));
        }

        match reg_rx.await {
            Ok(token) => {
                if let Token::Failure(e) = token {
                    return Err(ServiceError::RegisterFailed(e.to_string()));
                }
                // self.token = Some(token);
                Ok(token)
            }
            Err(_) => Err(ServiceError::RegisterFailed(
                "recv register ret failed: ".to_string(),
            )),
        }
    }

    async fn unregister(&mut self, mgr_tx: &mpsc::UnboundedSender<StreamEvent>, req: &Request) {
        let stream_key = req.app_stream();
        let role = match req.conn_type.is_publish() {
            true => RoleType::Producer,
            false => RoleType::Consumer,
        };
        let msg = StreamEvent::Unregister(UnregisterEv {
            uid: self.uid.clone(),
            stream_key,
            role,
        });
        if let Err(e) = mgr_tx.send(msg) {
            warn!("send unregister event failed: {}", e);
        }
    }

    async fn playing(
        &mut self,
        req: &Request,
        token: Token,
    ) -> Result<Option<RtmpCtrlAction>, ServiceError> {
        let mut rx = match token {
            Token::ComsumerToken(rx) => rx,
            _ => return Err(ServiceError::InvalidToken),
        };
        let mut msgs = Vec::with_capacity(128);
        let mut pause = false;
        let mut cache_size = 0;
        let mut start_ts = 0;
        loop {
            tokio::select! {
                msg = self.rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Command { .. } => {
                                    info!("Server receive: {}", msg);
                                    if let Some(act) = self.rtmp.process_amf_command(msg).await? {
                                        match act {
                                            RtmpCtrlAction::Pause(p) => {
                                                info!("Player change pause state {}=>{}", pause, p);
                                                pause = p;
                                            },
                                            _ => {}
                                        }
                                    }
                                }
                                RtmpMessage::Acknowledgement { .. } => {} // Do not trace
                                other => info!("Server receive and ignore: {}", other)
                            }
                        }
                        Err(err) => return Err(ServiceError::ConnectionError(err)),
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            if pause {
                                continue;
                            }
                            let cur_ts = msg.timestamp().unwrap_or(0);
                            cache_size += msg.len().unwrap_or(0);
                            msgs.push(msg);
                            // Merge msgs in 350ms for performance, but will be harmful to latency
                            // TODO: rasie the priority of I frame
                            if cur_ts >= (start_ts + 350) || cur_ts == 0 {
                                debug!("Merged send msgs len {} total-size {}", msgs.len(), cache_size);
                                self.rtmp.send_messages(&msgs, 0, 0).await?;
                                msgs.clear();
                                start_ts = cur_ts;
                                cache_size = 0;
                            }
                        }
                        None => return Err(ServiceError::PublishDone)
                    }
                }
            }
        }
    }

    async fn publishing(
        &mut self,
        req: &Request,
        token: Token,
    ) -> Result<Option<RtmpCtrlAction>, ServiceError> {
        let hub = match token {
            Token::ProducerToken(hub) => hub,
            _ => return Err(ServiceError::InvalidToken),
        };
        loop {
            match self.rtmp.recv_message().await {
                Ok(msg) => match msg {
                    RtmpMessage::Amf0Command { .. } => {
                        info!("Server receive: {}", msg);
                        if let Some(act) = self.rtmp.process_amf_command(msg).await? {
                            return Ok(Some(act));
                        }
                    }
                    RtmpMessage::Amf0Data { .. } => {
                        info!("Server receive: {}", msg);
                        if msg.is_metadata() {
                            hub.send(HubEvent::Meta(msg))?;
                        }
                    }
                    RtmpMessage::VideoData { .. } => {
                        hub.send(HubEvent::Frame(msg))?;
                    }
                    RtmpMessage::AudioData { .. } => {
                        hub.send(HubEvent::Frame(msg))?;
                    }
                    other => info!("Server receive msg and ignore: {}", other),
                },
                Err(err) => return Err(ServiceError::ConnectionError(err)),
            }
        }
    }
}
