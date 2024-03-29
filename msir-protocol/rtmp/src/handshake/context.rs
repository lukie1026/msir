use super::{error::HandshakeError, RTMP_HANDSHAKE_SIZE, RTMP_VERSION};
use bytes::{BufMut, BytesMut};
use msir_core::{transport::Transport, utils};
use rand::Rng;
use tracing::{error, info, info_span, instrument, trace};

pub struct Context {
    // [1+1536]
    pub c0c1: BytesMut,
    // [1+1536+1536]
    pub s0s1s2: BytesMut,
    // [1536]
    pub c2: BytesMut,
}

impl Context {
    pub fn new() -> Self {
        Self {
            c0c1: BytesMut::new(),
            s0s1s2: BytesMut::new(),
            c2: BytesMut::new(),
        }
    }
    pub async fn read_c0c1(&mut self, io: &mut Transport) -> Result<(), HandshakeError> {
        if self.c0c1.is_empty() {
            self.c0c1
                .extend_from_slice(io.read_exact(RTMP_HANDSHAKE_SIZE + 1).await?);
        }
        Ok(())
    }
    pub async fn read_s0s1s2(&mut self, io: &mut Transport) -> Result<(), HandshakeError> {
        if self.s0s1s2.is_empty() {
            self.s0s1s2
                .extend_from_slice(io.read_exact(1 + RTMP_HANDSHAKE_SIZE * 2).await?);
        }
        Ok(())
    }
    pub async fn read_c2(&mut self, io: &mut Transport) -> Result<(), HandshakeError> {
        if self.c2.is_empty() {
            self.c2
                .extend_from_slice(io.read_exact(RTMP_HANDSHAKE_SIZE).await?);
        }
        Ok(())
    }
    pub fn create_c0c1(&mut self) -> Result<(), HandshakeError> {
        if self.c0c1.is_empty() {
            self.c0c1.reserve(1 + RTMP_HANDSHAKE_SIZE);
            self.c0c1.put_u8(RTMP_VERSION);
            self.c0c1.put_u32(utils::current_time());
            self.c0c1.put_u32(0);
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE - 8) {
                self.c0c1.put_u8(rng.gen());
            }
        }
        Ok(())
    }
    pub fn create_s0s1s2(&mut self) -> Result<(), HandshakeError> {
        if self.s0s1s2.is_empty() {
            self.s0s1s2.reserve(1 + RTMP_HANDSHAKE_SIZE * 2);
            self.s0s1s2.put_u8(RTMP_VERSION);
            self.s0s1s2.put_u32(utils::current_time());
            if self.c0c1.is_empty() {
                self.s0s1s2.put_u32(0);
            } else {
                self.s0s1s2.put_slice(&self.c0c1[1..5])
            }
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE - 8) {
                self.s0s1s2.put_u8(rng.gen());
            }
            if self.c0c1.is_empty() {
                for _ in 0..(RTMP_HANDSHAKE_SIZE) {
                    self.s0s1s2.put_u8(rng.gen());
                }
            } else {
                self.s0s1s2
                    .put_slice(&self.c0c1[1..RTMP_HANDSHAKE_SIZE + 1]);
            }
        }
        Ok(())
    }
    pub fn create_c2(&mut self) -> Result<(), HandshakeError> {
        if self.c2.is_empty() {
            self.c2.reserve(RTMP_HANDSHAKE_SIZE);
            self.c2.put_u32(utils::current_time());
            if self.s0s1s2.is_empty() {
                self.c2.put_u32(0);
            } else {
                self.c2.put_slice(&self.s0s1s2[1..5]);
            }
            let mut rng = rand::thread_rng();
            for _ in 0..(RTMP_HANDSHAKE_SIZE - 8) {
                self.c0c1.put_u8(rng.gen());
            }
        }
        Ok(())
    }
}
