use rtmp::connection::server as rtmp_conn;
use rtmp::connection::RtmpConnType;
use rtmp::message::request::Request;
use rtmp::message::RtmpMessage;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::{error, info, trace, warn};
use uuid::Uuid;

use crate::error::ServiceError;
use crate::resource::RegisterEvent;
use crate::resource::RoleType;
use crate::resource::Token;

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
        mgr_tx: mpsc::UnboundedSender<RegisterEvent>,
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
        self.register(mgr_tx, &req).await?;
        info!("Register succeed");
        return match req.conn_type.is_publish() {
            true => self.publishing(&req).await,
            false => self.playing(&req).await,
        };
    }

    async fn register(
        &mut self,
        mgr_tx: mpsc::UnboundedSender<RegisterEvent>,
        req: &Request,
    ) -> Result<(), ServiceError> {
        let resource_id = req.app_stream();
        let role = match req.conn_type.is_publish() {
            true => RoleType::Producer,
            false => RoleType::Consumer,
        };
        let (reg_tx, reg_rx) = oneshot::channel();
        if let Err(_) = mgr_tx.send(RegisterEvent::new(self.uid, resource_id, role, reg_tx)) {
            return Err(ServiceError::RegisterFailed(
                "send register event failed".to_string(),
            ));
        }

        match reg_rx.await {
            Ok(token) => {
                if let Token::Failure(e) = token {
                    return Err(ServiceError::RegisterFailed(e));
                }
                self.token = Some(token)
            }
            Err(_) => {
                return Err(ServiceError::RegisterFailed(
                    "recv register ret failed".to_string(),
                ))
            }
        }

        Ok(())
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