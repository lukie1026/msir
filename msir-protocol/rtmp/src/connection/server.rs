use msir_core::transport::Transport;
use rml_amf0::Amf0Value;
use tokio::net::TcpStream;
use tracing::{info, trace, warn};

use crate::{
    handshake,
    message::{
        request::Request,
        types::{
            amf0_command_type::*, peer_bw_limit_type, rtmp_sig::*, user_ctrl_ev_type::*,
            DEFAULT_SID,
        },
        RtmpMessage,
    },
};

use super::{context::Context, error::ConnectionError, RtmpConnType, RtmpCtrlAction};

pub struct Server {
    ctx: Context,
    conn_type: RtmpConnType,
}

impl Server {
    pub async fn new(mut io: Transport) -> Result<Self, ConnectionError> {
        let mut hs = handshake::Server::new();
        hs.handshake(&mut io).await?;
        Ok(Self {
            ctx: Context::new(io),
            conn_type: RtmpConnType::Unknow,
        })
    }
    pub async fn recv_message(&mut self) -> Result<RtmpMessage, ConnectionError> {
        self.ctx.recv_message().await
    }
    pub async fn send_message(
        &mut self,
        msg: RtmpMessage,
        timestamp: u32,
        csid: u32,
    ) -> Result<(), ConnectionError> {
        self.ctx.send_message(msg, timestamp, csid).await
    }
    pub async fn send_messages(
        &mut self,
        msgs: &[RtmpMessage],
        timestamp: u32,
        csid: u32,
    ) -> Result<(), ConnectionError> {
        self.ctx.send_messages(&msgs, timestamp, csid).await
    }
    pub async fn identify_client(&mut self) -> Result<Request, ConnectionError> {
        let mut req = self.connect_app().await?;
        loop {
            if let RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                additional_arguments,
                ..
            } = self.ctx.expect_amf_command(&[]).await?
            {
                match command_name.as_str() {
                    COMMAND_PLAY => {
                        self.process_play(&mut req, additional_arguments)?;
                    }
                    COMMAND_CREATE_STREAM => {
                        self.process_create_stream(&mut req, transaction_id).await?;
                    }
                    COMMAND_RELEASE_STREAM => {
                        self.process_fmle_publish(&mut req, transaction_id, additional_arguments)
                            .await?
                    }
                    _ => {
                        // Response null first for the other call msg
                        self.send_message(RtmpMessage::new_null(transaction_id), 0, 0)
                            .await?;
                        continue;
                    }
                };
                self.conn_type = req.conn_type.clone();
                info!(
                    "Identify {:?} app={} stream={:?} param={:?}",
                    req.conn_type,
                    req.tc_url.path(),
                    req.stream,
                    req.tc_url.query()
                );
                return Ok(req);
            }
            return Err(ConnectionError::UnexpectedMessage);
        }
    }
    async fn connect_app(&mut self) -> Result<Request, ConnectionError> {
        if let RtmpMessage::Amf0Command {
            transaction_id,
            command_object,
            ..
        } = self.ctx.expect_amf_command(&[COMMAND_CONNECT]).await?
        {
            if transaction_id != 1.0 {
                warn!("Invalid transaction_id={} of connect_app", transaction_id);
            }

            let mut properties = match command_object {
                Amf0Value::Object(properties) => properties,
                _ => return Err(ConnectionError::InvalidConnectApp),
            };
            let tc_url = match properties.remove("tcUrl") {
                Some(value) => match value {
                    Amf0Value::Utf8String(tc_url) => tc_url,
                    _ => return Err(ConnectionError::InvalidConnectApp),
                },
                None => return Err(ConnectionError::InvalidConnectApp),
            };

            let object_encoding = match properties.remove("objectEncoding") {
                Some(value) => match value {
                    Amf0Value::Number(number) => number,
                    _ => RTMP_SIG_AMF0_VER,
                },
                None => RTMP_SIG_AMF0_VER,
            };

            let request = Request::parse_from(tc_url)?;

            // Set in_win_ack, default = 0
            self.ctx.set_in_window_ack_size(0);

            // Set out_win_ack, default = 2500000
            self.send_message(
                RtmpMessage::SetWindowAckSize {
                    ack_window_size: 2500000,
                },
                0,
                0,
            )
            .await?;

            // Set peer_bandwidth, default = 2500000
            self.send_message(
                RtmpMessage::SetPeerBandwidth {
                    size: 2500000,
                    limit_type: peer_bw_limit_type::DYNAMIC,
                },
                0,
                0,
            )
            .await?;

            // Set chunk_size, default = 60000
            self.send_message(RtmpMessage::SetChunkSize { chunk_size: 60000 }, 0, 0)
                .await?;

            // Response connect
            self.send_message(RtmpMessage::new_connect_app_res(object_encoding), 0, 0)
                .await?;

            // on bw_done
            self.send_message(RtmpMessage::new_on_bw_done(), 0, 0)
                .await?;

            return Ok(request);
        }
        return Err(ConnectionError::UnexpectedMessage);
    }
    async fn process_create_stream(
        &mut self,
        req: &mut Request,
        transaction_id: f64,
    ) -> Result<(), ConnectionError> {
        let mut res_transaction_id = transaction_id;
        for _ in 0..3 {
            // Response CreateStream
            self.send_message(RtmpMessage::new_create_stream_res(res_transaction_id), 0, 0)
                .await?;

            if let RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                additional_arguments,
                ..
            } = self
                .ctx
                .expect_amf_command(&[
                    COMMAND_PLAY,
                    COMMAND_PUBLISH,
                    COMMAND_CREATE_STREAM,
                    COMMAND_FC_PUBLISH,
                ])
                .await?
            {
                return match command_name.as_str() {
                    COMMAND_PLAY => self.process_play(req, additional_arguments),
                    COMMAND_PUBLISH => self.process_flash_publish(req, additional_arguments),
                    COMMAND_FC_PUBLISH => {
                        self.process_haivision_publish(req, transaction_id, additional_arguments)
                            .await
                    }
                    COMMAND_CREATE_STREAM => {
                        res_transaction_id = transaction_id;
                        continue;
                    }
                    _ => Err(ConnectionError::UnexpectedMessage),
                };
            }
            return Err(ConnectionError::UnexpectedMessage);
        }
        Err(ConnectionError::CreateStreamDepth)
    }
    async fn process_fmle_publish(
        &mut self,
        req: &mut Request,
        transaction_id: f64,
        additional_arguments: Vec<Amf0Value>,
    ) -> Result<(), ConnectionError> {
        req.conn_type = RtmpConnType::FmlePublish;
        if additional_arguments.len() < 1 {
            return Err(ConnectionError::ReleaseStreamWithoutStream);
        }
        match &additional_arguments[0] {
            Amf0Value::Utf8String(stream) => req.stream = Some(stream.clone()),
            _ => return Err(ConnectionError::ReleaseStreamWithoutStream),
        }
        // Response releaseStream
        self.send_message(RtmpMessage::new_release_stream_res(transaction_id), 0, 0)
            .await?;
        Ok(())
    }
    fn process_flash_publish(
        &mut self,
        req: &mut Request,
        additional_arguments: Vec<Amf0Value>,
    ) -> Result<(), ConnectionError> {
        req.conn_type = RtmpConnType::FlashPublish;
        if additional_arguments.len() < 1 {
            return Err(ConnectionError::InvalidPublish);
        }
        match &additional_arguments[0] {
            Amf0Value::Utf8String(stream) => req.stream = Some(stream.clone()),
            _ => return Err(ConnectionError::InvalidPublish),
        }
        Ok(())
    }
    async fn process_haivision_publish(
        &mut self,
        req: &mut Request,
        transaction_id: f64,
        additional_arguments: Vec<Amf0Value>,
    ) -> Result<(), ConnectionError> {
        req.conn_type = RtmpConnType::HaivisionPublish;
        if additional_arguments.len() < 1 {
            return Err(ConnectionError::InvalidPublish);
        }
        match &additional_arguments[0] {
            Amf0Value::Utf8String(stream) => req.stream = Some(stream.clone()),
            _ => return Err(ConnectionError::InvalidPublish),
        }
        // Response FCPublish
        self.send_message(RtmpMessage::new_fcpublish_res(transaction_id), 0, 0)
            .await?;
        Ok(())
    }
    fn process_play(
        &mut self,
        req: &mut Request,
        additional_arguments: Vec<Amf0Value>,
    ) -> Result<(), ConnectionError> {
        req.conn_type = RtmpConnType::Play;
        if additional_arguments.len() < 1 {
            return Err(ConnectionError::InvalidPlay);
        }
        match &additional_arguments[0] {
            Amf0Value::Utf8String(stream) => req.stream = Some(stream.clone()),
            _ => return Err(ConnectionError::InvalidPlay),
        }
        if additional_arguments.len() >= 3 {
            match &additional_arguments[2] {
                Amf0Value::Number(n) => req.duration = *n as u32,
                _ => return Err(ConnectionError::InvalidPlay),
            }
        }
        Ok(())
    }

    pub async fn start_play(&mut self) -> Result<(), ConnectionError> {
        // StreamBegin
        self.send_message(
            RtmpMessage::UserControl {
                event_type: STREAM_BEGIN,
                event_data: DEFAULT_SID as u32,
                extra_data: 0,
            },
            0,
            0,
        )
        .await?;

        // onStatus(NetStream.Play.Reset)
        self.send_message(RtmpMessage::new_on_status_play_reset(), 0, 0)
            .await?;

        // onStatus(NetStream.Play.Start)
        self.send_message(RtmpMessage::new_on_status_play_start(), 0, 0)
            .await?;

        // |RtmpSampleAccess(true, true)
        self.send_message(RtmpMessage::new_sample_access(), 0, 0)
            .await?;

        // onStatus(NetStream.Data.Start)
        self.send_message(RtmpMessage::new_on_status_data_start(), 0, 0)
            .await?;
        Ok(())
    }
    pub async fn start_fmle_publish(&mut self) -> Result<(), ConnectionError> {
        // FCPublish
        if let RtmpMessage::Amf0Command { transaction_id, .. } =
            self.ctx.expect_amf_command(&[COMMAND_FC_PUBLISH]).await?
        {
            // response _result
            self.send_message(RtmpMessage::new_fcpublish_res(transaction_id), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        // createStream
        if let RtmpMessage::Amf0Command { transaction_id, .. } = self
            .ctx
            .expect_amf_command(&[COMMAND_CREATE_STREAM])
            .await?
        {
            // response _result
            self.send_message(RtmpMessage::new_create_stream_res(transaction_id), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        // publish
        if let RtmpMessage::Amf0Command { .. } =
            self.ctx.expect_amf_command(&[COMMAND_PUBLISH]).await?
        {
            // response onFCPublish(NetStream.Publish.Start)
            self.send_message(RtmpMessage::new_on_fcpublish(), 0, 0)
                .await?;
            // response onStatus(NetStream.Publish.Start)
            self.send_message(RtmpMessage::new_on_status_publish_start(), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }
        Ok(())
    }
    pub async fn start_haivision_publish(&mut self) -> Result<(), ConnectionError> {
        // publish
        if let RtmpMessage::Amf0Command { .. } =
            self.ctx.expect_amf_command(&[COMMAND_PUBLISH]).await?
        {
            // response onFCPublish(NetStream.Publish.Start)
            self.send_message(RtmpMessage::new_on_fcpublish(), 0, 0)
                .await?;
            // response onStatus(NetStream.Publish.Start)
            self.send_message(RtmpMessage::new_on_status_publish_start(), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }
        Ok(())
    }
    pub async fn start_flash_publish(&mut self) -> Result<(), ConnectionError> {
        // response onStatus(NetStream.Publish.Start)
        self.send_message(RtmpMessage::new_on_status_publish_start(), 0, 0)
            .await?;
        Ok(())
    }

    pub async fn process_amf_command(
        &mut self,
        msg: RtmpMessage,
    ) -> Result<Option<RtmpCtrlAction>, ConnectionError> {
        match msg {
            RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                additional_arguments,
                ..
            } => {
                match self.conn_type {
                    RtmpConnType::Play => {
                        match command_name.as_str() {
                            COMMAND_CLOSE_STREAM => Ok(Some(RtmpCtrlAction::Close)),
                            COMMAND_PAUSE => {
                                if additional_arguments.len() < 1 {
                                    return Ok(None);
                                }
                                match additional_arguments[0] {
                                    Amf0Value::Boolean(pause) => {
                                        if pause {
                                            // response onStatus(NetStream.Pause.Notify)
                                            self.send_message(
                                                RtmpMessage::new_on_status_pause(),
                                                0,
                                                0,
                                            )
                                            .await?;
                                            // response StreamEOF
                                            self.send_message(
                                                RtmpMessage::UserControl {
                                                    event_type: STREAM_EOF,
                                                    event_data: DEFAULT_SID as u32,
                                                    extra_data: 0,
                                                },
                                                0,
                                                0,
                                            )
                                            .await?;
                                        } else {
                                            // response onStatus(NetStream.Unpause.Notify)
                                            self.send_message(
                                                RtmpMessage::new_on_status_pause(),
                                                0,
                                                0,
                                            )
                                            .await?;
                                            // response StreamBegin
                                            self.send_message(
                                                RtmpMessage::UserControl {
                                                    event_type: STREAM_BEGIN,
                                                    event_data: DEFAULT_SID as u32,
                                                    extra_data: 0,
                                                },
                                                0,
                                                0,
                                            )
                                            .await?;
                                        }
                                        return Ok(Some(RtmpCtrlAction::Pause(pause)));
                                    }
                                    _ => return Ok(None),
                                }
                            }
                            _ => {
                                if transaction_id as u32 > 0 {
                                    // Response null first for the other call msg
                                    // FIXME: response in right way, or forward
                                    self.send_message(RtmpMessage::new_null(transaction_id), 0, 0)
                                        .await?;
                                }
                                Ok(None)
                            }
                        }
                    }
                    RtmpConnType::FmlePublish | RtmpConnType::HaivisionPublish => {
                        if command_name == COMMAND_UNPUBLISH {
                            // response onFCUnpublish(NetStream.Unpublish.Success)
                            self.send_message(RtmpMessage::new_on_fcunpublish(), 0, 0)
                                .await?;
                            // response FCUnpublish
                            self.send_message(RtmpMessage::new_fcpublish_res(transaction_id), 0, 0)
                                .await?;
                            // reponse onStatus(NetStream.Unpublish.Success)
                            self.send_message(RtmpMessage::new_on_status_unpublish(), 0, 0)
                                .await?;
                            return Ok(Some(RtmpCtrlAction::Republish));
                        }
                        Ok(None)
                    }
                    // for flash, any packet is republish
                    RtmpConnType::FlashPublish => Ok(Some(RtmpCtrlAction::Republish)),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}
