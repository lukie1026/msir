use std::time::Duration;

use crate::{
    error::ServiceError,
    statistic::{ConnStat, StatEvent, Statistic},
    stream::{RegisterEv, RoleType, StreamEvent, Token, UnregisterEv},
    utils,
};
use msir_core::{transport::Transport, utils::current_time};
use rtmp::connection::RtmpConnType;
use rtmp::connection::{server as rtmp_conn, RtmpCtrlAction};
use rtmp::message::request::Request;
use rtmp::message::RtmpMessage;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::{debug, error, info, trace, warn};

const CONN_PRINT_INTVAL: Duration = Duration::from_secs(5);

pub struct RtmpService {
    uid: String,
    rtmp: rtmp_conn::Server,
    mgr_tx: mpsc::UnboundedSender<StreamEvent>,
    stat_tx: mpsc::UnboundedSender<StatEvent>,
}

impl RtmpService {
    pub async fn new(
        io: Transport,
        uid: Option<String>,
        mgr_tx: mpsc::UnboundedSender<StreamEvent>,
        stat_tx: mpsc::UnboundedSender<StatEvent>,
    ) -> Result<Self, ServiceError> {
        let rtmp = rtmp_conn::Server::new(io).await?;
        let uid = uid.unwrap_or_else(|| utils::gen_uid());
        Ok(Self {
            uid,
            rtmp,
            mgr_tx,
            stat_tx,
        })
    }
    pub async fn run(&mut self) -> Result<(), ServiceError> {
        loop {
            // connect with client and identify conn type
            let req = self.rtmp.identify_client().await?;
            // start publish/play
            match req.conn_type {
                RtmpConnType::Play => {
                    self.rtmp.start_play().await?;
                }
                RtmpConnType::FmlePublish => {
                    self.rtmp.start_fmle_publish().await?;
                }
                RtmpConnType::FlashPublish => {
                    self.rtmp.start_flash_publish().await?;
                }
                RtmpConnType::HaivisionPublish => {
                    self.rtmp.start_haivision_publish().await?;
                }
                _ => {}
            }
            let token = self.register(&req).await?;
            debug!("Register to hub");
            let ret = match req.conn_type.is_publish() {
                true => self.publishing(&req, token).await,
                false => self.playing(&req, token).await,
            };
            self.unregister(&req).await;
            debug!("Unegister to hub");
            match ret? {
                Some(act) => match act {
                    RtmpCtrlAction::Republish | RtmpCtrlAction::Close => continue,
                    _ => return Ok(()),
                },
                None => return Ok(()),
            }
        }
    }

    async fn register(&self, req: &Request) -> Result<Token, ServiceError> {
        let stream_key = req.app_stream();
        let role = match req.conn_type.is_publish() {
            true => RoleType::Producer,
            false => RoleType::Consumer,
        };
        let (reg_tx, reg_rx) = oneshot::channel();
        let msg = StreamEvent::Register(RegisterEv {
            uid: self.uid.clone(),
            stream_key: stream_key.clone(),
            role,
            ret: reg_tx,
        });
        if let Err(_) = self.mgr_tx.send(msg) {
            return Err(ServiceError::RegisterFailed(
                "send register event failed".to_string(),
            ));
        }

        match reg_rx.await {
            Ok(token) => {
                if let Token::Failure(e) = token {
                    return Err(ServiceError::RegisterFailed(e.to_string()));
                }
                let _ = self.stat_tx.send(StatEvent::CreateConn(
                    self.uid.clone(),
                    ConnStat::new(stream_key, req.conn_type.clone()),
                ));
                Ok(token)
            }
            Err(_) => Err(ServiceError::RegisterFailed(
                "recv register ret failed: ".to_string(),
            )),
        }
    }

    async fn unregister(&mut self, req: &Request) {
        let stream_key = req.app_stream();
        let role = match req.conn_type.is_publish() {
            true => RoleType::Producer,
            false => RoleType::Consumer,
        };
        let msg = StreamEvent::Unregister(UnregisterEv {
            uid: self.uid.clone(),
            stream_key: stream_key.clone(),
            role,
        });
        if let Err(e) = self.mgr_tx.send(msg) {
            warn!("send unregister event failed: {}", e);
        }
        let _ = self.stat_tx.send(StatEvent::DeleteConn(self.uid.clone(), {
            let mut conn = ConnStat::new(stream_key, req.conn_type.clone());
            conn.recv_bytes = self.rtmp.get_recv_bytes();
            conn.send_bytes = self.rtmp.get_send_bytes();
            conn.audio_count = self.rtmp.get_audio_count();
            conn.video_count = self.rtmp.get_video_count();
            conn
        }));
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
        let stream_key = req.app_stream();
        let mut stat_report = tokio::time::interval(CONN_PRINT_INTVAL);
        loop {
            tokio::select! {
                msg = self.rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Command { .. } => {
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
                                _ => {} //debug!("Ignore {}", other)
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
                                trace!("Merged send msgs len {} total_size {}", msgs.len(), cache_size);
                                self.rtmp.send_messages(&msgs, 0, 0).await?;
                                msgs.clear();
                                start_ts = cur_ts;
                                cache_size = 0;
                            }
                        }
                        None => return Err(ServiceError::PublishDone)
                    }
                }
                _ = stat_report.tick() => {
                    let _ = self.stat_tx.send(StatEvent::UpdateConn(self.uid.clone(), {
                        let mut conn = ConnStat::new(stream_key.clone(), req.conn_type.clone());
                        conn.recv_bytes = self.rtmp.get_recv_bytes();
                        conn.send_bytes = self.rtmp.get_send_bytes();
                        conn.audio_count = self.rtmp.get_audio_count();
                        conn.video_count = self.rtmp.get_video_count();
                        conn
                    }));
                }
            }
        }
    }

    async fn publishing(
        &mut self,
        req: &Request,
        token: Token,
    ) -> Result<Option<RtmpCtrlAction>, ServiceError> {
        let mut hub = match token {
            Token::ProducerToken(hub) => hub,
            _ => return Err(ServiceError::InvalidToken),
        };
        let stream_key = req.app_stream();
        let mut stat_report = tokio::time::interval(CONN_PRINT_INTVAL);
        loop {
            tokio::select! {
                msg = self.rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Command { .. } => {
                                    if let Some(act) = self.rtmp.process_amf_command(msg).await? {
                                        return Ok(Some(act));
                                    }
                                }
                                RtmpMessage::Amf0Data { .. } => {
                                    if msg.is_metadata() {
                                        hub.on_metadata(msg)?;
                                    }
                                }
                                RtmpMessage::VideoData {..} => hub.on_frame(msg)?,
                                RtmpMessage::AudioData {..} => hub.on_frame(msg)?,
                                other => debug!("Ignore {}", other)
                            }
                        }
                        Err(err) => return Err(ServiceError::ConnectionError(err)),
                    }
                }
                ret = hub.process_hub_ev() => {
                    if let Err(e) = ret {
                        return Err(ServiceError::HubError(e));
                    }
                }
                _ = stat_report.tick() => {
                    let _ = self.stat_tx.send(StatEvent::UpdateConn(self.uid.clone(), {
                        let mut conn = ConnStat::new(stream_key.clone(), req.conn_type.clone());
                        conn.recv_bytes = self.rtmp.get_recv_bytes();
                        conn.send_bytes = self.rtmp.get_send_bytes();
                        conn.audio_count = self.rtmp.get_audio_count();
                        conn.video_count = self.rtmp.get_video_count();
                        conn
                    }));
                }
            }
        }
    }
}
