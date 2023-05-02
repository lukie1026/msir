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
    pub async fn send_message(&mut self, msg: RtmpPayload) -> Result<(), ConnectionError> {
        self.ctx.send_message(msg).await
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
                        _ => rtmp_sig::RTMP_SIG_AMF0_VER,
                    },
                    None => rtmp_sig::RTMP_SIG_AMF0_VER,
                };

                let mut request = Request::parse_from(tc_url)?;
                request.object_encoding = object_encoding;
                Ok(request)
            }
            _ => Err(ConnectionError::UnexpectedMessage),
        }
    }
    pub async fn relay_connect_app(&mut self, req: &Request) -> Result<(), ConnectionError> {
        // let mut props = HashMap::new();

        // add_amf(
        //     &mut props,
        //     "fmsVer",
        //     Amf0Value::Utf8String(rtmp_sig::RTMP_SIG_FMS_VER.to_string()),
        // );

        // let props = new_amfs(vec![
        //     ("fmsVer", Amf0Value::Utf8String(rtmp_sig::RTMP_SIG_FMS_VER.to_string())),
        //     ("capabilities", Amf0Value::Number(127.0)),
        //     ("mode", Amf0Value::Number(1.0)),
        // ]);

        let command_object = fast_create_amf0(vec![
            ("fmsVer", Amf0Value::Utf8String(RTMP_SIG_FMS_VER.to_string())),
            ("capabilities", Amf0Value::Number(127.0)),
            ("mode", Amf0Value::Number(1.0)),
        ]);

        let args = fast_create_amf0(vec![
            (STATUS_LEVEL, Amf0Value::Utf8String(STATUS_LEVEL.to_string())),
            (STATUS_CODE, Amf0Value::Utf8String(STATUS_CODE_CONNECT_SUCCESS.to_string())),
        ]);

        

        // let mut args = HashMap::new();
        // args.insert("objectEncoding".to_string(), Amf0Value::Number(0.0));

        let message = RtmpMessage::Amf0Command {
            command_name: "_result".to_string(),
            transaction_id: 1.0,
            command_object, //: Amf0Value::Object(props),
            additional_arguments: vec![args], // vec![Amf0Value::Object(args)],
        };

        let payload = crate::message::encode(message, 0, 0)?;

        self.send_message(payload).await
    }
}

// fn add_amf(map: &mut HashMap<String, Amf0Value>, key: &str, value: Amf0Value) {
//     map.insert(key.to_string(), value);
// }

// fn new_amfs(values: Vec<(&str, Amf0Value)>) -> HashMap<String, Amf0Value> {
//     let mut map = HashMap::new();
//     for (k, v) in values {
//         map.insert(k.to_string(), v);
//     }
//     map
// }

fn fast_create_amf0(values: Vec<(&str, Amf0Value)>) -> Amf0Value {
    let mut map = HashMap::new();
    for (k, v) in values {
        map.insert(k.to_string(), v);
    }
    Amf0Value::Object(map)
}