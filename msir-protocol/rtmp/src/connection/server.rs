use rml_amf0::Amf0Value;
use std::collections::HashMap;
use tokio::{net::TcpStream, stream};
use tracing::{info, warn};

use crate::{
    handshake,
    message::{
        request::Request,
        types::{
            amf0_command_type::*, peer_bw_limit_type, rtmp_sig::*, rtmp_status::*,
            user_ctrl_ev_type::*, DEFAULT_SID,
        },
        RtmpMessage,
    },
};

use super::{context::Context, error::ConnectionError, RtmpConnType};

pub struct Server {
    ctx: Context,
}

impl Server {
    pub async fn new(mut io: TcpStream) -> Result<Self, ConnectionError> {
        let mut hs = handshake::Server::new();
        hs.handshake(&mut io).await?;
        Ok(Self {
            ctx: Context::new(io),
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
        info!("Server response: {:?}", msg);
        self.ctx.send_message(msg, timestamp, csid).await
    }
    pub async fn identify_client(&mut self) -> Result<Request, ConnectionError> {
        let mut req = self.connect_app().await?;
        loop {
            if let RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                command_object,
                additional_arguments,
            } = self.ctx.expect_amf_command(&[]).await?
            {
                info!(
                    "Server receive {:?} transaction_id={}",
                    command_name, transaction_id
                );
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
                return Ok(req);
            }
            return Err(ConnectionError::UnexpectedMessage);
        }
    }
    async fn connect_app(&mut self) -> Result<Request, ConnectionError> {
        if let RtmpMessage::Amf0Command {
            command_name,
            transaction_id,
            command_object,
            ..
        } = self.ctx.expect_amf_command(&[COMMAND_CONNECT]).await?
        {
            info!("Server receive {:?} {:?}", command_name, command_object);

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
            // request.object_encoding = object_encoding;

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
                command_object,
                additional_arguments,
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
                info!(
                    "Server receive {:?} transaction_id={}",
                    command_name, transaction_id
                );
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
        // FCPublish && response _result
        if let RtmpMessage::Amf0Command { transaction_id, .. } =
            self.ctx.expect_amf_command(&[COMMAND_FC_PUBLISH]).await?
        {
            info!(
                "Server receive {:?} transaction_id={}",
                COMMAND_FC_PUBLISH, transaction_id
            );
            self.send_message(RtmpMessage::new_fcpublish_res(transaction_id), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        // createStream && response _result
        if let RtmpMessage::Amf0Command { transaction_id, .. } = self
            .ctx
            .expect_amf_command(&[COMMAND_CREATE_STREAM])
            .await?
        {
            info!(
                "Server receive {:?} transaction_id={}",
                COMMAND_CREATE_STREAM, transaction_id
            );
            self.send_message(RtmpMessage::new_create_stream_res(transaction_id), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        // publish && response onFCPublish, onStatus
        if let RtmpMessage::Amf0Command { .. } =
            self.ctx.expect_amf_command(&[COMMAND_PUBLISH]).await?
        {
            info!("Server receive {:?}", COMMAND_PUBLISH);
            self.send_message(RtmpMessage::new_on_fcpublish(), 0, 0)
                .await?;

            self.send_message(RtmpMessage::new_on_status_publish_start(), 0, 0)
                .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        Ok(())
    }
    pub async fn start_haivision_publish(&mut self) -> Result<(), ConnectionError> {
        // TODO: implement it
        Ok(())
    }
    pub async fn start_flash_publish(&mut self) -> Result<(), ConnectionError> {
        // TODO: implement it
        Ok(())
    }
}
