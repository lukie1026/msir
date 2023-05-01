use crate::{
    chunk::{ChunkCodec, ChunkStream},
    message::{types::*, RtmpMessage},
};
use bytes::BytesMut;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tracing::{error, info, trace, warn};

use super::error::ConnectionError;

// TODO: support FastStreamBuffer && sendtimeout recvtimeout
// struct StreamBuffer {
//     io: TcpStream,
//     snd_tm: Duration,
//     rcv_tm: Duration,
// }

// impl StreamBuffer {
//     fn read(&self) {

//     }
// }

#[derive(Default)]
struct AckWindowSize {
    window: u32,
    nb_recv_bytes: u64,
    sequence_number: u32,
}

pub struct Context {
    // For peer in/out
    chunkIo: ChunkCodec,
    // requests: HashMap<f64, String>,
    // For peer in
    chunk_streams: HashMap<u32, ChunkStream>, // TODO: Performance
    in_ack_size: AckWindowSize,
    out_ack_size: AckWindowSize,
    in_buffer_length: u32,
    in_chunk_size: u32,
    // For peer out
    // out_c0c3_caches: BytesMut,  // TODO: Performance
    // Whether warned user to increase the c0c3 header cache.
    // warned_c0c3_cache_dry: bool,
    // The output chunk size, default to 128, set by config.
    out_chunk_size: u32,
}

impl Context {
    pub fn new(io: TcpStream) -> Self {
        Self {
            chunkIo: ChunkCodec::new(io),
            // requests: HashMap::new(),
            chunk_streams: HashMap::new(),
            in_ack_size: AckWindowSize::default(),
            out_ack_size: AckWindowSize::default(),
            in_buffer_length: 0,
            in_chunk_size: 128,
            out_chunk_size: 128,
        }
    }
    pub fn set_in_window_ack_size(&mut self, ack_size: u32) {
        self.in_ack_size.window = ack_size;
    }
    pub async fn recv_message(&mut self) -> Result<RtmpMessage, ConnectionError> {
        let msg = self.chunkIo.recv_rtmp_message().await?;
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
                self.chunkIo.set_in_chunk_size(chunk_size);
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
            } => {
                match *event_type {
                    user_ctrl_ev_type::SET_BUFFER_LENGTH => self.in_buffer_length = *extra_data,
                    user_ctrl_ev_type::PING_REQUEST => {
                        // TODO: PING Request
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}
