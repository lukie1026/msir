use crate::{
    error::ServiceError,
    stream::{RegisterEv, RoleType, StreamEvent, Token, UnregisterEv},
};
use rtmp::connection::server as rtmp_conn;
use rtmp::connection::RtmpConnType;
use rtmp::message::request::Request;
use rtmp::message::RtmpMessage;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::{error, info, trace, warn};
use uuid::Uuid;

pub struct RtmpService {
    uid: Uuid,
    rtmp: rtmp_conn::Server,
}

impl RtmpService {
    pub async fn new(io: TcpStream, uid: Option<Uuid>) -> Result<Self, ServiceError> {
        let rtmp = rtmp_conn::Server::new(io).await?;
        let uid = uid.unwrap_or_else(|| Uuid::new_v4());
        Ok(Self { uid, rtmp })
    }
    pub async fn run(
        &mut self,
        mgr_tx: mpsc::UnboundedSender<StreamEvent>,
    ) -> Result<(), ServiceError> {
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
        return ret;
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
            uid: self.uid,
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
            uid: self.uid,
            stream_key,
            role,
        });
        if let Err(e) = mgr_tx.send(msg) {
            warn!("send unregister event failed: {}", e);
        }
    }

    async fn playing(&mut self, req: &Request, token: Token) -> Result<(), ServiceError> {
        let mut rx = match token {
            Token::ComsumerToken(rx) => rx,
            _ => return Err(ServiceError::InvalidToken),
        };
        loop {
            tokio::select! {
                msg = self.rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Command { .. } => {
                                    info!("Server receive: {}", msg);
                                }
                                RtmpMessage::Amf0Data { .. } => {
                                    info!("Server receive: {}", msg);
                                }
                                RtmpMessage::VideoData {..} => {}
                                RtmpMessage::AudioData {..} => {}
                                RtmpMessage::Acknowledgement { .. } => {} // Do not trace
                                other => info!("Server ignore msg {:?}", other)
                            }
                        }
                        Err(err) => return Err(ServiceError::ConnectionError(err)),
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => self.rtmp.send_message(msg, 0, 0).await?,
                        None => return Err(ServiceError::PublishDone)
                    }
                }
            }
        }
    }

    async fn publishing(&mut self, req: &Request, token: Token) -> Result<(), ServiceError> {
        let mut hub = match token {
            Token::ProducerToken(hub) => hub,
            _ => return Err(ServiceError::InvalidToken),
        };
        loop {
            tokio::select! {
                msg = self.rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Command { .. } => {
                                    info!("Server receive: {}", msg);
                                }
                                RtmpMessage::Amf0Data { .. } => {
                                    info!("Server receive: {}", msg);
                                    if msg.is_metadata() {
                                        hub.on_metadata(msg)?;
                                    }
                                }
                                RtmpMessage::VideoData {..} => hub.on_frame(msg)?,
                                RtmpMessage::AudioData {..} => hub.on_frame(msg)?,
                                RtmpMessage::Acknowledgement { .. } => {} // Do not trace
                                other => info!("Server ignore msg {:?}", other)
                            }
                        }
                        Err(err) => return Err(ServiceError::ConnectionError(err)),
                    }
                }
                ret = hub.process_hub_ev() => {
                    info!("Proccess hub event: {:?}", ret);
                    if let Err(e) = ret {
                        return Err(ServiceError::HubError(e));
                    }
                }
            }
        }
    }
}
