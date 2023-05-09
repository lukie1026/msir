use crate::{
    chunk::ChunkCodec,
    message::{types::*, RtmpMessage},
};

use msir_core::transport::Transport;
use std::{collections::HashMap, time::Duration};
use tokio::net::TcpStream;
use tracing::{error, info, trace, warn};

use super::error::ConnectionError;

#[derive(Default)]
struct AckWindowSize {
    window: u32,
    nb_recv_bytes: u64,
    sequence_number: u32,
}

pub struct Context {
    chunk_io: ChunkCodec,
    in_ack_size: AckWindowSize,
    out_ack_size: AckWindowSize,
    in_buffer_length: u32,
}

impl Context {
    pub fn new(io: Transport) -> Self {
        Self {
            chunk_io: ChunkCodec::new(io),
            // requests: HashMap::new(),
            in_ack_size: AckWindowSize::default(),
            out_ack_size: AckWindowSize::default(),
            in_buffer_length: 0,
        }
    }

    pub fn set_recv_timeout(&mut self, tm: Duration) {
        self.chunk_io.set_recv_timeout(tm);
    }

    pub fn set_send_timeout(&mut self, tm: Duration) {
        self.chunk_io.set_send_timeout(tm);
    }

    pub fn get_recv_bytes(&mut self) -> u64 {
        self.chunk_io.get_recv_bytes()
    }

    pub fn get_send_bytes(&mut self) -> u64 {
        self.chunk_io.get_send_bytes()
    }

    pub fn set_in_window_ack_size(&mut self, ack_size: u32) {
        self.in_ack_size.window = ack_size;
    }

    pub async fn recv_message(&mut self) -> Result<RtmpMessage, ConnectionError> {
        let msg = self.chunk_io.recv_rtmp_message().await?;
        self.on_recv_message(&msg).await?;
        Ok(msg)
    }
    async fn on_recv_message(&mut self, msg: &RtmpMessage) -> Result<(), ConnectionError> {
        // TODO: try to response acknowledgement
        match msg {
            RtmpMessage::SetChunkSize { chunk_size } => {
                let chunk_size = *chunk_size as usize;
                if chunk_size < 128 || chunk_size > 65536 {
                    warn!("Accept set_chunk_size {}", chunk_size);
                }
                trace!("Accept set_chunk_size {}", chunk_size);
                self.chunk_io.set_in_chunk_size(chunk_size);
            }
            RtmpMessage::SetWindowAckSize { ack_window_size } => {
                let ack_window_size = *ack_window_size;
                trace!("Accept set_window_ack_size {}", ack_window_size);
                self.in_ack_size.window = ack_window_size;
            }
            RtmpMessage::UserControl {
                event_type,
                event_data,
                extra_data,
            } => match *event_type {
                user_ctrl_ev_type::SET_BUFFER_LENGTH => self.in_buffer_length = *extra_data,
                user_ctrl_ev_type::PING_REQUEST => {
                    self.send_message(
                        RtmpMessage::UserControl {
                            event_type: user_ctrl_ev_type::PING_RESPONSE,
                            event_data: *event_data,
                            extra_data: 0,
                        },
                        0,
                        0,
                    )
                    .await?
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    pub async fn send_message(
        &mut self,
        msg: RtmpMessage,
        timestamp: u32,
        csid: u32,
    ) -> Result<(), ConnectionError> {
        self.on_send_message(&msg)?;
        let payload = crate::message::encode(msg, timestamp, csid)?;
        self.chunk_io.send_rtmp_message(payload).await?;
        Ok(())
    }
    pub async fn send_messages(
        &mut self,
        msgs: &[RtmpMessage],
        timestamp: u32,
        csid: u32,
    ) -> Result<(), ConnectionError> {
        let mut payloads = Vec::with_capacity(msgs.len());
        for msg in msgs {
            self.on_send_message(msg)?;
            payloads.push(crate::message::encode(msg.clone(), timestamp, csid)?);
        }
        self.chunk_io.send_rtmp_messages(&payloads).await?;
        Ok(())
    }
    fn on_send_message(&mut self, msg: &RtmpMessage) -> Result<(), ConnectionError> {
        match msg {
            RtmpMessage::SetChunkSize { chunk_size } => {
                self.chunk_io.set_out_chunk_size(*chunk_size as usize)
            }
            RtmpMessage::SetWindowAckSize { ack_window_size } => {
                self.out_ack_size.window = *ack_window_size
            }
            _ => return Ok(()),
        }
        Ok(())
    }

    pub async fn expect_amf_command(
        &mut self,
        specified_cmds: &[&str],
    ) -> Result<RtmpMessage, ConnectionError> {
        loop {
            let msg = self.recv_message().await?;
            if msg.expect_amf(specified_cmds) {
                return Ok(msg);
            }
            info!("Server ignore msg: {}", msg);
        }
    }
    // pub async fn expect_result_or_error(&mut self, transcation_id: f64) -> Result<RtmpMessage, ConnectionError> {

    // }
}
