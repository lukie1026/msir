use rml_amf0::Amf0Value;
use tokio::net::TcpStream;
use tracing::{info, warn};

use crate::{
    handshake,
    message::{
        request::Request, types::{amf0_command_type::*, rtmp_sig}, RtmpMessage,
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
    pub async fn connect_app(&mut self) -> Result<Request, ConnectionError> {
        match self.ctx.expect_amf_command(&[COMMAND_CONNECT]).await? {
            RtmpMessage::Amf0Command {
                command_name,
                transaction_id,
                command_object,
                additional_arguments,
            } => {
                info!("Server connect app received, {:?} {:?}", command_name, command_object);

                if transaction_id != 1.0 {
                    warn!("Invalid transaction_id={} of connect_app", transaction_id);
                }

                let mut properties = match command_object {
                    Amf0Value::Object(properties) => properties,
                    _ => return Err(ConnectionError::InvalidConnectApp),
                };
                let tc_url = match properties.remove("tcUrl") {
                    Some(value) => match value {
                        Amf0Value::Utf8String(mut tc_url) => tc_url,
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
            _ => Err(ConnectionError::UnexpectedMessage)
        }
    }
}
