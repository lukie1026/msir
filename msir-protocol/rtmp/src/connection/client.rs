use msir_core::transport::Transport;
use rml_amf0::Amf0Value;
use tokio::net::TcpStream;

use crate::{
    handshake,
    message::{
        request::Request,
        types::{amf0_command_type::*, user_ctrl_ev_type::SET_BUFFER_LENGTH, DEFAULT_SID},
        RtmpMessage,
    },
};

use super::{context::Context, error::ConnectionError};

pub struct Client {
    uid: String,
    ctx: Context,
    req: Request,
}

impl Client {
    pub async fn new(uid: String, tc_url: String, stream: String) -> Result<Self, ConnectionError> {
        let mut req = Request::parse_from(tc_url)?;
        req.stream = Some(stream);

        // TCP connect to peer
        let addr = req.tc_url.socket_addrs(|| Some(1935))?;
        let mut io = Transport::new(TcpStream::connect(addr[0]).await?);

        // Handshake with peer
        let mut hc = handshake::Client::new();
        hc.handshake(&mut io).await?;

        Ok(Self {
            uid,
            req,
            ctx: Context::new(io),
        })
    }

    pub async fn send_message(
        &mut self,
        msg: RtmpMessage,
        timestamp: u32,
        csid: u32,
    ) -> Result<(), ConnectionError> {
        self.ctx.send_message(msg, timestamp, csid).await
    }

    pub async fn connect(&mut self) -> Result<f64, ConnectionError> {
        // Connect app
        self.send_message(
            RtmpMessage::new_connect_app(&self.req, self.uid.clone()),
            0,
            0,
        )
        .await?;

        // Set Window Acknowledgement size(2500000)
        self.send_message(
            RtmpMessage::SetWindowAckSize {
                ack_window_size: 2500000,
            },
            0,
            0,
        )
        .await?;

        // Expect connect _result
        self.ctx.expect_amf_command(&[COMMAND_RESULT]).await?;

        // TODO: Get server info

        // Create stream
        self.send_message(RtmpMessage::new_create_stream(), 0, 0)
            .await?;

        // Expect create stream _result, get stream_id
        match self.ctx.expect_amf_command(&[COMMAND_RESULT]).await? {
            RtmpMessage::Amf0Command {
                additional_arguments,
                ..
            } => {
                if additional_arguments.len() > 0 {
                    return Ok(match &additional_arguments[0] {
                        Amf0Value::Number(sid) => *sid,
                        _ => DEFAULT_SID,
                    });
                }
            }
            _ => {}
        }

        Ok(DEFAULT_SID)
    }

    pub async fn play(&mut self, stream: String, stream_id: u32) -> Result<(), ConnectionError> {
        // Play stream
        self.send_message(RtmpMessage::new_play_stream(stream), 0, stream_id)
            .await?;

        // SetBufferLength(1000ms)
        self.send_message(
            RtmpMessage::UserControl {
                event_type: SET_BUFFER_LENGTH,
                event_data: stream_id,
                extra_data: 1000,
            },
            0,
            0,
        )
        .await?;

        // SetChunkSize
        self.send_message(RtmpMessage::SetChunkSize { chunk_size: 60000 }, 0, 0)
            .await?;

        Ok(())
    }
}
