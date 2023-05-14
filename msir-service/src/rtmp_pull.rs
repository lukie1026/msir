use rtmp::{
    connection::{client::Client as RtmpClient, RtmpConnType, RtmpCtrlAction},
    message::RtmpMessage,
};
use tracing::debug;

use crate::{
    error::ServiceError,
    statistic::{ConnStat, ConnToStatChanTx, StatEvent},
    stream::{hub::Hub, ConnToMgrChanTx},
    CONN_PRINT_INTVAL,
};

pub struct RtmpPull {
    uid: String,
    hub: Hub,
    stat_tx: ConnToStatChanTx,
}

impl RtmpPull {
    pub fn new(uid: String, hub: Hub, stat_tx: ConnToStatChanTx) -> Self {
        Self { uid, hub, stat_tx }
    }

    pub fn on_create_conn(&self, stream_key: String) {
        let _ = self.stat_tx.send(StatEvent::CreateConn(
            self.uid.clone(),
            ConnStat::new(stream_key, RtmpConnType::Pull),
        ));
    }

    pub fn on_delete_conn(&self, stream_key: String) {
        let _ = self.stat_tx.send(StatEvent::DeleteConn(
            self.uid.clone(),
            ConnStat::new(stream_key, RtmpConnType::Pull),
        ));
    }

    pub async fn run(&mut self, tc_url: String, stream: String) -> Result<(), ServiceError> {
        let mut rtmp = RtmpClient::new(tc_url, stream).await?;
        let sid = rtmp.connect(self.uid.clone()).await? as u32;
        rtmp.play(sid).await?;
        let stream_key = rtmp.req.app_stream();
        let mut stat_report = tokio::time::interval(CONN_PRINT_INTVAL);
        loop {
            tokio::select! {
                msg = rtmp.recv_message() => {
                    match msg {
                        Ok(msg) => {
                            match msg {
                                RtmpMessage::Amf0Data { .. } => {
                                    if msg.is_metadata() {
                                        self.hub.on_metadata(msg)?;
                                    }
                                }
                                RtmpMessage::VideoData {..} => self.hub.on_frame(msg)?,
                                RtmpMessage::AudioData {..} => self.hub.on_frame(msg)?,
                                _ => {} // debug!("Ignore {}", other)
                            }
                        }
                        Err(err) => return Err(ServiceError::ConnectionError(err)),
                    }
                }
                ret = self.hub.process_hub_ev() => {
                    if let Err(e) = ret {
                        return Err(ServiceError::HubError(e));
                    }
                }
                _ = stat_report.tick() => {
                    let _ = self.stat_tx.send(StatEvent::UpdateConn(self.uid.clone(), {
                        let mut conn = ConnStat::new(stream_key.clone(), RtmpConnType::Pull);
                        conn.recv_bytes = rtmp.get_recv_bytes();
                        conn.send_bytes = rtmp.get_send_bytes();
                        conn.audio_count = rtmp.get_audio_count();
                        conn.video_count = rtmp.get_video_count();
                        conn
                    }));
                }
            }
        }
    }
}

pub async fn start_pull_task(
    rtmp: &mut RtmpPull,
    tc_url: String,
    stream: String,
) -> Result<(), ServiceError> {
    rtmp.run(tc_url, stream).await
}
