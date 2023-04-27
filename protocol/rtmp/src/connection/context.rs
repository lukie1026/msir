use crate::chunk::ChunkStream;
use tokio::net::TcpStream;
use std::collections::HashMap;
use bytes::BytesMut;

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


struct AckWindowSize
{
    window: u32,
    nb_recv_bytes: u64,
    sequence_number: u32,
}

pub struct Context {
    // For peer in/out
    io: TcpStream,
    requests: HashMap<f64, String>,
    // For peer in
    chunk_streams: HashMap<u32, ChunkStream>,
    in_chunk_size: u32,
    in_ack_size: AckWindowSize,
    out_ack_size: AckWindowSize,
    in_buffer_length: u32,
    // For peer out
    out_c0c3_caches: BytesMut,
    // Whether warned user to increase the c0c3 header cache.
    warned_c0c3_cache_dry: bool,
    // The output chunk size, default to 128, set by config.
    out_chunk_size: u32,
}

impl Context {
    pub fn set_in_window_ack_size(&mut self) {
        
    }
}