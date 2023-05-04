use rml_amf0::Amf0Value;
use std::collections::HashMap;
use tokio::{net::TcpStream, stream};
use tracing::{info, warn};

use crate::{
    handshake,
    message::{
        request::Request,
        types::{amf0_command_type::*, rtmp_sig::*, rtmp_status::*},
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
        self.ctx.send_message(msg, timestamp, csid).await
    }
    pub async fn connect_app(&mut self) -> Result<Request, ConnectionError> {
        match self.ctx.expect_amf_command(&[COMMAND_CONNECT]).await? {
            RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                command_object,
                additional_arguments,
            } => {
                info!(
                    "Server receive connect, {:?} {:?}",
                    command_name, command_object
                );

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

                let mut request = Request::parse_from(tc_url)?;
                request.object_encoding = object_encoding;
                Ok(request)
            }
            _ => Err(ConnectionError::UnexpectedMessage),
        }
    }
    pub async fn response_connect_app(&mut self, req: &Request) -> Result<(), ConnectionError> {
        let command_object = fast_create_amf0_obj(vec![
            (
                "fmsVer",
                Amf0Value::Utf8String(RTMP_SIG_FMS_VER.to_string()),
            ),
            ("capabilities", Amf0Value::Number(127.0)),
            ("mode", Amf0Value::Number(1.0)),
        ]);

        let args = fast_create_amf0_obj(vec![
            (
                STATUS_LEVEL,
                Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
            ),
            (
                STATUS_CODE,
                Amf0Value::Utf8String(STATUS_CODE_CONNECT_SUCCESS.to_string()),
            ),
            (
                STATUS_DESCRIPTION,
                Amf0Value::Utf8String("Connection succeeded".to_string()),
            ),
            ("objectEncoding", Amf0Value::Number(req.object_encoding)),
            (
                "data",
                fast_create_amf0_obj(vec![
                    ("msir_version", Amf0Value::Utf8String("v0.1.0".to_string())),
                    ("msir_auther", Amf0Value::Utf8String("Lukie".to_string())),
                ]),
            ),
        ]);

        let message = RtmpMessage::Amf0Command {
            command_name: COMMAND_RESULT.to_string(),
            transaction_id: 1.0,
            command_object,
            additional_arguments: vec![args],
        };

        info!("Server response connect: {:?}", message);

        self.send_message(message, 0, 0).await
    }

    pub async fn identify_client(&mut self, req: &mut Request) -> Result<(), ConnectionError> {
        loop {
            match self.ctx.expect_amf_command(&[]).await? {
                RtmpMessage::Amf0Command {
                    command_name,
                    transaction_id,
                    command_object,
                    additional_arguments,
                } => {
                    info!("Server receive {:?}", command_name);
                    return match command_name.as_str() {
                        COMMAND_PLAY => self.process_play(req).await,
                        COMMAND_CREATE_STREAM => {
                            self.process_create_stream(req, transaction_id).await
                        }
                        COMMAND_RELEASE_STREAM => {
                            if additional_arguments.len() < 1 {
                                return Err(ConnectionError::ReleaseStreamWithoutStream);
                            }
                            match &additional_arguments[0] {
                                Amf0Value::Utf8String(stream) => req.stream = Some(stream.clone()),
                                _ => return Err(ConnectionError::ReleaseStreamWithoutStream),
                            }
                            self.process_fmle_publish(req, transaction_id).await
                        }
                        _ => {
                            // Response null first for the other call msg
                            self.send_message(
                                RtmpMessage::Amf0Command {
                                    command_name: "".to_string(),
                                    transaction_id,
                                    command_object: Amf0Value::Null,
                                    additional_arguments: vec![],
                                },
                                0,
                                0,
                            )
                            .await?;
                            continue;
                        }
                    };
                }
                _ => return Err(ConnectionError::UnexpectedMessage),
            }
        }
    }
    async fn process_create_stream(
        &mut self,
        req: &mut Request,
        transaction_id: f64,
    ) -> Result<(), ConnectionError> {
        let mut res_transaction_id = transaction_id;
        for _ in 0..3 {
            // Response CreateStream
            self.send_message(
                RtmpMessage::Amf0Command {
                    command_name: COMMAND_RESULT.to_string(),
                    transaction_id: res_transaction_id,
                    command_object: Amf0Value::Null,
                    additional_arguments: vec![Amf0Value::Number(super::DEFAULT_SID)],
                },
                0,
                0,
            )
            .await?;
            info!(
                "Server response createStream: transaction={:?}",
                res_transaction_id
            );

            match self
                .ctx
                .expect_amf_command(&[
                    COMMAND_PLAY,
                    COMMAND_PUBLISH,
                    COMMAND_CREATE_STREAM,
                    COMMAND_FC_PUBLISH,
                ])
                .await?
            {
                RtmpMessage::Amf0Command {
                    command_name,
                    transaction_id,
                    command_object,
                    additional_arguments,
                } => {
                    return match command_name.as_str() {
                        COMMAND_PLAY => self.process_play(req).await,
                        COMMAND_PUBLISH => self.process_flash_publish(req).await,
                        COMMAND_FC_PUBLISH => self.process_haivision_publish(req).await,
                        COMMAND_CREATE_STREAM => {
                            res_transaction_id = transaction_id;
                            continue;
                        }
                        _ => Err(ConnectionError::UnexpectedMessage),
                    };
                }
                _ => return Err(ConnectionError::UnexpectedMessage),
            }
        }
        Err(ConnectionError::CreateStreamDepth)
    }
    async fn process_fmle_publish(
        &mut self,
        req: &mut Request,
        transaction_id: f64,
    ) -> Result<(), ConnectionError> {
        req.conn_type = RtmpConnType::FmlePublish;
        self.send_message(
            RtmpMessage::Amf0Command {
                command_name: COMMAND_RESULT.to_string(),
                transaction_id,
                command_object: Amf0Value::Null,
                additional_arguments: vec![Amf0Value::Undefined],
            },
            0,
            0,
        )
        .await?;
        info!(
            "Server response releaseStream: transaction={:?}",
            transaction_id
        );
        Ok(())
    }
    async fn process_flash_publish(&mut self, req: &mut Request) -> Result<(), ConnectionError> {
        Ok(())
    }
    async fn process_haivision_publish(
        &mut self,
        req: &mut Request,
    ) -> Result<(), ConnectionError> {
        Ok(())
    }
    async fn process_play(&mut self, req: &mut Request) -> Result<(), ConnectionError> {
        Ok(())
    }

    pub async fn start_fmle_publish(&mut self) -> Result<(), ConnectionError> {
        // FCPublish && response _result
        if let RtmpMessage::Amf0Command { transaction_id, .. } =
            self.ctx.expect_amf_command(&[COMMAND_FC_PUBLISH]).await?
        {
            self.send_message(
                RtmpMessage::Amf0Command {
                    command_name: COMMAND_RESULT.to_string(),
                    transaction_id,
                    command_object: Amf0Value::Null,
                    additional_arguments: vec![Amf0Value::Undefined],
                },
                0,
                0,
            )
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
            self.send_message(
                RtmpMessage::Amf0Command {
                    command_name: COMMAND_RESULT.to_string(),
                    transaction_id,
                    command_object: Amf0Value::Null,
                    additional_arguments: vec![Amf0Value::Number(super::DEFAULT_SID)],
                },
                0,
                0,
            )
            .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }
        // publish && response onFCPublish, onStatus
        if let RtmpMessage::Amf0Command { .. } =
            self.ctx.expect_amf_command(&[COMMAND_PUBLISH]).await?
        {
            self.send_message(
                RtmpMessage::Amf0Command {
                    command_name: COMMAND_ON_FC_PUBLISH.to_string(),
                    transaction_id: 0.0,
                    command_object: Amf0Value::Null,
                    additional_arguments: vec![fast_create_amf0_obj(vec![
                        (
                            STATUS_CODE,
                            Amf0Value::Utf8String(STATUS_CODE_PUBLISH_START.to_string()),
                        ),
                        (
                            STATUS_DESCRIPTION,
                            Amf0Value::Utf8String("Started publishing stream.".to_string()),
                        ),
                    ])],
                },
                0,
                0,
            )
            .await?;

            self.send_message(
                RtmpMessage::Amf0Command {
                    command_name: COMMAND_ON_STATUS.to_string(),
                    transaction_id: 0.0,
                    command_object: Amf0Value::Null,
                    additional_arguments: vec![fast_create_amf0_obj(vec![
                        (
                            STATUS_LEVEL,
                            Amf0Value::Utf8String(STATUS_LEVEL_STATUS.to_string()),
                        ),
                        (
                            STATUS_CODE,
                            Amf0Value::Utf8String(STATUS_CODE_PUBLISH_START.to_string()),
                        ),
                        (
                            STATUS_DESCRIPTION,
                            Amf0Value::Utf8String("Started publishing stream.".to_string()),
                        ),
                        (
                            STATUS_CLIENT_ID,
                            Amf0Value::Utf8String(RTMP_SIG_CLIENT_ID.to_string()),
                        ),
                    ])],
                },
                0,
                0,
            )
            .await?;
        } else {
            return Err(ConnectionError::UnexpectedMessage);
        }

        Ok(())
    }
}

fn fast_create_amf0_obj(values: Vec<(&str, Amf0Value)>) -> Amf0Value {
    let mut map = HashMap::new();
    for (k, v) in values {
        map.insert(k.to_string(), v);
    }
    Amf0Value::Object(map)
}
