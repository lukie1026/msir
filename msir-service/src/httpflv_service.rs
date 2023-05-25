use futures::channel::mpsc::UnboundedSender;
use httpflv::FlvTransmuxer;
use hyper::{http::HeaderValue, Body, Request as HttpRequest};
use rtmp::message::request::Request;
use std::io;
use tokio::sync::oneshot;
use tracing::{info, trace, warn};

use crate::{
    error::ServiceError,
    statistic::{ConnStat, ConnToStatChanTx, StatEvent},
    stream::{ConnToMgrChanTx, RegisterEv, RoleType, StreamEvent, Token, UnregisterEv},
    CONN_PRINT_INTVAL, PERF_MERGE_SEND_MSG,
};

type FlvRespChanTx = UnboundedSender<io::Result<Vec<u8>>>;

pub struct HttpFlvService {
    uid: String,
    response: FlvRespChanTx,
    mgr_tx: ConnToMgrChanTx,
    stat_tx: ConnToStatChanTx,

    flv_enc: FlvTransmuxer,
}

impl HttpFlvService {
    pub fn new(
        uid: String,
        response: FlvRespChanTx,
        mgr_tx: ConnToMgrChanTx,
        stat_tx: ConnToStatChanTx,
    ) -> Self {
        Self {
            uid,
            response,
            mgr_tx,
            stat_tx,
            flv_enc: FlvTransmuxer::new(),
        }
    }

    pub async fn run(&mut self, req: HttpRequest<Body>) -> Result<(), ServiceError> {
        let req = Request::parse_from(format!(
            "http://{}{}",
            req.headers()
                .get("Host")
                .unwrap_or(&HeaderValue::from_static("0.0.0.0"))
                .to_str()
                .unwrap_or("0.0.0.0"),
            req.uri()
        ))?;

        info!(
            "Identify {:?} app:{} stream:{} param:{}",
            req.conn_type,
            req.tc_url.path(),
            req.stream(),
            req.tc_url.query().unwrap_or(""),
        );

        let token = self.register(&req).await?;

        let ret = self.playing(&req, token).await;

        self.unregister(&req).await;

        ret?;
        Ok(())
    }

    async fn register(&self, req: &Request) -> Result<Token, ServiceError> {
        let stream_key = req.app_stream();
        let (reg_tx, reg_rx) = oneshot::channel();
        let msg = StreamEvent::Register(RegisterEv {
            uid: self.uid.clone(),
            stream_key: stream_key.clone(),
            role: RoleType::Subscriber,
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
        let msg = StreamEvent::Unregister(UnregisterEv {
            uid: self.uid.clone(),
            stream_key: stream_key.clone(),
            role: RoleType::Subscriber,
        });
        if let Err(e) = self.mgr_tx.send(msg) {
            warn!("send unregister event failed: {}", e);
        }
        let _ = self.stat_tx.send(StatEvent::DeleteConn(self.uid.clone(), {
            let mut conn = ConnStat::new(stream_key, req.conn_type.clone());
            conn.recv_bytes = 0;
            conn.send_bytes = self.flv_enc.get_send_bytes();
            conn.audio_count = self.flv_enc.get_audio_count();
            conn.video_count = self.flv_enc.get_video_count();
            conn
        }));
    }

    async fn playing(&mut self, req: &Request, token: Token) -> Result<(), ServiceError> {
        let mut rx = match token {
            Token::SubscriberToken(rx) => rx,
            _ => return Err(ServiceError::InvalidToken),
        };
        let mut merge_msgs = Vec::with_capacity(128);
        let mut merge_size = 0;
        let mut start_ts = 0;
        let stream_key = req.app_stream();
        let mut stat_report = tokio::time::interval(CONN_PRINT_INTVAL);
        loop {
            tokio::select! {
                msgs = rx.recv() => {
                    match msgs {
                        Some(msgs) => {
                            let mut cur_ts = 0;
                            let mut has_key_frame = false;
                            for msg in msgs {
                                if !has_key_frame {
                                    has_key_frame = msg.is_key_frame();
                                }
                                cur_ts = msg.timestamp().unwrap_or(0);
                                merge_size += msg.len().unwrap_or(0);
                                merge_msgs.push(msg);
                            }
                            // Merge-send msgs to player in PERF_MERGE_SEND_MSG for improve performance
                            if cur_ts >= (start_ts + PERF_MERGE_SEND_MSG) || cur_ts == 0 || cur_ts < start_ts || has_key_frame {
                                trace!("Merged send msgs len {} total_size {}", merge_msgs.len(), merge_size);
                                self.response.start_send(Ok(self.flv_enc.write_tags(&merge_msgs, merge_size)?))?;
                                merge_msgs.clear();
                                start_ts = cur_ts;
                                merge_size = 0;
                            }
                        }
                        None => return Err(ServiceError::PublishDone)
                    }
                }
                _ = stat_report.tick() => {
                    let _ = self.stat_tx.send(StatEvent::UpdateConn(self.uid.clone(), {
                        let mut conn = ConnStat::new(stream_key.clone(), req.conn_type.clone());
                        conn.recv_bytes = 0;
                        conn.send_bytes = self.flv_enc.get_send_bytes();
                        conn.audio_count = self.flv_enc.get_audio_count();
                        conn.video_count = self.flv_enc.get_video_count();
                        conn
                    }));
                }
            }
        }
    }
}
