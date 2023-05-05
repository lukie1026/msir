use rtmp::connection::server as rtmp_conn;
use rtmp::connection::RtmpConnType;
use rtmp::message::request::Request;
use rtmp::message::RtmpMessage;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::{error, info, trace, warn};
use uuid::Uuid;

use crate::{
    error::ServiceError,
    stream::{RegisterEv, RoleType, StreamEvent, Token, UnregisterEv},
};
// use crate::stream::{RegisterEv, RoleType, StreamEvent, Token, UnregisterEv};

pub struct RtmpService {
    uid: Uuid,
    rtmp: rtmp_conn::Server,
    token: Option<Token>,
}

impl RtmpService {
    pub async fn new(io: TcpStream) -> Result<Self, ServiceError> {
        let rtmp = rtmp_conn::Server::new(io).await?;
        let uid = Uuid::new_v4();
        Ok(Self {
            uid,
            rtmp,
            token: None,
        })
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
        self.register(&mgr_tx, &req).await?;
        info!("Register {:?} succeed", self.uid);
        let ret = match req.conn_type.is_publish() {
            true => self.publishing(&req).await,
            false => self.playing(&req).await,
        };
        info!("Unegister {:?} succeed", self.uid);
        self.unregister(&mgr_tx, &req).await;
        return ret;
    }

    async fn register(
        &mut self,
        mgr_tx: &mpsc::UnboundedSender<StreamEvent>,
        req: &Request,
    ) -> Result<(), ServiceError> {
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
                self.token = Some(token);
                Ok(())
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

    async fn playing(&mut self, req: &Request) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn publishing(&mut self, req: &Request) -> Result<(), ServiceError> {
        loop {
            let msg = self.rtmp.recv_message().await?;
            match msg {
                RtmpMessage::Amf0Command {
                    command_name,
                    transaction_id,
                    command_object,
                    additional_arguments,
                } => {
                    info!("Server receive Amf0Command: {:?}", command_name);
                }
                RtmpMessage::Amf0Data { values } => {
                    info!("Server receive Amf0Data: {:?}", values);
                }
                RtmpMessage::VideoData {
                    timestamp,
                    stream_id,
                    payload,
                } => {
                    trace!(
                        "Server receive VideoData csid={} ts={} len={}",
                        stream_id,
                        timestamp,
                        payload.len()
                    );
                }
                RtmpMessage::AudioData {
                    timestamp,
                    stream_id,
                    payload,
                } => {
                    trace!(
                        "Server receive AudioData csid={} ts={} len={}",
                        stream_id,
                        timestamp,
                        payload.len()
                    );
                }
                other => {
                    info!("Server ignore msg {:?}", other);
                }
            }
        }
    }
}
