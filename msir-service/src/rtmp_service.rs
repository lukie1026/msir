use rtmp::connection::server as rtmp_conn;
use rtmp::connection::RtmpConnType;
use rtmp::message::RtmpMessage;
use tokio::net::TcpStream;
use tracing::{error, info, trace, warn};

use crate::error::ServiceError;

pub struct RtmpService {
    rtmp: rtmp_conn::Server,
}

impl RtmpService {
    pub async fn new(io: TcpStream) -> Result<Self, ServiceError> {
        let rtmp = rtmp_conn::Server::new(io).await?;
        Ok(Self { rtmp })
    }
    pub async fn run(&mut self) -> Result<(), ServiceError> {
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

        loop {
            let msg = self.rtmp.recv_message().await?;
            match msg {
                RtmpMessage::Amf0Data { values } => {
                    info!("Server receive Amf0Data: {:?}", values);
                }
                RtmpMessage::VideoData {
                    timestamp,
                    stream_id,
                    payload,
                } => {
                    info!(
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
                    info!(
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
