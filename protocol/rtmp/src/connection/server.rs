use core::time;
use std::collections::HashMap;

use rml_amf0::Amf0Value;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::{
    handshake,
    message::{
        request::Request,
        types::{amf0_command_type::*, rtmp_sig::*, rtmp_status::*},
        RtmpMessage, RtmpPayload,
    },
};

use super::{context::Context, error::ConnectionError};

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
    pub async fn send_message(&mut self, msg: RtmpMessage, timestamp: u32, csid: u32) -> Result<(), ConnectionError> {
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
                    "Server connect app received, {:?} {:?}",
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
    pub async fn relay_connect_app(&mut self, req: &Request) -> Result<(), ConnectionError> {
        let command_object = fast_create_amf0_obj(vec![
            ("fmsVer", Amf0Value::Utf8String(RTMP_SIG_FMS_VER.to_string())),
            ("capabilities", Amf0Value::Number(127.0)),
            ("mode", Amf0Value::Number(1.0)),
        ]);

        let args = fast_create_amf0_obj(vec![
            (STATUS_LEVEL, Amf0Value::Utf8String(STATUS_LEVEL.to_string())),
            (STATUS_CODE, Amf0Value::Utf8String(STATUS_CODE_CONNECT_SUCCESS.to_string())),
            (STATUS_DESCRIPTION, Amf0Value::Utf8String("Connection succeeded".to_string())),
            ("objectEncoding", Amf0Value::Number(req.object_encoding)),
            ("data", fast_create_amf0_obj(vec![
                ("rsrs_version", Amf0Value::Utf8String("v0.1.0".to_string())),
                ("rsrs_auther", Amf0Value::Utf8String("Lukie".to_string())),
            ])),
        ]);

        let message = RtmpMessage::Amf0Command {
            command_name: RESULT.to_string(),
            transaction_id: 1.0,
            command_object,
            additional_arguments: vec![args],
        };

        self.send_message(message, 0, 0).await
    }
}

fn fast_create_amf0_obj(values: Vec<(&str, Amf0Value)>) -> Amf0Value {
    let mut map = HashMap::new();
    for (k, v) in values {
        map.insert(k.to_string(), v);
    }
    Amf0Value::Object(map)
}